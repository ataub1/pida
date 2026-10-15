//! Signierplan aus Profil, Position und Dokument (Plan §6.4–§6.9; docs/analyse/03 Phase C,
//! docs/analyse/04 §4–§5).
//!
//! [`plan_signature`] verbindet `pdfover-sigblock` (Appearance, `/Name`, `/Reason`, `/TU`) mit
//! `pdfover-placement` (Breite, Rasterscan, neue Seite, Widget-Rect, Rotation) und liefert einen
//! fertigen [`SignPlan`] für [`crate::prepare`]. Ohne Renderer (kein PDFium) gilt die
//! Ausweichregel von `placement`: die Zielseite wird wie bei PDF-AS als leer behandelt
//! ([`PlacementNotice::ScanFailed`]).

use std::sync::Arc;

use pdfover_cades::{Certificate, CmsProfile};
use pdfover_pdf_cos::{PageInfo, Rect, SourcePdf};
use pdfover_placement::{
    CompatMode, EmptyFieldCandidate, FreeSpaceScanner, PlaceholderMode, PlaceholderQuery, PlacementError,
    PlacementNotice, PositionContext, QrPlaceholder, RasterScanner, ResolvedPosition, TablePos, appearance_rotation,
    find_empty_sig_fields, find_qr_placeholders, resolve_position, resolve_width, select_placeholder, widget_overlaps,
    widget_rect_with,
};
use pdfover_render::{ForcedColorOptions, RenderError, Renderer};
use pdfover_sigblock::{
    Appearance, BuildOptions, CertFields, FontProvider, PdfaMode, Profile, Resources, SigBlockInput,
};
use time::OffsetDateTime;

use crate::PadesError;
use crate::analyze::DocReport;
use crate::plan::{BlockPlacement, FieldTarget, Reserve, SignPlan, Visibility};

/// Optionen für [`plan_signature`].
#[derive(Debug, Clone, PartialEq)]
pub struct PlanOptions {
    /// `/M` und signing-time ([`crate::local_signing_time`])
    pub signing_time: OffsetDateTime,
    /// Position der Anfrage (`p:auto;x:auto;y:auto`, manuell, …); wird über das Profil-`pos`
    /// gelegt (`TablePos::merge`). Bei QR-Platzhalter ersetzt dessen Position diese komplett.
    pub position: TablePos,
    /// Zielfeld
    pub field: FieldTarget,
    /// Gewählter QR-Platzhalter ([`find_placeholder`]): Position und `/Location`
    pub placeholder: Option<QrPlaceholder>,
    /// Reserve
    pub reserve: Reserve,
    /// CMS-Profil (Modell A)
    pub cms_profile: CmsProfile,
    /// Kette einbetten (Modell A)
    pub include_intermediates: bool,
    /// Zertifikat ist bekannt (sonst kein `/Name`, Block mit Beispielzertifikat nur zur Vorschau)
    pub certificate_known: bool,
    /// PDF-AS-Originalverhalten der Positionierung (nur Golden-Tests)
    pub compat: CompatMode,
}

impl PlanOptions {
    /// Automatische Position, neues Feld, Modell A, `PdfasCompat`, Zertifikat bekannt.
    pub fn new(signing_time: OffsetDateTime) -> Self {
        PlanOptions {
            signing_time,
            position: TablePos::auto(),
            field: FieldTarget::New { name: None },
            placeholder: None,
            reserve: Reserve::ModelA,
            cms_profile: CmsProfile::PdfasCompat,
            include_intermediates: false,
            certificate_known: true,
            compat: CompatMode::Corrected,
        }
    }
}

/// Ergebnis von [`plan_signature`].
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSignature {
    /// fertiger Plan
    pub plan: SignPlan,
    /// endgültige Position (sichtbarer Block, neues Feld)
    pub position: Option<ResolvedPosition>,
    /// Hinweise der Positionierung (neue Seite nicht erlaubt, Überdeckung, Scan fehlgeschlagen …)
    pub notices: Vec<PlacementNotice>,
    /// Ersetzte Zeichen im Block (`sigblock-chars-replaced`)
    pub replaced_chars: Vec<char>,
}

/// Zertifikatsfelder für den Signaturblock (`subject.X`, `issuer.X`, `sn`).
pub fn cert_fields(cert: &Certificate) -> CertFields {
    let avas = |v: Vec<pdfover_cades::RdnAttr>| -> Vec<(String, String)> {
        v.into_iter().map(|a| (a.short_name.unwrap_or(a.oid), a.value)).collect()
    };
    CertFields {
        subject: avas(cert.subject_rdns()),
        issuer: avas(cert.issuer_rdns()),
        serial_decimal: cert.serial_decimal(),
        subject_dn: cert.subject_string(),
        issuer_dn: cert.issuer_string(),
    }
}

/// PDF/A-Behandlung des Blocks: aus der XMP-Kennung des Dokuments (Teile 1–3 gleich, Plan
/// §6.9; Teil 4 wie 3); ohne Kennung bei PDF/A-Profil wie PDF-AS Teil 1.
pub fn pdfa_mode(report: &DocReport, profile: &Profile) -> PdfaMode {
    match report.pdfa.as_ref().map(|p| p.part) {
        Some(1) => PdfaMode::Part1,
        Some(2) => PdfaMode::Part2,
        Some(3 | 4) => PdfaMode::Part3,
        _ if profile.is_pdfa() => PdfaMode::Part1,
        _ => PdfaMode::None,
    }
}

/// Leere, nicht gesperrte Signaturfelder (je Signatur wählbar; `/Lock`-gesperrte Felder hat
/// [`crate::analyze`] bereits als `ReadOnly` markiert).
pub fn empty_fields(report: &DocReport) -> Vec<EmptyFieldCandidate> {
    find_empty_sig_fields(&report.fields)
}

/// Sucht den QR-Platzhalter wie PDF-Over (`id` und Modus; beim Öffnen `Some("1")` + SORTED,
/// beim Signieren die gemerkte id + MODERATE). Benutzte Platzhalter (`/Location` vorhandener
/// Signaturen) werden übersprungen. Bilder in Form-XObjects werden wie bei PDF-AS nicht
/// durchsucht.
pub fn find_placeholder(
    src: &SourcePdf,
    report: &DocReport,
    renderer: &dyn Renderer,
    id: Option<&str>,
    mode: PlaceholderMode,
) -> Result<Option<QrPlaceholder>, PadesError> {
    let mut invocations = Vec::new();
    for p in 0..src.page_count() {
        invocations.extend(src.image_invocations(p, false)?);
    }
    let candidates = find_qr_placeholders(renderer, src.pages(), &invocations)?;
    let mut used = report.used_placeholder_names();
    for l in src.signature_locations()? {
        if !used.contains(&l) {
            used.push(l);
        }
    }
    let query = PlaceholderQuery { id: id.map(str::to_string), mode, used_names: used };
    Ok(select_placeholder(&candidates, &query)?.cloned())
}

/// Freiraumsuche ohne Renderer: liefert einen Fehler, `placement` behandelt die Seite dann wie
/// PDF-AS als leer (Hinweis `ScanFailed`).
struct NoRenderer;

impl FreeSpaceScanner for NoRenderer {
    fn lowest_content(&mut self, _page: usize, _footer: f32) -> pdfover_placement::Result<f32> {
        Err(PlacementError::Render(RenderError::LibraryNotFound("kein Renderer".into())))
    }
}

fn build_appearance(
    profile: &Profile,
    input: &SigBlockInput,
    width: f32,
    rotation: i32,
    pdfa: PdfaMode,
    fonts: &FontProvider,
    resources: &dyn Resources,
) -> Result<Appearance, PadesError> {
    let opts = BuildOptions { width, rotation, pdfa, preview_only: false };
    Ok(Appearance::build(profile, input, &opts, fonts, resources)?)
}

/// Baut den Signierplan: Appearance aus dem Profil, Position über `placement` (Breite →
/// Layout-Höhe → Rasterscan/neue Seite → Widget-Rect, Rotation), Feldwahl, `/Name`, `/Reason`,
/// `/Location`, PDF/A- und PDF/UA-Schalter.
///
/// `profile` ist das Profil mit Laufzeit-Overlays (`Profile::with_overrides`; bei QR-Platzhalter
/// mit `profile=` das Platzhalter-Profil), `input` die Blockdaten (Zertifikat bzw. Beispiel,
/// Hinweis, Emblem); die PDF/A-Behandlung setzt diese Funktion selbst ([`pdfa_mode`]).
pub fn plan_signature(
    src: &SourcePdf,
    report: &DocReport,
    profile: &Profile,
    input: &SigBlockInput,
    opts: &PlanOptions,
    fonts: &FontProvider,
    resources: &dyn Resources,
    renderer: Option<&dyn Renderer>,
) -> Result<PlannedSignature, PadesError> {
    let pdfa = pdfa_mode(report, profile);
    let mut input = input.clone();
    input.pdfa = pdfa;
    input.signing_time = opts.signing_time;
    let mut notices = Vec::new();
    let mut position = None;
    let mut replaced_chars = Vec::new();

    let name = if opts.certificate_known { pdfover_sigblock::expr::signer_name(profile, &input)? } else { None };
    let location = opts.placeholder.as_ref().map(|p| p.resource_name.clone());

    let visibility = if !profile.visible {
        Visibility::Invisible
    } else {
        let request = match &opts.placeholder {
            Some(p) => p.table_pos_with_min_width(profile.min_width),
            None => opts.position.clone(),
        };
        let profile_pos = match profile.pos.as_deref() {
            Some(s) if !s.trim().is_empty() => TablePos::parse(s)?,
            _ => TablePos::default(),
        };
        let pos = match &opts.placeholder {
            // Platzhalter-Position ersetzt die Position komplett (`prepareTablePosition`)
            Some(_) => request,
            None => TablePos::merge(&request, &profile_pos),
        };
        let ctx = PositionContext {
            pages: src.pages(),
            already_signed: report.is_signed(),
            signed_newpage_force: profile.signed_newpage_force,
            less_space_failing: false,
            compat: opts.compat,
        };
        match &opts.field {
            FieldTarget::Existing { widget_id, .. } => {
                // vorhandenes Feld: Rect bleibt, Breite wie beim Positionieren (PDF-AS rechnet sie)
                let w = report
                    .fields
                    .iter()
                    .flat_map(|f| f.widgets.iter())
                    .find(|w| w.id == *widget_id)
                    .ok_or(PadesError::FieldNotFound)?;
                let page_index = w.page_index.unwrap_or(0);
                let wd = resolve_width(&TablePos { page: None, ..pos.clone() }, &ctx)?;
                let page = &src.pages()[page_index.min(src.page_count() - 1)];
                let rotation = appearance_rotation(pos.block_rotation(), page);
                let app = build_appearance(profile, &input, wd.width, rotation, pdfa, fonts, resources)?;
                replaced_chars = app.layout.as_ref().map(|l| l.replaced_chars.clone()).unwrap_or_default();
                Visibility::Visible(BlockPlacement {
                    appearance: Arc::new(app),
                    page_index,
                    new_page: false,
                    rect: w.rect,
                })
            }
            FieldTarget::New { .. } => {
                let wd = resolve_width(&pos, &ctx)?;
                let first = build_appearance(profile, &input, wd.width, 0, pdfa, fonts, resources)?;
                let rp = match renderer {
                    Some(r) => {
                        let mut scanner = RasterScanner { renderer: r, options: ForcedColorOptions::default() };
                        resolve_position(&pos, &ctx, wd, first.height, &mut scanner)?
                    }
                    None => resolve_position(&pos, &ctx, wd, first.height, &mut NoRenderer)?,
                };
                let page: &PageInfo = if rp.new_page {
                    src.pages().last().ok_or(PadesError::Cos(pdfover_pdf_cos::CosError::NoPages))?
                } else {
                    src.pages().get(rp.page_index).ok_or(PadesError::InvalidPlan("Seite außerhalb".into()))?
                };
                let rotation = appearance_rotation(rp.rotation, page);
                let app = if rotation == 0 {
                    first
                } else {
                    build_appearance(profile, &input, wd.width, rotation, pdfa, fonts, resources)?
                };
                replaced_chars = app.layout.as_ref().map(|l| l.replaced_chars.clone()).unwrap_or_default();
                let rect: Rect = widget_rect_with(&rp, page, opts.compat);
                notices.extend(rp.notices.iter().cloned());
                if opts.placeholder.is_none() && widget_overlaps(&rp, page, &report.fields) {
                    notices.push(PlacementNotice::OverlapsWidget);
                }
                if opts.placeholder.is_some() {
                    // beabsichtigte Überdeckung des QR-Codes (Plan §6.1)
                    notices.retain(|n| *n != PlacementNotice::OverlapsContent);
                }
                let placement = BlockPlacement {
                    appearance: Arc::new(app),
                    page_index: if rp.new_page { src.page_count() } else { rp.page_index },
                    new_page: rp.new_page,
                    rect,
                };
                position = Some(rp);
                Visibility::Visible(placement)
            }
        }
    };

    let plan = SignPlan {
        profile_id: profile.id.clone(),
        add_output_intent_if_missing: matches!(visibility, Visibility::Visible(_)) && pdfa != PdfaMode::None,
        visibility,
        field: opts.field.clone(),
        signing_time: opts.signing_time,
        name,
        reason: Some(profile.reason_or_default().to_string()),
        location,
        reserve: opts.reserve,
        cms_profile: opts.cms_profile,
        include_intermediates: opts.include_intermediates,
        pdfua: report.tagged,
        signature_size: profile.signature_size,
    };
    Ok(PlannedSignature { plan, position, notices, replaced_chars })
}

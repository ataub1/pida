//! Signaturblock und Signierplan (Plan §6.4–§6.8; ADR 0001 §8.4, §8.7).
//!
//! Hier entsteht aus Einstellungen, Overlay, Richtlinie, Dokumentanalyse, Zertifikat und Position der
//! [`SignPlan`] für `pades::prepare` – mit genau demselben Code, der auch die Blockvorschau baut
//! (Vorschau = Ergebnis):
//!
//! 1. Profil-ID über [`Effective::pdfas_profile_id`] (Richtlinie, `-v`, Hinweis, PDF/A-Kennung),
//! 2. Laufzeit-Overlays wie PDF-Over (`SIG_LABEL`, `SIG_NOTE`, BASE_LOGO über
//!    [`ProfileOverrides::base_logo`]),
//! 3. Breite über [`resolve_width`] (Profil-`pos` mit der Anfrage gemischt, KR-F01), Höhe aus dem
//!    Layout, Position über [`resolve_position`] (Rasterscan mit PDFium, sonst „leere Seite“),
//! 4. Widget-`/Rect` und Rotation, `/Name` über [`pdfover_sigblock::expr::signer_name`].

use std::path::Path;
use std::sync::Arc;

use pdfover_cades::{CertChain, Certificate, CmsProfile};
use pdfover_config::{CmsProfileChoice, Effective};
use pdfover_i18n::Lang;
use pdfover_pades::{BlockPlacement, DocReport, FieldTarget, PdfaId, Reserve, SignPlan, Visibility};
use pdfover_pdf_cos::{PageInfo, Rect, Rotation};
use pdfover_placement::{
    BlockGeometry, CompatMode, FreeSpaceScanner, Placement, PositionContext, RasterScanner, ResolvedPosition, TablePos,
    WidthDecision, appearance_rotation, default_start_placement, resolve_position, resolve_width, widget_rect,
};
use pdfover_render::{ForcedColorOptions, Renderer};
use pdfover_sigblock::{
    Appearance, BuildOptions, CertFields, Emblem, FontProvider, PdfaMode, Profile, ProfileOverrides, ProfileStore,
    SigBlockInput,
};
use pdfover_workflow::PositionChoice;
use time::OffsetDateTime;

/// Fehler beim Bauen des Blocks bzw. Plans (stabiler Code für i18n).
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    /// Signaturblock
    #[error(transparent)]
    SigBlock(#[from] pdfover_sigblock::SigBlockError),
    /// Positionierung
    #[error(transparent)]
    Placement(#[from] pdfover_placement::PlacementError),
    /// Dokument ohne Seiten
    #[error("Dokument ohne Seiten")]
    NoPages,
}

impl PlanError {
    /// Stabiler Code.
    pub fn code(&self) -> String {
        match self {
            PlanError::SigBlock(e) => e.code().to_owned(),
            PlanError::Placement(e) => e.code().to_owned(),
            PlanError::NoPages => "error-MayNotBeAPDF".to_owned(),
        }
    }
}

/// Lokale Zeit mit Offset (ADR 0001 §8.7: `jiff` ermittelt den Offset, auch in Mehrthread-Prozessen),
/// auf Sekunden gekürzt (`/M` hat Sekundenauflösung).
pub fn local_now() -> OffsetDateTime {
    let offset_secs = jiff::Zoned::now().offset().seconds();
    let offset = time::UtcOffset::from_whole_seconds(offset_secs).unwrap_or(time::UtcOffset::UTC);
    let now = OffsetDateTime::now_utc().to_offset(offset);
    now.replace_nanosecond(0).unwrap_or(now)
}

/// Zertifikatsfelder für den Block (Kurzname der AVA, sonst OID).
pub fn cert_fields(cert: &Certificate) -> CertFields {
    let avas = |rdns: Vec<pdfover_cades::RdnAttr>| -> Vec<(String, String)> {
        rdns.into_iter().map(|a| (a.short_name.unwrap_or(a.oid), a.value)).collect()
    };
    CertFields {
        subject: avas(cert.subject_rdns()),
        issuer: avas(cert.issuer_rdns()),
        serial_decimal: cert.serial_decimal(),
        subject_dn: cert.subject_string(),
        issuer_dn: cert.issuer_string(),
    }
}

/// PDF/A-Behandlung: Kennung des Dokuments, sonst (Schalter bzw. `_PDFA`-Profil) Teil 1 wie PDF-AS.
pub fn pdfa_mode(pdfa: Option<&PdfaId>, profile: &Profile) -> PdfaMode {
    match pdfa.map(|p| p.part) {
        Some(1) => PdfaMode::Part1,
        Some(2) => PdfaMode::Part2,
        Some(3) => PdfaMode::Part3,
        _ if profile.is_pdfa() => PdfaMode::Part1,
        _ => PdfaMode::None,
    }
}

/// Bildmarke laden (ungültige Bilder werden wie in PDF-Over still verworfen, F-31/`-e`).
pub fn load_emblem(path: &Path) -> Option<Arc<Emblem>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e.kind(), "Bildmarke nicht lesbar – Profil-Logo wird verwendet");
            return None;
        }
    };
    match Emblem::decode(&bytes) {
        Ok(e) => Some(Arc::new(e)),
        Err(e) => {
            tracing::warn!(code = e.code(), "Bildmarke ungültig – Profil-Logo wird verwendet");
            None
        }
    }
}

/// Eingaben des Blocks (gemeinsam für Vorschau und Signatur).
#[derive(Debug, Clone)]
pub struct BlockSpec {
    /// Profil-ID (12 PDF-Over-IDs)
    pub profile_id: String,
    /// Profil mit Laufzeit-Overlays
    pub profile: Profile,
    /// Blockeingabe
    pub input: SigBlockInput,
    /// PDF/A-Modus
    pub pdfa: PdfaMode,
}

impl BlockSpec {
    /// Sichtbares Profil?
    pub fn visible(&self) -> bool {
        self.profile.visible
    }

    /// Block mit Breite und Rotation bauen.
    pub fn build(
        &self,
        width: f32,
        rotation: i32,
        store: &ProfileStore,
        fonts: &FontProvider,
    ) -> Result<Appearance, PlanError> {
        let opts = BuildOptions { width, rotation, pdfa: self.pdfa, preview_only: false };
        Ok(Appearance::build(&self.profile, &self.input, &opts, fonts, store.resources().as_ref())?)
    }

    /// Positionsstring des Profils (`pos`), ungültig → keiner.
    pub fn profile_pos(&self) -> TablePos {
        self.profile.pos.as_deref().and_then(|p| TablePos::parse(p).ok()).unwrap_or_default()
    }
}

/// Eingaben für [`block_spec`]. `Debug` ohne Einstellungswerte (Regel 9).
#[derive(Clone, Copy)]
pub struct SpecInput<'a> {
    /// Einstellungen + Overlay + Richtlinie
    pub effective: Effective<'a>,
    /// Systemsprache (Blocksprache ohne Einstellung)
    pub system_language: Lang,
    /// PDF/A-Kennung des Dokuments
    pub pdfa: Option<&'a PdfaId>,
    /// Zertifikat (sonst Beispielzertifikat, Plan §6.6)
    pub cert: Option<&'a Certificate>,
    /// Zeitpunkt (`/M`, `SIG_DATE`)
    pub signing_time: OffsetDateTime,
    /// Bildmarke (bereits geladen)
    pub emblem: Option<&'a Arc<Emblem>>,
}

impl std::fmt::Debug for SpecInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpecInput")
            .field("system_language", &self.system_language)
            .field("pdfa", &self.pdfa)
            .field("cert", &self.cert.is_some())
            .field("emblem", &self.emblem.is_some())
            .finish_non_exhaustive()
    }
}

/// Profil und Blockeingabe wie PDF-Over (`PdfAs4SignatureParameter`, docs/analyse/04 §2.8).
pub fn block_spec(store: &ProfileStore, inp: SpecInput<'_>) -> Result<BlockSpec, PlanError> {
    let eff = inp.effective;
    let profile_id = eff.pdfas_profile_id(inp.system_language, inp.pdfa.is_some());
    let base = store.profile(&profile_id)?;
    let lang = eff.block_language(inp.system_language);
    let note = eff.note_text(lang);
    let emblem = inp.emblem.cloned();
    let mut overrides = if profile_id == "BASE_LOGO" {
        let px = emblem.as_ref().map(|e| (e.width, e.height)).unwrap_or((1, 1));
        ProfileOverrides::base_logo(emblem.clone(), px, eff.settings.signature.logo_only_size_mm)
    } else {
        ProfileOverrides { sig_label: emblem.clone(), ..ProfileOverrides::default() }
    };
    if note.is_some() {
        overrides.sig_note = note;
    }
    let profile = base.with_overrides(&overrides);
    let pdfa = pdfa_mode(inp.pdfa, &profile);
    let cert = inp.cert.map(cert_fields).unwrap_or_else(CertFields::example);
    let mut input = SigBlockInput::new(cert, inp.signing_time);
    input.emblem = emblem;
    input.pdfa = pdfa;
    Ok(BlockSpec { profile_id, profile, input, pdfa })
}

/// Blockgröße für die Positionierung auf `page` (Breite aus [`resolve_width`] mit manuellem `x`,
/// KR-F01: Vorschau = Ergebnis).
pub fn block_geometry(
    spec: &BlockSpec,
    pages: &[PageInfo],
    page: usize,
    store: &ProfileStore,
    fonts: &FontProvider,
) -> Result<BlockGeometry, PlanError> {
    if !spec.visible() {
        return Ok(BlockGeometry { width: 0.0, height: 0.0 });
    }
    if pages.is_empty() {
        return Err(PlanError::NoPages);
    }
    let page = page.min(pages.len() - 1);
    let request = TablePos::manual(u32::try_from(page + 1).unwrap_or(u32::MAX), 0.0, 0.0);
    let merged = TablePos::merge(&request, &spec.profile_pos());
    let ctx = PositionContext::new(pages);
    let width = resolve_width(&merged, &ctx)?;
    let app = spec.build(width.width, 0, store, fonts)?;
    Ok(BlockGeometry { width: app.width, height: app.height })
}

/// Startposition der UI (letzte Seite, 50 %/25 %).
pub fn start_placement(
    spec: &BlockSpec,
    pages: &[PageInfo],
    store: &ProfileStore,
    fonts: &FontProvider,
) -> Result<Option<Placement>, PlanError> {
    if pages.is_empty() {
        return Ok(None);
    }
    let geom = block_geometry(spec, pages, pages.len() - 1, store, fonts)?;
    Ok(default_start_placement(pages, geom))
}

/// A4 hochkant (Blockvorschau ohne Dokument).
pub fn a4_page() -> PageInfo {
    let r = Rect { llx: 0.0, lly: 0.0, urx: 595.0, ury: 842.0 };
    PageInfo {
        index: 0,
        object_id: (0, 0),
        media_box: r,
        crop_box: r,
        rotation: Rotation::R0,
        user_unit: 1.0,
        annots_indirect: None,
    }
}

/// Freiraumsuche ohne Renderer: Seite gilt als leer (wie PDF-AS bei Renderfehlern).
#[derive(Debug, Default)]
struct EmptyPages;

impl FreeSpaceScanner for EmptyPages {
    fn lowest_content(&mut self, _page_index: usize, _footer: f32) -> pdfover_placement::Result<f32> {
        Ok(0.0)
    }
}

/// CMS-Profil der Einstellung.
pub fn cms_profile(choice: CmsProfileChoice) -> CmsProfile {
    match choice {
        CmsProfileChoice::PdfasCompat => CmsProfile::PdfasCompat,
        CmsProfileChoice::PadesBb => CmsProfile::PadesBB,
    }
}

/// Eingaben für [`build_plan`].
pub struct PlanInput<'a> {
    /// Analyse
    pub report: &'a DocReport,
    /// Seiten
    pub pages: &'a [PageInfo],
    /// Position
    pub position: &'a PositionChoice,
    /// Block
    pub spec: &'a BlockSpec,
    /// Zertifikat (für `/Name`)
    pub chain: Option<&'a CertChain>,
    /// Reserve
    pub reserve: Reserve,
    /// CMS-Profil (Modell A)
    pub cms_profile: CmsProfile,
    /// Renderer für die Freiraumsuche
    pub renderer: Option<&'a dyn Renderer>,
    /// Profile
    pub store: &'a ProfileStore,
    /// Fonts
    pub fonts: &'a FontProvider,
}

impl std::fmt::Debug for PlanInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanInput")
            .field("pages", &self.pages.len())
            .field("position", &self.position)
            .field("profile", &self.spec.profile_id)
            .field("reserve", &self.reserve)
            .finish_non_exhaustive()
    }
}

/// Ergebnis von [`build_plan`]: Plan und Hinweise der Platzierung (`placement.notice.*`). `Debug`
/// ohne `/Name` (Zertifikats-Personendaten, Regel 9).
#[derive(Clone)]
pub struct BuiltPlan {
    /// Plan
    pub plan: SignPlan,
    /// Hinweise (Codes)
    pub notices: Vec<String>,
    /// berechnete Position (neues Feld; PAGE_ABS, linke obere Ecke)
    pub resolved: Option<ResolvedPosition>,
}

impl std::fmt::Debug for BuiltPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltPlan")
            .field("profile_id", &self.plan.profile_id)
            .field("field", &self.plan.field)
            .field("reserve", &self.plan.reserve)
            .field("name", &self.plan.name.as_ref().map(|n| n.chars().count()))
            .field("notices", &self.notices)
            .field("resolved", &self.resolved)
            .finish_non_exhaustive()
    }
}

/// Baut den [`SignPlan`].
pub fn build_plan(inp: PlanInput<'_>) -> Result<BuiltPlan, PlanError> {
    let spec = inp.spec;
    let name = match inp.chain {
        Some(chain) => {
            let mut input = spec.input.clone();
            input.cert = cert_fields(&chain.signer);
            pdfover_sigblock::expr::signer_name(&spec.profile, &input)?
        }
        None => None,
    };
    let mut notices = Vec::new();
    let mut location = None;
    let mut resolved_out = None;
    let (visibility, field) = if !spec.visible() || matches!(inp.position, PositionChoice::Invisible) {
        (Visibility::Invisible, FieldTarget::New { name: None })
    } else if let PositionChoice::Field(f) = inp.position {
        // vorhandenes leeres Feld: Rect bleibt, Appearance wird hineinskaliert (Plan §6.8)
        let rect = f.rect.normalized();
        let page_index = f.page_index.unwrap_or(0);
        let page = inp.pages.get(page_index).ok_or(PlanError::NoPages)?;
        let rotation = appearance_rotation(0, page);
        let appearance = spec.build(rect.width().max(1.0), rotation, inp.store, inp.fonts)?;
        let placement = BlockPlacement { appearance: Arc::new(appearance), page_index, new_page: false, rect };
        let target = FieldTarget::Existing {
            field_id: f.field_id,
            widget_id: f.widget_id,
            qualified_name: f.qualified_name.clone(),
        };
        (Visibility::Visible(placement), target)
    } else {
        if inp.pages.is_empty() {
            return Err(PlanError::NoPages);
        }
        let request = match inp.position {
            PositionChoice::Manual(p) => TablePos::merge(&p.to_table_pos(), &spec.profile_pos()),
            PositionChoice::Placeholder(qr) => {
                location = Some(qr.resource_name.clone());
                // Die Platzhalter-Position ersetzt die Position komplett (`prepareTablePosition`)
                qr.table_pos_with_min_width(spec.profile.min_width)
            }
            _ => TablePos::merge(&TablePos::auto(), &spec.profile_pos()),
        };
        let ctx = PositionContext {
            pages: inp.pages,
            already_signed: inp.report.is_signed(),
            signed_newpage_force: spec.profile.signed_newpage_force,
            less_space_failing: false,
            compat: CompatMode::Corrected,
        };
        let width: WidthDecision = resolve_width(&request, &ctx)?;
        let measured = spec.build(width.width, 0, inp.store, inp.fonts)?;
        let resolved: ResolvedPosition = match inp.renderer {
            Some(renderer) => {
                let mut scanner = RasterScanner { renderer, options: ForcedColorOptions::default() };
                resolve_position(&request, &ctx, width, measured.height, &mut scanner)?
            }
            None => resolve_position(&request, &ctx, width, measured.height, &mut EmptyPages)?,
        };
        notices.extend(resolved.notices.iter().map(|n| n.code().to_owned()));
        let page = if resolved.new_page {
            inp.pages.last().ok_or(PlanError::NoPages)?
        } else {
            inp.pages.get(resolved.page_index).ok_or(PlanError::NoPages)?
        };
        let rect = widget_rect(&resolved, page);
        let rotation = appearance_rotation(resolved.rotation, page);
        let appearance = spec.build(width.width, rotation, inp.store, inp.fonts)?;
        let placement = BlockPlacement {
            appearance: Arc::new(appearance),
            page_index: resolved.page_index,
            new_page: resolved.new_page,
            rect,
        };
        resolved_out = Some(resolved);
        (Visibility::Visible(placement), FieldTarget::New { name: None })
    };
    let plan = SignPlan {
        profile_id: spec.profile_id.clone(),
        visibility,
        field,
        signing_time: spec.input.signing_time,
        name,
        reason: Some(spec.profile.reason_or_default().to_owned()),
        location,
        reserve: inp.reserve,
        cms_profile: inp.cms_profile,
        include_intermediates: false,
        add_output_intent_if_missing: spec.pdfa != PdfaMode::None,
        pdfua: inp.report.tagged,
    };
    Ok(BuiltPlan { plan, notices, resolved: resolved_out })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfover_config::{Overlay, Settings, SignatureProfile};

    fn store() -> ProfileStore {
        ProfileStore::builtin().expect("eingebaute Profile")
    }

    fn spec(settings: &Settings, overlay: &Overlay) -> BlockSpec {
        let eff = Effective::new(settings, overlay, None);
        block_spec(
            &store(),
            SpecInput {
                effective: eff,
                system_language: Lang::De,
                pdfa: None,
                cert: None,
                signing_time: local_now(),
                emblem: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn profile_id_follows_settings_and_overlay() {
        let mut s = Settings::default();
        s.signature.block_language = Some(Lang::De);
        let o = Overlay::default();
        assert_eq!(spec(&s, &o).profile_id, "SIGNATURBLOCK_SMALL_DE_NOTE");
        s.signature.note = pdfover_config::NoteSetting::None;
        assert_eq!(spec(&s, &o).profile_id, "SIGNATURBLOCK_SMALL_DE");
        let o = Overlay { invisible: Some(true), ..Overlay::default() };
        let sp = spec(&s, &o);
        assert_eq!(sp.profile_id, "INVISIBLE");
        assert!(!sp.visible());
        s.signature.profile = SignatureProfile::Amtssignaturblock;
        s.signature.block_language = Some(Lang::En);
        assert_eq!(spec(&s, &Overlay::default()).profile_id, "AMTSSIGNATURBLOCK_EN_RECOMMENDED");
    }

    #[test]
    fn small_block_geometry_matches_pdfas() {
        let mut s = Settings::default();
        s.signature.note = pdfover_config::NoteSetting::None;
        s.signature.block_language = Some(Lang::De);
        let sp = spec(&s, &Overlay::default());
        let pages = [a4_page()];
        let g = block_geometry(&sp, &pages, 0, &store(), &FontProvider::builtin()).unwrap();
        // Plan §6.5 Zielwert: SMALL_DE 230 × 49 pt
        assert!((g.width - 230.0).abs() < 0.5, "{g:?}");
        assert!((g.height - 49.0).abs() < 1.5, "{g:?}");
        let start = start_placement(&sp, &pages, &store(), &FontProvider::builtin()).unwrap().unwrap();
        assert_eq!(start.page_index, 0);
        assert!((start.x - 297.5).abs() < 0.01);
        assert!((start.y - 210.5).abs() < 0.01);
    }

    #[test]
    fn local_time_has_offset_and_whole_seconds() {
        let t = local_now();
        assert_eq!(t.nanosecond(), 0);
    }

    #[test]
    fn cms_profile_mapping() {
        assert_eq!(cms_profile(CmsProfileChoice::PadesBb), CmsProfile::PadesBB);
        assert_eq!(cms_profile(CmsProfileChoice::PdfasCompat), CmsProfile::PdfasCompat);
    }

    #[test]
    fn missing_emblem_is_ignored() {
        assert!(load_emblem(Path::new("/nicht/vorhanden.png")).is_none());
    }
}

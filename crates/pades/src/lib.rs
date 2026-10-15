//! `pdfover-pades` – PAdES-Signatur-Engine (Plan §3.6, §6; docs/analyse/03).
//!
//! Ablauf: [`analyze`] → Annahmeklasse K1–K5 → [`SignPlan`] (Profil, Position, Feld; bequem über
//! [`plan_signature`]) → ggf. Zertifikat vom Backend → [`prepare`] → Backend `sign` →
//! [`Prepared::finish_cms`] (Modell B) bzw. [`Prepared::finish_raw`] (Modell A) →
//! [`SignedPdf::self_check`] (Invarianten I1–I6) → [`SignedPdf::write_atomically`]. Verletzt die
//! Ausgabe eine Invariante, wird **nichts** gespeichert.
//!
//! Zweiphasenmodell nach dem Vorbild der PDF-AS-API `startSign`/`finishSign`
//! (docs/analyse/03 §6.4): [`Prepared`] ist nach `prepare()` unveränderlich (WYSIWYS).
//!
//! # Was ins Increment kommt (docs/analyse/03 §4, Plan §6.4)
//!
//! | Objekt | neu / fortgeschrieben |
//! |---|---|
//! | Signaturdictionary `/Type /Sig /Filter /SubFilter /M [/Location] [/Name] /Reason /Contents /ByteRange` | neu (serialisiert von `pdf-incremental`) |
//! | Feld + Widget `/FT /Sig /Type /Annot /Subtype /Widget /F 132 /T /V /P /Rect /AP /TU [/StructParent]` | neu (Feldname `PDF-AS Signatur<n>`) |
//! | vorhandenes leeres Feld: `/V` angehängt, Widget `/AP` ersetzt, `/F` \|= 4, `/Rect` bleibt | fortgeschrieben (Widget in `/Kids` eigenständig) |
//! | Appearance (Holder, Inner `FRM1`, Fonts, Bilder) aus `pdfover-sigblock` | neu |
//! | AcroForm: neu direkt im Katalog (`/Fields /SigFlags 3 /DR /DA`), sonst fortgeschrieben | Katalog bzw. AcroForm |
//! | Seite: `/Annots` (direkt; indirektes Array wird selbst fortgeschrieben), ggf. `/Tabs /S` | fortgeschrieben |
//! | neue Seite (gleiche MediaBox/`/Rotate` wie die letzte), `/Pages` mit `/Kids`/`/Count` | neu/fortgeschrieben |
//! | sRGB-OutputIntent (nur PDF/A, nur wenn keiner vorhanden, nicht bei Vorsignaturen) | Katalog + neu |
//! | PDF/UA: StructElem `/Form` mit OBJR, ParentTree, `/ParentTreeNextKey` (nicht bei Vorsignaturen) | neu/fortgeschrieben |
//!
//! # Kompatibilitätsregister (Abweichungen von PDF-AS, Plan §6.4/§6.9)
//!
//! | ID | PDF-AS | hier |
//! |---|---|---|
//! | KR-P01 | kein Pauschal-Fixup fremder Felder (`NeedAppearances` → Appearances neu) | fremde Felder bleiben unverändert (I6) |
//! | KR-P02 | OutputIntents werden **ersetzt** (nur sichtbar, nur PDF/A-1/-3-Logik) | nur ergänzt, wenn keiner vorhanden; Teile 1–3 gleich |
//! | KR-P03 | Formularfeld + PDF/UA: NullPointer, kein StructElem | StructElem auch für vorhandene Felder (wenn das Widget noch keins hat) |
//! | KR-P04 | `/Contents` wird bei verschlüsselten Dokumenten mitverschlüsselt | nie verschlüsselt (ISO 32000) |
//! | KR-P05 | keine Prüfung von DocMDP, `/Lock`, Seed Values | Analyse K4/K5, gesperrte Felder nicht wählbar |
//! | KR-P06 | unsichtbar auf vorhandenem Feld: `/Rect [0 0 0 0]` | `/Rect` bleibt, leeres AP (Änderungsanalyse) |
//! | KR-P07 | Struktur-/OutputIntent-Ergänzung auch bei Vorsignaturen | entfällt bei Vorsignaturen (sonst `OTHER` in der Änderungsanalyse), Hinweis |
//! | KR-P08 | keine Längenprüfung vor dem Einpatchen | harte Prüfung, `cms-too-large` mit benötigter Größe |
//! | KR-P09 | PDFBox-Fixup `/DR /Font /Helv /ZaDb` + `/DA` auch bei PDF/A | bei PDF/A-Eingaben entfällt der Fixup (nicht eingebettete Standard-14-Fonts; Signaturfelder brauchen sie nicht) |
//!
//! # Stabile Codes (ADR 0001 Regel 11; Texte trägt die Integration in die Kataloge ein)
//!
//! Befunde von [`analyze`] stehen in [`codes`] (`pdf-…`), Fehler in [`PadesError::code`] und
//! [`FinishError::code`], Invarianten in [`Invariant::code`], Hinweise beim Aufbau in
//! [`Prepared::notices`]:
//!
//! | Code | Deutsch | English |
//! |---|---|---|
//! | `pades-pdf-unreadable` | Das Dokument konnte nicht gelesen werden. | The document could not be read. |
//! | `pades-write` | Die Signatur konnte nicht in das Dokument geschrieben werden. | The signature could not be written into the document. |
//! | `pades-field-already-signed` | Das gewählte Signaturfeld ist bereits signiert. | The selected signature field is already signed. |
//! | `pades-field-locked` | Das gewählte Signaturfeld ist gesperrt. | The selected signature field is locked. |
//! | `pades-certificate-not-valid` | Das Zertifikat ist zum Signierzeitpunkt nicht gültig. | The certificate is not valid at the time of signing. |
//! | `pades-io` | Lese- oder Schreibfehler. | Read or write error. |
//! | `pades-field-not-found` | Das gewählte Signaturfeld wurde nicht gefunden. | The selected signature field was not found. |
//! | `pades-new-page-not-allowed` | Für dieses Dokument darf keine neue Seite angehängt werden. | No new page may be added to this document. |
//! | `pades-invalid-plan` | Die Signatur kann so nicht platziert werden. | The signature cannot be placed this way. |
//! | `pades-invalid-state` | Der gespeicherte Signaturvorgang ist ungültig und wird verworfen. | The saved signing process is invalid and will be discarded. |
//! | `pades-invariant-i1` … `-i6` | Die Selbstprüfung der Signatur ist fehlgeschlagen; es wurde nichts gespeichert. | The signature self-check failed; nothing was saved. |
//! | `pades-pdfua-skipped` | Die Barrierefreiheits-Struktur (PDF/UA) wurde nicht ergänzt. | The accessibility structure (PDF/UA) was not extended. |
//! | `pades-output-intent-skipped` | Der Farbraum für PDF/A (OutputIntent) wurde nicht ergänzt, weil das Dokument bereits signiert ist. | The PDF/A colour profile (output intent) was not added because the document is already signed. |
//! | `pades-i5-without-pdfium` | Die Prüfung mit PDFium wurde übersprungen. | The check with PDFium was skipped. |
//! | `pdf-signature-subfilter` | Das Signaturverfahren einer vorhandenen Signatur wird nicht geprüft. | The signature method of an existing signature is not verified. |
//! | `pdf-signature-byterange` | Der signierte Bereich einer vorhandenen Signatur ist ungültig. | The signed range of an existing signature is invalid. |
//!
//! Die Befund-Codes ([`codes`]) mit Texten führt der Abschlussbericht von Welle 2 auf.
//!
//! Ownership: **Integration** (nutzt `pdf-cos`, `pdf-incremental`, `cades`, `sigblock`, `render`,
//! `placement`).

mod analyze;
mod build;
mod check;
mod icc;
mod plan;
mod planner;
mod prepared;
mod signed;
mod sigs;
mod util;

pub use analyze::{
    AcceptanceClass, DocMdp, DocReport, ExistingSignature, Finding, PdfaId, XfaKind, analyze, codes, reject_open_error,
};
pub use build::{NOTICE_OUTPUT_INTENT_SKIPPED, NOTICE_PDFUA_SKIPPED};
pub use plan::{BlockPlacement, FieldTarget, Reserve, SignPlan, Visibility};
pub use planner::{
    PlanOptions, PlannedSignature, cert_fields, empty_fields, find_placeholder, pdfa_mode, plan_signature,
};
pub use prepared::{Prepared, PreparedHandle, prepare};
pub use signed::{
    CheckContext, CheckReport, Invariant, InvariantViolation, OutputInfo, OutputTarget, PriorStatus, SignedPdf,
    WARNING_I5_WITHOUT_PDFIUM,
};

pub use pdfover_pdf_incremental::{ByteRange, ExcludedRange};

/// Fehler der Engine.
#[derive(Debug, thiserror::Error)]
pub enum PadesError {
    /// Leser
    #[error(transparent)]
    Cos(#[from] pdfover_pdf_cos::CosError),
    /// Writer
    #[error(transparent)]
    Incremental(#[from] pdfover_pdf_incremental::IncError),
    /// CMS
    #[error(transparent)]
    Cms(#[from] pdfover_cades::CmsError),
    /// Signaturblock
    #[error(transparent)]
    SigBlock(#[from] pdfover_sigblock::SigBlockError),
    /// Eingabe abgelehnt (K5 oder Rechte; CLI-Exit 3). Inhalt = stabiler Befund-Code
    /// ([`codes`]), z. B. `pdf-permissions`.
    #[error("Dokument kann nicht signiert werden: {0}")]
    Rejected(String),
    /// Zielfeld ist bereits signiert.
    #[error("Signaturfeld {} ist bereits signiert", util::redacted_name(.0))]
    FieldAlreadySigned(String),
    /// Zielfeld ist gesperrt (`/Lock`).
    #[error("Signaturfeld {} ist gesperrt", util::redacted_name(.0))]
    FieldLocked(String),
    /// Zertifikat abgelaufen bzw. noch nicht gültig (PDF-AS 11021).
    #[error("Zertifikat ist zum Signierzeitpunkt nicht gültig")]
    CertificateNotValid,
    /// E/A
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
    /// Positionierung (Positionsstring, Seite, Platz, Renderer).
    #[error(transparent)]
    Placement(#[from] pdfover_placement::PlacementError),
    /// Zielfeld existiert nicht (mehr) bzw. ist kein Signaturfeld.
    #[error("Signaturfeld nicht gefunden")]
    FieldNotFound,
    /// Neue Seite nicht erlaubt (Vorsignaturen, DocMDP, vorhandenes Feld; Plan §6.1).
    #[error("eine neue Seite ist für dieses Dokument nicht erlaubt")]
    NewPageNotAllowed,
    /// Signierplan widersprüchlich (Seite außerhalb, Feldname belegt …).
    #[error("Signierplan ungültig: {0}")]
    InvalidPlan(String),
    /// Persistierter Zustand passt nicht (Prüfsumme, Original, Format).
    #[error("gespeicherter Signaturvorgang ist ungültig: {0}")]
    InvalidState(String),
}

impl PadesError {
    /// Stabiler Fehlercode für UI/CLI (ADR 0001 Regel 11). Bei [`PadesError::Rejected`] der
    /// Befund-Code, bei eingebetteten Fehlern der Code der Quelle, wo es einen gibt.
    pub fn code(&self) -> &str {
        match self {
            PadesError::Cos(_) => "pades-pdf-unreadable",
            PadesError::Incremental(_) => "pades-write",
            PadesError::Cms(e) => e.code(),
            PadesError::SigBlock(e) => e.code(),
            PadesError::Rejected(code) => code,
            PadesError::FieldAlreadySigned(_) => "pades-field-already-signed",
            PadesError::FieldLocked(_) => "pades-field-locked",
            PadesError::CertificateNotValid => "pades-certificate-not-valid",
            PadesError::Io(_) => "pades-io",
            PadesError::Placement(e) => e.code(),
            PadesError::FieldNotFound => "pades-field-not-found",
            PadesError::NewPageNotAllowed => "pades-new-page-not-allowed",
            PadesError::InvalidPlan(_) => "pades-invalid-plan",
            PadesError::InvalidState(_) => "pades-invalid-state",
        }
    }
}

/// Fehler beim Abschließen.
#[derive(Debug, thiserror::Error)]
pub enum FinishError {
    /// CMS größer als die Reserve – nichts geschrieben; neu vorbereiten mit größerer Reserve
    /// (zweite Bestätigung nötig, Plan §6.2). Code `cms-too-large`; `needed` ist die benötigte
    /// Größe in Bytes (für `Reserve::Bytes`).
    #[error("CMS zu groß: {needed} Bytes, Reserve {reserve}")]
    CmsTooLarge {
        /// benötigt
        needed: usize,
        /// vorhanden
        reserve: usize,
    },
    /// CMS-Prüfung (I3) fehlgeschlagen.
    #[error(transparent)]
    Cms(#[from] pdfover_cades::CmsError),
    /// Writer.
    #[error(transparent)]
    Incremental(#[from] pdfover_pdf_incremental::IncError),
}

impl FinishError {
    /// Stabiler Fehlercode (ADR 0001 Regel 11): `cms-too-large`, sonst der Code des CMS-Fehlers
    /// bzw. `pades-write`.
    pub fn code(&self) -> &'static str {
        match self {
            FinishError::CmsTooLarge { .. } => "cms-too-large",
            FinishError::Cms(e) => e.code(),
            FinishError::Incremental(_) => "pades-write",
        }
    }

    /// Für [`FinishError::CmsTooLarge`]: Reserve für die Neuvorbereitung (benötigt + 1 KiB,
    /// auf KiB gerundet) – Wert für [`Reserve::Bytes`].
    pub fn suggested_reserve(&self) -> Option<usize> {
        match self {
            FinishError::CmsTooLarge { needed, .. } => Some(needed.saturating_add(1024).div_ceil(1024) * 1024),
            _ => None,
        }
    }
}

/// Aktuelle lokale Zeit mit Offset (Sommerzeit über `jiff`, ADR 0001 §8.7) für
/// [`SignPlan::signing_time`] – `/M` und signing-time; Sekundenbruchteile entfallen (PDF-Datum und
/// UTCTime haben Sekundenauflösung).
pub fn local_signing_time() -> time::OffsetDateTime {
    let now = time::OffsetDateTime::now_utc();
    let offset = jiff::Zoned::now().offset().seconds();
    let off = time::UtcOffset::from_whole_seconds(offset).unwrap_or(time::UtcOffset::UTC);
    let t = now.to_offset(off);
    t.replace_nanosecond(0).unwrap_or(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(PadesError::Rejected("pdf-docmdp-p1".into()).code(), "pdf-docmdp-p1");
        assert_eq!(PadesError::NewPageNotAllowed.code(), "pades-new-page-not-allowed");
        let e = FinishError::CmsTooLarge { needed: 5000, reserve: 4096 };
        assert_eq!(e.code(), "cms-too-large");
        assert_eq!(e.suggested_reserve(), Some(6144));
        assert_eq!(FinishError::Cms(pdfover_cades::CmsError::DigestMismatch).code(), "cms-digest-mismatch");
    }

    #[test]
    fn field_names_are_not_displayed() {
        let e = PadesError::FieldAlreadySigned("Max Mustermann".into());
        assert!(!e.to_string().contains("Mustermann"), "{e}");
    }

    #[test]
    fn local_time_has_whole_seconds() {
        assert_eq!(local_signing_time().nanosecond(), 0);
    }
}

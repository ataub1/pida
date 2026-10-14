//! `pdfover-pades` – PAdES-Signatur-Engine (Plan §3.6, §6; docs/analyse/03).
//!
//! Ablauf: [`analyze`] → Annahmeklasse K1–K5 → [`SignPlan`] (Profil, Position, Feld) → ggf.
//! Zertifikat vom Backend → [`prepare`] → Backend `sign` → [`Prepared::finish_cms`] (Modell B) bzw.
//! [`Prepared::finish_raw`] (Modell A) → [`SignedPdf::self_check`] (Invarianten I1–I6) →
//! [`SignedPdf::write_atomically`]. Verletzt die Ausgabe eine Invariante, wird **nichts**
//! gespeichert.
//!
//! Zweiphasenmodell nach dem Vorbild der PDF-AS-API `startSign`/`finishSign`
//! (docs/analyse/03 §6.4): [`Prepared`] ist nach `prepare()` unveränderlich (WYSIWYS).
//!
//! Ownership: **Integration** (nutzt `pdf-cos`, `pdf-incremental`, `cades`, `sigblock`, `render`).

mod analyze;
mod plan;
mod prepared;
mod signed;

pub use analyze::{AcceptanceClass, DocMdp, DocReport, ExistingSignature, Finding, PdfaId, XfaKind, analyze};
pub use plan::{BlockPlacement, FieldTarget, Reserve, SignPlan, Visibility};
pub use prepared::{Prepared, PreparedHandle, prepare};
pub use signed::{
    CheckContext, CheckReport, Invariant, InvariantViolation, OutputInfo, OutputTarget, PriorStatus, SignedPdf,
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
    /// Eingabe abgelehnt (K5 oder Rechte; CLI-Exit 3).
    #[error("Dokument kann nicht signiert werden: {0}")]
    Rejected(String),
    /// Zielfeld ist bereits signiert.
    #[error("Signaturfeld {0} ist bereits signiert")]
    FieldAlreadySigned(String),
    /// Zielfeld ist gesperrt (`/Lock`).
    #[error("Signaturfeld {0} ist gesperrt")]
    FieldLocked(String),
    /// Zertifikat abgelaufen bzw. noch nicht gültig (PDF-AS 11021).
    #[error("Zertifikat ist zum Signierzeitpunkt nicht gültig")]
    CertificateNotValid,
    /// E/A
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
}

/// Fehler beim Abschließen.
#[derive(Debug, thiserror::Error)]
pub enum FinishError {
    /// CMS größer als die Reserve – nichts geschrieben; neu vorbereiten mit größerer Reserve
    /// (zweite Bestätigung nötig, Plan §6.2).
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

//! Signiertes PDF, Selbstprüfung I1–I6 und atomares Schreiben (Plan §3.6, §6.2 „Ausgabe“).

use std::path::PathBuf;

use pdfover_cades::Certificate;
use pdfover_pdf_cos::SourcePdf;
use pdfover_render::RenderBackend;

use crate::PadesError;

/// Invariante (Plan §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Invariant {
    /// Original ist byte-identischer Präfix
    I1Prefix,
    /// ByteRange deckt alles außer `<…>` ab, `b + c` = Dateilänge
    I2ByteRange,
    /// genau ein SignerInfo, message-digest, Signatur, signing-certificate(-v2), erwarteter Signer
    I3Cms,
    /// Status jeder Vorsignatur unverändert, ByteRange endet unverändert im Präfix
    I4PriorSignatures,
    /// mit `pdf-cos` **und** PDFium parsebar, Seitenzahl gleich bzw. +1, neue Signatur gefunden
    I5Reparse,
    /// Increment enthält nur erlaubte Änderungen (Whitelist)
    I6AllowedChanges,
}

/// Verletzung (Fehlercode für UI/CLI-Exit 6; optional Diagnosepaket ohne Dokumentinhalt).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("Invariante {invariant:?} verletzt: {detail}")]
pub struct InvariantViolation {
    /// Invariante
    pub invariant: Invariant,
    /// Detail (ohne Dokumentinhalt)
    pub detail: String,
}

/// Status einer Vorsignatur vorher/nachher (I4: gleich, nicht zwingend gültig).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PriorStatus {
    /// gültig
    Valid,
    /// ungültig
    Invalid,
    /// vom eigenen Verifier nicht prüfbar (z. B. `adbe.pkcs7.sha1`)
    NotVerifiable,
}

/// Kontext der Selbstprüfung.
#[derive(Clone, Copy)]
pub struct CheckContext<'a> {
    /// Original
    pub source: &'a SourcePdf,
    /// erwarteter Signer (InfoboxRead, Cache, Keystore)
    pub expected_signer: Option<&'a Certificate>,
    /// Renderer für I5 (PDFium); `None` nur in Tests ohne Bibliothek
    pub renderer: Option<&'a dyn RenderBackend>,
}

impl std::fmt::Debug for CheckContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckContext")
            .field("expected_signer", &self.expected_signer)
            .field("renderer", &self.renderer.is_some())
            .finish()
    }
}

/// Bericht der Selbstprüfung.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CheckReport {
    /// geprüfte Invarianten
    pub checked: Vec<Invariant>,
    /// Status der Vorsignaturen (Feldname, vorher, nachher)
    pub prior: Vec<(String, PriorStatus, PriorStatus)>,
    /// Warnungen (z. B. I5 ohne PDFium übersprungen)
    pub warnings: Vec<String>,
}

/// Ziel der Ausgabe (Regeln F-81…F-85 löst der Workflow auf).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputTarget {
    /// Zieldatei
    pub path: PathBuf,
    /// Überschreiben erlaubt (nach Rückfrage)
    pub overwrite: bool,
}

/// Ergebnis des Schreibens.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputInfo {
    /// Pfad
    pub path: PathBuf,
    /// Länge
    pub len: u64,
    /// SHA-256 (hex)
    pub sha256: String,
}

/// Signiertes PDF (Original + Increment mit eingesetztem CMS).
#[derive(Debug)]
pub struct SignedPdf {
    original: std::sync::Arc<[u8]>,
    increment: Vec<u8>,
}

impl SignedPdf {
    /// Prüft I1–I6.
    pub fn self_check(&self, ctx: &CheckContext<'_>) -> Result<CheckReport, InvariantViolation> {
        todo!("Integration")
    }

    /// Länge.
    pub fn len(&self) -> u64 {
        self.original.len() as u64 + self.increment.len() as u64
    }

    /// `true` bei Länge 0 (nie).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Vollständige Bytes (Kopie; für große Dateien [`Self::write_to`] bevorzugen).
    pub fn bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.len() as usize);
        v.extend_from_slice(&self.original);
        v.extend_from_slice(&self.increment);
        v
    }

    /// Schreibt in einen Writer (Mobile: SAF/`UIDocumentPicker`-Stream).
    pub fn write_to(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        w.write_all(&self.original)?;
        w.write_all(&self.increment)
    }

    /// Temp-Datei im Zielordner, `fsync`, atomares Umbenennen; das Original wird nie
    /// überschrieben, außer `target.overwrite`.
    pub fn write_atomically(&self, target: &OutputTarget) -> Result<OutputInfo, PadesError> {
        todo!("Integration")
    }
}

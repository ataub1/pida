//! Zweiphasenmodell: vorbereitetes PDF mit Platzhalter (Plan §3.4, §6.2).

use std::io::Read;
use std::path::{Path, PathBuf};

use pdfover_cades::{CertChain, Digest, DigestAlg, Expect, RawSignature, SignedAttrs};
use pdfover_pdf_cos::SourcePdf;
use pdfover_pdf_incremental::{ByteRange, ExcludedRange, Increment};
use time::OffsetDateTime;

use crate::plan::SignPlan;
use crate::signed::SignedPdf;
use crate::{FinishError, PadesError};

/// Vorbereitetes, nach `prepare()` unveränderliches Dokument. `Debug` ohne Inhalt (Regel 9).
pub struct Prepared {
    original: std::sync::Arc<[u8]>,
    increment: Increment,
    signing_time: OffsetDateTime,
    plan_profile_id: String,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("original_len", &self.original.len())
            .field("increment", &self.increment)
            .field("signing_time", &self.signing_time)
            .field("profile_id", &self.plan_profile_id)
            .finish()
    }
}

/// Persistierter Zustand (Mobile-Lebenszyklus: App-Wechsel ist kein Abbruch, Plan §3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedHandle {
    /// Datei in der App-Sandbox (Rechte 0600)
    pub path: PathBuf,
    /// SHA-256 des fertigen Platzhalter-PDFs (Integritätsprüfung beim Fortsetzen)
    pub sha256: Vec<u8>,
}

/// Baut das Increment (AcroForm, Feld/Widget, Signaturdictionary, Appearance, ggf. neue Seite,
/// OutputIntent, PDF/UA) und patcht die ByteRange. Prüft vorher die Zertifikatsgültigkeit.
pub fn prepare(src: &SourcePdf, plan: &SignPlan, cert: Option<&CertChain>) -> Result<Prepared, PadesError> {
    todo!("Integration")
}

impl Prepared {
    /// `/ByteRange`.
    pub fn byte_range(&self) -> ByteRange {
        self.increment.byte_range
    }

    /// Für SL `ExcludedByteRange` (From/To inklusive).
    pub fn excluded_range(&self) -> ExcludedRange {
        self.increment.excluded_range()
    }

    /// Länge des fertigen PDFs.
    pub fn total_len(&self) -> u64 {
        self.increment.total_len()
    }

    /// CMS-Reserve in Bytes.
    pub fn reserve(&self) -> usize {
        self.increment.reserve()
    }

    /// Signierzeitpunkt (`/M`).
    pub fn signing_time(&self) -> OffsetDateTime {
        self.signing_time
    }

    /// Profil-ID des Plans.
    pub fn profile_id(&self) -> &str {
        &self.plan_profile_id
    }

    /// Modell B: komplettes PDF mit `0x00` in der Lücke `<…>`, gestreamt.
    pub fn blacked_out(&self) -> Box<dyn Read + Send + '_> {
        todo!("Integration")
    }

    /// Modell A: Hash über die ByteRange-Daten (gestreamt).
    pub fn digest(&self, alg: DigestAlg) -> Digest {
        todo!("Integration")
    }

    /// Fertiges Platzhalter-PDF (WYSIWYS-Ansicht rendert genau diese Bytes, Plan §4.5).
    pub fn placeholder_pdf(&self) -> Vec<u8> {
        todo!("Integration")
    }

    /// Persistiert für den Mobile-Lebenszyklus.
    pub fn persist(&self, dir: &Path) -> Result<PreparedHandle, PadesError> {
        todo!("Integration")
    }

    /// Lädt einen persistierten Zustand (prüft SHA-256).
    pub fn restore(handle: &PreparedHandle, src: &SourcePdf) -> Result<Prepared, PadesError> {
        todo!("Integration")
    }

    /// Modell B: Längenprüfung, `verify_detached` (I3), Einbetten.
    pub fn finish_cms(self, cms: &[u8], expect: &Expect) -> Result<SignedPdf, FinishError> {
        todo!("Integration")
    }

    /// Modell A: `assemble()` aus Attributen, Rohsignatur und Kette, dann wie `finish_cms`.
    pub fn finish_raw(
        self,
        attrs: &SignedAttrs,
        signature: &RawSignature,
        chain: &CertChain,
    ) -> Result<SignedPdf, FinishError> {
        todo!("Integration")
    }
}

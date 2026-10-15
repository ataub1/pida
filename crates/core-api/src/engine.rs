//! Naht zur PAdES-Engine (`pdfover-pades`).
//!
//! Der Kern ruft `pades` ausschließlich über [`PadesEngine`], [`PreparedDoc`] und [`SignedDoc`] auf.
//! [`RealPades`] leitet 1:1 an `pdfover_pades` weiter; Tests setzen eine Fake-Engine ein
//! ([`crate::CoreOptions::engine`]), damit der ganze Ablauf (Backends, Broker, Ausgabe) ohne die
//! echte Engine prüfbar ist. Die Typen an der Naht sind die von `pdfover-pades` (`DocReport`,
//! `SignPlan`, `FinishError`, `InvariantViolation`, …) – die Fake-Engine baut sie selbst.

use std::io::Read;
use std::sync::Arc;

use pdfover_cades::{CertChain, Digest, DigestAlg, Expect, RawSignature, SignedAttrs};
use pdfover_pades::{
    CheckContext, CheckReport, DocReport, ExcludedRange, FinishError, InvariantViolation, OutputInfo, OutputTarget,
    PadesError, Prepared, SignPlan, SignedPdf,
};
use pdfover_pdf_cos::SourcePdf;
use time::OffsetDateTime;

/// PAdES-Engine.
pub trait PadesEngine: Send + Sync + std::fmt::Debug {
    /// Eingangsanalyse (K1–K5).
    fn analyze(&self, src: &SourcePdf) -> Result<DocReport, PadesError>;

    /// Vorbereiten (Increment mit Platzhalter).
    fn prepare(
        &self,
        src: &SourcePdf,
        plan: &SignPlan,
        cert: Option<&CertChain>,
    ) -> Result<Box<dyn PreparedDoc>, PadesError>;
}

/// Vorbereitetes Dokument (nach `prepare` unveränderlich, WYSIWYS).
pub trait PreparedDoc: Send + Sync + std::fmt::Debug {
    /// `ExcludedByteRange` (From/To inklusive).
    fn excluded_range(&self) -> ExcludedRange;
    /// Länge des fertigen PDFs.
    fn total_len(&self) -> u64;
    /// CMS-Reserve in Bytes.
    fn reserve(&self) -> usize;
    /// `/M`.
    fn signing_time(&self) -> OffsetDateTime;
    /// Modell B: Blackout-PDF.
    fn blacked_out(&self) -> Box<dyn Read + Send + '_>;
    /// Modell A: Hash über die ByteRange-Daten.
    fn digest(&self, alg: DigestAlg) -> Digest;
    /// Modell B abschließen.
    fn finish_cms(self: Box<Self>, cms: &[u8], expect: &Expect) -> Result<Box<dyn SignedDoc>, FinishError>;
    /// Modell A abschließen.
    fn finish_raw(
        self: Box<Self>,
        attrs: &SignedAttrs,
        signature: &RawSignature,
        chain: &CertChain,
    ) -> Result<Box<dyn SignedDoc>, FinishError>;
}

/// Signiertes Dokument.
pub trait SignedDoc: Send + Sync + std::fmt::Debug {
    /// Selbstprüfung I1–I6.
    fn self_check(&self, ctx: &CheckContext<'_>) -> Result<CheckReport, InvariantViolation>;
    /// Atomar schreiben.
    fn write_atomically(&self, target: &OutputTarget) -> Result<OutputInfo, PadesError>;
    /// Länge.
    fn len(&self) -> u64;
    /// Leer (nie)?
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Die echte Engine (`pdfover-pades`).
#[derive(Debug, Clone, Copy, Default)]
pub struct RealPades;

impl PadesEngine for RealPades {
    fn analyze(&self, src: &SourcePdf) -> Result<DocReport, PadesError> {
        pdfover_pades::analyze(src)
    }

    fn prepare(
        &self,
        src: &SourcePdf,
        plan: &SignPlan,
        cert: Option<&CertChain>,
    ) -> Result<Box<dyn PreparedDoc>, PadesError> {
        Ok(Box::new(pdfover_pades::prepare(src, plan, cert)?))
    }
}

impl PreparedDoc for Prepared {
    fn excluded_range(&self) -> ExcludedRange {
        Prepared::excluded_range(self)
    }
    fn total_len(&self) -> u64 {
        Prepared::total_len(self)
    }
    fn reserve(&self) -> usize {
        Prepared::reserve(self)
    }
    fn signing_time(&self) -> OffsetDateTime {
        Prepared::signing_time(self)
    }
    fn blacked_out(&self) -> Box<dyn Read + Send + '_> {
        Prepared::blacked_out(self)
    }
    fn digest(&self, alg: DigestAlg) -> Digest {
        Prepared::digest(self, alg)
    }
    fn finish_cms(self: Box<Self>, cms: &[u8], expect: &Expect) -> Result<Box<dyn SignedDoc>, FinishError> {
        Ok(Box::new(Prepared::finish_cms(*self, cms, expect)?))
    }
    fn finish_raw(
        self: Box<Self>,
        attrs: &SignedAttrs,
        signature: &RawSignature,
        chain: &CertChain,
    ) -> Result<Box<dyn SignedDoc>, FinishError> {
        Ok(Box::new(Prepared::finish_raw(*self, attrs, signature, chain)?))
    }
}

impl SignedDoc for SignedPdf {
    fn self_check(&self, ctx: &CheckContext<'_>) -> Result<CheckReport, InvariantViolation> {
        SignedPdf::self_check(self, ctx)
    }
    fn write_atomically(&self, target: &OutputTarget) -> Result<OutputInfo, PadesError> {
        SignedPdf::write_atomically(self, target)
    }
    fn len(&self) -> u64 {
        SignedPdf::len(self)
    }
}

/// Standard-Engine.
pub fn default_engine() -> Arc<dyn PadesEngine> {
    Arc::new(RealPades)
}

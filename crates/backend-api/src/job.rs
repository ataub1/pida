//! Signieraufträge und Ergebnisse (Plan §3.4; docs/analyse/02 §2.3).

use std::io::Read;
use std::sync::Arc;

use pida_cades::{CertChain, Certificate, DigestAlg, RawSignature};

/// `ExcludedByteRange` (Offset von `<` bis Offset von `>`, beide inklusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExcludedByteRange {
    /// From
    pub from: u64,
    /// To
    pub to: u64,
}

/// Vorbereitetes Dokument für Modell B. Implementiert vom Workflow als Adapter um
/// `pida_pades::Prepared` (das nach `prepare()` unveränderlich ist, WYSIWYS).
pub trait DocumentSource: Send + Sync + std::fmt::Debug {
    /// Anzeigename (Dateiname) für Masken und Kopfzeile.
    fn display_name(&self) -> &str;

    /// Länge des fertigen PDFs (Original + Increment).
    fn len(&self) -> u64;

    /// `true` bei Länge 0 (praktisch nie).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Ausgeschlossener Bereich.
    fn excluded_range(&self) -> ExcludedByteRange;

    /// Gestreamtes Blackout-PDF (Lücke `<…>` mit `0x00` gefüllt).
    fn open_blacked_out(&self) -> std::io::Result<Box<dyn Read + Send + '_>>;

    /// Blackout-PDF vollständig (Upload `fileupload` bzw. `Base64Content`).
    fn blacked_out_bytes(&self) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(self.len() as usize);
        self.open_blacked_out()?.read_to_end(&mut out)?;
        Ok(out)
    }
}

/// Modell-B-Auftrag.
///
/// Backends dürfen [`DocumentJob::source`] nicht über das Ende von `BackendSession::sign` hinaus
/// halten: Der Kern übernimmt danach das zugrunde liegende `pida_pades::Prepared` wieder
/// exklusiv (`finish_cms(self, …)`).
#[derive(Debug, Clone)]
pub struct DocumentJob {
    /// Vorbereitetes Dokument.
    pub source: Arc<dyn DocumentSource>,
    /// Erwartetes Signerzertifikat (InfoboxRead/Cache) – das Backend prüft nicht selbst, der Kern
    /// prüft I3 in `finish_cms`; das Feld dient der Diagnose und dem Zertifikats-Cache.
    pub expected_signer: Option<Certificate>,
}

/// Modell-A-Auftrag (Hash über die SignedAttributes).
///
/// Vertrag (ADR 0001 §8, wie `pida_cades::RawSigner`): Der Kern wählt `alg` mit
/// `pida_cades::select_algorithm(&chain.signer.key_type()?)` aus dem Zertifikat von
/// `BackendSession::certificate` und hasht damit ByteRange-Daten und SignedAttributes. Das Backend
/// signiert **genau mit `alg`**; das Ergebnis [`SignResult::Raw`] trägt `signature.alg` mit
/// `signature.alg.digest() == alg` und die Kette, deren Signer gleich dem vorab gelieferten
/// Zertifikat ist. Ein nicht unterstütztes `alg` ergibt `BackendError::Internal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestJob {
    /// Anzeigename.
    pub display_name: String,
    /// Hashverfahren (der SignedAttributes und der Signatur).
    pub alg: DigestAlg,
    /// Hashwert über das DER der SignedAttributes (`SignedAttrs::digest`).
    pub digest: Vec<u8>,
}

/// Auftrag.
#[derive(Debug, Clone)]
pub enum SignJob {
    /// Modell B
    Document(DocumentJob),
    /// Modell A
    Digest(DigestJob),
}

/// Ergebnis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignResult {
    /// DER-CMS (SignedData, detached)
    Cms(Vec<u8>),
    /// Rohsignatur mit Kette
    Raw {
        /// Signatur
        signature: RawSignature,
        /// Kette
        chain: CertChain,
    },
}

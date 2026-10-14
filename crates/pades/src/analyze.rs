//! Eingangsanalyse und Annahmepolitik (Plan §6.1).

use pdfover_pdf_cos::{SigField, SourcePdf, TailKind, XrefKind};
use pdfover_pdf_incremental::ByteRange;

use crate::PadesError;

/// Annahmeklasse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AcceptanceClass {
    /// K1 regulär (Tabelle, Stream, Hybrid, Object Streams, linearisiert, bereits signiert;
    /// Leerraum nach `%%EOF`)
    K1Regular,
    /// K2 reparierbar (nur ohne Signaturen, nur nach Zustimmung; erst nach Korpus-Freigabe aktiv)
    K2Repairable,
    /// K3 verschlüsselt (Standard-Handler ohne User-Passwort, Rechte wie PDF-AS)
    K3Encrypted,
    /// K4 eingeschränkt (DocMDP P=2/3, `/Lock`, Seed Values)
    K4Restricted,
    /// K5 abgelehnt (DocMDP P=1, User-Passwort, Public-Key, 0 Seiten, dynamisches XFA …)
    K5Rejected,
}

/// Einzelbefund mit stabilem Code (Fehlercodes für UI/CLI, Plan §8.9 G2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Finding {
    /// Code, z. B. `pdf-docmdp-p1`, `pdf-user-password`, `pdf-xfa-dynamic`, `pdf-usage-rights`
    pub code: String,
    /// Klasse, die der Befund bedingt
    pub class: AcceptanceClass,
    /// Nur Warnung (Abbruch möglich), z. B. Hybrid-XFA, `/Perms /UR3`, inkompatible Seed Values
    pub warning: bool,
}

/// DocMDP-Zertifizierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DocMdp {
    /// Berechtigung `P` (1, 2, 3)
    pub permission: u8,
}

/// PDF/A-Kennung aus XMP.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PdfaId {
    /// `pdfaid:part`
    pub part: u8,
    /// `pdfaid:conformance`
    pub conformance: Option<String>,
}

/// XFA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum XfaKind {
    /// kein XFA
    None,
    /// AcroForm + `/XFA` (Hinweis)
    Hybrid,
    /// dynamisch (K5)
    Dynamic,
}

/// Vorhandene Signatur (für I4, Feldnamen, belegte QR-Platzhalter).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExistingSignature {
    /// Feldname
    pub field_name: String,
    /// `/SubFilter`
    pub sub_filter: Option<String>,
    /// `/ByteRange`
    pub byte_range: Option<ByteRange>,
    /// `/Contents` (DER ohne Padding, falls trennbar)
    pub contents: Vec<u8>,
    /// `/Location` (Platzhaltername)
    pub location: Option<String>,
    /// ByteRange endet am Ende einer Revision
    pub covers_revision: Option<usize>,
}

/// Analysebericht.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocReport {
    /// Klasse (schärfster Befund)
    pub class: AcceptanceClass,
    /// Befunde
    pub findings: Vec<Finding>,
    /// Seitenzahl
    pub page_count: usize,
    /// xref-Art der letzten Revision
    pub xref_kind: XrefKind,
    /// Dateiende
    pub tail: TailKind,
    /// verschlüsselt
    pub encrypted: bool,
    /// Rechte erlauben das Signieren
    pub may_sign: bool,
    /// Signaturfelder (signiert und leer)
    pub fields: Vec<SigField>,
    /// vorhandene Signaturen
    pub signatures: Vec<ExistingSignature>,
    /// DocMDP
    pub doc_mdp: Option<DocMdp>,
    /// PDF/A-Kennung
    pub pdfa: Option<PdfaId>,
    /// StructTreeRoot mit Kindern (PDF/UA-Ergänzung)
    pub tagged: bool,
    /// XFA
    pub xfa: XfaKind,
    /// `/Perms /UR3`
    pub usage_rights: bool,
    /// `/NeedAppearances true`
    pub need_appearances: bool,
    /// nächste freie Nummer für „PDF-AS Signatur`<n>`“ (nur oberste Ebene, Präfix-Zählung)
    pub next_field_number: u32,
}

impl DocReport {
    /// Bereits signiert (neue Seite nicht erlaubt).
    pub fn is_signed(&self) -> bool {
        !self.signatures.is_empty()
    }

    /// `/Location`-Werte vorhandener Signaturen (belegte QR-Platzhalter).
    pub fn used_placeholder_names(&self) -> Vec<String> {
        self.signatures.iter().filter_map(|s| s.location.clone()).collect()
    }
}

/// Analysiert ein geöffnetes Dokument. Fehler beim Öffnen (Passwort, Public-Key, 0 Seiten) bildet
/// der Aufrufer über [`AcceptanceClass::K5Rejected`] ab.
pub fn analyze(src: &SourcePdf) -> Result<DocReport, PadesError> {
    todo!("Integration")
}

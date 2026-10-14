//! `pdfover-pdf-incremental` – eigener Append-only-Writer (Plan §6.2, §6.3, §6.4).
//!
//! Originalbytes werden **nie** neu serialisiert. Der Writer hängt ein Increment an, das nur neue
//! bzw. geänderte Objekte enthält, xref im Format der letzten Revision (Tabelle → Tabelle,
//! Stream → xref-Stream, Hybrid → beides mit `/XRefStm`), `/Prev`, `/Size`, `/ID[1]` neu.
//! Object Streams werden nie geschrieben.
//!
//! Das Signaturdictionary serialisiert der Writer selbst in PDF-AS-Reihenfolge
//! (`/Type /Filter /SubFilter /M /Location /Name /Reason /Contents /ByteRange`,
//! docs/analyse/03 §4.1–§4.2), damit `/Contents` strukturell unverschlüsselt bleibt und die Offsets
//! beim Schreiben protokolliert werden können:
//! - `/Contents <00…>` mit fester Hex-Länge `2 × reserve`,
//! - `/ByteRange [0 1000000000 1000000000 1000000000]`, nach dem Patchen linksbündig mit Leerzeichen
//!   auf 36 Zeichen aufgefüllt,
//! - CMS in Großbuchstaben-Hex, Rest `0`.
//!
//! Das Increment beginnt wie bei PDF-AS mit CRLF (docs/analyse/03 Phase E Schritt 4).
//!
//! Ownership: Paket **A**.

use std::ops::Range;

use pdfover_pdf_cos::{CosError, Object, ObjectId, SourcePdf};

/// Platzhalter der ByteRange (36 Zeichen, docs/analyse/03 §4.2).
pub const BYTE_RANGE_PLACEHOLDER: &str = "[0 1000000000 1000000000 1000000000]";

/// Standard-Reserve für Modell B (SL, A-Trust, BKU): 16 KiB (Plan §6.2).
pub const RESERVE_MODEL_B: usize = 16 * 1024;

/// Mindestreserve für Modell A wie PDF-AS (`0x1000`, docs/analyse/03 §4.2).
pub const RESERVE_MIN_MODEL_A: usize = 0x1000;

/// Fehler des Writers.
#[derive(Debug, thiserror::Error)]
pub enum IncError {
    /// Fehler aus dem Leser.
    #[error(transparent)]
    Cos(#[from] CosError),
    /// Objekt existiert im Original nicht (für `update_object`).
    #[error("Objekt {0:?} existiert nicht im Original")]
    UnknownObject(ObjectId),
    /// Objekt wurde im selben Increment bereits gesetzt.
    #[error("Objekt {0:?} wurde bereits gesetzt")]
    DuplicateObject(ObjectId),
    /// Mehr als ein Signaturdictionary in einem Increment.
    #[error("nur ein Signaturdictionary je Increment")]
    MultipleSignatureDictionaries,
    /// Operation verlangt ein Signaturdictionary.
    #[error("kein Signaturdictionary im Increment")]
    NoSignatureDictionary,
    /// CMS passt nicht in die Reserve (harte Längenprüfung vor dem Schreiben, Plan §6.2).
    #[error("CMS zu groß: {needed} Bytes benötigt, {available} reserviert")]
    ContentsTooLarge {
        /// Länge des CMS in Bytes.
        needed: usize,
        /// Reserve in Bytes.
        available: usize,
    },
    /// Der ByteRange-Text passt nicht in den 36-Zeichen-Platzhalter.
    #[error("ByteRange passt nicht in den Platzhalter")]
    ByteRangeOverflow,
    /// Fehler beim Verschlüsseln neuer Objekte.
    #[error("Verschlüsselung neuer Objekte fehlgeschlagen: {0}")]
    Encryption(String),
    /// Objekt nicht serialisierbar.
    #[error("Serialisierung fehlgeschlagen: {0}")]
    Serialize(String),
}

/// Ergebnis-Alias.
pub type Result<T, E = IncError> = std::result::Result<T, E>;

/// `/ByteRange [a b c d]` als absolute Offsets im fertigen PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ByteRange(pub [u64; 4]);

impl ByteRange {
    /// ByteRange aus der Lage des `/Contents`-Hex-Strings inklusive `<` und `>` (Formel aus
    /// docs/analyse/02 §2.3): `[0, a, b, len − b]` mit `a` = Offset von `<`, `b` = Offset nach `>`.
    pub fn for_contents(contents: Range<u64>, total_len: u64) -> Self {
        ByteRange([0, contents.start, contents.end, total_len - contents.end])
    }

    /// Ausgeschlossener Bereich für SL `ExcludedByteRange` (From/To inklusive).
    pub fn excluded(&self) -> ExcludedRange {
        ExcludedRange { from: self.0[0] + self.0[1], to: self.0[2] - 1 }
    }

    /// Anzahl der signierten Bytes.
    pub fn covered_len(&self) -> u64 {
        self.0[1] + self.0[3]
    }

    /// Invariante I2: `b + c` = Dateilänge, Lücke nicht leer, Start bei 0.
    pub fn is_consistent_with(&self, total_len: u64) -> bool {
        self.0[0] == 0 && self.0[1] < self.0[2] && self.0[2] + self.0[3] == total_len
    }

    /// PDF-Text `[0 a b c]` (ohne Padding).
    pub fn to_pdf_string(&self) -> String {
        format!("[{} {} {} {}]", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

/// Ausgeschlossener Bereich (Offset von `<` bis Offset von `>`, beide inklusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExcludedRange {
    /// Offset von `<`.
    pub from: u64,
    /// Offset von `>`.
    pub to: u64,
}

/// Inhalt des Signaturdictionarys (Reihenfolge der Schlüssel fest, docs/analyse/03 §4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureDictSpec {
    /// `/Filter`, Default `Adobe.PPKLite`.
    pub filter: String,
    /// `/SubFilter`, Default `ETSI.CAdES.detached`.
    pub sub_filter: String,
    /// `/M` als fertiger PDF-Datumsstring ohne Klammern, siehe [`format_pdf_date`].
    pub signing_time: Option<String>,
    /// `/Location` (nur QR-Platzhalter: Ressourcenname).
    pub location: Option<String>,
    /// `/Name` (Subject-CN; entfällt bei unsichtbarer Signatur ohne Zertifikat).
    pub name: Option<String>,
    /// `/Reason` (`adobeSignReasonValue` oder „PAdES Signature“).
    pub reason: Option<String>,
    /// Reservierte CMS-Bytes (Hex-Länge = 2 × reserve).
    pub contents_reserve: usize,
}

impl Default for SignatureDictSpec {
    fn default() -> Self {
        Self {
            filter: "Adobe.PPKLite".into(),
            sub_filter: "ETSI.CAdES.detached".into(),
            signing_time: None,
            location: None,
            name: None,
            reason: None,
            contents_reserve: RESERVE_MODEL_B,
        }
    }
}

/// `D:YYYYMMDDHHmmSS+HH'mm'` in der Zeitzone von `dt` (docs/analyse/03 §4.1 `/M`).
pub fn format_pdf_date(dt: time::OffsetDateTime) -> String {
    let off = dt.offset();
    let (h, m, _) = off.as_hms();
    let sign = if off.is_negative() { '-' } else { '+' };
    format!(
        "D:{:04}{:02}{:02}{:02}{:02}{:02}{}{:02}'{:02}'",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        sign,
        h.unsigned_abs(),
        m.unsigned_abs()
    )
}

/// Format des Querverweises des neuen Increments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum XrefFormat {
    /// Wie die letzte Revision (Plan §6.1 K1).
    #[default]
    LikeLastRevision,
    /// Erzwungen: Tabelle.
    Table,
    /// Erzwungen: xref-Stream (Flate, Feldbreiten nach Bedarf).
    Stream,
    /// Erzwungen: Tabelle + xref-Stream mit `/XRefStm`.
    Hybrid,
}

/// Optionen des Writers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementOptions {
    /// xref-Format.
    pub xref: XrefFormat,
    /// Increment mit CRLF beginnen (PDF-AS-Verhalten).
    pub leading_crlf: bool,
    /// Neues `/ID[1]`; `None` = aus Zeit und `/Info` berechnet (wie PDFBox, SHA-256).
    pub new_id: Option<Vec<u8>>,
    /// Neue Streams mit Flate komprimieren (Appearance-Streams bleiben wie PDF-AS unkomprimiert,
    /// das entscheidet der Aufrufer über `/Filter`).
    pub compress_new_streams: bool,
}

impl Default for IncrementOptions {
    fn default() -> Self {
        Self { xref: XrefFormat::LikeLastRevision, leading_crlf: true, new_id: None, compress_new_streams: false }
    }
}

/// Rolle eines geschriebenen Objekts (Grundlage für Invariante I6, Plan §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WrittenKind {
    /// Neues Objekt.
    New,
    /// Neue Version eines vorhandenen Objekts.
    Changed,
    /// Das vom Writer selbst serialisierte Signaturdictionary.
    SignatureDictionary,
    /// xref-Stream des Increments.
    XrefStream,
}

/// Baut ein Increment über einem [`SourcePdf`].
#[derive(Debug)]
pub struct IncrementBuilder<'a> {
    src: &'a SourcePdf,
    opts: IncrementOptions,
    next_number: u32,
    objects: Vec<(ObjectId, Object, WrittenKind)>,
    signature: Option<(ObjectId, SignatureDictSpec)>,
}

impl<'a> IncrementBuilder<'a> {
    /// Neuer Builder; Objektnummern beginnen bei [`SourcePdf::next_object_number`].
    pub fn new(src: &'a SourcePdf, opts: IncrementOptions) -> Self {
        todo!("Paket A")
    }

    /// Zugrunde liegendes Dokument.
    pub fn source(&self) -> &'a SourcePdf {
        self.src
    }

    /// Reserviert eine neue Objekt-ID (für Vorwärtsreferenzen, z. B. Appearance-Graph).
    pub fn allocate_id(&mut self) -> ObjectId {
        todo!("Paket A")
    }

    /// Fügt ein neues Objekt mit neuer ID hinzu.
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        todo!("Paket A")
    }

    /// Setzt ein neues Objekt unter einer zuvor mit [`Self::allocate_id`] reservierten ID.
    pub fn insert_new(&mut self, id: ObjectId, object: Object) -> Result<()> {
        todo!("Paket A")
    }

    /// Schreibt eine neue Version eines vorhandenen Objekts (Katalog, AcroForm, Seite, `/Annots`, Feld,
    /// Widget, StructTree …). Objekte aus Object Streams werden als normale Objekte fortgeschrieben.
    pub fn update_object(&mut self, id: ObjectId, object: Object) -> Result<()> {
        todo!("Paket A")
    }

    /// Legt das Signaturdictionary an (höchstens eines). Liefert seine ID für `/V` des Felds.
    pub fn add_signature_dictionary(&mut self, spec: SignatureDictSpec) -> Result<ObjectId> {
        todo!("Paket A")
    }

    /// IDs aller bisher gesetzten Objekte mit Rolle.
    pub fn pending(&self) -> Vec<(ObjectId, WrittenKind)> {
        self.objects.iter().map(|(id, _, k)| (*id, *k)).collect()
    }

    /// Serialisiert das Increment, schreibt xref/Trailer und patcht die ByteRange
    /// (ruft [`Increment::patch_byte_range`] auf). `/Contents` bleibt `0`-gefüllt.
    pub fn build(self) -> Result<Increment> {
        todo!(
            "Paket A: Objekte serialisieren (Offsets protokollieren), neue Strings/Streams außer /Contents verschlüsseln, xref im Format der Vorrevision"
        )
    }
}

/// Fertig serialisiertes Increment (nur die angehängten Bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Increment {
    /// Länge des Originals = Offset, an dem das Increment beginnt.
    pub base_len: u64,
    /// Angehängte Bytes (beginnen mit CRLF).
    pub bytes: Vec<u8>,
    /// Absolute Offsets des `/Contents`-Hex-Strings inklusive `<` und `>` (`start` = Offset von `<`,
    /// `end` = Offset nach `>`); leer, wenn kein Signaturdictionary geschrieben wurde.
    pub contents_range: Range<u64>,
    /// Absolute Offsets des 36-Zeichen-Feldes der ByteRange (inklusive `[`).
    pub byte_range_field: Range<u64>,
    /// ByteRange (nach [`Self::patch_byte_range`] gültig).
    pub byte_range: ByteRange,
    /// ID des Signaturdictionarys.
    pub signature_dict_id: Option<ObjectId>,
    /// Alle geschriebenen Objekte mit Rolle (für I6).
    pub written: Vec<(ObjectId, WrittenKind)>,
}

impl Increment {
    /// Länge der fertigen Datei (Original + Increment).
    pub fn total_len(&self) -> u64 {
        self.base_len + self.bytes.len() as u64
    }

    /// Kapazität für das CMS in Bytes (`(Hex-Länge) / 2`).
    pub fn reserve(&self) -> usize {
        let hex = (self.contents_range.end - self.contents_range.start).saturating_sub(2);
        (hex / 2) as usize
    }

    /// Ausgeschlossener Bereich für SL.
    pub fn excluded_range(&self) -> ExcludedRange {
        self.byte_range.excluded()
    }

    /// Relativer Bereich im Increment zu einem absoluten Bereich.
    pub fn local(&self, abs: Range<u64>) -> Range<usize> {
        (abs.start - self.base_len) as usize..(abs.end - self.base_len) as usize
    }

    /// Berechnet die ByteRange aus [`Self::contents_range`] und schreibt sie linksbündig,
    /// mit Leerzeichen aufgefüllt, in den Platzhalter.
    pub fn patch_byte_range(&mut self) -> Result<()> {
        todo!("Paket A")
    }

    /// Setzt das DER-CMS als Großbuchstaben-Hex ab `<`+1 ein; Rest bleibt `0`.
    /// Harte Längenprüfung: `cms.len() <= reserve()`, sonst [`IncError::ContentsTooLarge`].
    pub fn embed_contents(&mut self, cms_der: &[u8]) -> Result<()> {
        todo!("Paket A")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_range_formula() {
        let br = ByteRange::for_contents(80285..96479, 96479 + 71069);
        assert_eq!(br.0, [0, 80285, 96479, 71069]);
        assert_eq!(br.excluded(), ExcludedRange { from: 80285, to: 96478 });
        assert!(br.is_consistent_with(96479 + 71069));
        assert_eq!(br.to_pdf_string(), "[0 80285 96479 71069]");
        assert_eq!(BYTE_RANGE_PLACEHOLDER.len(), 36);
    }

    #[test]
    fn pdf_date() {
        let dt = time::macros::datetime!(2024-03-15 19:33:33 +01:00);
        assert_eq!(format_pdf_date(dt), "D:20240315193333+01'00'");
    }
}

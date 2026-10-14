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
//! Weitere Regeln (docs/adr/0010): neue Strings/Streams werden mit dem Handler des Dokuments
//! verschlüsselt (nie `/Contents`, xref-Streams oder `/Metadata` bei `EncryptMetadata false`);
//! fortgeschriebene Objekte behalten die Originalschreibweise unveränderter Reals (sonst wertet die
//! Änderungsanalyse von pyHanko die `f32`-Rundung als Änderung); `/Prev` zeigt auf den tatsächlichen
//! letzten Querverweisabschnitt; bei rekonstruiertem Querverweis wird kein Increment gebaut.
//!
//! Ownership: Paket **A**.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::ops::Range;

use pdfover_pdf_cos::{
    CosError, CryptTarget, Dictionary, Encryptor, Object, ObjectId, SourcePdf, StringFormat, XrefHealth, XrefKind,
    encode_text_string,
};

mod serialize;

/// Platzhalter der ByteRange (36 Zeichen, docs/analyse/03 §4.2).
pub const BYTE_RANGE_PLACEHOLDER: &str = "[0 1000000000 1000000000 1000000000]";

/// Standard-Reserve für Modell B (SL, A-Trust, BKU): 16 KiB (Plan §6.2). `pdfover-pades` wählt sie
/// über `Reserve::ModelB` bzw. `Capabilities::cms_reserve`.
pub const RESERVE_MODEL_B: usize = 16 * 1024;

/// Reserve wie PDF-AS (`signatureSize`-Default `0x1000`, docs/analyse/03 §4.2) – Default von
/// [`SignatureDictSpec`] und Mindestwert für Modell A.
pub const RESERVE_MIN_MODEL_A: usize = 0x1000;

/// Obergrenze der `/Contents`-Reserve (32 MiB CMS, 64 MiB Hex). Reale CMS liegen bei wenigen KB
/// (auch mit Kette und Zeitstempel unter 100 KB); die Grenze verhindert, dass ein fehlerhafter
/// Profilwert (`signatureSize`) oder eine Backend-Angabe eine riesige Allokation auslöst.
pub const RESERVE_MAX: usize = 32 * 1024 * 1024;

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
///
/// `Debug` zeigt `/Name` (Zertifikats-Personendaten) und `/Reason` nur als Länge (Regel 9).
#[derive(Clone, PartialEq, Eq)]
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
    /// Reservierte CMS-Bytes (Hex-Länge = 2 × reserve). Default [`RESERVE_MIN_MODEL_A`] (4096 wie
    /// PDF-AS); der Aufrufer (`pdfover-pades`) setzt sie immer explizit (Modell A: berechnet,
    /// ≥ 4096; Modell B: [`RESERVE_MODEL_B`] bzw. `Capabilities::cms_reserve`).
    pub contents_reserve: usize,
}

impl std::fmt::Debug for SignatureDictSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = |v: &Option<String>| v.as_ref().map(|s| s.chars().count());
        f.debug_struct("SignatureDictSpec")
            .field("filter", &self.filter)
            .field("sub_filter", &self.sub_filter)
            .field("signing_time", &self.signing_time)
            .field("location", &self.location)
            .field("name_len", &len(&self.name))
            .field("reason_len", &len(&self.reason))
            .field("contents_reserve", &self.contents_reserve)
            .finish()
    }
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
            contents_reserve: RESERVE_MIN_MODEL_A,
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

/// Baut ein Increment über einem [`SourcePdf`]. `Debug` ohne Objektinhalte (Regel 9).
pub struct IncrementBuilder<'a> {
    src: &'a SourcePdf,
    opts: IncrementOptions,
    next_number: u32,
    objects: Vec<(ObjectId, Object, WrittenKind)>,
    signature: Option<(ObjectId, SignatureDictSpec)>,
    /// mit `allocate_id` reserviert, aber noch nicht gesetzt
    allocated: BTreeSet<ObjectId>,
}

impl std::fmt::Debug for IncrementBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IncrementBuilder")
            .field("opts", &self.opts)
            .field("next_number", &self.next_number)
            .field("objects", &self.objects.len())
            .field("signature_dict", &self.signature.as_ref().map(|(id, _)| *id))
            .finish_non_exhaustive()
    }
}

/// Schlüssel des alten Trailers, die nicht übernommen werden (neu berechnet bzw. xref-Stream-Struktur).
const TRAILER_SKIP: &[&[u8]] = &[
    b"Prev",
    b"XRefStm",
    b"Size",
    b"ID",
    b"Root",
    b"Info",
    b"Encrypt",
    b"Type",
    b"W",
    b"Index",
    b"Filter",
    b"DecodeParms",
    b"Length",
    b"F",
    b"FFilter",
    b"FDecodeParms",
    b"DL",
];

impl<'a> IncrementBuilder<'a> {
    /// Neuer Builder; Objektnummern beginnen bei [`SourcePdf::next_object_number`].
    pub fn new(src: &'a SourcePdf, opts: IncrementOptions) -> Self {
        Self {
            src,
            opts,
            next_number: src.next_object_number(),
            objects: Vec::new(),
            signature: None,
            allocated: BTreeSet::new(),
        }
    }

    /// Zugrunde liegendes Dokument.
    pub fn source(&self) -> &'a SourcePdf {
        self.src
    }

    /// Reserviert eine neue Objekt-ID (für Vorwärtsreferenzen, z. B. Appearance-Graph).
    pub fn allocate_id(&mut self) -> ObjectId {
        let id = (self.next_number, 0);
        self.next_number = self.next_number.saturating_add(1);
        self.allocated.insert(id);
        id
    }

    /// Fügt ein neues Objekt mit neuer ID hinzu.
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        let id = (self.next_number, 0);
        self.next_number = self.next_number.saturating_add(1);
        self.objects.push((id, object, WrittenKind::New));
        id
    }

    /// Setzt ein neues Objekt unter einer zuvor mit [`Self::allocate_id`] reservierten ID.
    pub fn insert_new(&mut self, id: ObjectId, object: Object) -> Result<()> {
        if self.objects.iter().any(|(i, _, _)| *i == id) {
            return Err(IncError::DuplicateObject(id));
        }
        if !self.allocated.remove(&id) {
            return Err(IncError::UnknownObject(id));
        }
        self.objects.push((id, object, WrittenKind::New));
        Ok(())
    }

    /// Schreibt eine neue Version eines vorhandenen Objekts (Katalog, AcroForm, Seite, `/Annots`, Feld,
    /// Widget, StructTree …). Objekte aus Object Streams werden als normale Objekte fortgeschrieben.
    pub fn update_object(&mut self, id: ObjectId, object: Object) -> Result<()> {
        if !self.src.document().objects.contains_key(&id) {
            return Err(IncError::UnknownObject(id));
        }
        if self.objects.iter().any(|(i, _, _)| *i == id) {
            return Err(IncError::DuplicateObject(id));
        }
        self.objects.push((id, object, WrittenKind::Changed));
        Ok(())
    }

    /// Legt das Signaturdictionary an (höchstens eines). Liefert seine ID für `/V` des Felds.
    ///
    /// Fehler: [`IncError::MultipleSignatureDictionaries`]; [`IncError::Serialize`], wenn
    /// `contents_reserve` größer als [`RESERVE_MAX`] ist.
    pub fn add_signature_dictionary(&mut self, spec: SignatureDictSpec) -> Result<ObjectId> {
        if self.signature.is_some() {
            return Err(IncError::MultipleSignatureDictionaries);
        }
        if spec.contents_reserve > RESERVE_MAX {
            return Err(IncError::Serialize(format!(
                "/Contents-Reserve {} Bytes größer als {RESERVE_MAX}",
                spec.contents_reserve
            )));
        }
        let id = (self.next_number, 0);
        self.next_number = self.next_number.saturating_add(1);
        self.objects.push((id, Object::Null, WrittenKind::SignatureDictionary));
        self.signature = Some((id, spec));
        Ok(id)
    }

    /// IDs aller bisher gesetzten Objekte mit Rolle.
    pub fn pending(&self) -> Vec<(ObjectId, WrittenKind)> {
        self.objects.iter().map(|(id, _, k)| (*id, *k)).collect()
    }

    /// Serialisiert das Increment, schreibt xref/Trailer und patcht die ByteRange
    /// (ruft [`Increment::patch_byte_range`] auf). `/Contents` bleibt `0`-gefüllt.
    pub fn build(self) -> Result<Increment> {
        let src = self.src;
        if src.xref_health() == XrefHealth::Reconstructed {
            return Err(IncError::Cos(CosError::InvalidStructure(
                "Querverweis rekonstruiert – Increment mit /Prev nicht möglich (K2)".into(),
            )));
        }
        if let Some(id) = self.allocated.iter().next() {
            return Err(IncError::Serialize(format!("reservierte ID {} {} nicht gesetzt", id.0, id.1)));
        }
        // Der Zähler sättigt bei u32::MAX (die ID-Vergabe ist unfehlbar); erreicht er die Grenze,
        // wurden IDs womöglich doppelt vergeben und für den xref-Stream bleibt keine Nummer
        // (feindliches `/Size`).
        if self.next_number == u32::MAX {
            return Err(IncError::Serialize("Objektnummern erschöpft (/Size zu groß)".into()));
        }
        let base = src.len();
        let offset_base = src.xref_offset_base();
        let enc = src.encryptor();
        let encrypt_metadata = src.encryption().is_none_or(|e| e.encrypt_metadata);

        let mut out: Vec<u8> = Vec::new();
        if self.opts.leading_crlf {
            out.extend_from_slice(b"\r\n");
        } else if !src.bytes().last().is_some_and(|b| matches!(b, b'\n' | b'\r')) {
            out.push(b'\n');
        }

        let mut entries: BTreeMap<u32, (u64, u16)> = BTreeMap::new();
        let mut contents_range = 0..0;
        let mut byte_range_field = 0..0;
        let mut written: Vec<(ObjectId, WrittenKind)> = Vec::with_capacity(self.objects.len() + 1);

        for (id, obj, kind) in &self.objects {
            let abs = base + out.len() as u64;
            if entries.insert(id.0, (abs - offset_base, id.1)).is_some() {
                return Err(IncError::DuplicateObject(*id));
            }
            match kind {
                WrittenKind::SignatureDictionary => {
                    let spec = &self.signature.as_ref().ok_or(IncError::NoSignatureDictionary)?.1;
                    let (c, b) = write_signature_dict(&mut out, *id, spec, enc)?;
                    contents_range = base + c.start as u64..base + c.end as u64;
                    byte_range_field = base + b.start as u64..base + b.end as u64;
                }
                _ => {
                    let prepared = prepare_object(*id, obj, enc, encrypt_metadata, self.opts.compress_new_streams)?;
                    // Fortgeschriebene Objekte behalten die Originalschreibweise unveränderter Reals.
                    let lits = (*kind == WrittenKind::Changed).then(|| src.real_literals(*id));
                    serialize::write_indirect(&mut out, *id, &prepared, lits.as_ref())?;
                }
            }
            written.push((*id, *kind));
        }

        // /ID: ID[0] bleibt, ID[1] neu (PDFBox: SHA-256 aus Zeit und /Info).
        let new_id = match &self.opts.new_id {
            Some(v) => v.clone(),
            None => compute_new_id(src),
        };
        let id0 = src.trailer_id().map(|[a, _]| a).unwrap_or_else(|| new_id.clone());
        let id_pair = Object::Array(vec![
            Object::String(id0, StringFormat::Hexadecimal),
            Object::String(new_id, StringFormat::Hexadecimal),
        ]);

        let format = match self.opts.xref {
            XrefFormat::LikeLastRevision => match src.xref_kind() {
                XrefKind::Table => XrefFormat::Table,
                XrefKind::Stream => XrefFormat::Stream,
                XrefKind::Hybrid => XrefFormat::Hybrid,
            },
            other => other,
        };
        let prev = src.prev_offset();
        let mut next_number = self.next_number;

        let startxref = match format {
            XrefFormat::Stream => {
                let xid = (next_number, 0);
                next_number += 1;
                let size = next_number.max(src.next_object_number());
                let trailer = trailer_entries(src, size, &id_pair, Some(prev));
                let off = write_xref_stream(&mut out, base, offset_base, &entries, xid, trailer)?;
                written.push((xid, WrittenKind::XrefStream));
                off
            }
            XrefFormat::Hybrid => {
                let xid = (next_number, 0);
                next_number += 1;
                let size = next_number.max(src.next_object_number());
                // Stream ohne /Prev: Leser folgen der Kette über den Tabellen-Trailer.
                let stream_trailer = trailer_entries(src, size, &id_pair, None);
                let stm_off = write_xref_stream(&mut out, base, offset_base, &entries, xid, stream_trailer)?;
                written.push((xid, WrittenKind::XrefStream));
                entries.insert(xid.0, (stm_off, 0));
                let mut trailer = trailer_entries(src, size, &id_pair, Some(prev));
                trailer.set("XRefStm", Object::Integer(stm_off as i64));
                write_xref_table(&mut out, base, offset_base, &entries, &trailer)?
            }
            XrefFormat::Table | XrefFormat::LikeLastRevision => {
                let size = next_number.max(src.next_object_number());
                let trailer = trailer_entries(src, size, &id_pair, Some(prev));
                write_xref_table(&mut out, base, offset_base, &entries, &trailer)?
            }
        };
        let _ = write!(out, "startxref\n{startxref}\n%%EOF\n");

        let signature_dict_id = self.signature.as_ref().map(|(id, _)| *id);
        let mut inc = Increment {
            base_len: base,
            bytes: out,
            contents_range,
            byte_range_field,
            byte_range: ByteRange([0, 0, 0, 0]),
            signature_dict_id,
            written,
        };
        if signature_dict_id.is_some() {
            inc.patch_byte_range()?;
        }
        Ok(inc)
    }
}

/// Bereitet ein neues/geändertes Objekt vor: optional Flate, dann Verschlüsselung (außer
/// xref-Streams, `/Metadata` bei `EncryptMetadata false`, `/Contents` von Signaturdictionaries).
fn prepare_object(
    id: ObjectId,
    obj: &Object,
    enc: Option<&Encryptor>,
    encrypt_metadata: bool,
    compress: bool,
) -> Result<Object> {
    let mut o = obj.clone();
    // XMP-Metadaten bleiben ungefiltert (PDF/A-1 §6.7.2), xref-Streams komprimiert der Writer selbst.
    if let Object::Stream(s) = &mut o
        && compress
        && !s.dict.has(b"Filter")
        && !s.content.is_empty()
        && !s.dict.has_type(b"Metadata")
        && !s.dict.has_type(b"XRef")
    {
        s.content = serialize::deflate(&s.content)?;
        s.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    }
    if let Some(enc) = enc {
        let skip = match &o {
            Object::Stream(s) => s.dict.has_type(b"XRef") || (s.dict.has_type(b"Metadata") && !encrypt_metadata),
            _ => false,
        };
        if !skip {
            serialize::encrypt_object(&mut o, id, enc, 0)?;
        }
    }
    Ok(o)
}

/// Textwert für das Signaturdictionary (verschlüsselt, falls nötig).
fn sig_string(out: &mut Vec<u8>, id: ObjectId, value: Object, enc: Option<&Encryptor>) -> Result<()> {
    let Object::String(bytes, format) = value else {
        return Err(IncError::Serialize("Textwert erwartet".into()));
    };
    match enc {
        Some(e) => {
            let c = e.encrypt(id, &bytes, CryptTarget::String).map_err(|err| IncError::Encryption(err.to_string()))?;
            serialize::write_hex(out, &c);
        }
        None => serialize::write_string(out, &bytes, format),
    }
    Ok(())
}

/// Schreibt das Signaturdictionary in PDF-AS-Reihenfolge (docs/analyse/03 §4.1/§4.2) und liefert
/// die lokalen Bereiche von `/Contents` (inkl. `<` `>`) und des 36-Zeichen-ByteRange-Felds.
fn write_signature_dict(
    out: &mut Vec<u8>,
    id: ObjectId,
    spec: &SignatureDictSpec,
    enc: Option<&Encryptor>,
) -> Result<(Range<usize>, Range<usize>)> {
    let _ = write!(out, "{} {} obj\n<<\n/Type /Sig\n/Filter ", id.0, id.1);
    serialize::write_name(out, spec.filter.as_bytes());
    out.extend_from_slice(b"\n/SubFilter ");
    serialize::write_name(out, spec.sub_filter.as_bytes());
    out.push(b'\n');
    if let Some(m) = &spec.signing_time {
        out.extend_from_slice(b"/M ");
        sig_string(out, id, Object::String(m.as_bytes().to_vec(), StringFormat::Literal), enc)?;
        out.push(b'\n');
    }
    for (key, value) in [(&b"/Location "[..], &spec.location), (b"/Name ", &spec.name), (b"/Reason ", &spec.reason)] {
        if let Some(v) = value {
            out.extend_from_slice(key);
            sig_string(out, id, encode_text_string(v), enc)?;
            out.push(b'\n');
        }
    }
    out.extend_from_slice(b"/Contents ");
    let c_start = out.len();
    out.push(b'<');
    out.resize(out.len() + 2 * spec.contents_reserve, b'0');
    out.push(b'>');
    let c_end = out.len();
    out.extend_from_slice(b"\n/ByteRange ");
    let b_start = out.len();
    out.extend_from_slice(BYTE_RANGE_PLACEHOLDER.as_bytes());
    let b_end = out.len();
    out.extend_from_slice(b"\n>>\nendobj\n");
    Ok((c_start..c_end, b_start..b_end))
}

/// Trailer-Einträge: `/Size /Root /Info /Encrypt /ID /Prev`, danach übrige Schlüssel des alten Trailers.
fn trailer_entries(src: &SourcePdf, size: u32, id_pair: &Object, prev: Option<u64>) -> Dictionary {
    let old = src.trailer();
    let mut d = Dictionary::new();
    d.set("Size", Object::Integer(i64::from(size)));
    for key in [&b"Root"[..], b"Info", b"Encrypt"] {
        if let Ok(v) = old.get(key) {
            d.set(key.to_vec(), v.clone());
        }
    }
    d.set("ID", id_pair.clone());
    if let Some(p) = prev {
        d.set("Prev", Object::Integer(p as i64));
    }
    for (k, v) in old.iter() {
        if !TRAILER_SKIP.contains(&k.as_slice()) {
            d.set(k.clone(), v.clone());
        }
    }
    d
}

/// Zusammenhängende Läufe `(Start, Anzahl)` über sortierten Objektnummern.
fn runs(nums: impl Iterator<Item = u32>) -> Vec<(u32, u64)> {
    let mut out: Vec<(u32, u64)> = Vec::new();
    for n in nums {
        match out.last_mut() {
            Some((s, c)) if u64::from(*s) + *c == u64::from(n) => *c += 1,
            _ => out.push((n, 1)),
        }
    }
    out
}

fn write_xref_table(
    out: &mut Vec<u8>,
    base: u64,
    offset_base: u64,
    entries: &BTreeMap<u32, (u64, u16)>,
    trailer: &Dictionary,
) -> Result<u64> {
    let off = base + out.len() as u64 - offset_base;
    out.extend_from_slice(b"xref\n");
    // Wie PDFBox immer mit dem Freieintrag für Objekt 0.
    let mut all: BTreeMap<u32, Option<(u64, u16)>> = entries.iter().map(|(n, e)| (*n, Some(*e))).collect();
    all.entry(0).or_insert(None);
    for (start, count) in runs(all.keys().copied()) {
        let _ = writeln!(out, "{start} {count}");
        for n in (u64::from(start)..u64::from(start) + count).filter_map(|n| u32::try_from(n).ok()) {
            match all.get(&n).copied().flatten() {
                Some((o, g)) => {
                    if o > 9_999_999_999 {
                        return Err(IncError::Serialize("Offset zu groß für xref-Tabelle".into()));
                    }
                    let _ = write!(out, "{o:010} {g:05} n\r\n");
                }
                None => out.extend_from_slice(b"0000000000 65535 f\r\n"),
            }
        }
    }
    out.extend_from_slice(b"trailer\n");
    serialize::write_dict_block(out, trailer)?;
    out.push(b'\n');
    Ok(off)
}

fn bytes_needed(v: u64) -> usize {
    (((64 - v.leading_zeros()) as usize).div_ceil(8)).max(1)
}

/// Schreibt einen xref-Stream (Typ-1-Einträge inkl. Eigeneintrag, `/W [1 n m]` minimal, Flate).
fn write_xref_stream(
    out: &mut Vec<u8>,
    base: u64,
    offset_base: u64,
    entries: &BTreeMap<u32, (u64, u16)>,
    xid: ObjectId,
    trailer: Dictionary,
) -> Result<u64> {
    let self_off = base + out.len() as u64 - offset_base;
    let mut all = entries.clone();
    all.insert(xid.0, (self_off, xid.1));
    let max_off = all.values().map(|(o, _)| *o).max().unwrap_or(0);
    let max_gen = all.values().map(|(_, g)| *g).max().unwrap_or(0);
    let w2 = bytes_needed(max_off);
    let w3 = if max_gen == 0 { 0 } else { bytes_needed(u64::from(max_gen)) };
    let mut rows = Vec::with_capacity(all.len() * (1 + w2 + w3));
    for (o, g) in all.values() {
        rows.push(1u8);
        rows.extend_from_slice(&o.to_be_bytes()[8 - w2..]);
        if w3 > 0 {
            rows.extend_from_slice(&u64::from(*g).to_be_bytes()[8 - w3..]);
        }
    }
    let data = serialize::deflate(&rows)?;
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XRef".to_vec()));
    let index: Vec<Object> = runs(all.keys().copied())
        .into_iter()
        .flat_map(|(s, c)| [Object::Integer(i64::from(s)), Object::Integer(c as i64)])
        .collect();
    dict.set("Index", Object::Array(index));
    dict.set("W", Object::Array(vec![1.into(), Object::Integer(w2 as i64), Object::Integer(w3 as i64)]));
    for (k, v) in trailer.iter() {
        dict.set(k.clone(), v.clone());
    }
    dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    dict.set("Length", Object::Integer(data.len() as i64));
    let _ = writeln!(out, "{} {} obj", xid.0, xid.1);
    serialize::write_dict_block(out, &dict)?;
    out.extend_from_slice(b"\nstream\r\n");
    out.extend_from_slice(&data);
    out.extend_from_slice(b"\r\nendstream\nendobj\n");
    Ok(self_off)
}

/// `/ID[1]` wie PDFBox 2.0.34: SHA-256 über Zeit in Millisekunden und die `/Info`-Werte.
fn compute_new_id(src: &SourcePdf) -> Vec<u8> {
    use sha2::Digest as _;
    let mut h = sha2::Sha256::new();
    let millis =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default();
    h.update(millis.to_string().as_bytes());
    if let Some(info) = src.info_id().and_then(|id| src.get_object(id).ok())
        && let Object::Dictionary(d) = info
    {
        for (_, v) in d.iter() {
            let mut buf = Vec::new();
            let _ = serialize::write_direct(&mut buf, v, 0);
            h.update(&buf);
        }
    }
    h.update(src.len().to_le_bytes());
    h.finalize().to_vec()
}

/// Fertig serialisiertes Increment (nur die angehängten Bytes). `Debug` ohne Inhalt (Regel 9).
#[derive(Clone, PartialEq, Eq)]
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

impl std::fmt::Debug for Increment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Increment")
            .field("base_len", &self.base_len)
            .field("len", &self.bytes.len())
            .field("contents_range", &self.contents_range)
            .field("byte_range_field", &self.byte_range_field)
            .field("byte_range", &self.byte_range)
            .field("signature_dict_id", &self.signature_dict_id)
            .field("written", &self.written.len())
            .finish()
    }
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

    /// Relativer Bereich im Increment zu einem absoluten Bereich (Bereiche vor dem Increment
    /// werden auf 0 begrenzt).
    pub fn local(&self, abs: Range<u64>) -> Range<usize> {
        let rel = |v: u64| usize::try_from(v.saturating_sub(self.base_len)).unwrap_or(usize::MAX);
        rel(abs.start)..rel(abs.end)
    }

    /// Wie [`Self::local`], aber nur für Bereiche vollständig innerhalb des Increments.
    fn checked_local(&self, abs: &Range<u64>) -> Option<Range<usize>> {
        if abs.start < self.base_len || abs.end < abs.start {
            return None;
        }
        let r = self.local(abs.clone());
        (r.end <= self.bytes.len()).then_some(r)
    }

    /// Berechnet die ByteRange aus [`Self::contents_range`] und schreibt sie linksbündig,
    /// mit Leerzeichen aufgefüllt, in den Platzhalter.
    pub fn patch_byte_range(&mut self) -> Result<()> {
        if self.contents_range.is_empty() || self.byte_range_field.is_empty() {
            return Err(IncError::NoSignatureDictionary);
        }
        if self.checked_local(&self.contents_range).is_none_or(|r| r.len() < 2) {
            return Err(IncError::Serialize("/Contents-Bereich liegt nicht im Increment".into()));
        }
        let br = ByteRange::for_contents(self.contents_range.clone(), self.total_len());
        let text = br.to_pdf_string();
        let Some(field) = self.checked_local(&self.byte_range_field) else {
            return Err(IncError::ByteRangeOverflow);
        };
        if text.len() > field.len() {
            return Err(IncError::ByteRangeOverflow);
        }
        let slot = &mut self.bytes[field];
        slot.fill(b' ');
        slot[..text.len()].copy_from_slice(text.as_bytes());
        self.byte_range = br;
        Ok(())
    }

    /// Setzt das DER-CMS als Großbuchstaben-Hex ab `<`+1 ein; Rest bleibt `0`.
    /// Harte Längenprüfung: `cms.len() <= reserve()`, sonst [`IncError::ContentsTooLarge`].
    pub fn embed_contents(&mut self, cms_der: &[u8]) -> Result<()> {
        if self.contents_range.is_empty() {
            return Err(IncError::NoSignatureDictionary);
        }
        let available = self.reserve();
        if cms_der.len() > available {
            return Err(IncError::ContentsTooLarge { needed: cms_der.len(), available });
        }
        let Some(r) = self.checked_local(&self.contents_range).filter(|r| r.len() >= 2) else {
            return Err(IncError::Serialize("/Contents-Bereich liegt nicht im Increment".into()));
        };
        let hex = &mut self.bytes[r.start + 1..r.end - 1];
        hex.fill(b'0');
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for (i, b) in cms_der.iter().enumerate() {
            hex[2 * i] = HEX[usize::from(b >> 4)];
            hex[2 * i + 1] = HEX[usize::from(b & 0x0F)];
        }
        Ok(())
    }

    /// Die drei Teile der fertigen Datei: Original, Increment bis vor `<`, Increment ab `>`+1.
    /// Teil 1 und 2 sind der erste, Teil 3 der zweite ByteRange-Abschnitt – gestreamt hashbar.
    /// `original` muss das Original (Länge [`Self::base_len`]) sein.
    pub fn byte_range_parts<'b>(&'b self, original: &'b [u8]) -> [&'b [u8]; 3] {
        debug_assert_eq!(original.len() as u64, self.base_len);
        match self.checked_local(&self.contents_range) {
            Some(r) if !r.is_empty() => [original, &self.bytes[..r.start], &self.bytes[r.end..]],
            _ => [original, &self.bytes, &[]],
        }
    }

    /// Signierte Daten (ByteRange-Konkatenation) als zusammenhängender Puffer.
    pub fn byte_range_data(&self, original: &[u8]) -> Vec<u8> {
        let parts = self.byte_range_parts(original);
        let mut v = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
        for p in parts {
            v.extend_from_slice(p);
        }
        v
    }

    /// Fertige Datei (Original + Increment).
    pub fn to_vec(&self, original: &[u8]) -> Vec<u8> {
        debug_assert_eq!(original.len() as u64, self.base_len);
        let mut v = Vec::with_capacity(original.len() + self.bytes.len());
        v.extend_from_slice(original);
        v.extend_from_slice(&self.bytes);
        v
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
    fn xref_field_widths_and_runs() {
        assert_eq!(bytes_needed(0), 1);
        assert_eq!(bytes_needed(255), 1);
        assert_eq!(bytes_needed(256), 2);
        assert_eq!(bytes_needed(65_535), 2);
        assert_eq!(bytes_needed(16_777_215), 3); // /W [1 3 0] bis 16 MiB (Plan §6.1)
        assert_eq!(bytes_needed(16_777_216), 4);
        assert_eq!(bytes_needed(u64::from(u32::MAX) + 1), 5);
        assert_eq!(runs([0u32, 1, 2, 5, 7, 8].into_iter()), vec![(0, 3), (5, 1), (7, 2)]);
        assert!(runs(std::iter::empty()).is_empty());
        // kein Überlauf am Ende des Nummernraums
        assert_eq!(runs([u32::MAX - 1, u32::MAX].into_iter()), vec![(u32::MAX - 1, 2)]);
    }

    #[test]
    fn spec_debug_hides_personal_data() {
        let s = SignatureDictSpec {
            name: Some("Max Mustermann".into()),
            reason: Some("Vertrag 4711".into()),
            ..SignatureDictSpec::default()
        };
        let d = format!("{s:?}");
        assert!(!d.contains("Mustermann") && !d.contains("4711"), "{d}");
        assert!(d.contains("name_len: Some(14)") && d.contains("contents_reserve: 4096"), "{d}");
    }

    #[test]
    fn spec_defaults_like_pdfas() {
        let s = SignatureDictSpec::default();
        assert_eq!(
            (s.filter.as_str(), s.sub_filter.as_str(), s.contents_reserve),
            ("Adobe.PPKLite", "ETSI.CAdES.detached", 4096)
        );
        let o = IncrementOptions::default();
        assert!(o.leading_crlf && o.new_id.is_none() && !o.compress_new_streams);
        assert_eq!(o.xref, XrefFormat::LikeLastRevision);
    }

    #[test]
    fn pdf_date() {
        let dt = time::macros::datetime!(2024-03-15 19:33:33 +01:00);
        assert_eq!(format_pdf_date(dt), "D:20240315193333+01'00'");
    }
}

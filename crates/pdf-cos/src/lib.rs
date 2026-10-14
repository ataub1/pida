//! `pdfover-pdf-cos` – Fassade um `lopdf` (nur Parser und Objektmodell).
//!
//! Verantwortung (Plan §3.2, §6.1, §6.3; docs/analyse/03 §4.13, §10 Punkt 1):
//! - lenientes Lesen eines Eingabe-PDFs aus unveränderlichen Bytes ([`SourcePdf::open`]),
//! - Revisionen und xref-Typ (Tabelle, Stream, Hybrid) für den Append-only-Writer,
//! - Standard-Security-Handler (RC4/AES R2–R6, nur ohne User-Passwort) und Rechteprüfung wie
//!   PDF-AS `PdfBoxUtils` ([`Rights::may_sign`]),
//! - Seitengeometrie ([`PageInfo`]), Signaturfelder ([`SigField`]) und Bild-XObject-Aufrufe
//!   ([`ImageInvocation`]) als **reine Datenstrukturen**, die andere Pakete in Tests selbst bauen
//!   können (ADR 0001 §5.3),
//! - Grenzen gegen Dekompressionsbomben und Rekursion ([`Limits`]).
//!
//! Andere Crates verwenden lopdf ausschließlich über den Re-Export [`lopdf`] dieses Crates, damit die
//! gepinnte Version (0.45.x) an genau einer Stelle festgelegt ist.
//!
//! Ownership: Paket **A** (zusammen mit `pdfover-pdf-incremental`).

use std::path::Path;
use std::sync::Arc;

pub use lopdf;
pub use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};

/// Fehler beim Lesen/Analysieren eines PDFs.
#[derive(Debug, thiserror::Error)]
pub enum CosError {
    /// Ein-/Ausgabefehler.
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
    /// Keine PDF-Datei (kein `%PDF-`-Header, keine Objekte).
    #[error("keine PDF-Datei")]
    NotAPdf,
    /// Syntaxfehler, die auch der leniente Leser nicht überbrücken kann.
    #[error("PDF nicht lesbar: {0}")]
    Parse(String),
    /// Dokument verlangt ein User-Passwort (Annahmeklasse K5, Plan §6.1).
    #[error("PDF ist mit einem Benutzerpasswort geschützt")]
    PasswordRequired,
    /// Public-Key- oder unbekannter Security-Handler (K5, Plan §6.3).
    #[error("nicht unterstützter Security-Handler: {0}")]
    UnsupportedSecurityHandler(String),
    /// Verschlüsselung ist defekt oder nicht entschlüsselbar.
    #[error("Entschlüsselung fehlgeschlagen: {0}")]
    Decryption(String),
    /// Eine Grenze aus [`Limits`] wurde überschritten.
    #[error("Grenze überschritten: {what} (Limit {limit})")]
    LimitExceeded {
        /// Welche Grenze.
        what: &'static str,
        /// Wert der Grenze.
        limit: u64,
    },
    /// Referenziertes Objekt fehlt.
    #[error("Objekt {0:?} fehlt")]
    MissingObject(ObjectId),
    /// Struktur entspricht nicht der Erwartung (z. B. Seitenbaum defekt).
    #[error("ungültige PDF-Struktur: {0}")]
    InvalidStructure(String),
    /// Dokument ohne Seiten (`Cannot sign an empty document`, docs/analyse/03 §7).
    #[error("PDF hat keine Seiten")]
    NoPages,
}

/// Ergebnis-Alias.
pub type Result<T, E = CosError> = std::result::Result<T, E>;

// ------------------------------------------------------------------------------------------------
// Geometrie
// ------------------------------------------------------------------------------------------------

/// Rechteck im PDF-User-Space (`[llx lly urx ury]`), Einheit pt.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    /// linke untere Ecke x
    pub llx: f32,
    /// linke untere Ecke y
    pub lly: f32,
    /// rechte obere Ecke x
    pub urx: f32,
    /// rechte obere Ecke y
    pub ury: f32,
}

impl Rect {
    /// Neues Rechteck.
    pub const fn new(llx: f32, lly: f32, urx: f32, ury: f32) -> Self {
        Self { llx, lly, urx, ury }
    }

    /// Normalisiert (kleinere Koordinate zuerst), wie ISO 32000-1 §7.9.5 verlangt.
    pub fn normalized(self) -> Self {
        Self {
            llx: self.llx.min(self.urx),
            lly: self.lly.min(self.ury),
            urx: self.llx.max(self.urx),
            ury: self.lly.max(self.ury),
        }
    }

    /// Breite (nicht normalisiert: `urx - llx`).
    pub fn width(&self) -> f32 {
        self.urx - self.llx
    }

    /// Höhe (nicht normalisiert: `ury - lly`).
    pub fn height(&self) -> f32 {
        self.ury - self.lly
    }

    /// `true` bei Fläche 0 (unsichtbare Signatur hat `[0 0 0 0]`, docs/analyse/03 §4.6).
    pub fn is_empty(&self) -> bool {
        self.width() == 0.0 || self.height() == 0.0
    }
}

/// Seitenrotation (`/Rotate`), normalisiert auf 0/90/180/270.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Rotation {
    /// 0°
    #[default]
    R0,
    /// 90° im Uhrzeigersinn
    R90,
    /// 180°
    R180,
    /// 270° im Uhrzeigersinn
    R270,
}

impl Rotation {
    /// Normalisiert beliebige Vielfache von 90 (auch negative); andere Werte ergeben `None`.
    pub fn from_degrees(deg: i64) -> Option<Self> {
        if deg % 90 != 0 {
            return None;
        }
        Some(match deg.rem_euclid(360) {
            0 => Rotation::R0,
            90 => Rotation::R90,
            180 => Rotation::R180,
            _ => Rotation::R270,
        })
    }

    /// Grad (0, 90, 180, 270).
    pub fn degrees(self) -> u16 {
        match self {
            Rotation::R0 => 0,
            Rotation::R90 => 90,
            Rotation::R180 => 180,
            Rotation::R270 => 270,
        }
    }

    /// `true` bei 90/270 (Breite und Höhe vertauscht, docs/analyse/01 F-40).
    pub fn swaps_axes(self) -> bool {
        matches!(self, Rotation::R90 | Rotation::R270)
    }
}

/// Affine Matrix `[a b c d e f]` (PDF-Konvention).
pub type Matrix = [f32; 6];

/// Geometrie einer Seite (docs/analyse/04 §4.6/§4.7).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageInfo {
    /// 0-basierter Seitenindex.
    pub index: usize,
    /// Objekt-ID des Seitendictionarys.
    pub object_id: ObjectId,
    /// `/MediaBox` (geerbt aufgelöst).
    pub media_box: Rect,
    /// `/CropBox` (geerbt aufgelöst; fehlt sie, gleich MediaBox).
    pub crop_box: Rect,
    /// `/Rotate` (geerbt aufgelöst).
    pub rotation: Rotation,
    /// `/UserUnit` (Default 1.0).
    pub user_unit: f32,
    /// `/Annots` ist ein indirektes Array (dann wird das Array-Objekt fortgeschrieben, Plan §6.2).
    pub annots_indirect: Option<ObjectId>,
}

impl PageInfo {
    /// Sichtbare Seitengröße (CropBox, bei 90/270° vertauscht) – Grundlage für Positionierung und UI
    /// (docs/analyse/04 §4.6).
    pub fn visible_size(&self) -> (f32, f32) {
        let b = self.crop_box.normalized();
        if self.rotation.swaps_axes() { (b.height(), b.width()) } else { (b.width(), b.height()) }
    }
}

// ------------------------------------------------------------------------------------------------
// Revisionen, xref, Dateiende
// ------------------------------------------------------------------------------------------------

/// Art des Querverweises einer Revision (docs/analyse/03 Phase E Schritt 8; Plan §6.1 K1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum XrefKind {
    /// klassische `xref`-Tabelle mit `trailer`
    Table,
    /// `/Type /XRef`-Stream
    Stream,
    /// Tabelle mit `/XRefStm` im Trailer (Hybrid)
    Hybrid,
}

/// Eine Revision (Original oder Increment) im Byte-Strom.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Revision {
    /// 0 = älteste Revision.
    pub index: usize,
    /// Wert hinter `startxref` dieser Revision.
    pub startxref: u64,
    /// Offset direkt nach `%%EOF` (inkl. EOL) dieser Revision.
    pub end_offset: u64,
    /// xref-Art dieser Revision.
    pub xref_kind: XrefKind,
    /// `/Prev` des Trailers.
    pub prev: Option<u64>,
    /// `/Size` des Trailers.
    pub size: u32,
}

/// Was nach dem letzten `%%EOF` steht (Plan §6.1 „Weitere Sonderfälle“).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TailKind {
    /// Nichts.
    Clean,
    /// Nur Leerraum, Zeilenumbrüche oder Nullbytes – K1, das Increment wird dahinter angehängt.
    Whitespace,
    /// Echter Fremdinhalt – K2.
    Garbage,
}

/// Dateiende-Befund.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TailInfo {
    /// Art.
    pub kind: TailKind,
    /// Offset nach dem letzten `%%EOF`.
    pub eof_offset: u64,
    /// Länge des Rests.
    pub len: u64,
}

// ------------------------------------------------------------------------------------------------
// Verschlüsselung und Rechte
// ------------------------------------------------------------------------------------------------

/// Security-Handler laut `/Filter` im Encrypt-Dictionary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SecurityHandler {
    /// `/Standard` – unterstützt (ohne User-Passwort).
    Standard,
    /// `/Adobe.PubSec` – K5.
    PublicKey,
    /// Anderer Handler – K5.
    Other(String),
}

/// Verschlüsselungsverfahren.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CryptAlgorithm {
    /// RC4 40 Bit (R2)
    Rc4_40,
    /// RC4 128 Bit (R3/R4)
    Rc4_128,
    /// AES-128 (AESV2)
    Aes128,
    /// AES-256 (AESV3, R5/R6)
    Aes256,
    /// `/Identity`
    Identity,
    /// Unbekannt
    Unknown(String),
}

/// Benutzerrechte `/P` (ISO 32000-1 Tabelle 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Permissions(pub u32);

impl Permissions {
    /// Alle Rechte (unverschlüsseltes Dokument).
    pub const ALL: Permissions = Permissions(0xFFFF_FFFC);

    fn bit(self, n: u32) -> bool {
        self.0 & (1 << (n - 1)) != 0
    }
    /// Bit 3: Drucken
    pub fn can_print(self) -> bool {
        self.bit(3)
    }
    /// Bit 4: Ändern
    pub fn can_modify(self) -> bool {
        self.bit(4)
    }
    /// Bit 5: Extrahieren
    pub fn can_extract(self) -> bool {
        self.bit(5)
    }
    /// Bit 6: Annotationen/Formulare ändern
    pub fn can_modify_annotations(self) -> bool {
        self.bit(6)
    }
    /// Bit 9: Formulare ausfüllen (auch Signieren)
    pub fn can_fill_in_form(self) -> bool {
        self.bit(9)
    }
    /// Bit 10: Barrierefreiheit
    pub fn can_extract_for_accessibility(self) -> bool {
        self.bit(10)
    }
    /// Bit 11: Zusammenstellen
    pub fn can_assemble(self) -> bool {
        self.bit(11)
    }
    /// Bit 12: hochwertiges Drucken
    pub fn can_print_high_quality(self) -> bool {
        self.bit(12)
    }
}

/// Befund zur Verschlüsselung (Plan §6.3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EncryptionInfo {
    /// Handler.
    pub handler: SecurityHandler,
    /// `/V`
    pub v: u8,
    /// `/R`
    pub r: u8,
    /// Schlüssellänge in Bit.
    pub length_bits: u16,
    /// Verfahren für Strings/Streams (`/StrF`/`/StmF` bzw. V/R).
    pub algorithm: CryptAlgorithm,
    /// `/P`
    pub permissions: Permissions,
    /// `/EncryptMetadata`
    pub encrypt_metadata: bool,
    /// Objekt-ID von `/Encrypt`, falls indirekt.
    pub encrypt_dict_id: Option<ObjectId>,
}

/// Rechte für das Signieren (docs/analyse/03 §4.13, Plan §6.1 K3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rights {
    /// Dokument ist verschlüsselt.
    pub encrypted: bool,
    /// Rechte (bei unverschlüsselten Dokumenten [`Permissions::ALL`]).
    pub permissions: Permissions,
}

impl Rights {
    /// Rechteprüfung wie PDF-AS `PdfBoxUtils.checkPDFPermissions`:
    /// unverschlüsselt → erlaubt; sonst `(canModify && canModifyAnnotations) || canFillInForm`.
    pub fn may_sign(&self) -> bool {
        !self.encrypted
            || (self.permissions.can_modify() && self.permissions.can_modify_annotations())
            || self.permissions.can_fill_in_form()
    }
}

/// Ziel der Objektverschlüsselung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CryptTarget {
    /// String-Objekt
    String,
    /// Stream-Inhalt
    Stream,
}

/// Verschlüsselt neue Objekte mit dem objektspezifischen Schlüssel des vorhandenen Handlers.
///
/// Wird von `pdfover-pdf-incremental` benutzt (Plan §6.3): neue Strings und Streams werden
/// verschlüsselt, `/Contents` des Signaturdictionarys **nie**. Innerer Aufbau ist Sache von Paket A.
#[derive(Debug)]
pub struct Encryptor {
    _private: (),
}

impl Encryptor {
    /// Verschlüsselt `data` für Objekt `id`.
    pub fn encrypt(&self, id: ObjectId, data: &[u8], target: CryptTarget) -> Result<Vec<u8>> {
        todo!("Paket A: RC4/AES mit objektspezifischem Schlüssel (ISO 32000-1 §7.6.2, Algorithmus 1/1.A)")
    }
}

// ------------------------------------------------------------------------------------------------
// Felder und Bilder
// ------------------------------------------------------------------------------------------------

/// Widget eines Signaturfelds.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SigWidget {
    /// Objekt-ID des Widgets (gleich `SigField::field_id`, wenn Feld und Widget verschmolzen sind).
    pub id: ObjectId,
    /// Seite (über `/P` oder Suche in `/Annots`); `None`, wenn nicht auffindbar.
    pub page_index: Option<usize>,
    /// `/Rect`
    pub rect: Rect,
    /// `/F` (Annotation-Flags)
    pub annot_flags: u32,
}

/// Signaturfeld im AcroForm-Baum (docs/analyse/03 §4.4/§4.5, Plan §6.4).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SigField {
    /// Voll qualifizierter Name (`a.b.c`), auch für hierarchische Felder (Abweichung zu PDF-AS, Register).
    pub qualified_name: String,
    /// `/T` des Blatts.
    pub partial_name: Option<String>,
    /// Objekt-ID des Felds.
    pub field_id: ObjectId,
    /// Tiefe im Feldbaum (0 = oberste Ebene von `/Fields`).
    pub depth: usize,
    /// Widgets (direkt verschmolzen oder `/Kids`).
    pub widgets: Vec<SigWidget>,
    /// `/V` vorhanden (Feld ist signiert).
    pub signed: bool,
    /// Objekt-ID des Signaturdictionarys `/V`, falls indirekt.
    pub value_id: Option<ObjectId>,
    /// `/Lock` vorhanden.
    pub has_lock: bool,
    /// `/SV` (Seed Value) vorhanden.
    pub has_seed_value: bool,
    /// `/Ff`
    pub field_flags: u32,
}

/// Aufruf eines Bild-XObjects per `Do` im Seiteninhalt (für QR-Platzhalter, docs/analyse/04 §5.1).
///
/// Nur `Do` auf `/Subtype /Image` zählt; Inline-Bilder (`BI … EI`) werden wie bei PDF-AS ignoriert.
/// `pdfover-placement` ordnet die Einträge den PDFium-Bildobjekten (`pdfover_render::ImageObject`)
/// **primär über [`ImageInvocation::ctm`]** zu, `order` ist nur Tie-Breaker (ADR 0001 §8).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImageInvocation {
    /// 0-basierter Seitenindex.
    pub page_index: usize,
    /// 0-basierte laufende Nummer unter den Bild-`Do`-Aufrufen der Seite in Content-Stream-Reihenfolge
    /// (Tiefensuche: Aufrufe in Form-XObjects – nur mit `descend_forms` – an der Stelle des
    /// Form-`Do`; mehrfach aufgerufene XObjects zählen je Aufruf).
    pub order: usize,
    /// Ressourcenname (z. B. `Im3`) – wird als `/Location` der Signatur gesetzt.
    pub resource_name: String,
    /// Objekt-ID des XObjects, falls indirekt.
    pub xobject_id: Option<ObjectId>,
    /// CTM beim `Do` im Seitenraum (Bild = Einheitsquadrat; bei Form-XObjects mit deren `/Matrix`
    /// und dem CTM des Form-`Do` verkettet).
    pub ctm: Matrix,
    /// `/Width`
    pub pixel_width: u32,
    /// `/Height`
    pub pixel_height: u32,
    /// Innerhalb eines Form-XObjects gefunden (nur mit Flag ausgewertet, Plan §6.8).
    pub inside_form_xobject: bool,
}

// ------------------------------------------------------------------------------------------------
// Grenzen
// ------------------------------------------------------------------------------------------------

/// Grenzen gegen feindliche Eingaben (Plan §7.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Maximale Dateigröße.
    pub max_file_size: u64,
    /// Maximale Objektanzahl.
    pub max_objects: usize,
    /// Maximale dekomprimierte Größe eines Streams.
    pub max_decoded_stream: u64,
    /// Maximales Verhältnis dekomprimiert/komprimiert.
    pub max_decompression_ratio: u32,
    /// Maximale Rekursionstiefe (Seitenbaum, Feldbaum, Form-XObjects).
    pub max_depth: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_size: 2_000_000_000,
            max_objects: 5_000_000,
            max_decoded_stream: 512 * 1024 * 1024,
            max_decompression_ratio: 1000,
            max_depth: 64,
        }
    }
}

// ------------------------------------------------------------------------------------------------
// SourcePdf
// ------------------------------------------------------------------------------------------------

/// Unveränderliches Eingabe-PDF: Original-Bytes plus geparstes Objektmodell.
///
/// Die Original-Bytes bleiben unverändert und sind der Präfix jeder Ausgabe (Invariante I1,
/// Plan §3.6). Das `lopdf::Document` ist bei verschlüsselten Dateien bereits entschlüsselt.
///
/// `Debug` zeigt nur Kennzahlen, nie Dokumentinhalt (ADR 0001 §4 Regel 9).
pub struct SourcePdf {
    bytes: Arc<[u8]>,
    document: Document,
    revisions: Vec<Revision>,
    tail: TailInfo,
    encryption: Option<EncryptionInfo>,
    encryptor: Option<Encryptor>,
    rights: Rights,
    pages: Vec<PageInfo>,
}

impl std::fmt::Debug for SourcePdf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourcePdf")
            .field("len", &self.bytes.len())
            .field("revisions", &self.revisions.len())
            .field("tail", &self.tail)
            .field("encryption", &self.encryption)
            .field("rights", &self.rights)
            .field("pages", &self.pages.len())
            .finish_non_exhaustive()
    }
}

impl SourcePdf {
    /// Liest ein PDF aus Bytes (leniente Variante, Plan §6.1 K1/K2).
    ///
    /// Fehler: [`CosError::PasswordRequired`], [`CosError::UnsupportedSecurityHandler`],
    /// [`CosError::NoPages`] usw. – `pdfover-pades::analyze` bildet sie auf K5 ab.
    pub fn open(bytes: impl Into<Arc<[u8]>>, limits: &Limits) -> Result<SourcePdf> {
        todo!("Paket A: lopdf laden, Revisionen/xref-Kette, Tail, Verschlüsselung, Seiten (docs/analyse/03 §10)")
    }

    /// Liest eine Datei (Bytes vollständig oder per Memory-Map, Plan §6.2 „Speicher“).
    pub fn open_file(path: &Path, limits: &Limits) -> Result<SourcePdf> {
        todo!("Paket A")
    }

    /// Original-Bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Original-Bytes als geteilter Puffer (für Renderer und Ausgabe ohne Kopie).
    pub fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }

    /// Länge der Originaldatei.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// `true` bei leerer Datei (praktisch nie, `open` lehnt sie ab).
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Geparstes (ggf. entschlüsseltes) Objektmodell.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Revisionen, älteste zuerst.
    pub fn revisions(&self) -> &[Revision] {
        &self.revisions
    }

    /// Letzte Revision (bestimmt das Format des neuen Increments, Plan §6.1 K1).
    pub fn last_revision(&self) -> &Revision {
        self.revisions.last().expect("open() garantiert mindestens eine Revision")
    }

    /// xref-Art der letzten Revision.
    pub fn xref_kind(&self) -> XrefKind {
        self.last_revision().xref_kind
    }

    /// Dateiende-Befund.
    pub fn tail(&self) -> TailInfo {
        self.tail
    }

    /// Verschlüsselung, falls vorhanden.
    pub fn encryption(&self) -> Option<&EncryptionInfo> {
        self.encryption.as_ref()
    }

    /// Verschlüsseler für neue Objekte, falls das Dokument verschlüsselt ist.
    pub fn encryptor(&self) -> Option<&Encryptor> {
        self.encryptor.as_ref()
    }

    /// Rechte.
    pub fn rights(&self) -> Rights {
        self.rights
    }

    /// Seiten in Dokumentreihenfolge.
    pub fn pages(&self) -> &[PageInfo] {
        &self.pages
    }

    /// Anzahl Seiten.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// PDF-Version aus dem Header (bzw. `/Version` im Katalog, falls höher) – `iui.pdfVersion`.
    pub fn pdf_version(&self) -> &str {
        todo!("Paket A")
    }

    /// Trailer der letzten Revision.
    pub fn trailer(&self) -> &Dictionary {
        &self.document.trailer
    }

    /// Objekt-ID des Katalogs (`/Root`).
    pub fn catalog_id(&self) -> Result<ObjectId> {
        todo!("Paket A")
    }

    /// Objekt-ID von `/Info`, falls vorhanden.
    pub fn info_id(&self) -> Option<ObjectId> {
        todo!("Paket A")
    }

    /// `/ID` des Trailers (`[ID0, ID1]`), falls vorhanden.
    pub fn trailer_id(&self) -> Option<[Vec<u8>; 2]> {
        todo!("Paket A")
    }

    /// Nächste freie Objektnummer (`/Size` der letzten Revision bzw. höchste Nummer + 1).
    pub fn next_object_number(&self) -> u32 {
        todo!("Paket A")
    }

    /// Objekt holen.
    pub fn get_object(&self, id: ObjectId) -> Result<&Object> {
        self.document.get_object(id).map_err(|_| CosError::MissingObject(id))
    }

    /// Referenzen bis zum Zielobjekt auflösen (Tiefe begrenzt durch [`Limits::max_depth`]).
    pub fn resolve<'a>(&'a self, obj: &'a Object) -> Result<&'a Object> {
        todo!("Paket A")
    }

    /// Dekodierter Stream-Inhalt mit Grenzen.
    pub fn decoded_stream(&self, id: ObjectId) -> Result<Vec<u8>> {
        todo!("Paket A")
    }

    /// XMP-Metadaten des Katalogs (`/Metadata`), dekodiert – für PDF/A-Erkennung (Plan §6.9).
    pub fn xmp_metadata(&self) -> Result<Option<Vec<u8>>> {
        todo!("Paket A")
    }

    /// Alle Signaturfelder (signiert und leer), inklusive `/Kids`-Widgets und hierarchischer Namen.
    pub fn signature_fields(&self) -> Result<Vec<SigField>> {
        todo!("Paket A: AcroForm-Baum durchlaufen (docs/analyse/03 §4.5, Plan §6.4)")
    }

    /// Alle `Do`-Aufrufe von Bild-XObjects auf einer Seite mit CTM (docs/analyse/04 §5.1).
    /// Form-XObjects werden nur mit `descend_forms = true` betreten.
    pub fn image_invocations(&self, page_index: usize, descend_forms: bool) -> Result<Vec<ImageInvocation>> {
        todo!("Paket A: Content-Stream mit q/Q/cm-Stack auswerten")
    }
}

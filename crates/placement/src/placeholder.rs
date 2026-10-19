//! Platzhalter (Plan §6.8; docs/analyse/04 §5).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use pida_pdf_cos::{ImageInvocation, Matrix, ObjectId, PageInfo, Rect, SigField};
use pida_render::{Renderer, RgbaImage, concat_matrix};
use rxing::common::HybridBinarizer;
use rxing::qrcode::QRCodeReader;
use rxing::{BinaryBitmap, DecodeHints, Luma8LuminanceSource, Reader};

use crate::position::{Coord, PageMapping, PageSelector, TablePos, java_split};
use crate::{CompatMode, PlacementError, Result};

/// Präfix des QR-Inhalts (`PlaceholderExtractorConstants.QR_PLACEHOLDER_IDENTIFIER`).
pub const QR_PLACEHOLDER_IDENTIFIER: &str = "PDF-AS-POS";

/// Bilder kleiner als dieses Maß (in einer Dimension, Pixel) werden nicht dekodiert.
pub const MIN_QR_IMAGE_PX: u32 = 10;

/// Toleranz der Matrix-Zuordnung `ImageObject` ↔ `ImageInvocation` (ADR 0001 §8.2).
pub const MATRIX_TOLERANCE: f32 = 0.01;

/// Inhalt eines QR-Platzhalters `PDF-AS-POS;id=…;profile=…;type=…;key=…`.
///
/// `Debug` zeigt `key` und den Rohtext nicht im Klartext (ADR 0001 Regel 9).
#[derive(Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct QrPayload {
    /// `id`
    pub id: Option<String>,
    /// `profile` (überschreibt das Profil, `placeholder_profile_overwrite=true`)
    pub profile: Option<String>,
    /// `type`
    pub kind: Option<String>,
    /// `key`
    pub key: Option<String>,
    /// Rohtext
    pub raw: String,
}

impl std::fmt::Debug for QrPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QrPayload")
            .field("id", &self.id)
            .field("profile", &self.profile)
            .field("kind", &self.kind)
            .field("key", &self.key.as_ref().map(|_| "***"))
            .field("raw_len", &self.raw.len())
            .finish()
    }
}

/// Gefundener QR-Platzhalter.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QrPlaceholder {
    /// Seite (0-basiert)
    pub page_index: usize,
    /// Ressourcenname (`/Location` der Signatur markiert ihn als benutzt)
    pub resource_name: String,
    /// Inhalt
    pub payload: QrPayload,
    /// Position: `x = floor(e)`, `y = ceil(f + d)`, `w = ceil(a)` nach Herausrechnen der Rotation;
    /// die Höhe wird ignoriert.
    pub pos: TablePos,
    /// CTM beim `Do`
    pub ctm: Matrix,
}

impl QrPlaceholder {
    /// Positionsstring für das Signieren: die Platzhalter-Position ersetzt die Position komplett;
    /// eine Breite unter `min_width` (Profil-Key `minWidth`, nur wenn > 0) wird angehoben
    /// (`PADESPDFBOXSigner.prepareTablePosition`).
    pub fn table_pos_with_min_width(&self, min_width: Option<f32>) -> TablePos {
        let mut pos = self.pos.clone();
        if let (Some(min), Some(Coord::Value(w))) = (min_width, pos.w)
            && min > 0.0
            && w < min
        {
            pos.w = Some(Coord::Value(min));
        }
        pos
    }
}

/// Suchmodus (`PlaceholderExtractorConstants`): PDF-Over sucht mit id 1 im Modus SORTED und
/// signiert im Modus MODERATE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlaceholderMode {
    /// 0 – Fehler, wenn nichts passt
    Strict,
    /// 1 – sonst keiner
    Moderate,
    /// 2 – sonst erster überhaupt
    Lenient,
    /// 3 – kleinste id
    Sorted,
}

impl PlaceholderMode {
    /// Aus dem Konfigurationswert `placeholder_mode` (0–3).
    pub fn from_config(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Strict),
            1 => Some(Self::Moderate),
            2 => Some(Self::Lenient),
            3 => Some(Self::Sorted),
            _ => None,
        }
    }
}

/// Suchanfrage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderQuery {
    /// gesuchte id (Vergleich numerisch, sonst ohne Groß-/Kleinschreibung; IDs > 127 funktionieren –
    /// Korrektur des Integer-`==`-Fehlers, Register)
    pub id: Option<String>,
    /// Modus
    pub mode: PlaceholderMode,
    /// Bereits benutzte Ressourcennamen (`/Location` vorhandener Signaturen)
    pub used_names: Vec<String>,
}

/// Leeres Signaturfeld als Auswahlkandidat.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmptyFieldCandidate {
    /// Voll qualifizierter Name (Anzeige)
    pub qualified_name: String,
    /// Feld
    pub field_id: ObjectId,
    /// Widget (eigenes Kind oder verschmolzen)
    pub widget_id: ObjectId,
    /// Seite
    pub page_index: Option<usize>,
    /// vorhandenes Rect (der Block steht darin proportional und mittig, [`crate::field_fit`])
    pub rect: Rect,
    /// für die **automatische** Wahl geeignet (Platzhaltersuche „Leere Signaturfelder“, Stapel): Der Kern setzt
    /// das nach der Einrastregel ([`crate::field_suitable`], ADR 0004 Nachtrag 16) für den aktuellen Block; ohne
    /// Bewertung `true` (Verhalten vor Nachtrag 22). Angeboten wird ein Feld davon unabhängig.
    #[serde(default = "auto_ok_default")]
    pub auto_ok: bool,
}

fn auto_ok_default() -> bool {
    true
}

/// Parst den QR-Text (`startsWith("PDF-AS-POS")`, Paare an `;` und `=`, Schlüssel ohne
/// Groß-/Kleinschreibung; ungültige Paare ignoriert).
///
/// Wie `SignaturePlaceholderExtractor.checkImage`: Java-`split` (leere Teile am Ende entfallen,
/// `id=` ist damit ungültig), spätere Schlüssel überschreiben frühere, der erste Teil (Präfix)
/// wird nicht ausgewertet.
pub fn parse_qr_payload(text: &str) -> Option<QrPayload> {
    if !text.starts_with(QR_PLACEHOLDER_IDENTIFIER) {
        return None;
    }
    let mut payload = QrPayload { raw: text.to_owned(), ..Default::default() };
    for pair in java_split(text, ';').into_iter().skip(1) {
        let kv = java_split(pair, '=');
        if kv.len() != 2 {
            continue;
        }
        let value = Some(kv[1].to_owned());
        if java_equals_ignore_case(kv[0], "id") {
            payload.id = value;
        } else if java_equals_ignore_case(kv[0], "profile") {
            payload.profile = value;
        } else if java_equals_ignore_case(kv[0], "key") {
            payload.key = value;
        } else if java_equals_ignore_case(kv[0], "type") {
            payload.kind = value;
        }
    }
    Some(payload)
}

/// Bilder bis zu dieser Pixelzahl werden ohne Treffer ein zweites Mal 2× vergrößert dekodiert.
pub const QR_UPSCALE_MAX_PIXELS: u64 = 4_000_000;

fn decode_luma(luma: Vec<u8>, w: u32, h: u32) -> Option<String> {
    std::panic::catch_unwind(move || {
        let source = Luma8LuminanceSource::new(luma, w, h).ok()?;
        let mut bitmap = BinaryBitmap::new(HybridBinarizer::new(source));
        let hints = DecodeHints::default();
        QRCodeReader::new().decode_with_hints(&mut bitmap, &hints).ok().map(|r| r.getText().to_owned())
    })
    .ok()
    .flatten()
}

/// Dekodiert QR-Codes in einem Bild (`rxing`, HybridBinarizer, nur QR, ohne TRY_HARDER; Bilder
/// < 10 px werden übersprungen).
///
/// Höchstens ein Ergebnis wie ZXing `MultiFormatReader.decode`; Luminanz wie
/// `BufferedImageLuminanceSource` ([`RgbaImage::to_luma8`]). Ein Absturz des Dekoders bei
/// feindlichen Bildern wird abgefangen (PDF-AS fängt ZXing-`ArrayIndexOutOfBounds` ebenso).
/// `rxing` erkennt sehr kleine Module (≈ 2 px, z. B. JPEG-Platzhalter des PDF-AS-Generators mit
/// 300 × 60 px) schlechter als ZXing; ohne Treffer wird ein Bild bis
/// [`QR_UPSCALE_MAX_PIXELS`] deshalb einmal 2× (nächster Nachbar) vergrößert erneut dekodiert
/// (Register KR-F07).
pub fn decode_qr(image: &RgbaImage) -> Vec<String> {
    if image.width < MIN_QR_IMAGE_PX || image.height < MIN_QR_IMAGE_PX || image.is_empty() {
        return Vec::new();
    }
    if image.data.len() as u64 != u64::from(image.width) * u64::from(image.height) * 4 {
        return Vec::new();
    }
    let luma = image.to_luma8();
    let (w, h) = (image.width, image.height);
    if u64::from(w) * u64::from(h) > QR_UPSCALE_MAX_PIXELS {
        // große Bilder (bis zum Seitenbudget des Renderers): ein Versuch, ohne Kopie
        return decode_luma(luma, w, h).into_iter().collect();
    }
    if let Some(text) = decode_luma(luma.clone(), w, h) {
        return vec![text];
    }
    let (w2, h2) = (w * 2, h * 2);
    let mut up = Vec::with_capacity(w2 as usize * h2 as usize);
    for y in 0..h2 {
        let row = &luma[(y / 2 * w) as usize..((y / 2 + 1) * w) as usize];
        for x in 0..w2 {
            up.push(row[(x / 2) as usize]);
        }
    }
    decode_luma(up, w2, h2).into_iter().collect()
}

/// Rohposition eines Platzhalters (`x`, Oberkante `y`, Breite `w`) in sichtbaren
/// Seitenkoordinaten aus der CTM: `M = CTM · (Benutzerraum → sichtbar)`, `x = M.e`,
/// `y = M.f + M.d`, `w = M.a` (`SignaturePlaceholderExtractor.processOperator`).
///
/// Korrigiert gegenüber PDF-AS um den CropBox-Ursprung (Register KR-F02); bei CropBox-Ursprung
/// (0, 0) identisch mit PDF-AS.
pub fn placeholder_position(page: &PageInfo, ctm: &Matrix) -> (f32, f32, f32) {
    let mapping = PageMapping::new(page, CompatMode::Corrected);
    let m = concat_matrix(*ctm, mapping.user_to_visible_matrix());
    (m[4], m[5] + m[3], m[0])
}

fn placeholder_table_pos(page: &PageInfo, ctm: &Matrix) -> Option<TablePos> {
    let (x, y, w) = placeholder_position(page, ctm);
    let (x, y, w) = (x.floor(), y.ceil(), w.ceil());
    // PDF-AS bricht hier die gesamte Suche ab (TablePos-Prüfung x,y ≥ 0, w ≥ 1); wir
    // überspringen nur diesen Platzhalter (Register KR-F05).
    if !(x.is_finite() && y.is_finite() && w.is_finite()) || x < 0.0 || y < 0.0 || w < 1.0 {
        return None;
    }
    Some(TablePos {
        page: Some(PageSelector::Exact(page.index as u32 + 1)),
        x: Some(Coord::Value(x)),
        y: Some(Coord::Value(y)),
        w: Some(Coord::Value(w)),
        footer: None,
        rotation: None,
    })
}

fn matrix_close(a: &[f32; 6], b: &[f32; 6]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() <= MATRIX_TOLERANCE)
}

/// Aufrufe einer Seite, sortiert nach der Matrixkomponente `e`, für die Zuordnung in
/// `O(n log n)` statt `O(n²)` (feindliche Seiten mit sehr vielen Bildern). Aufrufe mit nicht
/// endlicher Matrix können nie zugeordnet werden und fehlen im Index.
struct MatrixIndex {
    /// (e, Index in der nach `order` sortierten Liste)
    by_e: Vec<(f32, usize)>,
}

impl MatrixIndex {
    fn new(invs: &[&ImageInvocation]) -> Self {
        let mut by_e: Vec<(f32, usize)> = invs
            .iter()
            .enumerate()
            .filter(|(_, inv)| inv.ctm.iter().all(|v| v.is_finite()))
            .map(|(k, inv)| (inv.ctm[4], k))
            .collect();
        by_e.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Self { by_e }
    }

    /// Kandidaten mit `|e − inv.e|` höchstens doppelte Toleranz (die genaue Prüfung macht
    /// `matrix_close`; der breitere Rahmen fängt Rundung von `e ± tol` ab).
    fn near(&self, e: f32) -> impl Iterator<Item = usize> + '_ {
        let (lo, hi) = if e.is_finite() {
            let margin = 2.0 * MATRIX_TOLERANCE;
            (self.by_e.partition_point(|(v, _)| *v < e - margin), self.by_e.partition_point(|(v, _)| *v <= e + margin))
        } else {
            (0, 0)
        };
        self.by_e[lo..hi.max(lo)].iter().map(|(_, k)| *k)
    }
}

/// Passen die Pixelmaße? Nur wenn beide bekannt sind (dekodiertes Bild nicht leer, `/Width` und
/// `/Height` des Aufrufs > 0) – sonst entscheidet allein die Matrix. Verhindert, dass ein
/// Inline-Bild mit derselben Matrix (PDFium zählt es mit, `pdf-cos` nicht) den Namen eines
/// Bild-XObjects bekommt.
fn size_compatible(bitmap: &RgbaImage, inv: &ImageInvocation) -> bool {
    bitmap.is_empty()
        || inv.pixel_width == 0
        || inv.pixel_height == 0
        || (bitmap.width == inv.pixel_width && bitmap.height == inv.pixel_height)
}

/// Alle QR-Platzhalter der Seiten in Seitenreihenfolge. Bitmaps kommen aus dem Renderer
/// (`image_objects`), Namen und CTM aus `invocations`. Zuordnung je Seite **primär über die
/// Matrix** (`ImageObject::matrix` ≈ `ImageInvocation::ctm`, Abweichung ≤ 0,01), dazu passende
/// Pixelmaße (wenn beide bekannt), `order` nur als Tie-Breaker – PDFium zählt Inline-Bilder mit,
/// `pdf-cos` nicht (ADR 0001 §8).
///
/// Nur Bilder mit passendem `Do`-Aufruf werden dekodiert (Inline-Bilder und – ohne
/// `descend_forms` in `pdf-cos` – Bilder in Form-XObjects bleiben wie bei PDF-AS außen vor).
/// Seiten ohne Aufrufe werden nicht gerendert. Ein Renderfehler bricht die Suche ab (PDF-Over
/// ignoriert Fehler der Platzhaltersuche, F-22 – das entscheidet der Aufrufer).
pub fn find_qr_placeholders(
    renderer: &dyn Renderer,
    pages: &[PageInfo],
    invocations: &[ImageInvocation],
) -> Result<Vec<QrPlaceholder>> {
    let mut by_page: BTreeMap<usize, Vec<&ImageInvocation>> = BTreeMap::new();
    for inv in invocations {
        by_page.entry(inv.page_index).or_default().push(inv);
    }
    let mut out = Vec::new();
    for (page_index, mut invs) in by_page {
        let Some(page) = pages.iter().find(|p| p.index == page_index).or_else(|| pages.get(page_index)) else {
            continue;
        };
        if page_index >= renderer.page_count() {
            return Err(PlacementError::NoSuchPage(page_index));
        }
        invs.sort_by_key(|i| i.order);
        let objects = renderer.image_objects(page_index)?;
        let index = MatrixIndex::new(&invs);
        let mut used = vec![false; invs.len()];
        let mut found: Vec<(usize, QrPlaceholder)> = Vec::new();
        for obj in &objects {
            let candidate = index
                .near(obj.matrix[4])
                .filter(|&k| {
                    !used[k] && matrix_close(&obj.matrix, &invs[k].ctm) && size_compatible(&obj.bitmap, invs[k])
                })
                .min_by_key(|&k| (invs[k].order.abs_diff(obj.order), k));
            let Some(k) = candidate else { continue };
            used[k] = true;
            let inv = invs[k];
            let Some(payload) = decode_qr(&obj.bitmap).iter().find_map(|t| parse_qr_payload(t)) else {
                continue;
            };
            let Some(pos) = placeholder_table_pos(page, &inv.ctm) else { continue };
            found.push((
                inv.order,
                QrPlaceholder { page_index, resource_name: inv.resource_name.clone(), payload, pos, ctm: inv.ctm },
            ));
        }
        found.sort_by_key(|(order, _)| *order);
        out.extend(found.into_iter().map(|(_, p)| p));
    }
    Ok(out)
}

/// Java `Integer.valueOf` (optionales Vorzeichen, ASCII-Ziffern, 32 Bit).
fn java_parse_int(s: &str) -> Option<i32> {
    s.parse().ok()
}

fn java_fold(c: char) -> char {
    let mut up = c.to_uppercase();
    match (up.next(), up.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

fn java_fold_lower(c: char) -> char {
    let mut low = c.to_lowercase();
    match (low.next(), low.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Java `String.compareToIgnoreCase` (zeichenweise Groß-, dann Kleinschreibung).
fn java_compare_ignore_case(a: &str, b: &str) -> Ordering {
    let mut ia = a.chars();
    let mut ib = b.chars();
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c1), Some(c2)) => {
                if c1 == c2 {
                    continue;
                }
                let (u1, u2) = (java_fold(c1), java_fold(c2));
                if u1 == u2 {
                    continue;
                }
                let (l1, l2) = (java_fold_lower(u1), java_fold_lower(u2));
                if l1 != l2 {
                    return l1.cmp(&l2);
                }
            }
        }
    }
}

/// Java `String.equalsIgnoreCase`.
fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    java_compare_ignore_case(a, b) == Ordering::Equal
}

/// `matchPlaceHolderId` **ohne** den Integer-`==`-Fehler: numerisch, wenn beide ganze Zahlen
/// sind, sonst ohne Groß-/Kleinschreibung (Register KR-F03).
fn ids_match(query: &str, candidate: &str) -> bool {
    match (java_parse_int(query), java_parse_int(candidate)) {
        (Some(a), Some(b)) => a == b,
        _ => java_equals_ignore_case(query, candidate),
    }
}

/// `placeHolderIdMatcher`: liefert den kleineren Kandidaten (bei Gleichstand den bisherigen).
fn smaller_id<'a>(current: &'a QrPlaceholder, test: &'a QrPlaceholder) -> &'a QrPlaceholder {
    let (Some(cur), Some(tst)) = (current.payload.id.as_deref(), test.payload.id.as_deref()) else {
        return current;
    };
    let test_is_smaller = match (java_parse_int(cur), java_parse_int(tst)) {
        (Some(c), Some(t)) => t < c,
        _ => java_compare_ignore_case(tst, cur) == Ordering::Less,
    };
    if test_is_smaller { test } else { current }
}

/// Auswahl nach Modus (docs/analyse/04 §5.2).
///
/// Wie `SignaturePlaceholderExtractor.extract`: zuerst der erste Kandidat (Dokumentreihenfolge,
/// ohne benutzte Namen) mit passender id bzw. – außer in SORTED – bei fehlender gesuchter id der
/// erste ohne id; sonst STRICT ⇒ [`PlacementError::PlaceholderNotFound`], SORTED ⇒ kleinste id,
/// dann erster ohne id, LENIENT ⇒ erster überhaupt, MODERATE ⇒ keiner. Eine leere gesuchte id
/// gilt als „keine id“ (PDF-AS `StringUtils.isEmpty`).
pub fn select_placeholder<'a>(
    candidates: &'a [QrPlaceholder],
    query: &PlaceholderQuery,
) -> Result<Option<&'a QrPlaceholder>> {
    let wanted = query.id.as_deref().filter(|id| !id.is_empty());
    let available: Vec<&QrPlaceholder> =
        candidates.iter().filter(|c| !query.used_names.contains(&c.resource_name)).collect();
    for c in &available {
        if let (Some(w), Some(id)) = (wanted, c.payload.id.as_deref())
            && ids_match(w, id)
        {
            return Ok(Some(c));
        }
        if query.mode != PlaceholderMode::Sorted && wanted.is_none() && c.payload.id.is_none() {
            return Ok(Some(c));
        }
    }
    if query.mode == PlaceholderMode::Strict {
        return Err(PlacementError::PlaceholderNotFound);
    }
    if available.is_empty() {
        return Ok(None);
    }
    if query.mode == PlaceholderMode::Sorted {
        let mut best: Option<&QrPlaceholder> = None;
        for c in available.iter().filter(|c| c.payload.id.is_some()) {
            best = Some(match best {
                None => c,
                Some(cur) => smaller_id(cur, c),
            });
        }
        if let Some(b) = best {
            return Ok(Some(b));
        }
    }
    if let Some(c) = available.iter().find(|c| c.payload.id.is_none()) {
        return Ok(Some(c));
    }
    if query.mode == PlaceholderMode::Lenient {
        return Ok(available.first().copied());
    }
    Ok(None)
}

/// Feldflag `ReadOnly` (`/Ff` Bit 1).
const FF_READ_ONLY: u32 = 1;
/// Annotation-Flag `ReadOnly` (`/F` Bit 7).
const F_READ_ONLY: u32 = 1 << 6;
/// Annotation-Flag `LockedContents` (`/F` Bit 10).
const F_LOCKED_CONTENTS: u32 = 1 << 9;

/// Leere, nicht gesperrte Signaturfelder mit Widget (je Signatur wählbar, B-05; gesperrte und
/// signierte Felder werden nicht angeboten).
///
/// Wie `SignatureFieldsAndPlaceHolderExtractor.findEmptySignatureFields`: Feld ohne `/V` mit
/// `/T`. Erweiterungen (Plan §6.4, Register): auch hierarchische Felder (voll qualifizierter
/// Name); „gesperrt“ = Feldflag `ReadOnly` (so markieren Viewer Felder, die eine vorhandene
/// Signatur über `/Lock` sperrt) oder Widget-Flag `ReadOnly`/`LockedContents`; ohne Widget
/// nicht wählbar. Verwendet wird das erste Widget. Reihenfolge wie `fields`.
pub fn find_empty_sig_fields(fields: &[SigField]) -> Vec<EmptyFieldCandidate> {
    fields
        .iter()
        .filter(|f| !f.signed && f.partial_name.is_some() && f.field_flags & FF_READ_ONLY == 0)
        .filter_map(|f| {
            let w = f.widgets.first()?;
            if w.annot_flags & (F_READ_ONLY | F_LOCKED_CONTENTS) != 0 {
                return None;
            }
            Some(EmptyFieldCandidate {
                qualified_name: f.qualified_name.clone(),
                field_id: f.field_id,
                widget_id: w.id,
                page_index: w.page_index,
                rect: w.rect,
                auto_ok: true,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_string_helpers() {
        assert!(java_equals_ignore_case("ID", "id"));
        assert!(java_equals_ignore_case("Profile", "PROFILE"));
        assert!(!java_equals_ignore_case("id", "ids"));
        assert_eq!(java_compare_ignore_case("a", "B"), Ordering::Less);
        assert_eq!(java_compare_ignore_case("b", "A"), Ordering::Greater);
        assert_eq!(java_compare_ignore_case("abc", "ABC"), Ordering::Equal);
        assert_eq!(java_compare_ignore_case("ab", "abc"), Ordering::Less);
        assert_eq!(java_parse_int("+7"), Some(7));
        assert_eq!(java_parse_int("007"), Some(7));
        assert_eq!(java_parse_int(" 7"), None);
        assert_eq!(java_parse_int("2147483648"), None);
    }

    #[test]
    fn id_matching_without_integer_cache_bug() {
        assert!(ids_match("1", "1"));
        assert!(ids_match("128", "128"), "Java-Integer-Cache-Fehler darf nicht nachgebaut werden");
        assert!(ids_match("1000", "01000"));
        assert!(ids_match("abc", "ABC"));
        assert!(!ids_match("1", "2"));
        assert!(!ids_match("1", "a"));
    }
}

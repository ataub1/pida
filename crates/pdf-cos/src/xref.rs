//! Querverweiskette, Revisionen und Dateiende (Plan §6.1 K1/K2, docs/analyse/03 Phase E Schritt 8).
//!
//! Eigene Auswertung der Rohbytes, unabhängig von lopdf:
//! - alle `%%EOF`-Marken mit vorangehendem `startxref` (Revisionen; die Dummy-Marke
//!   `startxref 0` linearisierter Dateien zählt nicht),
//! - die Kette ab dem letzten gültigen `startxref` über `/Prev` (Tabelle, xref-Stream, Hybrid mit
//!   `/XRefStm`),
//! - die korrekt zusammengeführte Objekttabelle (neueste Revision gewinnt; in Hybrid-Abschnitten
//!   ergänzt der xref-Stream die Tabelle). lopdf 0.45 wertet `/XRefStm` älterer Revisionen nicht aus
//!   und mischt in der falschen Reihenfolge; `SourcePdf::open` korrigiert das Objektmodell damit.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use lopdf::{Dictionary, Object, ObjectId};

use crate::filters;
use crate::lexer::{self, EndstreamIndex, Lexer, Token};
use crate::{CosError, Limits, Result, Revision, TailInfo, TailKind, XrefKind};

/// Eintrag einer Querverweistabelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Entry {
    /// frei (oder ungültig)
    Free,
    /// Typ 1: Objekt an Offset (relativ zur Offset-Basis)
    InUse { offset: u64, generation: u16 },
    /// Typ 2: Objekt in einem Object Stream
    Compressed { stream: u32, index: u32 },
}

/// Art eines Querverweisabschnitts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SectionKind {
    Table,
    Stream,
}

/// Ein Querverweisabschnitt (Tabelle mit Trailer oder xref-Stream).
#[derive(Debug, Clone)]
pub(crate) struct Section {
    /// tatsächlicher (ggf. korrigierter) absoluter Offset
    pub offset: u64,
    pub kind: SectionKind,
    pub entries: Vec<(u32, Entry)>,
    /// Einträge des `/XRefStm`-Streams eines Hybrid-Abschnitts
    pub hybrid_entries: Vec<(u32, Entry)>,
    /// Trailer bzw. xref-Stream-Dictionary
    pub trailer: Dictionary,
    /// Objekt-ID des xref-Streams
    pub stream_id: Option<ObjectId>,
    /// Objekt-ID des `/XRefStm`-Streams (Hybrid)
    pub hybrid_stream_id: Option<ObjectId>,
}

impl Section {
    pub(crate) fn prev(&self) -> Option<u64> {
        dict_u64(&self.trailer, b"Prev")
    }

    pub(crate) fn xref_stm(&self) -> Option<u64> {
        if self.kind == SectionKind::Table { dict_u64(&self.trailer, b"XRefStm") } else { None }
    }

    pub(crate) fn size(&self) -> u32 {
        dict_u64(&self.trailer, b"Size").map(|s| s.min(u64::from(u32::MAX)) as u32).unwrap_or(0)
    }

    pub(crate) fn xref_kind(&self) -> XrefKind {
        match self.kind {
            SectionKind::Stream => XrefKind::Stream,
            SectionKind::Table if self.trailer.has(b"XRefStm") => XrefKind::Hybrid,
            SectionKind::Table => XrefKind::Table,
        }
    }
}

fn dict_u64(d: &Dictionary, key: &[u8]) -> Option<u64> {
    match d.get(key).ok()? {
        Object::Integer(n) if *n >= 0 => Some(*n as u64),
        Object::Real(r) if *r >= 0.0 && r.fract() == 0.0 => Some(*r as u64),
        _ => None,
    }
}

/// Eine `%%EOF`-Marke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EofMarker {
    /// Offset von `%%EOF`
    pub pos: usize,
    /// Offset direkt nach `%%EOF` inklusive eines EOL (CRLF, LF oder CR)
    pub end: usize,
    /// Wert des unmittelbar vorangehenden `startxref`
    pub startxref: Option<u64>,
}

/// Alle `%%EOF`-Marken in Dateireihenfolge.
pub(crate) fn find_eof_markers(data: &[u8]) -> Vec<EofMarker> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = lexer::find(data, b"%%EOF", from) {
        let mut end = pos + 5;
        if data.get(end) == Some(&b'\r') {
            end += 1;
            if data.get(end) == Some(&b'\n') {
                end += 1;
            }
        } else if data.get(end) == Some(&b'\n') {
            end += 1;
        }
        out.push(EofMarker { pos, end, startxref: startxref_before(data, pos) });
        from = pos + 5;
    }
    out
}

/// `startxref <ws> <ziffern> <ws>` direkt vor `pos`.
fn startxref_before(data: &[u8], pos: usize) -> Option<u64> {
    let mut q = pos;
    while q > 0 && lexer::is_whitespace(data[q - 1]) {
        q -= 1;
    }
    let digits_end = q;
    while q > 0 && data[q - 1].is_ascii_digit() {
        q -= 1;
    }
    if q == digits_end || digits_end - q > 19 {
        return None;
    }
    let digits = std::str::from_utf8(&data[q..digits_end]).ok()?;
    let value: u64 = digits.parse().ok()?;
    while q > 0 && lexer::is_whitespace(data[q - 1]) {
        q -= 1;
    }
    if q >= 9 && &data[q - 9..q] == b"startxref" { Some(value) } else { None }
}

/// Grenzen für die Kette.
const MAX_SECTIONS: usize = 4096;
/// Fenster für die Offset-Korrektur (wie lopdf/qpdf).
const RECOVERY_WINDOW: usize = 64;

/// Gefundener Abschnitt und ob der Offset korrigiert werden musste.
type Located = Option<(Rc<Section>, bool)>;

/// Liest Querverweisabschnitte mit Zwischenspeicher: Jede (um Leerraum bereinigte) Position wird
/// höchstens einmal eingelesen. Ohne Speicher würde eine Datei mit vielen `%%EOF`-Marken, die alle
/// auf dieselbe große Tabelle zeigen, diese für jede Marke neu einlesen (quadratische Laufzeit).
pub(crate) struct SectionReader<'a> {
    data: &'a [u8],
    limits: &'a Limits,
    index: &'a EndstreamIndex,
    exact: RefCell<HashMap<usize, Option<Rc<Section>>>>,
    located: RefCell<HashMap<(u64, u64), Located>>,
    /// ein xref-Stream mit unzulässigen Predictor-Parametern wurde gefunden (lopdf würde ihn beim
    /// Laden ungeprüft dekodieren, siehe `filters::MAX_PREDICTOR_ROW`)
    insane_params: Cell<bool>,
}

impl<'a> SectionReader<'a> {
    pub(crate) fn new(data: &'a [u8], limits: &'a Limits, index: &'a EndstreamIndex) -> Self {
        Self {
            data,
            limits,
            index,
            exact: RefCell::default(),
            located: RefCell::default(),
            insane_params: Cell::new(false),
        }
    }

    /// Liest einen Abschnitt exakt an `abs` (nach optionalem Leerraum).
    fn exact(&self, abs: usize) -> Option<Rc<Section>> {
        let data = self.data;
        let mut p = abs;
        while p < data.len() && lexer::is_whitespace(data[p]) {
            p += 1;
        }
        if let Some(hit) = self.exact.borrow().get(&p) {
            return hit.clone();
        }
        let parsed = parse_section_exact(data, p, self.limits, self.index, &self.insane_params).map(Rc::new);
        self.exact.borrow_mut().insert(p, parsed.clone());
        parsed
    }

    /// Sucht einen Abschnitt an `declared + base`, notfalls im Fenster ±64 Bytes.
    /// Liefert den Abschnitt und ob korrigiert werden musste.
    pub(crate) fn locate(&self, declared: u64, base: u64) -> Located {
        if let Some(hit) = self.located.borrow().get(&(declared, base)) {
            return hit.clone();
        }
        let found = self.locate_uncached(declared, base);
        self.located.borrow_mut().insert((declared, base), found.clone());
        found
    }

    fn locate_uncached(&self, declared: u64, base: u64) -> Located {
        let data = self.data;
        let abs = usize::try_from(declared.checked_add(base)?).ok()?;
        if abs >= data.len() {
            return None;
        }
        if let Some(s) = self.exact(abs) {
            return Some((s, false));
        }
        let lo = abs.saturating_sub(RECOVERY_WINDOW);
        let hi = (abs + RECOVERY_WINDOW).min(data.len());
        let mut best: Option<usize> = None;
        for pos in lo..hi {
            let is_xref = data[pos..].starts_with(b"xref") && !(pos >= 5 && &data[pos - 5..pos] == b"start");
            let is_obj = (pos == 0 || !data[pos - 1].is_ascii_digit())
                && data[pos].is_ascii_digit()
                && lexer::object_header_at(data, pos).is_some();
            if (is_xref || is_obj) && best.is_none_or(|b| pos.abs_diff(abs) < b.abs_diff(abs)) {
                best = Some(pos);
            }
        }
        let pos = best?;
        self.exact(pos).map(|s| (s, true))
    }
}

/// Liest einen Abschnitt an `p` (Tabelle mit Trailer oder xref-Stream).
fn parse_section_exact(
    data: &[u8],
    p: usize,
    limits: &Limits,
    index: &EndstreamIndex,
    insane: &Cell<bool>,
) -> Option<Section> {
    if data.get(p..)?.starts_with(b"xref") {
        let (entries, trailer) = parse_table(data, p, limits)?;
        return Some(Section {
            offset: p as u64,
            kind: SectionKind::Table,
            entries,
            hybrid_entries: Vec::new(),
            trailer,
            stream_id: None,
            hybrid_stream_id: None,
        });
    }
    let (entries, trailer, id) = parse_stream_section(data, p, limits, index, insane)?;
    Some(Section {
        offset: p as u64,
        kind: SectionKind::Stream,
        entries,
        hybrid_entries: Vec::new(),
        trailer,
        stream_id: Some(id),
        hybrid_stream_id: None,
    })
}

fn parse_table(data: &[u8], pos: usize, limits: &Limits) -> Option<(Vec<(u32, Entry)>, Dictionary)> {
    let mut lx = Lexer::new(data, pos);
    if !lx.expect_keyword(b"xref") {
        return None;
    }
    let mut entries: Vec<(u32, Entry)> = Vec::new();
    let mut first_subsection = true;
    loop {
        match lx.next_token() {
            Some(Token::Keyword(b"trailer")) => break,
            Some(Token::Int(start)) => {
                let Some(Token::Int(count)) = lx.next_token() else {
                    return None;
                };
                if start < 0 || count < 0 {
                    return None;
                }
                let mut start = start as u64;
                let mut i: u64 = 0;
                // Einträge solange sie wie Einträge aussehen (`n g n|f`); `count` ist nur ein Hinweis:
                // überzählige Einträge gehören zum Unterabschnitt, ein neuer Kopf (`n m` + Zahl) oder
                // `trailer` beendet ihn (lenient wie PDFBox/pdf.js-Recovery).
                let _ = count;
                loop {
                    let esave = lx.pos;
                    let (Some(Token::Int(off)), Some(Token::Int(g)), Some(Token::Keyword(t))) =
                        (lx.next_token(), lx.next_token(), lx.next_token())
                    else {
                        lx.pos = esave;
                        break;
                    };
                    // Verbreiteter Fehler: erster Unterabschnitt beginnt bei 1, enthält aber den
                    // Freieintrag für Objekt 0 (qpdf/pdf.js korrigieren das ebenso).
                    if first_subsection && i == 0 && start == 1 && off == 0 && g == 65535 && t == b"f" {
                        start = 0;
                    }
                    let num = start + i;
                    if num > u64::from(u32::MAX) {
                        return None;
                    }
                    let entry = match t {
                        b"n" if off > 0 && (0..=i64::from(u16::MAX)).contains(&g) => {
                            Entry::InUse { offset: off as u64, generation: g as u16 }
                        }
                        b"n" | b"f" => Entry::Free,
                        _ => {
                            lx.pos = esave;
                            break;
                        }
                    };
                    entries.push((num as u32, entry));
                    if entries.len() > limits.max_objects {
                        return None;
                    }
                    i += 1;
                }
                first_subsection = false;
            }
            _ => return None,
        }
    }
    let trailer = match lx.parse_object(true)? {
        Object::Dictionary(d) => d,
        _ => return None,
    };
    Some((entries, trailer))
}

/// Einträge, Dictionary und Objekt-ID eines xref-Streams.
type StreamSection = (Vec<(u32, Entry)>, Dictionary, ObjectId);

fn parse_stream_section(
    data: &[u8],
    pos: usize,
    limits: &Limits,
    index: &EndstreamIndex,
    insane: &Cell<bool>,
) -> Option<StreamSection> {
    let io = lexer::parse_indirect_object(data, pos, &|_| None, index)?;
    let Object::Stream(stream) = io.object else { return None };
    if !stream.dict.has_type(b"XRef") {
        return None;
    }
    if !filters::stream_params_sane(&stream.dict) {
        insane.set(true);
        return None;
    }
    let decoded = if stream.dict.has(b"Filter") {
        let limit = usize::try_from(limits.max_decoded_stream).unwrap_or(usize::MAX);
        stream.decompressed_content_with_limit(limit).ok()?
    } else {
        stream.content.clone()
    };
    let w: Vec<usize> = stream
        .dict
        .get(b"W")
        .ok()?
        .as_array()
        .ok()?
        .iter()
        .map(|o| o.as_i64().ok().filter(|v| (0..=8).contains(v)).map(|v| v as usize))
        .collect::<Option<Vec<_>>>()?;
    if w.len() != 3 {
        return None;
    }
    let row = w[0] + w[1] + w[2];
    if row == 0 {
        return None;
    }
    let size = dict_u64(&stream.dict, b"Size").unwrap_or(0);
    let index: Vec<(u64, u64)> = match stream.dict.get(b"Index") {
        Ok(Object::Array(a)) => a
            .chunks(2)
            .filter_map(|c| match c {
                [s, n] => Some((s.as_i64().ok()?.max(0) as u64, n.as_i64().ok()?.max(0) as u64)),
                _ => None,
            })
            .collect(),
        _ => vec![(0, size)],
    };
    let mut entries = Vec::new();
    let mut p = 0usize;
    'outer: for (start, count) in index {
        for i in 0..count {
            if p + row > decoded.len() {
                break 'outer;
            }
            let field = |off: usize, width: usize| -> u64 {
                decoded[p + off..p + off + width].iter().fold(0u64, |acc, &b| acc << 8 | u64::from(b))
            };
            let t = if w[0] == 0 { 1 } else { field(0, w[0]) };
            let f2 = field(w[0], w[1]);
            let f3 = field(w[0] + w[1], w[2]);
            p += row;
            let num = start + i;
            if num > u64::from(u32::MAX) {
                break 'outer;
            }
            let entry = match t {
                0 => Entry::Free,
                1 if f2 > 0 && f3 <= u64::from(u16::MAX) => Entry::InUse { offset: f2, generation: f3 as u16 },
                2 if f2 <= u64::from(u32::MAX) && f3 <= u64::from(u32::MAX) => {
                    Entry::Compressed { stream: f2 as u32, index: f3 as u32 }
                }
                _ => Entry::Free,
            };
            entries.push((num as u32, entry));
            if entries.len() > limits.max_objects {
                return None;
            }
        }
    }
    Some((entries, stream.dict, io.id))
}

/// Querverweiskette ab einem `startxref`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Chain {
    /// neueste zuerst
    pub sections: Vec<Section>,
    /// Kette unvollständig (`/Prev` oder `/XRefStm` nicht lesbar) oder Offsets korrigiert
    pub repaired: bool,
}

impl Chain {
    /// Zusammengeführte Tabelle (neueste Revision gewinnt; Hybrid: Tabelle, ergänzt um den
    /// xref-Stream – dessen Einträge ersetzen auch Freieinträge der Tabelle).
    pub(crate) fn merged(&self) -> BTreeMap<u32, Entry> {
        let mut merged = BTreeMap::new();
        for s in &self.sections {
            let mut sec: BTreeMap<u32, Entry> = BTreeMap::new();
            for (n, e) in &s.entries {
                sec.insert(*n, *e);
            }
            for (n, e) in &s.hybrid_entries {
                match sec.get(n) {
                    None | Some(Entry::Free) if *e != Entry::Free => {
                        sec.insert(*n, *e);
                    }
                    _ => {}
                }
            }
            for (n, e) in sec {
                merged.entry(n).or_insert(e);
            }
        }
        merged
    }

    /// Trailer der neuesten Revision, fehlende `/Root`, `/Info`, `/ID`, `/Encrypt` aus älteren
    /// Trailern ergänzt; Strukturschlüssel von xref-Streams entfernt.
    pub(crate) fn effective_trailer(&self) -> Option<Dictionary> {
        let newest = self.sections.first()?;
        let mut t = strip_stream_keys(&newest.trailer);
        for key in [&b"Root"[..], b"Info", b"ID", b"Encrypt"] {
            if !t.has(key)
                && let Some(v) = self.sections.iter().skip(1).find_map(|s| s.trailer.get(key).ok())
            {
                t.set(key.to_vec(), v.clone());
            }
        }
        Some(t)
    }
}

/// Entfernt die Strukturschlüssel eines xref-Stream-Dictionarys.
pub(crate) fn strip_stream_keys(d: &Dictionary) -> Dictionary {
    const STREAM_KEYS: &[&[u8]] =
        &[b"Type", b"W", b"Index", b"Filter", b"DecodeParms", b"Length", b"F", b"FFilter", b"FDecodeParms", b"DL"];
    let mut out = Dictionary::new();
    for (k, v) in d.iter() {
        if !STREAM_KEYS.contains(&k.as_slice()) {
            out.set(k.clone(), v.clone());
        }
    }
    out
}

/// Folgt der Kette ab `startxref`.
pub(crate) fn walk_chain(reader: &SectionReader<'_>, startxref: u64, base: u64) -> Chain {
    let limits = reader.limits;
    let mut chain = Chain::default();
    let mut visited: HashSet<u64> = HashSet::new();
    // tatsächliche Offsets: verschiedene deklarierte Werte können (über die Fensterkorrektur) auf
    // denselben Abschnitt führen – das ist ebenfalls ein Zyklus.
    let mut visited_actual: HashSet<u64> = HashSet::new();
    let mut next = Some(startxref);
    let mut total_entries = 0usize;
    while let Some(declared) = next.take() {
        if !visited.insert(declared) || chain.sections.len() >= MAX_SECTIONS {
            break;
        }
        let Some((found, corrected)) = reader.locate(declared, base) else {
            chain.repaired = true;
            break;
        };
        if !visited_actual.insert(found.offset) {
            chain.repaired = true;
            break;
        }
        if corrected {
            chain.repaired = true;
        }
        let mut section = (*found).clone();
        if let Some(stm) = section.xref_stm() {
            match reader.locate(stm, base) {
                Some((hs, c)) if hs.kind == SectionKind::Stream => {
                    chain.repaired |= c;
                    section.hybrid_entries.clone_from(&hs.entries);
                    section.hybrid_stream_id = hs.stream_id;
                }
                _ => chain.repaired = true,
            }
        }
        total_entries += section.entries.len() + section.hybrid_entries.len();
        if total_entries > limits.max_objects.saturating_mul(4) {
            chain.repaired = true;
            break;
        }
        next = section.prev();
        chain.sections.push(section);
    }
    chain
}

/// Ergebnis der Strukturanalyse.
#[derive(Debug, Clone)]
pub(crate) struct Structure {
    pub revisions: Vec<Revision>,
    pub tail: TailInfo,
    pub chain: Chain,
    /// Basis für xref-Offsets (0 oder Offset des `%PDF-`-Headers bei vorangestelltem Fremdinhalt)
    pub base: u64,
    /// keine verwertbare Kette – lopdf rekonstruiert das Objektverzeichnis
    pub reconstructed: bool,
    /// Ende der letzten gültigen Revision (lopdf lädt nur bis hier)
    pub load_end: usize,
}

/// Analysiert Revisionen, Kette und Dateiende.
pub(crate) fn analyze(data: &[u8], header_offset: usize, limits: &Limits, index: &EndstreamIndex) -> Result<Structure> {
    let reader = SectionReader::new(data, limits, index);
    let markers = find_eof_markers(data);
    let bases: Vec<u64> = if header_offset > 0 { vec![0, header_offset as u64] } else { vec![0] };

    // Offset-Basis an der letzten Marke mit startxref bestimmen.
    let mut base = 0u64;
    if let Some(m) = markers.iter().rev().find(|m| m.startxref.is_some_and(|s| s > 0)) {
        let sx = m.startxref.unwrap_or(0);
        for &b in &bases {
            if reader.locate(sx, b).is_some_and(|(_, corrected)| !corrected) {
                base = b;
                break;
            }
        }
    }

    let mut revisions = Vec::new();
    let mut last_valid_end: Option<usize> = None;
    for m in &markers {
        let Some(sx) = m.startxref else { continue };
        if sx == 0 || sx.saturating_add(base) >= m.pos as u64 {
            continue; // Dummy-Marke der Linearisierung bzw. unplausibel
        }
        let Some((section, _)) = reader.locate(sx, base) else { continue };
        revisions.push(Revision {
            index: revisions.len(),
            startxref: sx,
            end_offset: m.end as u64,
            xref_kind: section.xref_kind(),
            prev: section.prev(),
            size: section.size(),
        });
        last_valid_end = Some(m.end);
    }

    let total = data.len();
    let (tail, chain, reconstructed, load_end) = match (revisions.last(), last_valid_end) {
        (Some(last), Some(end)) => {
            let rest = &data[end..];
            let kind = if rest.is_empty() {
                TailKind::Clean
            } else if rest.iter().all(|&b| lexer::is_whitespace(b)) {
                TailKind::Whitespace
            } else {
                TailKind::Garbage
            };
            let chain = walk_chain(&reader, last.startxref, base);
            let reconstructed = chain.sections.is_empty();
            (TailInfo { kind, eof_offset: end as u64, len: (total - end) as u64 }, chain, reconstructed, end)
        }
        _ => {
            // Keine gültige Revision: lopdf rekonstruiert (K2).
            let tail = TailInfo { kind: TailKind::Garbage, eof_offset: 0, len: total as u64 };
            (tail, Chain::default(), true, total)
        }
    };

    if reconstructed && revisions.is_empty() {
        // Mindestens eine (synthetische) Revision, damit `last_revision()` immer gilt.
        revisions.push(Revision {
            index: 0,
            startxref: 0,
            end_offset: total as u64,
            xref_kind: XrefKind::Table,
            prev: None,
            size: 0,
        });
    }

    if reader.insane_params.get() {
        // lopdf dekodiert xref-Streams beim Laden selbst; mit solchen Parametern liefe das in eine
        // unbegrenzte Speicheranforderung (K5 statt Absturz).
        return Err(filters::insane_predictor());
    }
    let merged_len = chain.merged().len();
    if merged_len > limits.max_objects {
        return Err(CosError::LimitExceeded { what: "Objektanzahl", limit: limits.max_objects as u64 });
    }

    Ok(Structure { revisions, tail, chain, base, reconstructed, load_end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startxref_detection() {
        let d = b"...startxref\n123\n%%EOF\n";
        let m = find_eof_markers(d);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].startxref, Some(123));
        assert_eq!(m[0].end, d.len());
        let d = b"startxref 0 %%EOF\r\nstuff %%EOF";
        let m = find_eof_markers(d);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].startxref, Some(0));
        assert_eq!(m[0].end, 19);
        assert_eq!(m[1].startxref, None);
    }

    #[test]
    fn table_parse_and_off_by_one() {
        let d =
            b"xref\n1 3\n0000000000 65535 f\r\n0000000010 00000 n\r\n0000000020 00001 n\r\ntrailer\n<< /Size 3 >>\n";
        let (e, t) = parse_table(d, 0, &Limits::default()).unwrap();
        assert_eq!(e[0], (0, Entry::Free));
        assert_eq!(e[1], (1, Entry::InUse { offset: 10, generation: 0 }));
        assert_eq!(e[2], (2, Entry::InUse { offset: 20, generation: 1 }));
        assert_eq!(t.get(b"Size").unwrap().as_i64().unwrap(), 3);
    }

    #[test]
    fn table_with_wrong_counts() {
        // zu viele Einträge im ersten, zu wenige im zweiten Unterabschnitt
        let d = b"xref\n0 2\n0000000000 65535 f \n0000000010 00000 n \n0000000020 00000 n \n7 5\n0000000070 00000 n \ntrailer << /Size 8 >>";
        let (e, t) = parse_table(d, 0, &Limits::default()).unwrap();
        assert_eq!(
            e,
            vec![
                (0, Entry::Free),
                (1, Entry::InUse { offset: 10, generation: 0 }),
                (2, Entry::InUse { offset: 20, generation: 0 }),
                (7, Entry::InUse { offset: 70, generation: 0 }),
            ]
        );
        assert!(t.has(b"Size"));
        // Müll statt Trailer → ungültig
        assert!(parse_table(b"xref\n0 1\n0000000000 65535 f\nfoo", 0, &Limits::default()).is_none());
    }

    #[test]
    fn merge_hybrid_rules() {
        let table = Section {
            offset: 0,
            kind: SectionKind::Table,
            entries: vec![(1, Entry::InUse { offset: 10, generation: 0 }), (5, Entry::Free)],
            hybrid_entries: vec![
                (5, Entry::Compressed { stream: 6, index: 0 }),
                (1, Entry::InUse { offset: 99, generation: 0 }),
                (7, Entry::Compressed { stream: 6, index: 1 }),
            ],
            trailer: Dictionary::new(),
            stream_id: None,
            hybrid_stream_id: None,
        };
        let older = Section {
            offset: 0,
            kind: SectionKind::Table,
            entries: vec![
                (5, Entry::InUse { offset: 50, generation: 0 }),
                (8, Entry::InUse { offset: 80, generation: 0 }),
            ],
            hybrid_entries: vec![],
            trailer: Dictionary::new(),
            stream_id: None,
            hybrid_stream_id: None,
        };
        let chain = Chain { sections: vec![table, older], repaired: false };
        let m = chain.merged();
        assert_eq!(m[&1], Entry::InUse { offset: 10, generation: 0 }); // Tabelle gewinnt
        assert_eq!(m[&5], Entry::Compressed { stream: 6, index: 0 }); // Stream ersetzt Freieintrag
        assert_eq!(m[&7], Entry::Compressed { stream: 6, index: 1 });
        assert_eq!(m[&8], Entry::InUse { offset: 80, generation: 0 }); // ältere Revision ergänzt
    }
}

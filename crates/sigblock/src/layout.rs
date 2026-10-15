//! Tabellenlayout in `f32` mit Java-Semantik (docs/analyse/04 §3.1–§3.5, Quirks Q1–Q12).
//!
//! Zielwerte (Plan §6.5, ±0,01 pt): SMALL_DE 230×49 (mit Hinweis 230×82), Standardlogo 33×34,
//! AMTS auf A4 495×120, Demo-Logo 61×61, BASE_LOGO bei 23 mm 62,118×65,167.
//!
//! Nachgebildet sind `TableFactory.createSigTable` (Zellen, Default-Zellstile, Normalisierung),
//! `PDFBoxTable` (fixe Breite: Spaltenbreiten, Umbruch, Zeilenhöhen, Streckung von Untertabellen),
//! `PDFAsVisualSignatureBuilder.readTableResources` (Bildgrößen; überschreibt dabei – wie im
//! Original – die relativen Spaltenbreiten mit absoluten) und `TableDrawUtils` (Zeichenpositionen).
//! Alle Rechenschritte laufen in derselben Reihenfolge in `f32` wie in Java.

use std::sync::Arc;

use crate::emblem::Emblem;
use crate::expr::{ExprContext, resolve_value};
use crate::fonts::{FontProvider, FontSpec, LoadedFont};
use crate::input::{PdfaMode, SigBlockInput};
use crate::profile::{CellSpec, HAlign, Profile, Resources, Style, VAlign};
use crate::{Result, SigBlockError, afm, java};

/// Rolle einer Textzelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// Caption (`font`, `halign`/`valign`)
    Caption,
    /// Wert (`valuefont`, `valuehalign`/`valuevalign`)
    Value,
}

/// Inhalt einer Zelle nach dem Layout. `Debug` zeigt keine Texte (Personendaten, ADR 0001 Regel 9).
#[derive(Clone, PartialEq)]
pub enum CellContent {
    /// Text (bereits umbrochen).
    Text {
        /// Zeilen
        lines: Vec<String>,
        /// Font
        font: FontSpec,
        /// Rolle
        role: TextRole,
        /// Ausrichtung horizontal
        halign: Option<HAlign>,
        /// Ausrichtung vertikal
        valign: Option<VAlign>,
    },
    /// Bild mit berechneter Zeichengröße in ganzen pt (Q5).
    Image {
        /// Profil-Key (`SIG_LABEL`)
        key: String,
        /// Breite in pt
        draw_width: f32,
        /// Höhe in pt
        draw_height: f32,
        /// Ausrichtung
        halign: Option<HAlign>,
        /// Ausrichtung
        valign: Option<VAlign>,
    },
    /// Verschachtelte Tabelle.
    SubTable(Box<LaidOutTable>),
    /// „IMG MISSING“ bzw. leer.
    Empty,
}

/// Zelle mit Geometrie im Formularraum (y nach oben).
#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutCell {
    /// Zeile (0-basiert)
    pub row: usize,
    /// Spalte (0-basiert)
    pub col: usize,
    /// linke Kante
    pub x: f32,
    /// Oberkante
    pub top: f32,
    /// Breite
    pub width: f32,
    /// Höhe (= Zeilenhöhe)
    pub height: f32,
    /// Inhalt
    pub content: CellContent,
}

/// Tabelle nach dem Layout.
#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutTable {
    /// Name
    pub name: String,
    /// Spaltenbreiten
    pub col_widths: Vec<f32>,
    /// Zeilenhöhen
    pub row_heights: Vec<f32>,
    /// Gesamtbreite
    pub width: f32,
    /// Gesamthöhe
    pub height: f32,
    /// Padding (immer das der Tabelle, nie das der Zelle)
    pub padding: f32,
    /// Rahmenbreite (0 = keine)
    pub border: f32,
    /// Hintergrund
    pub bgcolor: Option<[u8; 3]>,
    /// Zellen
    pub cells: Vec<LaidOutCell>,
}

/// Ergebnis des Layouts. `Debug` zeigt keine Texte (ADR 0001 Regel 9).
#[derive(Clone, PartialEq)]
pub struct Layout {
    /// Blockbreite W.
    pub width: f32,
    /// Blockhöhe H.
    pub height: f32,
    /// Haupttabelle.
    pub table: LaidOutTable,
    /// `/TU`: Texte der äußersten Tabelle, je Zelle mit `\n` (Q10).
    pub tu_text: String,
    /// Ersetzte Zeichen ohne Glyph (Meldung an die UI, Register).
    pub replaced_chars: Vec<char>,
}

impl std::fmt::Debug for CellContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CellContent::Text { lines, font, role, halign, valign } => f
                .debug_struct("Text")
                .field("lines", &lines.len())
                .field("chars", &lines.iter().map(|l| l.chars().count()).sum::<usize>())
                .field("font", font)
                .field("role", role)
                .field("halign", halign)
                .field("valign", valign)
                .finish(),
            CellContent::Image { key, draw_width, draw_height, halign, valign } => f
                .debug_struct("Image")
                .field("key", key)
                .field("draw_width", draw_width)
                .field("draw_height", draw_height)
                .field("halign", halign)
                .field("valign", valign)
                .finish(),
            CellContent::SubTable(t) => f.debug_tuple("SubTable").field(t).finish(),
            CellContent::Empty => f.write_str("Empty"),
        }
    }
}

impl std::fmt::Debug for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layout")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("table", &self.table)
            .field("tu_text_len", &self.tu_text.chars().count())
            .field("replaced_chars", &self.replaced_chars.len())
            .finish()
    }
}

/// Layout mit fester Breite `width` (aus `pida-placement`).
pub fn layout(
    profile: &Profile,
    input: &SigBlockInput,
    width: f32,
    fonts: &FontProvider,
    resources: &dyn Resources,
) -> Result<Layout> {
    Ok(prepare(profile, input, width, input.pdfa, fonts, resources)?.layout)
}

/// `breakString(text, maxwidth, font, size)` exakt nach docs/analyse/04 §3.3 (Java-`split`,
/// gieriger Umbruch mit strikt-größer-Vergleich, NBSP zählt als Leerzeichen, `trim`).
/// Zeichen, die der Font nicht kodiert (auch ein `\n` am Wortende), sind wie in PDF-AS ein Fehler.
pub fn break_string(text: &str, max_width: f32, font: &LoadedFont, size: f32) -> Result<Vec<String>> {
    break_string_with(text, max_width, font, size, false)
}

const NBSP: char = '\u{a0}';

fn measure(font: &LoadedFont, text: &str, size: f32) -> Result<f32> {
    let t = text.replace(NBSP, " ");
    Ok(font.string_width(&t)? / 1000.0 * size)
}

/// `breakString`; `lenient_newlines` behandelt ein `\n` am Wortende (Java-`split` entfernt dort
/// den leeren Rest, PDF-AS misst dann `\n` und bricht ab) als Umbruch (Register).
pub(crate) fn break_string_with(
    text: &str,
    max_width: f32,
    font: &LoadedFont,
    size: f32,
    lenient_newlines: bool,
) -> Result<Vec<String>> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in java::split(text, ' ') {
        let mut parts = java::split(word, '\n');
        if lenient_newlines && parts.len() <= 1 && word.contains('\n') {
            parts = word.split('\n').collect();
        }
        if parts.len() > 1 {
            for (j, sub) in parts.iter().enumerate() {
                if (j == 0 && word.starts_with('\n')) || j != 0 {
                    lines.push(java::trim(&cur).to_string());
                    cur.clear();
                }
                let w = measure(font, &format!("{cur}{sub}"), size)?;
                if w > max_width && !cur.is_empty() {
                    lines.push(java::trim(&cur).to_string());
                    cur.clear();
                }
                cur.push_str(sub);
                cur.push(' ');
            }
        } else {
            let w = measure(font, &format!("{cur}{word}"), size)?;
            if w > max_width && !cur.is_empty() {
                lines.push(java::trim(&cur).to_string());
                cur.clear();
            }
            cur.push_str(word);
            cur.push(' ');
        }
    }
    lines.push(java::trim(&cur).to_string());
    Ok(lines.into_iter().map(|l| l.replace(NBSP, " ")).collect())
}

// ------------------------------------------------------------------------------------------------
// Modell (TableFactory)
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellDefault {
    Image,
    Caption,
    Value,
}

impl CellDefault {
    /// `TableFactory.setDefaultStyles`.
    fn style(self) -> Style {
        match self {
            CellDefault::Image => Style {
                padding: Some(3.0),
                halign: Some(HAlign::Center),
                valign: Some(VAlign::Middle),
                ..Style::default()
            },
            CellDefault::Caption => {
                Style { halign: Some(HAlign::Center), valign: Some(VAlign::Middle), ..Style::default() }
            }
            CellDefault::Value => {
                Style { halign: Some(HAlign::Left), valign: Some(VAlign::Middle), ..Style::default() }
            }
        }
    }
}

#[derive(Clone)]
struct ImageRef {
    /// Profil-Key (`SIG_LABEL`)
    key: String,
    /// Wert (`./images/…`, Pfad) – Identität wie `createHashedId(value)`
    id: String,
    emblem: Arc<Emblem>,
}

#[derive(Clone)]
enum Kind {
    Caption(String),
    Value(String),
    Image(ImageRef),
    Table(Box<Model>),
    /// nach dem Layout: Untertabelle
    Node(Box<Node>),
}

#[derive(Clone)]
struct Entry {
    kind: Kind,
    colspan: usize,
    default: CellDefault,
}

#[derive(Clone)]
struct Model {
    name: String,
    rows: Vec<Vec<Entry>>,
    cols_rel: Option<Vec<f32>>,
    style: Option<Style>,
    max_cols: usize,
}

/// Obergrenze der Verschachtelung (PDF-AS: StackOverflow bei Selbstreferenz).
const MAX_TABLE_DEPTH: usize = 8;

struct Env<'a> {
    profile: &'a Profile,
    input: &'a SigBlockInput,
    ctx: ExprContext,
    fonts: &'a FontProvider,
    resources: &'a dyn Resources,
    pdfa: PdfaMode,
    quirks: bool,
    replaced: Vec<char>,
}

fn missing(what: String) -> SigBlockError {
    SigBlockError::Profile(what)
}

impl Env<'_> {
    fn label_override(&self, key: &str) -> Option<Arc<Emblem>> {
        if key == "SIG_LABEL" { self.input.emblem.clone().or_else(|| self.profile.label_emblem.clone()) } else { None }
    }

    fn raw_value(&self, key: &str) -> Option<String> {
        if key == "SIG_NOTE"
            && let Some(note) = &self.input.note
        {
            return Some(note.clone());
        }
        self.profile.values.get(key).cloned()
    }

    fn value(&self, key: &str) -> Result<Option<String>> {
        let raw = self.raw_value(key);
        resolve_value(key, raw.as_deref(), self.profile, self.input, &self.ctx)
    }

    /// `TableFactory.createSigTable`.
    fn build_model(&self, name: &str, depth: usize) -> Result<Option<Model>> {
        if depth > MAX_TABLE_DEPTH {
            return Err(missing(format!("Profil {}: Tabellen zu tief verschachtelt (Zyklus?)", self.profile.id)));
        }
        let Some(table) = self.profile.tables.get(name) else { return Ok(None) };
        let pid = &self.profile.id;
        let mut rows = Vec::with_capacity(table.rows.len());
        for row in &table.rows {
            let mut entries = Vec::new();
            for spec in &row.0 {
                match spec {
                    CellSpec::SubTable(sub) => {
                        if let Some(m) = self.build_model(sub, depth + 1)? {
                            entries.push(Entry {
                                kind: Kind::Table(Box::new(m)),
                                colspan: 1,
                                default: CellDefault::Value,
                            });
                        }
                    }
                    CellSpec::Image(key) => {
                        if let Some(emblem) = self.label_override(key) {
                            emblem.check()?;
                            let id = format!("emblem:{}", hex_id(&emblem.source_sha256));
                            entries.push(Entry {
                                kind: Kind::Image(ImageRef { key: key.clone(), id, emblem }),
                                colspan: 1,
                                default: CellDefault::Image,
                            });
                        } else if let Some(v) = self.raw_value(key) {
                            let emblem = self.resources.image(&v)?;
                            emblem.check()?;
                            entries.push(Entry {
                                kind: Kind::Image(ImageRef { key: key.clone(), id: v, emblem }),
                                colspan: 1,
                                default: CellDefault::Image,
                            });
                        } else {
                            entries.push(Entry {
                                kind: Kind::Value("IMG MISSING".into()),
                                colspan: 1,
                                default: CellDefault::Value,
                            });
                        }
                    }
                    CellSpec::Value(key) => {
                        let v =
                            self.value(key)?.ok_or_else(|| missing(format!("Profil {pid}: kein Wert für {key}")))?;
                        entries.push(Entry { kind: Kind::Value(v), colspan: 1, default: CellDefault::Value });
                    }
                    CellSpec::Caption(key) => {
                        let raw = self.profile.captions.get(key).map(String::as_str);
                        let c = resolve_value(key, raw, self.profile, self.input, &self.ctx)?
                            .ok_or_else(|| missing(format!("Profil {pid}: keine Caption für {key}")))?;
                        entries.push(Entry { kind: Kind::Caption(c), colspan: 1, default: CellDefault::Caption });
                    }
                    CellSpec::CaptionValue(key) => {
                        let c = self
                            .profile
                            .captions
                            .get(key)
                            .cloned()
                            .ok_or_else(|| missing(format!("Profil {pid}: keine Caption für {key}")))?;
                        let v =
                            self.value(key)?.ok_or_else(|| missing(format!("Profil {pid}: kein Wert für {key}")))?;
                        entries.push(Entry { kind: Kind::Caption(c), colspan: 1, default: CellDefault::Caption });
                        entries.push(Entry { kind: Kind::Value(v), colspan: 1, default: CellDefault::Value });
                    }
                }
            }
            rows.push(entries);
        }
        // `Table.normalize`: letzte Zelle bekommt den Fehlbetrag als Colspan
        let max_cols = rows.iter().map(|r| r.iter().map(|e| e.colspan).sum::<usize>()).max().unwrap_or(0);
        for (i, row) in rows.iter_mut().enumerate() {
            let size: usize = row.iter().map(|e| e.colspan).sum();
            if row.len() == max_cols || size == max_cols {
                continue;
            }
            let last =
                row.last_mut().ok_or_else(|| missing(format!("Profil {pid}: table.{name}.{} ist leer", i + 1)))?;
            last.colspan += max_cols - size;
        }
        Ok(Some(Model {
            name: name.to_string(),
            rows,
            cols_rel: table.cols_width.clone(),
            style: table.style.clone(),
            max_cols,
        }))
    }
}

fn hex_id(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ------------------------------------------------------------------------------------------------
// PDFBoxTable (fixe Breite)
// ------------------------------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct Node {
    name: String,
    rows: Vec<Vec<Entry>>,
    max_cols: usize,
    cols_rel: Option<Vec<f32>>,
    style: Style,
    font_spec: FontSpec,
    font: Arc<LoadedFont>,
    value_spec: FontSpec,
    value_font: Arc<LoadedFont>,
    padding: f32,
    bg: Option<[u8; 3]>,
    col_widths: Vec<f32>,
    row_heights: Vec<f32>,
    width: f32,
    height: f32,
}

impl Env<'_> {
    fn effective_spec(&self, spec: &FontSpec) -> FontSpec {
        if self.pdfa != PdfaMode::None && !self.quirks { spec.embeddable() } else { spec.clone() }
    }

    /// `PDFBoxTable(abstractTable, parent, fixSize)`.
    fn layout_node(&mut self, model: Model, parent_style: Option<&Style>, fix: f32) -> Result<Node> {
        let pid = self.profile.id.clone();
        let style = match parent_style {
            Some(ps) => model.style.clone().unwrap_or_default().inherit(ps),
            None => {
                model.style.clone().ok_or_else(|| missing(format!("Profil {pid}: table.{} ohne Style", model.name)))?
            }
        };
        let font_spec =
            style.font.clone().ok_or_else(|| missing(format!("Profil {pid}: table.{} ohne Style.font", model.name)))?;
        let value_spec = style
            .valuefont
            .clone()
            .ok_or_else(|| missing(format!("Profil {pid}: table.{} ohne Style.valuefont", model.name)))?;
        let font_spec = self.effective_spec(&font_spec);
        let value_spec = self.effective_spec(&value_spec);
        let font = self.fonts.load(&font_spec)?;
        let value_font = self.fonts.load(&value_spec)?;
        let padding = style.padding_or_default();
        let bg = style.bgcolor;

        let mut rows = model.rows;
        // normalizeContent
        for row in rows.iter_mut() {
            for e in row.iter_mut() {
                match &mut e.kind {
                    Kind::Caption(t) => *t = self.normalize_text(t, &font, &value_font),
                    Kind::Value(t) => *t = self.normalize_text(t, &value_font, &value_font),
                    _ => {}
                }
            }
        }

        let col_widths: Vec<f32> = match &model.cols_rel {
            Some(rel) => {
                let mut total = 0f32;
                for r in rel {
                    total += r;
                }
                let unit = fix / total;
                rel.iter().map(|r| unit * r).collect()
            }
            None => {
                let unit = fix / model.max_cols as f32;
                vec![unit; model.max_cols]
            }
        };
        if col_widths.iter().any(|c| !c.is_finite()) {
            // PDF-AS: `writeOperand` lehnt NaN/Infinity ab
            return Err(missing(format!(
                "Profil {pid}: table.{}: ColsWidth ergibt keine endlichen Breiten",
                model.name
            )));
        }

        let mut node = Node {
            name: model.name,
            rows: Vec::new(),
            max_cols: model.max_cols,
            cols_rel: model.cols_rel,
            style,
            font_spec,
            font,
            value_spec,
            value_font,
            padding,
            bg,
            col_widths,
            row_heights: vec![0.0; rows.len()],
            width: 0.0,
            height: 0.0,
        };

        // calculateHeightsBasedOnWidths
        for (i, row) in rows.iter_mut().enumerate() {
            let mut j = 0usize;
            while j < row.len() {
                let span = row[j].colspan;
                if j + span > node.col_widths.len() {
                    return Err(missing(format!(
                        "Profil {pid}: table.{}: Spaltenbreiten passen nicht (Configuration is wrong)",
                        node.name
                    )));
                }
                let mut col_width = 0f32;
                for k in 0..span {
                    col_width += node.col_widths[j + k];
                }
                let h = self.cell_height(&mut row[j], col_width, &node)?;
                if node.row_heights[i] < h {
                    node.row_heights[i] = h;
                }
                j += span; // Eintragsindex wird um den Colspan weitergeschoben (wie PDF-AS)
            }
        }
        node.rows = rows;
        calc_totals(&mut node);
        Ok(node)
    }

    /// `normalizeContent` (Quirk Q11) bzw. Ersetzung nicht darstellbarer Zeichen (Register).
    fn normalize_text(&mut self, text: &str, own_font: &LoadedFont, value_font: &LoadedFont) -> String {
        if self.quirks {
            if value_font.can_encode(text) {
                return text.to_string();
            }
            if text.chars().all(afm::cp1252_roundtrips) {
                return text.replace('\t', " ");
            }
            return java::url_encode(text);
        }
        let replacement = if own_font.can_encode_char('?') { Some('?') } else { None };
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                // Zeilenenden aus Windows-Eingaben (`\r\n`, `\r`) wie `\n` (Register; PDF-AS bricht ab)
                '\r' => {
                    if chars.peek() != Some(&'\n') {
                        out.push('\n');
                    }
                }
                '\n' => out.push('\n'),
                '\t' => out.push(' '),
                NBSP => out.push(NBSP),
                c if own_font.can_encode_char(c) => out.push(c),
                c => {
                    if !self.replaced.contains(&c) {
                        self.replaced.push(c);
                    }
                    if let Some(r) = replacement {
                        out.push(r);
                    }
                }
            }
        }
        out
    }

    /// `getCellHeight(cell, width)`.
    fn cell_height(&mut self, e: &mut Entry, width: f32, node: &Node) -> Result<f32> {
        let padding = node.padding;
        let quirks = self.quirks;
        match &mut e.kind {
            Kind::Caption(t) => {
                let lines = break_string_with(t, width - padding * 2.0, &node.font, node.font_spec.size(), !quirks)?;
                let n = lines.len() as f32;
                *t = lines.join("\n");
                Ok(node.font_spec.size() * n + padding * 2.0)
            }
            Kind::Value(t) => {
                let size = node.value_spec.size();
                let lines = break_string_with(t, width - padding * 2.0, &node.value_font, size, !quirks)?;
                let n = lines.len() as f32;
                *t = lines.join("\n");
                Ok(size * n + padding * 2.0)
            }
            Kind::Image(img) => {
                if let Some((_, h)) = node.style.imagescaletofit {
                    return Ok(h + padding * 2.0);
                }
                let wf = (f64::from(width - padding * 2.0) / f64::from(img.emblem.width)) as f32;
                let ih = (f64::from(wf) * f64::from(img.emblem.height)).floor() as f32;
                Ok(ih + padding * 2.0)
            }
            Kind::Table(_) => {
                let Kind::Table(model) = std::mem::replace(&mut e.kind, Kind::Value(String::new())) else {
                    unreachable!()
                };
                let sub = self.layout_node(*model, Some(&node.style), width)?;
                let h = sub.height;
                e.kind = Kind::Node(Box::new(sub));
                Ok(h)
            }
            Kind::Node(n) => Ok(n.height),
        }
    }
}

/// `calcTotals` inklusive Streckung der Untertabellen über deren letzte Zeile.
fn calc_totals(node: &mut Node) {
    let mut h = 0f32;
    for r in &node.row_heights {
        h += r;
    }
    node.height = h;
    let row_heights = node.row_heights.clone();
    for (i, row) in node.rows.iter_mut().enumerate() {
        for e in row.iter_mut() {
            if let Kind::Node(sub) = &mut e.kind
                && row_heights[i] != sub.height
            {
                set_height(sub, row_heights[i]);
            }
        }
    }
    let mut w = 0f32;
    for c in &node.col_widths {
        w += c;
    }
    node.width = w;
}

/// `setHeight`: positiver Unterschied geht in die letzte Zeile, negativer wird ignoriert.
fn set_height(node: &mut Node, height: f32) {
    let diff = height - node.height;
    if diff > 0.0 {
        if let Some(last) = node.row_heights.last_mut() {
            *last += diff;
        }
        calc_totals(node);
    }
}

// ------------------------------------------------------------------------------------------------
// Ressourcen (readTableResources) und Zeichengeometrie (TableDrawUtils)
// ------------------------------------------------------------------------------------------------

/// Bild, wie es in den Ressourcen des Inner-Forms landet (`Im1`, `Im2` …).
#[derive(Debug, Clone)]
pub(crate) struct PreparedImage {
    pub id: String,
    pub emblem: Arc<Emblem>,
    pub width: f32,
    pub height: f32,
}

/// Layout plus alles, was [`crate::Appearance::build`] zum Zeichnen braucht.
pub(crate) struct Prepared {
    pub layout: Layout,
    pub root: Node,
    pub images: Vec<PreparedImage>,
    pub quirks: bool,
}

impl Node {
    /// Von `readTableResources` überschriebene Spaltenwerte: `(rel / Σrel) · width`.
    fn absolute_cols(&self) -> Vec<f32> {
        match &self.cols_rel {
            Some(rel) => {
                let mut total = 0f32;
                for r in rel {
                    total += r;
                }
                rel.iter().map(|r| (r / total) * self.width).collect()
            }
            None => {
                let mut total = 0f32;
                for _ in 0..self.max_cols {
                    total += 1.0;
                }
                vec![(1.0 / total) * self.width; self.max_cols]
            }
        }
    }

    /// `TableDrawUtils.getColSizes` (nach der Überschreibung durch `readTableResources`).
    fn draw_cols(&self) -> Vec<f32> {
        let base: Vec<f32> = match &self.cols_rel {
            Some(_) => self.absolute_cols().into_iter().take(self.max_cols).collect(),
            None => vec![1.0; self.max_cols],
        };
        let mut total = 0f32;
        for c in &base {
            total += c;
        }
        base.iter().map(|c| (c / total) * self.width).collect()
    }
}

/// `readTableResources`: Bildgrößen (erste Verwendung eines Bildes gewinnt) in Traversierreihenfolge.
fn read_resources(node: &Node, images: &mut Vec<PreparedImage>) {
    let cols = node.absolute_cols();
    let padding = node.padding;
    for (i, row) in node.rows.iter().enumerate() {
        for (j, e) in row.iter().enumerate() {
            match &e.kind {
                Kind::Image(img) => {
                    if images.iter().any(|p| p.id == img.id) {
                        continue;
                    }
                    let width = cols.get(j).copied().unwrap_or(0.0);
                    let height = node.row_heights[i] + padding * 2.0;
                    let mut iw = f64::from(width).floor() as i32 as f32;
                    iw -= 2.0 * padding;
                    let mut ih = f64::from(height).floor() as i32 as f32;
                    ih -= 2.0 * padding;
                    let ow = img.emblem.width as f32;
                    let oh = img.emblem.height as f32;
                    if let Some((sw, sh)) = node.style.imagescaletofit {
                        iw = sw;
                        ih = sh;
                    }
                    let wf = iw / ow;
                    let hf = ih / oh;
                    let sf = if hf < wf { hf } else { wf };
                    let dw = f64::from(sf * ow).floor() as f32;
                    let dh = f64::from(sf * oh).floor() as f32;
                    images.push(PreparedImage {
                        id: img.id.clone(),
                        emblem: img.emblem.clone(),
                        width: dw,
                        height: dh,
                    });
                }
                Kind::Node(sub) => read_resources(sub, images),
                _ => {}
            }
        }
    }
}

/// Zeichenoperationen in Java-Reihenfolge (`TableDrawUtils`); [`crate::Appearance::build`] schreibt
/// sie als Content-Stream, [`layout`] verwirft sie.
pub(crate) trait Sink {
    /// Hintergrund: `setNonStrokingColor(bg)`, `addRect` + `fill`, `setNonStrokingColor(BLACK)`.
    fn background(&mut self, rgb: [u8; 3], x: f32, y: f32, w: f32, h: f32);
    /// `setLineWidth`
    fn line_width(&mut self, w: f32);
    /// `drawLine`
    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32);
    /// Textblock (`BT … ET`).
    fn text(&mut self, op: TextOp<'_>) -> Result<()>;
    /// Bild: `q w 0 0 h x y cm /Imn Do Q` (`index` in [`Prepared::images`]).
    fn image(&mut self, index: usize, x: f32, y: f32, w: f32, h: f32);
}

/// Position eines Textblocks.
pub(crate) enum TextPlacement {
    /// Alle Zeilen ab `(tx, ty)`, Folgezeilen `0 -size Td`.
    Block { tx: f32, ty: f32 },
    /// `lineCenter` mit mehreren Zeilen: je Zeile eigener x-Versatz (absolut), erste Grundlinie `y0`.
    LineCenter { y0: f32, offsets: Vec<f32> },
}

/// Ein Textblock.
pub(crate) struct TextOp<'a> {
    pub font_key: String,
    pub font: &'a Arc<LoadedFont>,
    pub size: f32,
    pub lines: &'a [String],
    pub placement: TextPlacement,
}

/// Sink ohne Ausgabe.
struct NoSink;

impl Sink for NoSink {
    fn background(&mut self, _: [u8; 3], _: f32, _: f32, _: f32, _: f32) {}
    fn line_width(&mut self, _: f32) {}
    fn line(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn text(&mut self, _: TextOp<'_>) -> Result<()> {
        Ok(())
    }
    fn image(&mut self, _: usize, _: f32, _: f32, _: f32, _: f32) {}
}

struct GeomCtx<'a> {
    images: &'a [PreparedImage],
    all_tu: String,
    outer_tu: String,
}

/// `drawBorder`: Linien einzeln; Spaltenkanten über **alle** Einträge der Zeile (Eintragsindex als
/// Spaltenindex, wie PDF-AS).
fn draw_border(node: &Node, cols: &[f32], x: f32, y: f32, width: f32, height: f32, sink: &mut dyn Sink) {
    let border = node.style.border_or_default();
    if border <= 0.0 || node.row_heights.is_empty() {
        return;
    }
    sink.line_width(border);
    let x_to = x + width;
    let mut y_from = y + height;
    sink.line(x, y_from, x_to, y_from);
    for rh in &node.row_heights {
        y_from -= rh;
        sink.line(x, y_from, x_to, y_from);
    }
    let mut y_from = y + height;
    let mut y_to = y_from - node.row_heights[0];
    let rows = node.rows.len();
    for (i, row) in node.rows.iter().enumerate() {
        let mut x_from = x;
        sink.line(x_from, y_from, x_from, y_to);
        for (j, e) in row.iter().enumerate() {
            for k in 0..e.colspan {
                if let Some(w) = cols.get(k + j) {
                    x_from += w;
                }
            }
            sink.line(x_from, y_from, x_from, y_to);
        }
        if i + 1 < rows {
            y_from = y_to;
            y_to = y_from - node.row_heights[i + 1];
        }
    }
}

/// `drawString`: Positionen exakt wie PDF-AS (Blockausrichtung, Q1/Q2/Q3).
#[allow(clippy::too_many_arguments)]
fn place_text(
    x: f32,
    top: f32,
    width: f32,
    height: f32,
    padding: f32,
    lines: &[String],
    font: &LoadedFont,
    size: f32,
    halign: Option<HAlign>,
    valign: Option<VAlign>,
) -> Result<TextPlacement> {
    let text_height = size * lines.len() as f32;
    let mut ty = top - padding;
    let mut tx = x + padding;
    let inner_h = height - 2.0 * padding;
    let inner_w = width - 2.0 * padding;
    match valign {
        Some(VAlign::Bottom) => ty -= inner_h - text_height,
        Some(VAlign::Middle) => ty -= (inner_h - text_height) / 2.0,
        _ => {}
    }
    let descent = font.descent() / 1000.0 * size;
    // Java: `ty - fontSize + (descent * (-1))`; Negation ist in IEEE exakt
    let baseline = ty - size + (-descent);
    if lines.len() > 1 && halign == Some(HAlign::LineCenter) {
        let mut offsets = Vec::with_capacity(lines.len());
        for l in lines {
            let lw = font.string_width(l)? / 1000.0 * size;
            offsets.push(tx + (inner_w - lw) / 2.0);
        }
        return Ok(TextPlacement::LineCenter { y0: baseline, offsets });
    }
    let mut max_w = 0f32;
    for l in lines {
        let lw = font.string_width(l)? / 1000.0 * size;
        if max_w < lw {
            max_w = lw;
        }
    }
    match halign {
        Some(HAlign::Center) | Some(HAlign::LineCenter) => {
            let off = inner_w - max_w;
            if off > 0.0 {
                tx += off / 2.0;
            }
        }
        Some(HAlign::Right) => {
            let off = inner_w - max_w;
            if off > 0.0 {
                tx += off;
            }
        }
        _ => {}
    }
    Ok(TextPlacement::Block { tx, ty: baseline })
}

/// `drawTable`/`drawContent`: zeichnet über `sink` und liefert die Zellgeometrie.
#[allow(clippy::too_many_arguments)]
fn draw_table(
    node: &Node,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    g: &mut GeomCtx<'_>,
    sink: &mut dyn Sink,
    outer: bool,
) -> Result<LaidOutTable> {
    if let Some(bg) = node.bg {
        sink.background(bg, x, y, node.width, node.height);
    }
    let cols = node.draw_cols();
    draw_border(node, &cols, x, y, width, height, sink);
    let mut cells = Vec::new();
    let mut tu = String::new();
    let mut content_y = y + height;
    for (i, row) in node.rows.iter().enumerate() {
        let mut content_x = x;
        let row_h = node.row_heights[i];
        let mut j = 0usize;
        while j < row.len() {
            let e = &row[j];
            let cell_style = node.style.inherit(&e.default.style());
            if j + e.colspan > cols.len() {
                return Err(missing(format!(
                    "table.{}: Spaltenbreiten passen nicht (Configuration is wrong)",
                    node.name
                )));
            }
            let mut col_width = 0f32;
            for k in 0..e.colspan {
                col_width += cols[j + k];
            }
            let content = match &e.kind {
                Kind::Caption(t) | Kind::Value(t) => {
                    tu.push_str(t);
                    tu.push('\n');
                    g.all_tu.push_str(t);
                    g.all_tu.push('\n');
                    let caption = matches!(e.kind, Kind::Caption(_));
                    let lines: Vec<String> = java::split(t, '\n').into_iter().map(str::to_string).collect();
                    let (spec, font, halign, valign, role) = if caption {
                        (&node.font_spec, &node.font, cell_style.halign, cell_style.valign, TextRole::Caption)
                    } else {
                        (
                            &node.value_spec,
                            &node.value_font,
                            cell_style.valuehalign,
                            cell_style.valuevalign,
                            TextRole::Value,
                        )
                    };
                    let size = spec.size();
                    let placement = place_text(
                        content_x,
                        content_y,
                        col_width,
                        row_h,
                        node.padding,
                        &lines,
                        font,
                        size,
                        halign,
                        valign,
                    )?;
                    sink.text(TextOp { font_key: spec.cache_key(), font, size, lines: &lines, placement })?;
                    CellContent::Text { lines, font: spec.clone(), role, halign, valign }
                }
                Kind::Image(img) => {
                    let idx = g.images.iter().position(|p| p.id == img.id);
                    let (iw, ih) = idx.map(|k| (g.images[k].width, g.images[k].height)).unwrap_or((0.0, 0.0));
                    // drawImage: ohne Padding; imagehalign=left wirkt wie center (Q4)
                    let mut img_x = content_x;
                    let hoff = col_width - iw;
                    match cell_style.imagehalign {
                        Some(HAlign::Right) => img_x += hoff,
                        _ => img_x += hoff / 2.0,
                    }
                    let mut img_y = content_y;
                    let voff = row_h - ih;
                    match cell_style.imagevalign {
                        Some(VAlign::Middle) => img_y -= voff / 2.0,
                        Some(VAlign::Bottom) => img_y -= voff,
                        _ => {}
                    }
                    if let Some(k) = idx {
                        sink.image(k, img_x, img_y - ih, iw, ih);
                    }
                    CellContent::Image {
                        key: img.key.clone(),
                        draw_width: iw,
                        draw_height: ih,
                        halign: cell_style.imagehalign,
                        valign: cell_style.imagevalign,
                    }
                }
                Kind::Node(sub) => {
                    let t = draw_table(sub, content_x, content_y - row_h, col_width, row_h, g, sink, false)?;
                    CellContent::SubTable(Box::new(t))
                }
                Kind::Table(_) => CellContent::Empty,
            };
            cells.push(LaidOutCell {
                row: i,
                col: j,
                x: content_x,
                top: content_y,
                width: col_width,
                height: row_h,
                content,
            });
            content_x += col_width;
            j += e.colspan;
        }
        content_y -= row_h;
    }
    if outer {
        g.outer_tu = tu;
    }
    Ok(LaidOutTable {
        name: node.name.clone(),
        col_widths: cols,
        row_heights: node.row_heights.clone(),
        width: node.width,
        height: node.height,
        padding: node.padding,
        border: node.style.border_or_default(),
        bgcolor: node.bg,
        cells,
    })
}

/// Alle Maße einer Tabelle (rekursiv) sind endlich.
fn node_is_finite(node: &Node) -> bool {
    let style = &node.style;
    [node.padding, style.border_or_default(), node.width, node.height, node.font_spec.size(), node.value_spec.size()]
        .iter()
        .chain(&node.col_widths)
        .chain(&node.row_heights)
        .chain(&node.draw_cols())
        .all(|v| v.is_finite())
        && node.rows.iter().flatten().all(|e| match &e.kind {
            Kind::Node(sub) => node_is_finite(sub),
            _ => true,
        })
}

impl Prepared {
    /// Zeichnet die Haupttabelle bei (0, 1) (`drawTable(page, cs, 0, 1, W, H, …)`).
    pub(crate) fn draw(&self, sink: &mut dyn Sink) -> Result<()> {
        let mut g = GeomCtx { images: &self.images, all_tu: String::new(), outer_tu: String::new() };
        draw_table(&self.root, 0.0, 1.0, self.root.width, self.root.height, &mut g, sink, true)?;
        Ok(())
    }
}

/// Vollständiges Layout (Modell, fixe Breite, Ressourcen, Geometrie).
pub(crate) fn prepare(
    profile: &Profile,
    input: &SigBlockInput,
    width: f32,
    pdfa: PdfaMode,
    fonts: &FontProvider,
    resources: &dyn Resources,
) -> Result<Prepared> {
    if !width.is_finite() || width <= 0.0 {
        return Err(SigBlockError::Profile(format!("ungültige Blockbreite {width}")));
    }
    let mut env = Env {
        profile,
        input,
        ctx: ExprContext::from_input(input),
        fonts,
        resources,
        pdfa,
        quirks: input.pdfas_quirks,
        replaced: Vec::new(),
    };
    let model = env
        .build_model("main", 0)?
        .ok_or_else(|| missing(format!("Profil {}: table.main fehlt oder wurde verworfen", profile.id)))?;
    let root = env.layout_node(model, None, width)?;
    let mut images = Vec::new();
    read_resources(&root, &mut images);
    let mut g = GeomCtx { images: &images, all_tu: String::new(), outer_tu: String::new() };
    let table = draw_table(&root, 0.0, 1.0, root.width, root.height, &mut g, &mut NoSink, true)?;
    let tu_text = if env.quirks { g.outer_tu } else { g.all_tu };
    if !(root.width.is_finite() && root.height.is_finite()) {
        return Err(missing(format!("Profil {}: Blockmaße nicht endlich (ColsWidth?)", profile.id)));
    }
    // PDFBox lehnt NaN/Infinity beim Schreiben ab (`writeOperand`); hier schon beim Layout, damit
    // `layout` und `Appearance::build` dieselben Profile akzeptieren.
    if !node_is_finite(&root) || images.iter().any(|i| !(i.width.is_finite() && i.height.is_finite())) {
        return Err(missing(format!(
            "Profil {}: Rahmen, Padding, Spalten oder Bildgröße nicht endlich (Style prüfen)",
            profile.id
        )));
    }
    let layout = Layout { width: root.width, height: root.height, table, tu_text, replaced_chars: env.replaced };
    Ok(Prepared { layout, root, images, quirks: env.quirks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::Std14;

    #[test]
    fn break_string_greedy_strict() {
        let mono = FontProvider::builtin().load(&FontSpec::parse("TTF:DejaVuSansMono.ttf,5")).unwrap();
        // 42 Zeichen passen in 129.3219 pt (42·3.01 = 126.42), 43 nicht (129.43)
        let s42 = "a".repeat(42);
        assert_eq!(break_string(&s42, 129.3219, &mono, 5.0).unwrap(), vec![s42.clone()]);
        let words = format!("{} b", "a".repeat(41)); // 43 Zeichen inkl. Leerzeichen
        assert_eq!(break_string(&words, 129.3219, &mono, 5.0).unwrap(), vec!["a".repeat(41), "b".to_string()]);
        // genau gleich breit ⇒ kein Umbruch (strikt größer)
        assert_eq!(break_string("aa bb", 15.06, &mono, 5.0).unwrap(), vec!["aa bb"]);
        assert_eq!(break_string("aa bb", 15.04, &mono, 5.0).unwrap(), vec!["aa", "bb"]);
        // überlanges Wort ragt hinaus
        assert_eq!(break_string("xxxxxxxxxx", 5.0, &mono, 5.0).unwrap(), vec!["xxxxxxxxxx"]);
        // Leerzeile durch \n\n, mehrfache Leerzeichen bleiben erhalten
        assert_eq!(break_string("a\n\nb", 100.0, &mono, 5.0).unwrap(), vec!["a", "", "b"]);
        assert_eq!(break_string("a  b", 100.0, &mono, 5.0).unwrap(), vec!["a  b"]);
        assert_eq!(break_string("", 100.0, &mono, 5.0).unwrap(), vec![""]);
        assert_eq!(break_string(" ", 100.0, &mono, 5.0).unwrap(), vec![""]);
        // PDF-AS-Quirk: Leerzeichen vor \n erzeugt eine Leerzeile
        assert_eq!(break_string("a \nb", 100.0, &mono, 5.0).unwrap(), vec!["a", "", "b"]);
        // NBSP verbindet, wird als Leerzeichen gemessen und ausgegeben
        assert_eq!(break_string("a\u{a0}b c", 100.0, &mono, 5.0).unwrap(), vec!["a b c"]);
        // \n am Wortende: PDF-AS scheitert (kein Glyph), lenient bricht um
        assert!(break_string("abc\n def", 100.0, &mono, 5.0).is_err());
        assert_eq!(break_string_with("abc\n def", 100.0, &mono, 5.0, true).unwrap(), vec!["abc", "def"]);
    }

    #[test]
    fn break_string_caption_block() {
        let hb = LoadedFont::standard(Std14::HelveticaBold);
        let lines = break_string("Unterzeichner/\nSiegelersteller", 62.857, &hb, 8.0).unwrap();
        assert_eq!(lines, vec!["Unterzeichner/", "Siegelersteller"]);
    }
}

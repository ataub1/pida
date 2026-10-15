//! Positionsstring (`TablePos`) und PDF-AS-Positionierung (docs/analyse/04 §4.1–§4.5).

use std::fmt;
use std::str::FromStr;

use pdfover_pdf_cos::{PageInfo, Rect, Rotation, SigField};
use pdfover_render::{ForcedColorOptions, Renderer, RgbaImage};

use crate::coords::{Point, clamp_top_left};
use crate::{CompatMode, MARGIN_H, MARGIN_V, PlacementError, Result};

/// Seitenmodus `p:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum PageSelector {
    /// `p:auto` (Default) – letzte Seite, Freiraumsuche.
    #[default]
    Auto,
    /// `p:new`
    New,
    /// `p:last`
    Last,
    /// `p:<n>` (1-basiert; `n > Seitenzahl` ⇒ neue Seite)
    Exact(u32),
}

/// Wert oder `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Coord {
    /// `auto`
    #[default]
    Auto,
    /// fester Wert in pt
    Value(f32),
}

impl Coord {
    /// Fester Wert, sonst `None`.
    pub fn value(self) -> Option<f32> {
        match self {
            Coord::Auto => None,
            Coord::Value(v) => Some(v),
        }
    }
}

/// PDF-AS-Positionsstring `p:..;x:..;y:..;w:..;f:..;r:..` (docs/analyse/04 §4.1).
/// `None` = Schlüssel nicht genannt (wichtig für [`TablePos::merge`]).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct TablePos {
    /// `p`
    pub page: Option<PageSelector>,
    /// `x` (linke Kante)
    pub x: Option<Coord>,
    /// `y` (Oberkante, von **unten** gemessen)
    pub y: Option<Coord>,
    /// `w` (≥ 1)
    pub w: Option<Coord>,
    /// `f` Fußzeilenhöhe (nur bei `y:auto` und nicht `p:new` wirksam)
    pub footer: Option<f32>,
    /// `r` Blockrotation (Vielfaches von 90, gegen den Uhrzeigersinn)
    pub rotation: Option<i32>,
}

/// Name des Positionsstrings in PDF-Over (`SignaturePosition`); identisch mit [`TablePos`].
pub type SignaturePosition = TablePos;

/// Java `String.split` (ein Zeichen): leere Teile am Ende entfallen, `""` ergibt `[""]`.
pub(crate) fn java_split(s: &str, sep: char) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut parts: Vec<&str> = s.split(sep).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Java `Float.parseFloat` (Dezimalschreibweise, Leerraum ≤ U+0020 am Rand, Suffix `f`/`d`);
/// nicht endliche Werte (`NaN`, `Infinity`, Überlauf) und Hex-Gleitkomma werden abgelehnt.
pub(crate) fn java_parse_float(v: &str) -> Option<f32> {
    let t = v.trim_matches(|c: char| c <= ' ');
    let t = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E')) {
        return None;
    }
    let f: f32 = t.parse().ok()?;
    f.is_finite().then_some(f)
}

fn invalid(s: &str) -> PlacementError {
    PlacementError::InvalidPosition(s.to_owned())
}

fn parse_min(val: &str, min: f32, whole: &str) -> Result<f32> {
    let v = java_parse_float(val).ok_or_else(|| invalid(whole))?;
    if v < min {
        return Err(invalid(whole));
    }
    Ok(v)
}

fn parse_coord(val: &str, min: f32, whole: &str) -> Result<Coord> {
    if val.eq_ignore_ascii_case("auto") { Ok(Coord::Auto) } else { parse_min(val, min, whole).map(Coord::Value) }
}

fn fmt_num(v: f32) -> String {
    // kürzeste Darstellung, die exakt zurückgelesen wird; nie Exponentenschreibweise
    format!("{v}")
}

impl TablePos {
    /// Vollautomatisch (`p:auto;x:auto;y:auto`, was PDF-Over bei Auto-Positionierung übergibt).
    pub fn auto() -> Self {
        TablePos { page: Some(PageSelector::Auto), x: Some(Coord::Auto), y: Some(Coord::Auto), ..Default::default() }
    }

    /// Manuell auf Seite `page_1based` (PDF-Over: `p:%d;x:%f;y:%f`).
    pub fn manual(page_1based: u32, x: f32, y: f32) -> Self {
        TablePos {
            page: Some(PageSelector::Exact(page_1based)),
            x: Some(Coord::Value(x)),
            y: Some(Coord::Value(y)),
            ..Default::default()
        }
    }

    /// Parst die PDF-AS-Syntax; `k` genau ein Zeichen, sonst Fehler.
    ///
    /// Semantik wie `TablePos.parsePosString` (PDF-AS 4.4): Paare an `;` und `:` mit Java-`split`
    /// (leere Teile am Ende entfallen, `"p:1;"` ist gültig, `""` und `";p:1"` nicht), Werte
    /// `auto` ohne Groß-/Kleinschreibung, `x`/`y`/`f` ≥ 0, `w` ≥ 1, `p` ≥ 1 bzw.
    /// `auto`/`new`/`last`, `r` Vielfaches von 90 (modulo 360, Vorzeichen bleibt). Spätere
    /// Schlüssel überschreiben frühere. Abweichung: nicht endliche Zahlen werden abgelehnt
    /// (PDF-AS akzeptiert z. B. `x:NaN`).
    pub fn parse(s: &str) -> Result<Self> {
        let mut pos = TablePos::default();
        for cmd in java_split(s, ';') {
            let kv = java_split(cmd, ':');
            if kv.len() != 2 {
                return Err(invalid(s));
            }
            let mut chars = kv[0].chars();
            let (Some(key), None) = (chars.next(), chars.next()) else {
                return Err(invalid(s));
            };
            let val = kv[1];
            match key {
                'x' => pos.x = Some(parse_coord(val, 0.0, s)?),
                'y' => pos.y = Some(parse_coord(val, 0.0, s)?),
                'w' => pos.w = Some(parse_coord(val, 1.0, s)?),
                'p' => {
                    pos.page = Some(if val.eq_ignore_ascii_case("auto") {
                        PageSelector::Auto
                    } else if val.eq_ignore_ascii_case("new") {
                        PageSelector::New
                    } else if val.eq_ignore_ascii_case("last") {
                        PageSelector::Last
                    } else {
                        let n: i32 = val.parse().map_err(|_| invalid(s))?;
                        if n < 1 {
                            return Err(invalid(s));
                        }
                        PageSelector::Exact(n as u32)
                    })
                }
                'f' => pos.footer = Some(parse_min(val, 0.0, s)?),
                'r' => {
                    let r = java_parse_float(val).ok_or_else(|| invalid(s))? % 360.0;
                    if r % 90.0 != 0.0 {
                        return Err(invalid(s));
                    }
                    pos.rotation = Some(r as i32);
                }
                _ => return Err(invalid(s)),
            }
        }
        Ok(pos)
    }

    /// Request-Werte überschreiben **nur die genannten Schlüssel** des Profil-Defaults
    /// (`new TablePos(request, profileDefault)`).
    ///
    /// Java-Semantik (`TablePos.readFromPos` + `parsePosString`): `x:auto`, `y:auto` und `w:auto`
    /// im Request lassen einen **festen** Profilwert stehen (sie setzen in PDF-AS nichts);
    /// `p:auto` setzt den Seitenmodus dagegen zurück. Die Fußzeile des Profils geht verloren,
    /// wenn das Profil `y` fest oder `p:new` angibt (`getFooterLine()` liefert dann 0).
    pub fn merge(request: &TablePos, profile_default: &TablePos) -> TablePos {
        let coord = |req: Option<Coord>, base: Option<Coord>| match (req, base) {
            (Some(Coord::Value(v)), _) => Some(Coord::Value(v)),
            (Some(Coord::Auto), Some(Coord::Value(v))) => Some(Coord::Value(v)),
            (Some(Coord::Auto), _) => Some(Coord::Auto),
            (None, base) => base,
        };
        let base_footer =
            if matches!(profile_default.y, Some(Coord::Value(_))) || profile_default.page == Some(PageSelector::New) {
                profile_default.footer.map(|_| 0.0)
            } else {
                profile_default.footer
            };
        TablePos {
            page: request.page.or(profile_default.page),
            x: coord(request.x, profile_default.x),
            y: coord(request.y, profile_default.y),
            w: coord(request.w, profile_default.w),
            footer: request.footer.or(base_footer),
            rotation: request.rotation.or(profile_default.rotation),
        }
    }

    /// Wirksamer Seitenmodus (Default `auto`).
    pub fn page_mode(&self) -> PageSelector {
        self.page.unwrap_or_default()
    }

    /// Wirksame Fußzeile wie `TablePos.getFooterLine()`: 0 bei festem `y` oder `p:new`.
    pub fn footer_line(&self) -> f32 {
        if matches!(self.y, Some(Coord::Value(_))) || self.page_mode() == PageSelector::New {
            0.0
        } else {
            self.footer.unwrap_or(0.0)
        }
    }

    /// Wirksame Blockrotation (Default 0).
    pub fn block_rotation(&self) -> i32 {
        self.rotation.unwrap_or(0)
    }
}

impl FromStr for TablePos {
    type Err = PlacementError;
    fn from_str(s: &str) -> Result<Self> {
        TablePos::parse(s)
    }
}

impl fmt::Display for TablePos {
    /// PDF-AS-Syntax (Werte mit Punkt als Dezimaltrenner).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        if let Some(p) = self.page {
            parts.push(match p {
                PageSelector::Auto => "p:auto".into(),
                PageSelector::New => "p:new".into(),
                PageSelector::Last => "p:last".into(),
                PageSelector::Exact(n) => format!("p:{n}"),
            });
        }
        for (k, c) in [('x', self.x), ('y', self.y), ('w', self.w)] {
            if let Some(c) = c {
                parts.push(match c {
                    Coord::Auto => format!("{k}:auto"),
                    Coord::Value(v) => format!("{k}:{}", fmt_num(v)),
                });
            }
        }
        if let Some(v) = self.footer {
            parts.push(format!("f:{}", fmt_num(v)));
        }
        if let Some(r) = self.rotation {
            parts.push(format!("r:{r}"));
        }
        f.write_str(&parts.join(";"))
    }
}

/// Vom Nutzer gewählte Position (UI-Zustand, PAGE_ABS, linke obere Ecke).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Placement {
    /// 0-basierter Seitenindex; `== Seitenzahl` bedeutet „neue Seite“ (virtuelle Seite F-38).
    pub page_index: usize,
    /// linke Kante in pt
    pub x: f32,
    /// Oberkante in pt (von unten)
    pub y: f32,
    /// Blockbreite in pt
    pub width: f32,
    /// Blockhöhe in pt
    pub height: f32,
}

impl Placement {
    /// Als `TablePos` (`p:<page+1>;x;y`).
    pub fn to_table_pos(&self) -> TablePos {
        TablePos::manual(u32::try_from(self.page_index).unwrap_or(u32::MAX).saturating_add(1), self.x, self.y)
    }
}

/// Blockmaße.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockGeometry {
    /// Breite
    pub width: f32,
    /// Höhe
    pub height: f32,
}

/// Dokumentkontext der Positionierung.
#[derive(Debug, Clone, Copy)]
pub struct PositionContext<'a> {
    /// Seiten.
    pub pages: &'a [PageInfo],
    /// Dokument hat ≥ 1 signiertes Feld (neue Seite dann nicht erlaubt, docs/analyse/04 §4.5).
    pub already_signed: bool,
    /// Profil `SIGNED_NEWPAGE_FORCE`.
    pub signed_newpage_force: bool,
    /// `sigblock.placement.less.space.failing` (Fehler statt Ausweichen).
    pub less_space_failing: bool,
    /// Korrigiertes Verhalten (Default) oder PDF-AS-Originalverhalten (`compat.pdfas_quirks`,
    /// nur für Golden-Tests), siehe Kompatibilitätsregister in der Crate-Doku.
    pub compat: CompatMode,
}

impl<'a> PositionContext<'a> {
    /// Kontext für ein unsigniertes Dokument mit Standardeinstellungen.
    pub fn new(pages: &'a [PageInfo]) -> Self {
        Self {
            pages,
            already_signed: false,
            signed_newpage_force: false,
            less_space_failing: false,
            compat: CompatMode::default(),
        }
    }
}

/// Ergebnis von [`resolve_width`] (vor dem Layout benötigt).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WidthDecision {
    /// 0-basierte Zielseite (vorläufig): die Seite, deren Geometrie die Breite bestimmt (bei
    /// einer neuen Seite die letzte vorhandene).
    pub page_index: usize,
    /// linke Kante
    pub x: f32,
    /// Blockbreite (AMTS ohne `w`: `pw − 2x`; Korrektur gegenüber PDF-AS: bei manuellem x gilt die
    /// Breite der Vorschau, Register)
    pub width: f32,
}

/// Hinweise an die UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlacementNotice {
    /// Neue Seite auf signiertem Dokument nicht erlaubt – letzte Seite verwendet (sichtbarer
    /// Hinweis statt stillem Ausweichen, Plan §6.1).
    NewPageNotAllowed,
    /// Block überdeckt Seiteninhalt (RTR 8852, Plan §6.1).
    OverlapsContent,
    /// Block überdeckt vorhandene Widgets.
    OverlapsWidget,
    /// Position wurde auf die Seite geklemmt.
    Clamped,
    /// Freiraumsuche fehlgeschlagen (Renderfehler); die Seite wurde wie bei PDF-AS als leer
    /// behandelt.
    ScanFailed,
}

impl PlacementNotice {
    /// Stabiler Code (ADR 0001 Regel 11; Texte im Abschlussbericht).
    pub fn code(&self) -> &'static str {
        match self {
            PlacementNotice::NewPageNotAllowed => "placement.notice.new_page_not_allowed",
            PlacementNotice::OverlapsContent => "placement.notice.overlaps_content",
            PlacementNotice::OverlapsWidget => "placement.notice.overlaps_widget",
            PlacementNotice::Clamped => "placement.notice.clamped",
            PlacementNotice::ScanFailed => "placement.notice.scan_failed",
        }
    }
}

/// Endgültige Position.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedPosition {
    /// 0-basierter Seitenindex; bei `new_page` = Index der neu angehängten Seite.
    pub page_index: usize,
    /// Neue Seite anhängen (gleiche MediaBox und `/Rotate` wie die letzte Seite).
    pub new_page: bool,
    /// linke Kante (PAGE_ABS)
    pub x: f32,
    /// Oberkante (PAGE_ABS)
    pub y: f32,
    /// Breite
    pub width: f32,
    /// Höhe
    pub height: f32,
    /// Blockrotation `r`
    pub rotation: i32,
    /// Hinweise
    pub notices: Vec<PlacementNotice>,
}

/// Freiraumsuche: Abstand der untersten belegten Pixelzeile oberhalb der Fußzeile von der
/// Oberkante in pt (0 = leere Seite).
pub trait FreeSpaceScanner {
    /// Rasterscan einer Seite.
    fn lowest_content(&mut self, page_index: usize, footer: f32) -> Result<f32>;

    /// Ist im sichtbaren Rechteck `area` (PAGE_ABS, pt) Seiteninhalt? `None` = unbekannt
    /// (Default; dann gibt es keinen Hinweis [`PlacementNotice::OverlapsContent`]).
    fn region_occupied(&mut self, page_index: usize, area: Rect) -> Result<Option<bool>> {
        let _ = (page_index, area);
        Ok(None)
    }
}

/// Rasterscan über das Forced-Color-Rendering (docs/analyse/04 §4.4, Plan §6.7).
#[derive(Debug)]
pub struct RasterScanner<'a, R: Renderer + ?Sized> {
    /// Renderer des Dokuments.
    pub renderer: &'a R,
    /// Optionen (Signalfarbe, 72 dpi).
    pub options: ForcedColorOptions,
}

/// Unterste belegte Pixelzeile wie `PDFUtilities.getMaxYPosition`: von Zeile
/// `height − 1 − footer_px` aufwärts bis Zeile 1 die erste Zeile mit einem Pixel ≠ `background`
/// (exakter Vergleich inklusive Alpha); nichts gefunden ⇒ 0. Ist die Fußzeile höher als das
/// Bild, liefert PDF-AS den (negativen) Startwert – ebenso hier.
pub fn lowest_content_row(image: &RgbaImage, footer_px: i64, background: [u8; 4]) -> i64 {
    lowest_content_row_with_tolerance(image, footer_px, background, 0)
}

/// Kantenglättungs-Toleranz des [`RasterScanner`]: Pixel, deren Kanalbeträge zusammen um
/// weniger als diesen Wert vom Hintergrund abweichen, gelten als frei. Das entspricht etwa
/// 1/64 Deckung der Signalfarbe – der Subpixel-Auflösung des Java2D-Rasterizers (Marlin, 8 × 8),
/// mit dem PDF-AS rastert – und blendet Kantenreste von PDFium aus, die Java2D nicht erzeugt.
pub const SCAN_TOLERANCE: u32 = 6;

fn differs(p: &[u8; 4], background: &[u8; 4], tolerance: u32) -> bool {
    if tolerance == 0 {
        return p != background;
    }
    let d: u32 = p.iter().zip(background.iter()).map(|(a, b)| u32::from(a.abs_diff(*b))).sum();
    d >= tolerance
}

/// Wie [`lowest_content_row`], aber Pixel mit einer Abweichung (Summe der Kanalbeträge) unter
/// `tolerance` zählen als Hintergrund; `tolerance = 0` ist der exakte Vergleich von PDF-AS.
pub fn lowest_content_row_with_tolerance(
    image: &RgbaImage,
    footer_px: i64,
    background: [u8; 4],
    tolerance: u32,
) -> i64 {
    let start = (i64::from(image.height) - 1).saturating_sub(footer_px);
    if start < 0 {
        return start;
    }
    let start = start.min(i64::from(image.height) - 1);
    for row in (1..=start).rev() {
        if image.row(row as u32).as_chunks::<4>().0.iter().any(|p| differs(p, &background, tolerance)) {
            return row;
        }
    }
    0
}

/// Hintergrundfarbe wie `PDFUtilities.determineBackgroundColor`
/// (`sigblock.placement.bgcolor.detection.enabled=true`): häufigste Farbe der vier Pixel 5 px
/// innerhalb der Ecken (Gleichstand: Reihenfolge oben links, oben rechts, unten links, unten
/// rechts – in PDF-AS hängt das von der `HashMap`-Reihenfolge ab). Zu kleine Bilder ⇒ `None`.
pub fn detect_background(image: &RgbaImage) -> Option<[u8; 4]> {
    let inset = 5u32;
    if image.width <= inset || image.height <= inset {
        return None;
    }
    // PDF-AS liest `width − inset` bzw. `height − inset` (nicht `− 1 − inset`).
    let samples = [
        image.pixel(inset, inset),
        image.pixel(image.width - inset, inset),
        image.pixel(inset, image.height - inset),
        image.pixel(image.width - inset, image.height - inset),
    ];
    let mut best = samples[0];
    let mut best_count = 0;
    for s in samples {
        let c = samples.iter().filter(|o| **o == s).count();
        if c > best_count {
            best = s;
            best_count = c;
        }
    }
    Some(best)
}

impl<R: Renderer + ?Sized> FreeSpaceScanner for RasterScanner<'_, R> {
    fn lowest_content(&mut self, page_index: usize, footer: f32) -> Result<f32> {
        let img = self.renderer.render_forced_color(page_index, &self.options)?;
        let s = self.options.dpi / 72.0;
        let footer_px = (footer * s) as i64;
        Ok(lowest_content_row_with_tolerance(&img, footer_px, self.options.background, SCAN_TOLERANCE) as f32 / s)
    }

    fn region_occupied(&mut self, page_index: usize, area: Rect) -> Result<Option<bool>> {
        let page = self.renderer.page_size(page_index)?;
        let img = self.renderer.render_forced_color(page_index, &self.options)?;
        let s = self.options.dpi / 72.0;
        let a = area.normalized();
        let to_px = |v: f32, max: u32| (v * s).floor().clamp(0.0, max as f32) as u32;
        let x0 = to_px(a.llx, img.width);
        let x1 = to_px(a.urx, img.width);
        let y0 = to_px(page.height - a.ury, img.height);
        let y1 = to_px(page.height - a.lly, img.height);
        for y in y0..y1 {
            let row = &img.row(y)[x0 as usize * 4..x1 as usize * 4];
            if row.as_chunks::<4>().0.iter().any(|p| differs(p, &self.options.background, SCAN_TOLERANCE)) {
                return Ok(Some(true));
            }
        }
        Ok(Some(false))
    }
}

/// Geometrie einer neu angehängten Seite: gleiche MediaBox und `/Rotate` wie `last`, ohne
/// CropBox (= MediaBox), wie `new PDPage(lastPage.getMediaBox())` in PDF-AS.
pub fn new_page_geometry(last: &PageInfo) -> PageInfo {
    PageInfo {
        index: last.index + 1,
        object_id: (0, 0),
        media_box: last.media_box,
        crop_box: last.media_box,
        rotation: last.rotation,
        user_unit: last.user_unit,
        annots_indirect: None,
    }
}

/// Seitenwahl vor der Positionierung (gemeinsam für Schritt 1 und 2).
struct PagePlan {
    /// 1-basiert wie PDF-AS (`page`).
    page: usize,
    make_new_page: bool,
    notices: Vec<PlacementNotice>,
}

fn check_new_page_allowed(
    make_new_page: bool,
    ctx: &PositionContext<'_>,
    notices: &mut Vec<PlacementNotice>,
) -> Result<bool> {
    if make_new_page && ctx.already_signed {
        if ctx.signed_newpage_force {
            return Ok(true);
        }
        if ctx.less_space_failing {
            return Err(PlacementError::NotEnoughSpace);
        }
        if !notices.contains(&PlacementNotice::NewPageNotAllowed) {
            notices.push(PlacementNotice::NewPageNotAllowed);
        }
        return Ok(false);
    }
    Ok(make_new_page)
}

impl PagePlan {
    fn new(pos: &TablePos, ctx: &PositionContext<'_>) -> Result<Self> {
        let n = ctx.pages.len();
        if n == 0 {
            return Err(PlacementError::NoSuchPage(0));
        }
        let mut page = n;
        let mut make_new_page = pos.page_mode() == PageSelector::New;
        if let PageSelector::Exact(p) = pos.page_mode() {
            if p == 0 {
                return Err(PlacementError::InvalidPosition("p:0".into()));
            }
            page = p as usize;
            if page > n {
                make_new_page = true;
                page = n;
            }
        }
        let mut notices = Vec::new();
        let make_new_page = check_new_page_allowed(make_new_page, ctx, &mut notices)?;
        Ok(Self { page, make_new_page, notices })
    }
}

/// Geometrie für Breite/Höhe: die Seite selbst; im korrigierten Modus bei einer neuen Seite deren
/// Geometrie (MediaBox der letzten Seite).
fn geometry(ctx: &PositionContext<'_>, page_1based: usize, new_page: bool) -> PageInfo {
    let page = &ctx.pages[page_1based - 1];
    if new_page && ctx.compat == CompatMode::Corrected { new_page_geometry(page) } else { page.clone() }
}

/// Schritt 1 von `adjustSignatureTableandCalculatePosition`: Seite, x und Breite.
///
/// `x:auto` ⇒ 50 pt; Breite ohne `w` = `pw − 2x`; mit `w` und `x:auto` zentriert. Korrektur
/// (Register KR-F01, nur [`CompatMode::Corrected`]): ohne `w` und mit manuellem `x` gilt die Breite
/// der automatischen Position (`pw − 100`), damit Vorschau = Ergebnis; automatische Breiten sind
/// mindestens 1 pt.
pub fn resolve_width(pos: &TablePos, ctx: &PositionContext<'_>) -> Result<WidthDecision> {
    let plan = PagePlan::new(pos, ctx)?;
    let geom = geometry(ctx, plan.page, plan.make_new_page);
    let (pw, _) = geom.visible_size();
    let x_value = pos.x.and_then(Coord::value);
    let mut x = x_value.unwrap_or(MARGIN_H);
    let mut width = pw - 2.0 * x;
    match pos.w.and_then(Coord::value) {
        Some(w) => {
            width = w;
            if x_value.is_none() {
                x = (pw - w) / 2.0;
            }
        }
        None => {
            if ctx.compat == CompatMode::Corrected {
                if x_value.is_some() {
                    width = pw - 2.0 * MARGIN_H;
                }
                width = width.max(1.0);
            }
        }
    }
    Ok(WidthDecision { page_index: plan.page - 1, x, width })
}

/// Schritt 2: endgültige Position mit der Blockhöhe aus dem Layout (Algorithmus docs/analyse/04 §4.3).
///
/// Wie PDF-AS: `p:last` ⇒ letzte Seite, `y:auto` ⇒ `ph − 20`; festes `y` oder Blockhöhe 0 ⇒
/// keine Suche; neue Seite ⇒ `ph − 20`; sonst Rasterscan der Zielseite (`y = ph − lowest − 20`)
/// und bei `y − f ≤ h` neue Seite (nur `p:auto`, nur wenn erlaubt) bzw. `ph − 20` auf der
/// letzten Seite. Ein Fehler der Freiraumsuche wird wie in PDF-AS als leere Seite behandelt
/// (zusätzlich [`PlacementNotice::ScanFailed`]). Stammt die Position nicht aus der Freiraumsuche,
/// meldet der Scanner Überdeckungen als [`PlacementNotice::OverlapsContent`].
pub fn resolve_position(
    pos: &TablePos,
    ctx: &PositionContext<'_>,
    width: WidthDecision,
    block_height: f32,
    scanner: &mut dyn FreeSpaceScanner,
) -> Result<ResolvedPosition> {
    let n = ctx.pages.len();
    let plan = PagePlan::new(pos, ctx)?;
    let mut notices = plan.notices;
    let mut page = plan.page;
    let mut make_new_page = plan.make_new_page;
    let (_, ph) = geometry(ctx, page, make_new_page).visible_size();
    let x = width.x;
    let h = block_height;
    let mode = pos.page_mode();
    let y_value = pos.y.and_then(Coord::value);
    let footer = pos.footer_line();
    let finish = |page: usize, new_page: bool, y: f32, notices: Vec<PlacementNotice>| ResolvedPosition {
        page_index: page - 1,
        new_page,
        x,
        y,
        width: width.width,
        height: h,
        rotation: pos.block_rotation(),
        notices,
    };

    let mut scanned = false;
    let y = if mode == PageSelector::Last {
        y_value.unwrap_or(ph - MARGIN_V)
    } else if y_value.is_some() || h == 0.0 {
        if make_new_page {
            page += 1;
        }
        y_value.unwrap_or(0.0)
    } else if make_new_page {
        page += 1;
        ph - MARGIN_V
    } else {
        let quirk = match ctx.compat {
            CompatMode::PdfasQuirks => pdfas_rotated_scan(&ctx.pages[page - 1], footer),
            CompatMode::Corrected => None,
        };
        let lowest = match quirk.map_or_else(|| scanner.lowest_content(page - 1, footer), Ok) {
            Ok(v) => v,
            Err(_) => {
                notices.push(PlacementNotice::ScanFailed);
                0.0
            }
        };
        let mut y = ph - lowest - MARGIN_V;
        scanned = true;
        if y - footer <= h {
            let is_auto = mode == PageSelector::Auto;
            make_new_page = check_new_page_allowed(is_auto, ctx, &mut notices)?;
            if make_new_page {
                page += 1;
            }
            if !is_auto {
                page = n;
            }
            // PDF-AS nimmt die Höhe der ursprünglich geprüften Seite; korrigiert die der Zielseite.
            let ph = match ctx.compat {
                CompatMode::PdfasQuirks => ph,
                CompatMode::Corrected => geometry(ctx, page.min(n), make_new_page).visible_size().1,
            };
            y = ph - MARGIN_V;
            scanned = false;
        }
        y
    };

    let mut result = finish(page, make_new_page, y, notices);
    if !result.new_page && !scanned && result.height > 0.0 && result.width > 0.0 {
        let area = visible_block_rect(&result);
        if let Ok(Some(true)) = scanner.region_occupied(result.page_index, area) {
            result.notices.push(PlacementNotice::OverlapsContent);
        }
    }
    Ok(result)
}

/// Ergebnis des PDF-AS-Rasterscans auf Seiten mit `/Rotate 90/270`, soweit es nicht vom
/// Seiteninhalt abhängt (Register KR-F04, nur [`CompatMode::PdfasQuirks`]; Golden PDF-AS 4.4.2).
///
/// `PDFUtilities.getMaxYPosition` legt das Rasterbild mit den **ungedrehten** CropBox-Maßen an
/// (`(int) w × (int) h`), `PDFRenderer.renderPageToGraphics` zeichnet die Seite aber gedreht und
/// löscht mit `clearRect(0, 0, (int) w, (int) h)` im gedrehten Raum nur die Zeilen der gedrehten
/// Seite; die übrigen Zeilen bleiben transparent (≠ Hintergrund) und zählen als Inhalt. Bei 90°
/// beginnt das gelöschte Rechteck zudem bei `x = h − (int) h`: liegt dieser Bruchteil bei
/// mindestens einem halben Pixel, bleibt Spalte 0 transparent und **jede** Zeile gilt als belegt.
/// Liegt die Startzeile `(int) h − 1 − (int) f` in einer transparenten Zeile, ist sie das Ergebnis;
/// sonst `None` (normaler Scan, PDF-AS sähe dort nur die linken `(int) w` Spalten).
fn pdfas_rotated_scan(page: &PageInfo, footer: f32) -> Option<f32> {
    let crop = page.crop_box.normalized();
    let (w, h) = (crop.width(), crop.height());
    let start = (h as i64) - 1 - (footer as i64);
    let first_transparent_row = match page.rotation {
        Rotation::R90 => {
            if h - h.trunc() >= 0.5 {
                // Spalte 0 transparent: die Startzeile selbst ist belegt (Zeile 0 bzw. ein
                // negativer Start beenden die Suche ebenfalls mit dem Startwert)
                return Some(start as f32);
            }
            // gelöscht: y ∈ [0, (int) w] ⇒ Zeilen 0 … (int) w − 1
            w as i64
        }
        // gelöscht: y ∈ [w − (int) w, w] ⇒ Zeilen mit Mittelpunkt ≤ w
        Rotation::R270 => (w - 0.5).floor() as i64 + 1,
        _ => return None,
    };
    (start >= first_transparent_row || start < 0).then_some(start as f32)
}

/// Bequemlichkeit: [`resolve_position`] mit [`RasterScanner`].
pub fn auto_place(
    renderer: &dyn Renderer,
    pos: &TablePos,
    ctx: &PositionContext<'_>,
    width: WidthDecision,
    block_height: f32,
) -> Result<ResolvedPosition> {
    let mut scanner = RasterScanner { renderer, options: ForcedColorOptions::default() };
    resolve_position(pos, ctx, width, block_height, &mut scanner)
}

/// Normalisierte Blockrotation (0/90/180/270).
fn quarter_turns(rotation: i32) -> i32 {
    match rotation.rem_euclid(360) {
        r @ (90 | 180 | 270) => r,
        _ => 0,
    }
}

/// Ecken des (um die linke untere Ecke gegen den Uhrzeigersinn gedrehten) Blocks in sichtbaren
/// Seitenkoordinaten (PAGE_ABS).
fn visible_corners(pos: &ResolvedPosition) -> [(f32, f32); 4] {
    let (llx, lly) = (pos.x, pos.y - pos.height);
    let (w, h) = (pos.width, pos.height);
    let rot = |dx: f32, dy: f32| match quarter_turns(pos.rotation) {
        90 => (-dy, dx),
        180 => (-dx, -dy),
        270 => (dy, -dx),
        _ => (dx, dy),
    };
    [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)].map(|(dx, dy)| {
        let (rx, ry) = rot(dx, dy);
        (llx + rx, lly + ry)
    })
}

fn bounding(points: impl IntoIterator<Item = (f32, f32)>) -> Rect {
    let mut r = Rect::new(f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, y) in points {
        r.llx = r.llx.min(x);
        r.lly = r.lly.min(y);
        r.urx = r.urx.max(x);
        r.ury = r.ury.max(y);
    }
    r
}

/// Hülle des Blocks in sichtbaren Seitenkoordinaten (PAGE_ABS), inklusive Blockrotation.
pub fn visible_block_rect(pos: &ResolvedPosition) -> Rect {
    bounding(visible_corners(pos))
}

/// Abbildung sichtbare Seitenkoordinaten (PAGE_ABS) → Benutzerraum für eine Seite.
pub(crate) struct PageMapping {
    rotation: Rotation,
    /// unrotierte Breite/Höhe der Bezugsbox
    w0: f32,
    h0: f32,
    /// Ursprung der Bezugsbox
    ox: f32,
    oy: f32,
}

impl PageMapping {
    /// Korrigiert: CropBox (Ursprung und Maße). PDF-AS-Original: MediaBox-Maße, Ursprung (0, 0).
    pub(crate) fn new(page: &PageInfo, compat: CompatMode) -> Self {
        match compat {
            CompatMode::Corrected => {
                let b = page.crop_box.normalized();
                Self { rotation: page.rotation, w0: b.width(), h0: b.height(), ox: b.llx, oy: b.lly }
            }
            CompatMode::PdfasQuirks => {
                let b = page.media_box.normalized();
                Self { rotation: page.rotation, w0: b.width(), h0: b.height(), ox: 0.0, oy: 0.0 }
            }
        }
    }

    /// Sichtbar (u, v) → Benutzerraum (X, Y).
    pub(crate) fn to_user(&self, (u, v): (f32, f32)) -> (f32, f32) {
        let (ox, oy, w0, h0) = (self.ox, self.oy, self.w0, self.h0);
        match self.rotation {
            Rotation::R0 => (ox + u, oy + v),
            Rotation::R90 => (ox + w0 - v, oy + u),
            Rotation::R180 => (ox + w0 - u, oy + h0 - v),
            Rotation::R270 => (ox + v, oy + h0 - u),
        }
    }

    /// Matrix Benutzerraum → sichtbar (Zeilenvektor-Konvention, Umkehrung von [`Self::to_user`]).
    pub(crate) fn user_to_visible_matrix(&self) -> [f32; 6] {
        let (ox, oy, w0, h0) = (self.ox, self.oy, self.w0, self.h0);
        match self.rotation {
            Rotation::R0 => [1.0, 0.0, 0.0, 1.0, -ox, -oy],
            Rotation::R90 => [0.0, -1.0, 1.0, 0.0, -oy, ox + w0],
            Rotation::R180 => [-1.0, 0.0, 0.0, -1.0, ox + w0, oy + h0],
            Rotation::R270 => [0.0, 1.0, -1.0, 0.0, oy + h0, -ox],
        }
    }
}

/// Widget-`/Rect` aus der Position (unrotiert `[x, y−H, x+W, y]`; Rotation 90/180/270 nach
/// docs/analyse/04 §3.7; CropBox-Ursprung berücksichtigt – Korrektur, Register).
///
/// `page` ist die Zielseite; bei `pos.new_page` die **letzte vorhandene** Seite (die neue Seite
/// bekommt deren MediaBox und `/Rotate`). Das Ergebnis ist normalisiert (PDF-AS schreibt bei
/// gedrehten Seiten ein nicht normalisiertes Rect mit denselben Kanten).
pub fn widget_rect(pos: &ResolvedPosition, page: &PageInfo) -> Rect {
    widget_rect_with(pos, page, CompatMode::Corrected)
}

/// [`widget_rect`] mit wählbarem Kompatibilitätsmodus; [`CompatMode::PdfasQuirks`] rechnet wie
/// `PDFAsVisualSignatureBuilder.createSignatureRectangle` mit den MediaBox-Maßen und ohne
/// CropBox-Ursprung.
pub fn widget_rect_with(pos: &ResolvedPosition, page: &PageInfo, compat: CompatMode) -> Rect {
    let geom = if pos.new_page { new_page_geometry(page) } else { page.clone() };
    let mapping = PageMapping::new(&geom, compat);
    bounding(visible_corners(pos).map(|c| mapping.to_user(c)))
}

/// Rotation für die Holder-`/Matrix`: Blockrotation + Seitenrotation.
pub fn appearance_rotation(block_rotation: i32, page: &PageInfo) -> i32 {
    (block_rotation + i32::from(page.rotation.degrees())).rem_euclid(360)
}

/// Überdeckt der Block (Widget-Rect im Benutzerraum) ein Widget eines vorhandenen Felds auf
/// derselben Seite? Grundlage für [`PlacementNotice::OverlapsWidget`]; Felder ohne Seite oder mit
/// leerem Rect zählen nicht.
pub fn widget_overlaps(pos: &ResolvedPosition, page: &PageInfo, fields: &[SigField]) -> bool {
    if pos.new_page {
        return false;
    }
    let r = widget_rect(pos, page);
    fields.iter().flat_map(|f| f.widgets.iter()).any(|w| {
        let o = w.rect.normalized();
        w.page_index == Some(pos.page_index)
            && !o.is_empty()
            && r.llx < o.urx
            && o.llx < r.urx
            && r.lly < o.ury
            && o.lly < r.ury
    })
}

/// Startposition der UI: x = 50 % Breite, y = 25 % Höhe, geklemmt (F-33).
pub fn default_start_position(page: &PageInfo, block: BlockGeometry) -> Point {
    let (w, h) = page.visible_size();
    clamp_top_left(Point { x: 0.5 * w, y: 0.25 * h }, w, h, block.width, block.height)
}

/// Startzustand der Positionierung ohne gespeicherte Position: **letzte Seite**
/// (`PositioningComposite`: Startseite = letzte Seite) mit [`default_start_position`];
/// `None` ohne Seiten.
pub fn default_start_placement(pages: &[PageInfo], block: BlockGeometry) -> Option<Placement> {
    let page = pages.last()?;
    let p = default_start_position(page, block);
    Some(Placement { page_index: pages.len() - 1, x: p.x, y: p.y, width: block.width, height: block.height })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(w: f32, h: f32, rotation: Rotation) -> PageInfo {
        let r = Rect::new(0.0, 0.0, w, h);
        PageInfo {
            index: 0,
            object_id: (1, 0),
            media_box: r,
            crop_box: r,
            rotation,
            user_unit: 1.0,
            annots_indirect: None,
        }
    }

    /// KR-F04 (PdfasQuirks): Ergebnis des PDF-AS-Rasters auf gedrehten Seiten (Golden PDF-AS 4.4.2:
    /// A4 595,32 × 841,92 mit /Rotate 90/270 und f:80 ⇒ immer neue Seite).
    #[test]
    fn pdfas_rotated_scan_like_pdfas() {
        // 90°: Bruchteil 0,92 ≥ 0,5 ⇒ Spalte 0 transparent ⇒ Startzeile 841 − 1 − f
        assert_eq!(pdfas_rotated_scan(&page(595.32, 841.92, Rotation::R90), 80.0), Some(760.0));
        assert_eq!(pdfas_rotated_scan(&page(595.32, 841.92, Rotation::R90), 300.0), Some(540.0));
        // 90° ganzzahlig: Zeilen 0 … 594 gelöscht; Start 761 transparent, Start 541 ⇒ normaler Scan
        assert_eq!(pdfas_rotated_scan(&page(595.0, 842.0, Rotation::R90), 80.0), Some(761.0));
        assert_eq!(pdfas_rotated_scan(&page(595.0, 842.0, Rotation::R90), 300.0), None);
        // 270°: Zeilen 0 … 594 gelöscht (Mittelpunkt 594,5 ≤ 595,32), Zeile 595 transparent
        assert_eq!(pdfas_rotated_scan(&page(595.32, 841.92, Rotation::R270), 80.0), Some(760.0));
        assert_eq!(pdfas_rotated_scan(&page(595.32, 841.92, Rotation::R270), 245.0), Some(595.0));
        assert_eq!(pdfas_rotated_scan(&page(595.32, 841.92, Rotation::R270), 246.0), None);
        // ungedreht und 180°: kein Sonderfall
        assert_eq!(pdfas_rotated_scan(&page(595.32, 841.92, Rotation::R0), 80.0), None);
        assert_eq!(pdfas_rotated_scan(&page(595.32, 841.92, Rotation::R180), 80.0), None);
        // Fußzeile höher als das Bild: negativer Start wie PDF-AS
        assert_eq!(pdfas_rotated_scan(&page(100.0, 100.0, Rotation::R270), 200.0), Some(-101.0));
    }
}

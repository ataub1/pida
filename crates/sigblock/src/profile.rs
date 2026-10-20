//! Profil-IR und Profil-Store (docs/analyse/04 §2.1–§2.4, §2.8–§2.9).
//!
//! Nachgebildet sind `Settings` (Laden, `include*` mit Wildcards, schlüsselweise Vererbung über
//! `parent`/`augments`), `SignatureProfileSettings` (Captions, Werte, `default.*`-Fallbacks,
//! eingebaute Defaults aus `DefaultSignatureProfileSettings`), `TableFactory` (Zeilen `KEY-TYP`) und
//! `Style` (Attribute, `doInherit`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use pida_java_properties::Properties;

use crate::emblem::Emblem;
use crate::fonts::FontSpec;
use crate::{Result, SigBlockError, assets, java};

/// Horizontale Ausrichtung (`halign`, `valuehalign`, `imagehalign`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HAlign {
    /// links (bei Bildern Quirk Q4: wirkt wie `center`)
    Left,
    /// zentriert
    Center,
    /// rechts
    Right,
    /// zeilenweise zentriert (nur Werte)
    LineCenter,
}

/// Vertikale Ausrichtung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum VAlign {
    /// oben
    Top,
    /// Mitte
    Middle,
    /// unten
    Bottom,
}

/// `table.<T>.Style.*` (docs/analyse/04 §2.4). `None` = nicht gesetzt; Vererbung über
/// [`Style::inherit`] mit dem Sonderfall „padding/border = 1 gelten als nicht gesetzt“ (Q8).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Style {
    /// `bgcolor` (`R G B`); `None` = transparent.
    pub bgcolor: Option<[u8; 3]>,
    /// `padding` (Default 1).
    pub padding: Option<f32>,
    /// `border` (Default 1; 0 = keine Linien).
    pub border: Option<f32>,
    /// `halign` (Captions)
    pub halign: Option<HAlign>,
    /// `valign` (Captions)
    pub valign: Option<VAlign>,
    /// `valuehalign`
    pub valuehalign: Option<HAlign>,
    /// `valuevalign` (nicht gesetzt ⇒ oben, Q3)
    pub valuevalign: Option<VAlign>,
    /// `imagehalign`
    pub imagehalign: Option<HAlign>,
    /// `imagevalign`
    pub imagevalign: Option<VAlign>,
    /// `font` (Captions)
    pub font: Option<FontSpec>,
    /// `valuefont`
    pub valuefont: Option<FontSpec>,
    /// `imagescaletofit` (`W;H`)
    pub imagescaletofit: Option<(f32, f32)>,
}

/// Default von `padding` und `border` in `Style`.
const STYLE_DEFAULT_ONE: f32 = 1.0;

fn parse_halign(v: &str, line_center: bool) -> Option<HAlign> {
    match v {
        "left" => Some(HAlign::Left),
        "center" => Some(HAlign::Center),
        "right" => Some(HAlign::Right),
        "lineCenter" if line_center => Some(HAlign::LineCenter),
        _ => None,
    }
}

fn parse_valign(v: &str) -> Option<VAlign> {
    match v {
        "top" => Some(VAlign::Top),
        "middle" => Some(VAlign::Middle),
        "bottom" => Some(VAlign::Bottom),
        _ => None,
    }
}

impl Style {
    /// `Style.doInherit(base, inherit)`: Kopie von `self`, Lücken aus `inherit`.
    /// `padding`/`border` mit dem Wert 1 gelten als nicht gesetzt (Q8).
    pub fn inherit(&self, inherit: &Style) -> Style {
        let unset_one = |v: Option<f32>| v.is_none() || v == Some(STYLE_DEFAULT_ONE);
        Style {
            bgcolor: self.bgcolor.or(inherit.bgcolor),
            padding: if unset_one(self.padding) { inherit.padding.or(self.padding) } else { self.padding },
            border: if unset_one(self.border) { inherit.border.or(self.border) } else { self.border },
            halign: self.halign.or(inherit.halign),
            valign: self.valign.or(inherit.valign),
            valuehalign: self.valuehalign.or(inherit.valuehalign),
            valuevalign: self.valuevalign.or(inherit.valuevalign),
            imagehalign: self.imagehalign.or(inherit.imagehalign),
            imagevalign: self.imagevalign.or(inherit.imagevalign),
            font: self.font.clone().or_else(|| inherit.font.clone()),
            valuefont: self.valuefont.clone().or_else(|| inherit.valuefont.clone()),
            imagescaletofit: self.imagescaletofit.or(inherit.imagescaletofit),
        }
    }

    /// Parst `table.<T>.Style.<attr>`-Einträge (`(attr, wert)`) wie `Style.setStyle`: ungültige
    /// Ausrichtungen und Farben außerhalb 0–255 werden ignoriert, nicht lesbare Zahlen und
    /// unbekannte Fonts sind Fehler (PDF-AS bricht dort ab).
    pub fn from_entries<'a>(entries: impl Iterator<Item = (&'a str, &'a str)>) -> Result<Style> {
        let mut s = Style::default();
        let num = |attr: &str, v: &str| {
            java::parse_float(v).ok_or_else(|| SigBlockError::Profile(format!("Style.{attr}: keine Zahl: {v:?}")))
        };
        for (attr, value) in entries {
            match attr {
                "bgcolor" => {
                    let parts = java::split(value, ' ');
                    if parts.len() == 3 {
                        let mut rgb = [0i32; 3];
                        for (i, p) in parts.iter().enumerate() {
                            rgb[i] = java::parse_int(p).ok_or_else(|| {
                                SigBlockError::Profile(format!("Style.bgcolor: keine Ganzzahl: {value:?}"))
                            })?;
                        }
                        if rgb.iter().all(|c| (0..256).contains(c)) {
                            s.bgcolor = Some([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8]);
                        }
                    }
                }
                "halign" => {
                    if let Some(a) = parse_halign(value, false) {
                        s.halign = Some(a);
                    }
                }
                "valign" => {
                    if let Some(a) = parse_valign(value) {
                        s.valign = Some(a);
                    }
                }
                "valuehalign" => {
                    if let Some(a) = parse_halign(value, true) {
                        s.valuehalign = Some(a);
                    }
                }
                "valuevalign" => {
                    if let Some(a) = parse_valign(value) {
                        s.valuevalign = Some(a);
                    }
                }
                "imagehalign" => {
                    if let Some(a) = parse_halign(value, false) {
                        s.imagehalign = Some(a);
                    }
                }
                "imagevalign" => {
                    if let Some(a) = parse_valign(value) {
                        s.imagevalign = Some(a);
                    }
                }
                "padding" => s.padding = Some(num(attr, value)?),
                "border" => s.border = Some(num(attr, value)?),
                "font" => s.font = Some(FontSpec::try_parse(value)?),
                "valuefont" => s.valuefont = Some(FontSpec::try_parse(value)?),
                "imagescaletofit" => s.imagescaletofit = parse_scale_to_fit(value)?,
                _ => {}
            }
        }
        Ok(s)
    }

    /// Effektives Padding (Default 1).
    pub fn padding_or_default(&self) -> f32 {
        self.padding.unwrap_or(STYLE_DEFAULT_ONE)
    }

    /// Effektive Rahmenbreite (Default 1).
    pub fn border_or_default(&self) -> f32 {
        self.border.unwrap_or(STYLE_DEFAULT_ONE)
    }
}

/// `Style.parseImageScaleToFit`: leer ⇒ `None`, genau zwei `;`-Teile, sonst `None`.
fn parse_scale_to_fit(v: &str) -> Result<Option<(f32, f32)>> {
    if java::trim(v).is_empty() {
        return Ok(None);
    }
    let parts = java::split(v, ';');
    if parts.len() != 2 {
        return Ok(None);
    }
    let f =
        |p: &str| java::parse_float(p).ok_or_else(|| SigBlockError::Profile(format!("Style.imagescaletofit: {v:?}")));
    Ok(Some((f(parts[0])?, f(parts[1])?)))
}

/// Zellspezifikation aus `KEY-TYP` (docs/analyse/04 §2.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CellSpec {
    /// `TABLE-<name>`
    SubTable(String),
    /// `<K>-i`
    Image(String),
    /// `<K>-v`
    Value(String),
    /// `<K>-c`
    Caption(String),
    /// `<K>-cv` / `-vc` / `-req` (zwei Zellen: Caption roh + Wert aufgelöst)
    CaptionValue(String),
}

impl CellSpec {
    /// Profil-Key (`SIG_LABEL`) bzw. Tabellenname.
    pub fn key(&self) -> &str {
        match self {
            CellSpec::SubTable(k)
            | CellSpec::Image(k)
            | CellSpec::Value(k)
            | CellSpec::Caption(k)
            | CellSpec::CaptionValue(k) => k,
        }
    }
}

/// Zeile (`table.<T>.<n>`, `|`-getrennt).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Row(pub Vec<CellSpec>);

impl Row {
    /// Parst `ELEM ( '|' ELEM )*` wie `TableFactory`; `None`, wenn ein Element kein `-` hat
    /// (PDF-AS verwirft dann die ganze Tabelle). Unbekannte Typen ergeben keine Zelle.
    pub fn parse(def: &str) -> Option<Row> {
        let mut cells = Vec::new();
        for elem in java::split(def, '|') {
            let kt = java::split(elem, '-');
            if kt.len() < 2 {
                return None;
            }
            let (key, typ) = (kt[0], kt[1]);
            if key == "TABLE" {
                cells.push(CellSpec::SubTable(typ.to_string()));
                continue;
            }
            match typ {
                "i" => cells.push(CellSpec::Image(key.to_string())),
                "v" => cells.push(CellSpec::Value(key.to_string())),
                "c" => cells.push(CellSpec::Caption(key.to_string())),
                "vc" | "cv" | "req" => cells.push(CellSpec::CaptionValue(key.to_string())),
                _ => {}
            }
        }
        Some(Row(cells))
    }
}

/// Tabelle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Table {
    /// Name (`main`, `info` …).
    pub name: String,
    /// Zeilen 1..n (lückenlos).
    pub rows: Vec<Row>,
    /// `ColsWidth` (relative Gewichte).
    pub cols_width: Option<Vec<f32>>,
    /// Stil.
    pub style: Option<Style>,
}

/// Schalter, die PDF-AS als Caption ablegt (`key.SIG_PDFA1B_VALID=true` …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ProfileFlags {
    /// `SIG_PDFA1B_VALID`
    pub pdfa1b_valid: bool,
    /// `SIG_PDFA_VALID`
    pub pdfa_valid: bool,
    /// `SIG_PDFUA_FORCE`
    pub pdfua_force: bool,
    /// `latin1_encoding`
    pub latin1_encoding: bool,
}

/// Aufgelöstes Profil (nach Vererbung).
///
/// [`ProfileStore`] legt hier bereits **alle Fallbacks** ab (ADR 0001 §8): globale `default.*`-Werte
/// (`getValue` → `default.<key>`, z. B. `default.timezone`) und die eingebauten Caption-Defaults von PDF-AS
/// (`DefaultSignatureProfileSettings`). [`crate::layout::layout`] und [`crate::Appearance::build`]
/// brauchen deshalb nur das Profil, nicht den Store.
///
/// `Debug` zeigt von Captions, Werten und Rohschlüsseln nur die Schlüssel: Nach
/// [`Profile::with_overrides`] steht dort der Hinweistext der Nutzerin (ADR 0001 Regel 9).
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    /// Profil-ID.
    pub id: String,
    /// `isvisible` (und `table.main` existiert).
    pub visible: bool,
    /// `pos` (Positionsstring, Parsing in `pida-placement`).
    pub pos: Option<String>,
    /// Tabellen nach Name.
    pub tables: BTreeMap<String, Table>,
    /// `key.<K>` (Captions).
    pub captions: BTreeMap<String, String>,
    /// `value.<K>` (Werte, Ausdrücke, Datumsmuster, Bildpfade).
    pub values: BTreeMap<String, String>,
    /// Schalter.
    pub flags: ProfileFlags,
    /// `adobeSignReasonValue` (`/Reason`; Fallback „PAdES Signature“).
    pub reason: Option<String>,
    /// `timezone` für `SIG_DATE`.
    pub timezone: Option<String>,
    /// `signatureSize`.
    pub signature_size: Option<usize>,
    /// `SIGNED_NEWPAGE_FORCE`.
    pub signed_newpage_force: bool,
    /// `minWidth`.
    pub min_width: Option<f32>,
    /// `enable_placeholder_search`.
    pub placeholder_search: Option<bool>,
    /// `placeholder_id`.
    pub placeholder_id: Option<String>,
    /// Alle Rohschlüssel unter `sig_obj.<P>.` (ohne Präfix) – für Diagnose und Konverter.
    pub raw: BTreeMap<String, String>,
    /// Laufzeit-Bildmarke für `SIG_LABEL` aus [`ProfileOverrides::sig_label`] (nicht serialisiert;
    /// [`crate::SigBlockInput::emblem`] hat Vorrang).
    #[serde(skip)]
    pub label_emblem: Option<Arc<Emblem>>,
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profile")
            .field("id", &self.id)
            .field("visible", &self.visible)
            .field("pos", &self.pos)
            .field("tables", &self.tables)
            .field("captions", &self.captions.keys().collect::<Vec<_>>())
            .field("values", &self.values.keys().collect::<Vec<_>>())
            .field("flags", &self.flags)
            .field("reason", &self.reason)
            .field("timezone", &self.timezone)
            .field("signature_size", &self.signature_size)
            .field("signed_newpage_force", &self.signed_newpage_force)
            .field("min_width", &self.min_width)
            .field("placeholder_search", &self.placeholder_search)
            .field("placeholder_id", &self.placeholder_id)
            .field("raw_keys", &self.raw.len())
            .field("label_emblem", &self.label_emblem)
            .finish()
    }
}

impl Profile {
    /// `table.main`, falls vorhanden.
    pub fn main_table(&self) -> Option<&Table> {
        self.tables.get("main")
    }

    /// Wendet Laufzeit-Overlays an (nur auf genau diese Profil-ID, keine Weitervererbung).
    pub fn with_overrides(&self, overrides: &ProfileOverrides) -> Profile {
        let mut p = self.clone();
        if let Some(emblem) = &overrides.sig_label {
            p.label_emblem = Some(emblem.clone());
        }
        if let Some(note) = &overrides.sig_note {
            p.values.insert("SIG_NOTE".into(), note.clone());
            p.raw.insert("value.SIG_NOTE".into(), note.clone());
        }
        if let Some(pos) = &overrides.pos {
            p.pos = Some(pos.clone());
            p.raw.insert("pos".into(), pos.clone());
        }
        if (overrides.main_padding.is_some() || overrides.imagescaletofit.is_some())
            && let Some(main) = p.tables.get_mut("main")
        {
            let style = main.style.get_or_insert_with(Style::default);
            if let Some(pad) = overrides.main_padding {
                style.padding = Some(pad);
                p.raw.insert("table.main.Style.padding".into(), java::double_to_string(f64::from(pad)));
            }
            if let Some((w, h)) = overrides.imagescaletofit {
                style.imagescaletofit = Some((w, h));
                p.raw.insert(
                    "table.main.Style.imagescaletofit".into(),
                    format!("{};{}", java::double_to_string(f64::from(w)), java::double_to_string(f64::from(h))),
                );
            }
        }
        p
    }

    /// `/Reason` mit Fallback.
    pub fn reason_or_default(&self) -> &str {
        self.reason.as_deref().unwrap_or("PAdES Signature")
    }

    /// PDF/A-Schalter des Profils (`SIG_PDFA1B_VALID` oder `SIG_PDFA_VALID`).
    pub fn is_pdfa(&self) -> bool {
        self.flags.pdfa1b_valid || self.flags.pdfa_valid
    }
}

/// Laufzeit-Overlays wie PDF-Over (`PdfAs4Signer.prepare`, docs/analyse/04 §2.8). `Debug` zeigt
/// den Hinweistext nicht.
#[derive(Clone, PartialEq, Default)]
pub struct ProfileOverrides {
    /// `value.SIG_LABEL` (Bildmarke; `None` = Profil-Default-Logo).
    pub sig_label: Option<Arc<Emblem>>,
    /// `value.SIG_NOTE` (`None` = Profilwert).
    pub sig_note: Option<String>,
    /// `table.main.Style.padding` (BASE_LOGO: `0`).
    pub main_padding: Option<f32>,
    /// `pos` (BASE_LOGO: `w:<tw>;f:0`).
    pub pos: Option<String>,
    /// `table.main.Style.imagescaletofit` (BASE_LOGO: `<tw>;<th>`).
    pub imagescaletofit: Option<(f32, f32)>,
}

impl std::fmt::Debug for ProfileOverrides {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileOverrides")
            .field("sig_label", &self.sig_label)
            .field("sig_note_len", &self.sig_note.as_ref().map(|n| n.chars().count()))
            .field("main_padding", &self.main_padding)
            .field("pos", &self.pos)
            .field("imagescaletofit", &self.imagescaletofit)
            .finish()
    }
}

/// PDF-Einheiten je mm (`Constants.PDF_UNITS_PER_MM = 595.0 / 210.0`).
pub const PDF_UNITS_PER_MM: f64 = 595.0 / 210.0;
/// Obergrenze der Logo-Größe (PDF-Over „WORKAROUND FOR #117“; hier in Vorschau **und** Signatur).
pub const MAX_LOGO_SIZE_MM: f64 = 120.0;
/// Default der Logo-Größe in mm (`Constants.DEFAULT_LOGO_ONLY_SIZE`).
pub const DEFAULT_LOGO_SIZE_MM: f64 = 23.0;

impl ProfileOverrides {
    /// BASE_LOGO-Overlay aus Emblem-Pixelmaßen und Zielgröße in mm (Default 23):
    /// `tw = th = mm · 595/210`; `ar < 1 ⇒ tw *= ar`, sonst `th /= ar` (Java-`double`).
    /// Die 120-mm-Klemmung gilt in Vorschau **und** Signatur (Plan §6.5 Register).
    pub fn base_logo(emblem: Option<Arc<Emblem>>, emblem_px: (u32, u32), target_mm: f64) -> Self {
        let mm = if target_mm.is_finite() && target_mm > 0.0 {
            target_mm.min(MAX_LOGO_SIZE_MM)
        } else {
            DEFAULT_LOGO_SIZE_MM
        };
        let (ew, eh) = if emblem_px.0 == 0 || emblem_px.1 == 0 { (1, 1) } else { emblem_px };
        let ar = f64::from(ew) / f64::from(eh);
        let mut tw = mm * PDF_UNITS_PER_MM;
        let mut th = mm * PDF_UNITS_PER_MM;
        if ar < 1.0 {
            tw *= ar;
        } else {
            th /= ar;
        }
        let (tws, ths) = (java::double_to_string(tw), java::double_to_string(th));
        // PDF-AS liest `imagescaletofit` mit `Float.parseFloat` aus dem `Double.toString`-Text.
        let stf = (java::parse_float(&tws).unwrap_or(tw as f32), java::parse_float(&ths).unwrap_or(th as f32));
        ProfileOverrides {
            sig_label: emblem,
            sig_note: None,
            main_padding: Some(0.0),
            pos: Some(format!("w:{tws};f:0")),
            imagescaletofit: Some(stf),
        }
    }
}

/// Quelle für Profil-Ressourcen (Bilder `./images/…`, Fonts `TTF:<datei>`).
pub trait Resources: Send + Sync + std::fmt::Debug {
    /// Bild relativ zum Arbeitsordner (`./images/signatur-logo_de.png`) oder Base64-Wert.
    fn image(&self, reference: &str) -> Result<Arc<Emblem>>;
    /// TTF-Datei aus `fonts/`.
    fn font(&self, file: &str) -> Result<Arc<[u8]>>;
}

/// Name eines eingebetteten Bilds aus einer Profilreferenz (`./images/x.png`, `images/x.png`). Ohne Feature
/// `pdfas-reference` fehlt die Demo-Bildmarke der Amtssignatur (Backlog Nr. 10): Verweise darauf (PDF-AS-
/// Konfigurationsordner ohne `images/`, Overlays) bekommen die neutrale Platzhalter-Bildmarke – gleiches
/// Seitenverhältnis, also gleiche Blockmaße – statt eines Fehlers beim Signieren.
fn builtin_image_name(reference: &str) -> Option<&str> {
    let r = reference.trim_start_matches("./");
    let r = r.strip_prefix("images/").unwrap_or(r);
    if assets::image(r).is_some() {
        return Some(r);
    }
    (r == assets::AMTS_DEMO_FILE).then_some(assets::AMTS_PLACEHOLDER_FILE)
}

fn builtin_images() -> &'static HashMap<&'static str, std::result::Result<Arc<Emblem>, String>> {
    static CACHE: OnceLock<HashMap<&'static str, std::result::Result<Arc<Emblem>, String>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        assets::IMAGES
            .iter()
            .map(|&(n, bytes)| (n, Emblem::decode_original(bytes).map(Arc::new).map_err(|e| e.to_string())))
            .collect()
    })
}

/// Base64 wie `org.apache.commons.codec.binary.Base64.decodeBase64` (tolerant: fremde Zeichen
/// werden übersprungen, Standard- und URL-Alphabet, `=` beendet).
fn lenient_base64(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut n = 0u32;
    for b in s.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        n += 1;
        if n == 4 {
            out.extend_from_slice(&[(acc >> 16) as u8, (acc >> 8) as u8, acc as u8]);
            acc = 0;
            n = 0;
        }
    }
    match n {
        2 => out.push((acc >> 4) as u8),
        3 => out.extend_from_slice(&[(acc >> 10) as u8, (acc >> 2) as u8]),
        _ => {}
    }
    out
}

/// Bildreferenz für Fehlermeldungen (Base64-Werte können Megabytes groß sein).
fn reference_for_message(reference: &str) -> String {
    crate::shorten(reference, 80)
}

fn image_from_base64(reference: &str) -> Result<Arc<Emblem>> {
    let bytes = lenient_base64(reference);
    let shown = || reference_for_message(reference);
    if bytes.is_empty() {
        return Err(SigBlockError::MissingResource(format!("Bild {:?}", shown())));
    }
    Emblem::decode_original(&bytes)
        .map(Arc::new)
        .map_err(|_| SigBlockError::MissingResource(format!("Bild {:?} (weder Datei noch Base64-Bild)", shown())))
}

/// Eingebettete Ressourcen (`crates/sigblock/assets/`: Logos, DejaVu-Fonts; EUPL bzw. Bitstream Vera).
#[derive(Debug, Default)]
pub struct BuiltinResources {
    _private: (),
}

impl Resources for BuiltinResources {
    fn image(&self, reference: &str) -> Result<Arc<Emblem>> {
        if let Some(name) = builtin_image_name(reference) {
            return match builtin_images().get(name) {
                Some(Ok(e)) => Ok(e.clone()),
                Some(Err(e)) => Err(SigBlockError::Image(e.clone())),
                None => Err(SigBlockError::MissingResource(format!("Bild {:?}", reference_for_message(reference)))),
            };
        }
        image_from_base64(reference)
    }

    fn font(&self, file: &str) -> Result<Arc<[u8]>> {
        assets::font(file)
            .map(Arc::from)
            .ok_or_else(|| SigBlockError::MissingResource(format!("Font {file:?} (nicht eingebettet)")))
    }
}

/// Ressourcen aus einem PDF-AS-Arbeitsordner (`<workdir>/images`, `<workdir>/fonts`, relative Pfade
/// wie `./images/…`), mit Rückfall auf [`BuiltinResources`].
#[derive(Debug)]
pub struct DirResources {
    /// Arbeitsordner (enthält `cfg/`, `fonts/`, `images/`)
    pub workdir: PathBuf,
}

impl Resources for DirResources {
    fn image(&self, reference: &str) -> Result<Arc<Emblem>> {
        let p = Path::new(reference);
        let path = if p.is_absolute() { p.to_path_buf() } else { self.workdir.join(reference) };
        if path.is_file() {
            let bytes = std::fs::read(&path)?;
            return Emblem::decode_original(&bytes).map(Arc::new);
        }
        if builtin_image_name(reference).is_some() {
            return BuiltinResources::default().image(reference);
        }
        image_from_base64(reference)
    }

    fn font(&self, file: &str) -> Result<Arc<[u8]>> {
        let path = self.workdir.join("fonts").join(file);
        if path.is_file() {
            return Ok(Arc::from(std::fs::read(&path)?));
        }
        BuiltinResources::default().font(file)
    }
}

// ------------------------------------------------------------------------------------------------
// Laden: Dateien, Includes
// ------------------------------------------------------------------------------------------------

/// Obergrenze der Include-Tiefe (PDF-AS hat keine; dort endet eine Schleife im StackOverflow).
const MAX_INCLUDE_DEPTH: usize = 16;
/// Obergrenze geladener Dateien insgesamt (mehrere Dateien mit `include = *.properties` würden
/// sonst exponentiell oft geladen).
const MAX_INCLUDE_FILES: usize = 512;

/// Zustand beim rekursiven Laden.
#[derive(Default)]
struct LoadState {
    /// Dateien auf dem aktuellen Include-Pfad (Zykluserkennung)
    stack: Vec<String>,
    /// bisher geladene Dateien
    loads: usize,
    problems: Vec<String>,
}

/// Lesezugriff auf einen `cfg/`-Ordner (eingebettet oder Dateisystem).
trait CfgSource {
    fn read(&self, rel: &str) -> std::result::Result<Vec<u8>, String>;
    /// Dateinamen (ohne Pfad) im Unterordner `dir` (`""` = `cfg/`).
    fn list(&self, dir: &str) -> Vec<String>;
}

struct BuiltinCfg;

impl CfgSource for BuiltinCfg {
    fn read(&self, rel: &str) -> std::result::Result<Vec<u8>, String> {
        assets::cfg(rel).map(<[u8]>::to_vec).ok_or_else(|| format!("{rel} fehlt"))
    }

    fn list(&self, dir: &str) -> Vec<String> {
        assets::CFG_FILES
            .iter()
            .filter_map(|(p, _)| {
                let (d, f) = p.rsplit_once('/').unwrap_or(("", p));
                (d == dir).then(|| f.to_string())
            })
            .collect()
    }
}

struct DirCfg(PathBuf);

impl CfgSource for DirCfg {
    fn read(&self, rel: &str) -> std::result::Result<Vec<u8>, String> {
        std::fs::read(self.0.join(rel)).map_err(|e| format!("{rel}: {e}"))
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(self.0.join(dir)) else {
            return Vec::new();
        };
        rd.filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter_map(|e| e.file_name().into_string().ok())
            .collect()
    }
}

/// `WildcardFileFilter` (Groß-/Kleinschreibung beachtet): `*` beliebig viele, `?` genau ein Zeichen.
fn wildcard_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Normalisiert `a/./b/../c` (nur relativ, ohne Wurzel).
fn normalize_rel(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Lädt `rel` und rekursiv alle `include*`-Dateien. PDF-AS bricht bei Include-Zyklen mit einem
/// StackOverflow ab; hier wird eine Datei, die schon auf dem aktuellen Include-Pfad liegt, nicht
/// erneut geladen und als Problem gemeldet.
fn load_recursive(src: &dyn CfgSource, rel: &str, merged: &mut Properties, st: &mut LoadState) -> Result<()> {
    if st.stack.len() > MAX_INCLUDE_DEPTH {
        st.problems.push(format!("Include-Tiefe überschritten bei {rel}"));
        return Ok(());
    }
    if st.stack.iter().any(|f| f == rel) {
        st.problems.push(format!("Include-Zyklus bei {rel} (übersprungen)"));
        return Ok(());
    }
    if st.loads >= MAX_INCLUDE_FILES {
        st.problems.push(format!("zu viele Include-Dateien, {rel} übersprungen"));
        return Ok(());
    }
    st.loads += 1;
    let bytes = src.read(rel).map_err(SigBlockError::MissingResource)?;
    let props = Properties::load_latin1(&bytes).map_err(|e| SigBlockError::Profile(format!("{rel}: {e}")))?;
    merged.extend_from(&props);
    st.stack.push(rel.to_string());
    let r = load_includes(src, &props, merged, st);
    st.stack.pop();
    r
}

fn load_includes(src: &dyn CfgSource, props: &Properties, merged: &mut Properties, st: &mut LoadState) -> Result<()> {
    // `getValuesPrefix(INCLUDE, tmpProps)`: Reihenfolge in Java unbestimmt (HashMap) – hier sortiert.
    let mut includes: Vec<(String, String)> =
        props.with_prefix("include").map(|(k, v)| (k.to_string(), v.to_string())).collect();
    includes.sort();
    for (_, pattern) in includes {
        let full = normalize_rel(&pattern);
        let (dir, name) = full.rsplit_once('/').unwrap_or(("", full.as_str()));
        let mut files: Vec<String> = src.list(dir).into_iter().filter(|f| wildcard_match(name, f)).collect();
        files.sort();
        for f in files {
            let child = if dir.is_empty() { f } else { format!("{dir}/{f}") };
            load_recursive(src, &child, merged, st)?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Vererbung (Settings.buildProfiles)
// ------------------------------------------------------------------------------------------------

fn keys_with_prefix(props: &Properties, prefix: &str) -> Vec<String> {
    props.keys().filter(|k| k.starts_with(prefix)).map(str::to_string).collect()
}

fn copy_profile_keys(props: &mut Properties, from: &str, to: &str) {
    let src_base = format!("sig_obj.{from}.");
    let dst_base = format!("sig_obj.{to}.");
    for key in keys_with_prefix(props, &src_base) {
        let rest = &key[src_base.len()..];
        let target = format!("{dst_base}{rest}");
        if !props.contains_key(&target)
            && let Some(v) = props.get(&key).map(str::to_string)
        {
            props.insert(target, v);
        }
    }
}

/// Löst `parent`/`augments` schlüsselweise auf; liefert die aktiven Profile.
fn resolve_inheritance(props: &mut Properties, problems: &mut Vec<String>) -> Vec<String> {
    let mut active: Vec<String> = props
        .with_prefix("sig_obj.types.")
        .filter(|(k, v)| !k["sig_obj.types.".len()..].contains('.') && *v == "on")
        .map(|(k, _)| k["sig_obj.types.".len()..].to_string())
        .collect();
    active.sort();
    active.dedup();
    let is_active: BTreeSet<&str> = active.iter().map(String::as_str).collect();
    let mut parent: HashMap<String, Option<String>> = HashMap::new();
    let mut augments: HashMap<String, Vec<Option<String>>> = HashMap::new();
    for p in &active {
        let par = props.get(&format!("sig_obj.{p}.parent")).filter(|q| is_active.contains(q)).map(str::to_string);
        parent.insert(p.clone(), par);
        let prefix = format!("sig_obj.{p}.augments");
        let mut aug: Vec<(String, Option<String>)> = props
            .with_prefix(&prefix)
            .map(|(k, v)| (k.to_string(), is_active.contains(v).then(|| v.to_string())))
            .collect();
        aug.sort();
        augments.insert(p.clone(), aug.into_iter().map(|(_, v)| v).collect());
    }
    let mut initialized: BTreeSet<String> = BTreeSet::new();
    let mut pending: Vec<String> = active.clone();
    while !pending.is_empty() {
        let mut progressed = false;
        let mut still = Vec::new();
        for p in pending {
            let aug = &augments[&p];
            let par = &parent[&p];
            let aug_ready = aug.iter().all(|a| a.as_ref().is_some_and(|a| initialized.contains(a)));
            let par_ready = par.as_ref().is_none_or(|q| initialized.contains(q));
            if aug_ready && par_ready {
                // Augments vor Parent (Reihenfolge entscheidend)
                for a in aug.iter().flatten() {
                    copy_profile_keys(props, a, &p);
                }
                if let Some(q) = par {
                    copy_profile_keys(props, q, &p);
                }
                initialized.insert(p);
                progressed = true;
            } else {
                still.push(p);
            }
        }
        if !progressed {
            for p in &still {
                problems.push(format!("Profil {p}: Vererbung nicht auflösbar (Zyklus oder fehlendes Augment)"));
            }
            break;
        }
        pending = still;
    }
    active
}

// ------------------------------------------------------------------------------------------------
// Profil aus den Schlüsseln (SignatureProfileSettings + TableFactory)
// ------------------------------------------------------------------------------------------------

/// `DefaultSignatureProfileSettings`: (Key, Caption, Wert).
const BUILTIN_DEFAULTS: &[(&str, &str, &str)] = &[
    ("SIG_SUBJECT", "Unterzeichner", "${subject.T != null ? (subject.T + \" \") : \"\"}${subject.CN}"),
    ("SIG_ISSUER", "Aussteller-Zertifikat", "${issuer.DN}"),
    ("SIG_NUMBER", "Serien-Nr.", "${sn}"),
    ("SIG_KZ", "Methode", ""),
    ("SIG_ID", "Parameter", ""),
    (
        "SIG_META",
        "Pr\u{fc}finformation",
        "Informationen zur Pr\u{fc}fung der elektronischen Signatur finden Sie unter: https://www.signaturpruefung.gv.at\n\nInformationen zur Pr\u{fc}fung des Ausdrucks finden Sie unter: https://www.behoerde.gv.at/el_signatur/",
    ),
    ("SIG_DATE", "Datum/Zeit-UTC", "yyyy-MM-dd'T'HH:mm:ssXXX"),
];

fn builtin_default(key: &str) -> Option<(&'static str, &'static str)> {
    BUILTIN_DEFAULTS.iter().find(|(k, _, _)| *k == key).map(|(_, c, v)| (*c, *v))
}

fn last_segment(key: &str) -> &str {
    // `StringUtils.extractLastID` bzw. `key.substring(key.lastIndexOf('.') + 1)`
    key.rsplit_once('.').map(|(_, l)| l).unwrap_or(key)
}

struct ProfileKeys<'a> {
    props: &'a Properties,
    prefix: String,
}

impl ProfileKeys<'_> {
    fn get(&self, rest: &str) -> Option<&str> {
        self.props.get(&format!("{}{rest}", self.prefix))
    }

    /// `SignatureProfileSettings.getValue` für Keys ohne `key.`/`value.`-Eintrag:
    /// `profileSettings` (letztes Segment) → `default.<key>`.
    fn setting(&self, key: &str) -> Option<String> {
        if let Some(v) = self.get(key) {
            return Some(v.to_string());
        }
        let (kp, vp, tp) =
            (format!("{}key", self.prefix), format!("{}value", self.prefix), format!("{}table.", self.prefix));
        let mut hits: Vec<(&str, &str)> = self
            .props
            .with_prefix(&self.prefix)
            .filter(|(k, _)| !k.starts_with(&kp) && !k.starts_with(&vp) && !k.starts_with(&tp))
            .filter(|(k, _)| last_segment(k) == key)
            .collect();
        hits.sort();
        if let Some((_, v)) = hits.first() {
            return Some(v.to_string());
        }
        self.props.get(&format!("default.{key}")).map(str::to_string)
    }
}

fn parse_tables(keys: &ProfileKeys<'_>, id: &str, problems: &mut Vec<String>) -> BTreeMap<String, Table> {
    let tprefix = format!("{}table.", keys.prefix);
    let mut names: BTreeSet<String> = BTreeSet::new();
    for k in keys.props.keys().filter(|k| k.starts_with(&tprefix)) {
        let rest = &k[tprefix.len()..];
        if let Some((name, _)) = rest.split_once('.') {
            names.insert(name.to_string());
        }
    }
    let mut tables = BTreeMap::new();
    'tables: for name in names {
        let base = format!("{tprefix}{name}.");
        let mut rows: BTreeMap<String, Row> = BTreeMap::new();
        let mut cols_width = None;
        let mut style = None;
        let mut first_level: BTreeSet<String> = BTreeSet::new();
        for k in keys.props.keys().filter(|k| k.starts_with(&base)) {
            let rest = &k[base.len()..];
            first_level.insert(rest.split('.').next().unwrap_or(rest).to_string());
        }
        for def in first_level {
            if !def.bytes().any(|b| b.is_ascii_digit()) {
                if def == "ColsWidth" {
                    let Some(v) = keys.props.get(&format!("{base}ColsWidth")) else { continue };
                    let mut cols = Vec::new();
                    for c in java::split(v, ' ') {
                        match java::parse_float(c) {
                            Some(f) => cols.push(f),
                            None => {
                                problems.push(format!("Profil {id}: table.{name}.ColsWidth ungültig: {v:?}"));
                                continue 'tables;
                            }
                        }
                    }
                    cols_width = Some(cols);
                } else if def == "Style" {
                    let sprefix = format!("{base}Style");
                    let mut entries: Vec<(&str, &str)> =
                        keys.props.with_prefix(&sprefix).map(|(k, v)| (last_segment(k), v)).collect();
                    entries.sort_by(|a, b| a.0.cmp(b.0));
                    match Style::from_entries(entries.into_iter()) {
                        Ok(s) => style = Some(s),
                        Err(e) => {
                            problems.push(format!("Profil {id}: table.{name}.Style: {e}"));
                            continue 'tables;
                        }
                    }
                }
                continue;
            }
            let Some(v) = keys.props.get(&format!("{base}{def}")) else { continue };
            match Row::parse(v) {
                Some(row) => {
                    rows.insert(def, row);
                }
                None => {
                    problems.push(format!(
                        "Profil {id}: table.{name}.{def} ungültig (Element ohne '-'), Tabelle verworfen"
                    ));
                    continue 'tables;
                }
            }
        }
        // `Table.getRows()`: Zeilen "1".."n" müssen lückenlos vorhanden sein.
        let n = rows.len();
        let mut ordered = Vec::with_capacity(n);
        for i in 1..=n {
            match rows.remove(&i.to_string()) {
                Some(r) => ordered.push(r),
                None => {
                    problems.push(format!("Profil {id}: table.{name}: Zeilen nicht lückenlos ab 1, Tabelle verworfen"));
                    continue 'tables;
                }
            }
        }
        tables.insert(name.clone(), Table { name, rows: ordered, cols_width, style });
    }
    tables
}

fn build_profile(props: &Properties, id: &str, problems: &mut Vec<String>) -> Profile {
    let keys = ProfileKeys { props, prefix: format!("sig_obj.{id}.") };
    let raw: BTreeMap<String, String> =
        props.with_prefix(&keys.prefix).map(|(k, v)| (k[keys.prefix.len()..].to_string(), v.to_string())).collect();

    // `key.*` und `value.*`
    let kprefix = format!("{}key", keys.prefix);
    let vprefix = format!("{}value", keys.prefix);
    let mut captions: BTreeMap<String, String> = BTreeMap::new();
    let mut entry_values: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (k, _) in props.with_prefix(&kprefix) {
        let key = last_segment(k).to_string();
        let caption = keys
            .get(&format!("key.{key}"))
            .map(str::to_string)
            .or_else(|| builtin_default(&key).map(|(c, _)| c.to_string()));
        if let Some(c) = caption {
            captions.insert(key.clone(), c);
        }
        let value = keys
            .get(&format!("value.{key}"))
            .map(str::to_string)
            .or_else(|| builtin_default(&key).map(|(_, v)| v.to_string()));
        entry_values.insert(key, value);
    }
    for (k, _) in props.with_prefix(&vprefix) {
        let key = last_segment(k).to_string();
        if entry_values.contains_key(&key) {
            continue;
        }
        let value = keys
            .get(&format!("value.{key}"))
            .map(str::to_string)
            .or_else(|| builtin_default(&key).map(|(_, v)| v.to_string()));
        entry_values.insert(key.clone(), value);
        // Register: eingebaute Caption-Defaults auch für Keys ohne `key.*` (PDF-AS: null ⇒ Absturz).
        if let Some((c, _)) = builtin_default(&key) {
            captions.entry(key).or_insert_with(|| c.to_string());
        }
    }

    let tables = parse_tables(&keys, id, problems);

    // Werte mit allen Fallbacks für jede referenzierte Zelle (+ SIG_SUBJECT für `/Name`).
    let mut needed: BTreeSet<String> = entry_values.keys().cloned().collect();
    for t in tables.values() {
        for r in &t.rows {
            for c in &r.0 {
                if !matches!(c, CellSpec::SubTable(_)) {
                    needed.insert(c.key().to_string());
                }
            }
        }
    }
    needed.insert("SIG_SUBJECT".into());
    let mut values = BTreeMap::new();
    for key in needed {
        let v = match entry_values.get(&key) {
            Some(Some(v)) => Some(v.clone()),
            Some(None) => props.get(&format!("default.{key}")).map(str::to_string),
            None => keys.setting(&key),
        };
        if let Some(v) = v {
            values.insert(key, v);
        }
    }

    let flag = |k: &str| captions.get(k).is_some_and(|c| c == "true");
    let flags = ProfileFlags {
        pdfa1b_valid: flag("SIG_PDFA1B_VALID"),
        pdfa_valid: flag("SIG_PDFA_VALID"),
        pdfua_force: flag("SIG_PDFUA_FORCE"),
        latin1_encoding: flag("latin1_encoding"),
    };
    let has_main = props.keys().any(|k| k.starts_with(&format!("{}table.main", keys.prefix)));
    let visible = has_main && keys.get("isvisible") != Some("false");
    Profile {
        id: id.to_string(),
        visible,
        pos: keys.get("pos").map(str::to_string),
        tables,
        captions,
        values,
        flags,
        reason: keys.setting("adobeSignReasonValue"),
        timezone: keys.setting("timezone"),
        signature_size: keys
            .setting("signatureSize")
            .and_then(|s| java::parse_int(&s))
            .and_then(|n| usize::try_from(n).ok()),
        signed_newpage_force: keys.setting("SIGNED_NEWPAGE_FORCE").is_some_and(|v| v.eq_ignore_ascii_case("true")),
        min_width: keys.get("minWidth").and_then(java::parse_float),
        placeholder_search: keys.get("enable_placeholder_search").map(|v| v.eq_ignore_ascii_case("true")),
        placeholder_id: keys.get("placeholder_id").map(str::to_string),
        raw,
        label_emblem: None,
    }
}

/// Store aller aktiven Profile. `Debug` zeigt nur IDs und Anzahlen.
#[derive(Clone)]
pub struct ProfileStore {
    profiles: BTreeMap<String, Profile>,
    globals: Properties,
    resources: Arc<dyn Resources>,
    problems: Vec<String>,
}

impl std::fmt::Debug for ProfileStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileStore")
            .field("profiles", &self.profiles.keys().collect::<Vec<_>>())
            .field("globals", &self.globals.len())
            .field("resources", &self.resources)
            .field("problems", &self.problems.len())
            .finish()
    }
}

/// PIDA-Vorgaben über der eingebetteten PDF-AS-Konfiguration (ADR 0005): das Siegel als
/// Standard-Bildmarke der Profile [`assets::PIDA_SEAL_PROFILES`]. Als eigener Schlüssel des Profils
/// gesetzt (vor der Vererbung), damit es den geerbten Wert aus `BASE_NORMAL[_EN]` bzw. den eigenen
/// Wert von `BASE_LOGO` ersetzt.
fn apply_pida_defaults(merged: &mut Properties) {
    for id in assets::PIDA_SEAL_PROFILES {
        merged.insert(format!("sig_obj.{id}.value.SIG_LABEL"), assets::PIDA_SEAL);
    }
    for id in assets::AMTS_PLACEHOLDER_PROFILES {
        merged.insert(format!("sig_obj.{id}.value.SIG_LABEL"), assets::AMTS_PLACEHOLDER);
    }
}

/// Eingebettete Konfiguration laden, mit oder ohne PIDA-Vorgaben.
fn load_builtin(pida_defaults: bool) -> std::result::Result<ProfileStore, String> {
    let mut st = LoadState::default();
    let mut merged = Properties::new();
    load_recursive(&BuiltinCfg, "config.properties", &mut merged, &mut st).map_err(|e| e.to_string())?;
    if pida_defaults {
        apply_pida_defaults(&mut merged);
    }
    let mut store =
        ProfileStore::from_properties(&merged, Arc::new(BuiltinResources::default())).map_err(|e| e.to_string())?;
    store.problems.splice(0..0, st.problems);
    Ok(store)
}

impl ProfileStore {
    /// Eingebettete Konfiguration (schreibgeschützt, Plan §6.5): PDF-AS-Profile mit den
    /// PIDA-Vorgaben (ADR 0005) – Standard-Bildmarke der Profile `SIGNATURBLOCK_SMALL_{DE,EN}[_NOTE][_PDFA]`
    /// und `BASE_LOGO` ist das PIDA-Siegel ([`crate::assets::PIDA_SEAL`]) statt `signatur-logo_de/en.png`.
    /// Die Amtssignatur zeigt statt der Demo-Bildmarke der Behörde die neutrale Platzhalter-Bildmarke
    /// ([`crate::assets::AMTS_PLACEHOLDER`], Backlog Nr. 10); eine eigene Bildmarke
    /// ([`ProfileOverrides::sig_label`], [`crate::SigBlockInput::emblem`]) geht weiterhin vor.
    /// Die PDF-AS-Konfiguration ohne diese Vorgaben liefert `ProfileStore::builtin_pdfas` (Feature `pdfas-reference`).
    pub fn builtin() -> Result<ProfileStore> {
        static BUILTIN: OnceLock<std::result::Result<ProfileStore, String>> = OnceLock::new();
        BUILTIN.get_or_init(|| load_builtin(true)).clone().map_err(SigBlockError::Profile)
    }

    /// Eingebettete PDF-AS-Konfiguration **ohne** PIDA-Vorgaben (Bildmarken wie PDF-AS 4,
    /// `signatur-logo_de/en.png`, Demo-Bildmarke der Amtssignatur) – für den Abgleich mit PDF-AS-Referenzen
    /// (Golden-Tests). Nur mit dem Feature `pdfas-reference`: ohne es liegt die Demo-Bildmarke nicht bei.
    #[cfg(any(test, feature = "pdfas-reference"))]
    pub fn builtin_pdfas() -> Result<ProfileStore> {
        static BUILTIN_PDFAS: OnceLock<std::result::Result<ProfileStore, String>> = OnceLock::new();
        BUILTIN_PDFAS.get_or_init(|| load_builtin(false)).clone().map_err(SigBlockError::Profile)
    }

    /// Lädt `<cfg_dir>/config.properties` mit Includes (Wildcards, nicht rekursiv) – für eigene
    /// Profile aus `~/.pdf-over/cfg` (Erkennung per Prüfsumme in `pida-config`). Ressourcen kommen
    /// aus dem Elternordner von `cfg_dir` ([`DirResources`]).
    pub fn load_dir(cfg_dir: &Path) -> Result<ProfileStore> {
        let mut st = LoadState::default();
        let mut merged = Properties::new();
        load_recursive(&DirCfg(cfg_dir.to_path_buf()), "config.properties", &mut merged, &mut st)?;
        let workdir = cfg_dir.parent().map(Path::to_path_buf).unwrap_or_else(|| cfg_dir.to_path_buf());
        let mut store = ProfileStore::from_properties(&merged, Arc::new(DirResources { workdir }))?;
        store.problems.splice(0..0, st.problems);
        Ok(store)
    }

    /// Aus einer bereits zusammengeführten flachen Map (`putAll` aller Dateien).
    pub fn from_properties(merged: &Properties, resources: Arc<dyn Resources>) -> Result<ProfileStore> {
        let mut props = merged.clone();
        let mut problems = Vec::new();
        let active = resolve_inheritance(&mut props, &mut problems);
        let mut profiles = BTreeMap::new();
        for id in active {
            let p = build_profile(&props, &id, &mut problems);
            profiles.insert(id, p);
        }
        Ok(ProfileStore { profiles, globals: props, resources, problems })
    }

    /// Profil nach ID.
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.get(id)
    }

    /// Profil nach ID oder [`SigBlockError::UnknownProfile`] (PDF-AS 11009).
    pub fn profile(&self, id: &str) -> Result<&Profile> {
        self.get(id).ok_or_else(|| SigBlockError::UnknownProfile(id.to_string()))
    }

    /// Alle IDs.
    pub fn ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.profiles.keys().map(String::as_str)
    }

    /// Globaler Wert (`default.*`, `sig_obj.type.default` …).
    pub fn global(&self, key: &str) -> Option<&str> {
        self.globals.get(key)
    }

    /// Ressourcenquelle.
    pub fn resources(&self) -> &Arc<dyn Resources> {
        &self.resources
    }

    /// Beim Laden erkannte Probleme (verworfene Tabellen, unauflösbare Vererbung …), in der
    /// Reihenfolge des Auftretens; PDF-AS protokolliert diese nur.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard() {
        assert!(wildcard_match("*.properties", "BASE_LOGO.properties"));
        assert!(!wildcard_match("*.properties", "BASE_LOGO.properties.bak"));
        assert!(wildcard_match("BASE_?.p*", "BASE_X.properties"));
        assert!(!wildcard_match("base*", "BASE"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("a*b*c", "aXXbYYc"));
        assert!(!wildcard_match("a*b*c", "aXXbYY"));
    }

    #[test]
    fn normalize() {
        assert_eq!(normalize_rel("profiles/*.properties"), "profiles/*.properties");
        assert_eq!(normalize_rel("./a/../profiles/x"), "profiles/x");
    }

    #[test]
    fn base64_lenient() {
        assert_eq!(lenient_base64("aGFsbG8="), b"hallo");
        assert_eq!(lenient_base64("aGFs\nbG8"), b"hallo");
        assert_eq!(lenient_base64("-_-_"), vec![0xfb, 0xff, 0xbf]);
    }

    #[test]
    fn style_parse_and_inherit() {
        let s = Style::from_entries(
            [
                ("bgcolor", "255 255 255"),
                ("padding", "4"),
                ("border", "0.1"),
                ("halign", "right"),
                ("valign", "top"),
                ("valuehalign", "lineCenter"),
                ("imagehalign", "lineCenter"),
                ("font", "HELVETICA,5,BOLD"),
                ("imagescaletofit", "65.0;65.0"),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(s.bgcolor, Some([255, 255, 255]));
        assert_eq!(s.padding, Some(4.0));
        assert_eq!(s.border, Some(0.1));
        assert_eq!(s.halign, Some(HAlign::Right));
        assert_eq!(s.valuehalign, Some(HAlign::LineCenter));
        assert_eq!(s.imagehalign, None); // lineCenter nur für Werte
        assert_eq!(s.imagescaletofit, Some((65.0, 65.0)));
        // ungültige Farbe ignoriert, ungültige Zahl Fehler
        let s2 = Style::from_entries([("bgcolor", "300 0 0"), ("halign", "middle")].into_iter()).unwrap();
        assert_eq!(s2.bgcolor, None);
        assert_eq!(s2.halign, None);
        assert!(Style::from_entries([("bgcolor", "a b c")].into_iter()).is_err());
        assert!(Style::from_entries([("padding", "x")].into_iter()).is_err());
        assert!(Style::from_entries([("font", "ARIAL,5,BOLD")].into_iter()).is_err());
        assert_eq!(Style::from_entries([("imagescaletofit", "1;2;3")].into_iter()).unwrap().imagescaletofit, None);

        // Q8: padding/border = 1 gelten als nicht gesetzt
        let child = Style { padding: Some(1.0), border: Some(1.0), halign: Some(HAlign::Left), ..Style::default() };
        let inh = child.inherit(&s);
        assert_eq!(inh.padding, Some(4.0));
        assert_eq!(inh.border, Some(0.1));
        assert_eq!(inh.halign, Some(HAlign::Left));
        assert_eq!(inh.font, s.font);
        let child2 = Style { padding: Some(2.0), ..Style::default() };
        assert_eq!(child2.inherit(&s).padding, Some(2.0));
        assert_eq!(Style::default().inherit(&s), s);
        assert_eq!(
            Style { padding: Some(1.0), ..Style::default() }.inherit(&Style::default()).padding_or_default(),
            1.0
        );
    }

    #[test]
    fn row_parse() {
        assert_eq!(
            Row::parse("SIG_LABEL-i|TABLE-info").unwrap().0,
            vec![CellSpec::Image("SIG_LABEL".into()), CellSpec::SubTable("info".into())]
        );
        assert_eq!(Row::parse("SIG_NOTE-cv").unwrap().0, vec![CellSpec::CaptionValue("SIG_NOTE".into())]);
        assert_eq!(Row::parse("A-vc|B-req|C-c|D-v|E-x").unwrap().0.len(), 4);
        assert!(Row::parse("SIG_LABEL").is_none());
        assert!(Row::parse("A-v|B").is_none());
    }

    #[test]
    fn base_logo_overrides() {
        let o = ProfileOverrides::base_logo(None, (326, 342), 23.0);
        assert_eq!(o.main_padding, Some(0.0));
        assert_eq!(o.pos.as_deref(), Some("w:62.117933723196884;f:0"));
        let (w, h) = o.imagescaletofit.unwrap();
        assert!((w - 62.118).abs() < 0.001 && (h - 65.167).abs() < 0.001, "{w} {h}");
        // quadratisch ohne Emblem
        let sq = ProfileOverrides::base_logo(None, (0, 0), 23.0);
        assert_eq!(sq.pos.as_deref(), Some("w:65.16666666666667;f:0"));
        // 120-mm-Klemmung
        let big = ProfileOverrides::base_logo(None, (1, 1), 200.0);
        assert_eq!(big.pos.as_deref(), Some("w:340.0;f:0"));
        // breites Bild: th /= ar
        let wide = ProfileOverrides::base_logo(None, (200, 100), 23.0);
        assert_eq!(
            wide.imagescaletofit.unwrap().1,
            java::parse_float(&java::double_to_string(65.16666666666667 / 2.0)).unwrap()
        );
    }
}

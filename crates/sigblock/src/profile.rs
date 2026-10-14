//! Profil-IR und Profil-Store (docs/analyse/04 §2.1–§2.4, §2.8–§2.9).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pdfover_java_properties::Properties;

use crate::Result;
use crate::emblem::Emblem;
use crate::fonts::FontSpec;

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

impl Style {
    /// `Style.doInherit(base, inherit)`: Kopie von `self`, Lücken aus `inherit`.
    pub fn inherit(&self, inherit: &Style) -> Style {
        todo!("Paket C")
    }

    /// Parst `table.<T>.Style.<attr>`-Einträge.
    pub fn from_entries<'a>(entries: impl Iterator<Item = (&'a str, &'a str)>) -> Result<Style> {
        todo!("Paket C")
    }
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

/// Zeile (`table.<T>.<n>`, `|`-getrennt).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Row(pub Vec<CellSpec>);

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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    /// Profil-ID.
    pub id: String,
    /// `isvisible` (und `table.main` existiert).
    pub visible: bool,
    /// `pos` (Positionsstring, Parsing in `pdfover-placement`).
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
}

impl Profile {
    /// `table.main`, falls vorhanden.
    pub fn main_table(&self) -> Option<&Table> {
        self.tables.get("main")
    }

    /// Wendet Laufzeit-Overlays an (nur auf genau diese Profil-ID, keine Weitervererbung).
    pub fn with_overrides(&self, overrides: &ProfileOverrides) -> Profile {
        todo!("Paket C")
    }

    /// `/Reason` mit Fallback.
    pub fn reason_or_default(&self) -> &str {
        self.reason.as_deref().unwrap_or("PAdES Signature")
    }
}

/// Laufzeit-Overlays wie PDF-Over (`PdfAs4Signer.prepare`, docs/analyse/04 §2.8).
#[derive(Debug, Clone, PartialEq, Default)]
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

impl ProfileOverrides {
    /// BASE_LOGO-Overlay aus Emblem-Pixelmaßen und Zielgröße in mm (Default 23):
    /// `tw = th = mm · 595/210`; `ar < 1 ⇒ tw *= ar`, sonst `th /= ar` (Java-`double`).
    /// Die 120-mm-Klemmung gilt in Vorschau **und** Signatur (Plan §6.5 Register).
    pub fn base_logo(emblem: Option<Arc<Emblem>>, emblem_px: (u32, u32), target_mm: f64) -> Self {
        todo!("Paket C")
    }
}

/// Quelle für Profil-Ressourcen (Bilder `./images/…`, Fonts `TTF:<datei>`).
pub trait Resources: Send + Sync + std::fmt::Debug {
    /// Bild relativ zum Arbeitsordner (`./images/signatur-logo_de.png`) oder Base64-Wert.
    fn image(&self, reference: &str) -> Result<Arc<Emblem>>;
    /// TTF-Datei aus `fonts/`.
    fn font(&self, file: &str) -> Result<Arc<[u8]>>;
}

/// Eingebettete Ressourcen (`crates/sigblock/assets/`: Logos, DejaVu-Fonts; EUPL bzw. Bitstream Vera).
#[derive(Debug, Default)]
pub struct BuiltinResources {
    _private: (),
}

impl Resources for BuiltinResources {
    fn image(&self, reference: &str) -> Result<Arc<Emblem>> {
        todo!("Paket C")
    }

    fn font(&self, file: &str) -> Result<Arc<[u8]>> {
        todo!("Paket C")
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
        todo!("Paket C")
    }

    fn font(&self, file: &str) -> Result<Arc<[u8]>> {
        todo!("Paket C")
    }
}

/// Store aller aktiven Profile.
#[derive(Debug, Clone)]
pub struct ProfileStore {
    profiles: BTreeMap<String, Profile>,
    globals: Properties,
    resources: Arc<dyn Resources>,
}

impl ProfileStore {
    /// Eingebettete PDF-AS-Konfiguration (schreibgeschützt, Plan §6.5).
    pub fn builtin() -> Result<ProfileStore> {
        todo!("Paket C: assets/cfg einbetten (include_bytes!) und über from_properties laden")
    }

    /// Lädt `<cfg_dir>/config.properties` mit Includes (Wildcards, nicht rekursiv) – für eigene
    /// Profile aus `~/.pdf-over/cfg` (Erkennung per Prüfsumme in `pdfover-config`).
    pub fn load_dir(cfg_dir: &Path) -> Result<ProfileStore> {
        todo!("Paket C")
    }

    /// Aus einer bereits zusammengeführten flachen Map (`putAll` aller Dateien).
    pub fn from_properties(merged: &Properties, resources: Arc<dyn Resources>) -> Result<ProfileStore> {
        todo!("Paket C: sig_obj.types.X=on, parent/augments schlüsselweise, Tabellen parsen")
    }

    /// Profil nach ID.
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.get(id)
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
}

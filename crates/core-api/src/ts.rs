//! TypeScript-Typen der Bridge, erzeugt aus den Rust-Typen (Feature `ts`, ts-rs 12).
//!
//! [`typescript_bindings`] liefert den Inhalt von `apps/app/src/core/generated.ts`; der Test
//! `tests/ts_bindings.rs` bricht bei jeder Abweichung ab (neu schreiben mit
//! `PDFOVER_BLESS=1 cargo test -p pdfover-core-api --test ts_bindings`). `view.rs` bleibt damit die
//! einzige Quelle des Vertrags Kern ↔ Oberfläche.
//!
//! Typen aus Crates außerhalb der Bridge (`pdfover-config::Settings`, `pdfover-placement::Placement`)
//! können `ts_rs::TS` wegen der Orphan-Regel nicht erhalten; für sie stehen hier **Spiegel**, die über
//! `#[ts(as = …)]` eingesetzt werden. Gegen Abweichungen sichern ab:
//! - die vollständige Abbildung `From<&Original>` ohne `..` (ein neues Feld oder eine neue Variante
//!   bricht den Build) und
//! - der Test, dass Original und Spiegel identisch serialisieren (Feldnamen, Tags, Schreibweise).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

use pdfover_i18n::Lang;

// ------------------------------------------------------------------------------------------------
// Spiegel: pdfover-placement
// ------------------------------------------------------------------------------------------------

/// Spiegel von `pdfover_placement::Placement` (PAGE_ABS, linke obere Ecke, y = Oberkante von unten, pt).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
pub struct Placement {
    /// 0-basierter Seitenindex; `== Seitenzahl` = virtuelle neue Seite
    pub page_index: usize,
    /// linke Kante in pt
    pub x: f32,
    /// Oberkante in pt (von unten)
    pub y: f32,
    /// Breite in pt
    pub width: f32,
    /// Höhe in pt
    pub height: f32,
}

impl From<&pdfover_placement::Placement> for Placement {
    fn from(v: &pdfover_placement::Placement) -> Self {
        let pdfover_placement::Placement { page_index, x, y, width, height } = *v;
        Placement { page_index, x, y, width, height }
    }
}

// ------------------------------------------------------------------------------------------------
// Spiegel: pdfover-config (Settings-Schema, Plan §7.1)
// ------------------------------------------------------------------------------------------------

/// Spiegel von `pdfover_config::SignatureProfile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignatureProfile {
    /// `SIGNATURBLOCK_SMALL`
    SignaturblockSmall,
    /// `AMTSSIGNATURBLOCK`
    Amtssignaturblock,
    /// `BASE_LOGO`
    BaseLogo,
    /// `INVISIBLE`
    Invisible,
}

/// Spiegel von `pdfover_config::NoteSetting`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", content = "text", rename_all = "snake_case")]
pub enum NoteSetting {
    /// Standardtext
    Default,
    /// kein Hinweis
    None,
    /// eigener Text
    Custom(String),
}

/// Spiegel von `pdfover_config::WindowSize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WindowSize {
    /// Breite
    pub width: u32,
    /// Höhe
    pub height: u32,
}

/// Spiegel von `pdfover_config::GeneralSettings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct GeneralSettings {
    /// UI-Sprache (`None` = System)
    pub ui_language: Option<Lang>,
    /// Update-Prüfung
    pub update_check: bool,
    /// Fenstergröße
    pub window_size: Option<WindowSize>,
    /// letzter Ordner (Dateidialog, F-11)
    pub last_source_directory: Option<PathBuf>,
}

/// Spiegel von `pdfover_config::CmsProfileChoice`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CmsProfileChoice {
    /// kompatibel zu PDF-AS
    PdfasCompat,
    /// PAdES-BASELINE-B
    PadesBb,
}

/// Spiegel von `pdfover_config::SignatureSettings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SignatureSettings {
    /// Profil
    pub profile: SignatureProfile,
    /// Sprache des Signaturblocks (`None` = System)
    pub block_language: Option<Lang>,
    /// Hinweis
    pub note: NoteSetting,
    /// PDF/A erzwingen
    pub pdfa_force: bool,
    /// automatische Positionierung
    pub auto_position: bool,
    /// Bildmarke
    pub emblem: Option<PathBuf>,
    /// Größe „nur Bildmarke“ in mm
    pub logo_only_size_mm: f64,
    /// CMS-Profil
    pub cms_profile: CmsProfileChoice,
}

/// Spiegel von `pdfover_config::PlaceholderKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaceholderKind {
    /// leere Signaturfelder
    Fields,
    /// QR-Platzhalter
    Marker,
}

/// Spiegel von `pdfover_config::PlaceholderSettings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PlaceholderSettings {
    /// Suche aktiv
    pub search: bool,
    /// Art
    pub kind: PlaceholderKind,
}

/// Spiegel von `pdfover_config::BackendChoice`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BackendChoice {
    /// ID Austria
    IdAustria,
    /// lokale BKU
    LocalBku,
    /// Keystore
    Keystore,
}

/// Spiegel von `pdfover_config::BackendSettings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BackendSettings {
    /// Standard-Signiermittel
    pub default: Option<BackendChoice>,
}

/// Spiegel von `pdfover_config::IdAustriaSettings` (das Passwort wird nie persistiert).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct IdAustriaSettings {
    /// Benutzername/Telefonnummer
    pub username: Option<String>,
    /// „Merken“
    pub remember: bool,
    /// FIDO2 als Standard
    pub fido2_default: bool,
}

/// Spiegel von `pdfover_config::PasswordStorage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PasswordStorage {
    /// nie
    None,
    /// bis Programmende
    Memory,
    /// Schlüsselbund
    Keychain,
}

/// Spiegel von `pdfover_config::KeystoreSettings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct KeystoreSettings {
    /// aktiviert
    pub enabled: bool,
    /// Datei
    pub file: Option<PathBuf>,
    /// angegebener Typ
    pub declared_type: Option<String>,
    /// Alias
    pub alias: Option<String>,
    /// Passwort-Speicherart
    pub password_storage: PasswordStorage,
}

/// Spiegel von `pdfover_config::OutputSettings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct OutputSettings {
    /// Ausgabeordner
    pub folder: Option<PathBuf>,
    /// Dateinamenzusatz
    pub postfix: String,
}

/// Spiegel von `pdfover_config::ProxyMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    /// System
    System,
    /// kein Proxy
    None,
    /// manuell
    Manual,
}

/// Spiegel von `pdfover_config::ProxySettings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProxySettings {
    /// Modus
    pub mode: ProxyMode,
    /// Host
    pub host: Option<String>,
    /// Port
    pub port: Option<u16>,
    /// Benutzer
    pub user: Option<String>,
}

/// Spiegel von `pdfover_config::PrivacySettings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PrivacySettings {
    /// bestätigte Textversion je Backend
    pub acknowledged: BTreeMap<String, u32>,
}

/// Spiegel von `pdfover_config::CertificateCacheSettings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CertificateCacheSettings {
    /// Opt-in
    pub enabled: bool,
}

/// Wert eines unbekannten Eintrags (`Settings::unknown`): beliebiges JSON, unverändert zurückgeben.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(type = "unknown")]
pub struct UnknownValue(pub serde_json::Value);

/// Spiegel von `pdfover_config::Settings`. Unbekannte Einträge (`Settings::unknown`, `flatten`) stehen
/// auf oberster Ebene und gehen beim Speichern unverändert zurück.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Settings {
    /// Schema-Version
    pub schema_version: u32,
    /// Allgemein
    pub general: GeneralSettings,
    /// Signatur
    pub signature: SignatureSettings,
    /// Platzhalter
    pub placeholder: PlaceholderSettings,
    /// Signiermittel
    pub backend: BackendSettings,
    /// ID Austria
    pub id_austria: IdAustriaSettings,
    /// Keystore
    pub keystore: KeystoreSettings,
    /// Ausgabe
    pub output: OutputSettings,
    /// Proxy
    pub proxy: ProxySettings,
    /// Datenschutzhinweise
    pub privacy: PrivacySettings,
    /// Zertifikats-Cache
    pub certificate_cache: CertificateCacheSettings,
    /// unbekannte Einträge
    #[serde(flatten)]
    pub unknown: BTreeMap<String, UnknownValue>,
}

impl From<&pdfover_config::Settings> for Settings {
    fn from(v: &pdfover_config::Settings) -> Self {
        use pdfover_config as c;
        let c::Settings {
            schema_version,
            general,
            signature,
            placeholder,
            backend,
            id_austria,
            keystore,
            output,
            proxy,
            privacy,
            certificate_cache,
            unknown,
        } = v;
        let c::GeneralSettings { ui_language, update_check, window_size, last_source_directory } = general;
        let c::SignatureSettings {
            profile,
            block_language,
            note,
            pdfa_force,
            auto_position,
            emblem,
            logo_only_size_mm,
            cms_profile,
        } = signature;
        let c::PlaceholderSettings { search, kind } = placeholder;
        let c::BackendSettings { default } = backend;
        let c::IdAustriaSettings { username, remember, fido2_default } = id_austria;
        let c::KeystoreSettings { enabled, file, declared_type, alias, password_storage } = keystore;
        let c::OutputSettings { folder, postfix } = output;
        let c::ProxySettings { mode, host, port, user } = proxy;
        let c::PrivacySettings { acknowledged } = privacy;
        let c::CertificateCacheSettings { enabled: cache_enabled } = certificate_cache;
        Settings {
            schema_version: *schema_version,
            general: GeneralSettings {
                ui_language: *ui_language,
                update_check: *update_check,
                window_size: window_size.map(|c::WindowSize { width, height }| WindowSize { width, height }),
                last_source_directory: last_source_directory.clone(),
            },
            signature: SignatureSettings {
                profile: match profile {
                    c::SignatureProfile::SignaturblockSmall => SignatureProfile::SignaturblockSmall,
                    c::SignatureProfile::Amtssignaturblock => SignatureProfile::Amtssignaturblock,
                    c::SignatureProfile::BaseLogo => SignatureProfile::BaseLogo,
                    c::SignatureProfile::Invisible => SignatureProfile::Invisible,
                },
                block_language: *block_language,
                note: match note {
                    c::NoteSetting::Default => NoteSetting::Default,
                    c::NoteSetting::None => NoteSetting::None,
                    c::NoteSetting::Custom(t) => NoteSetting::Custom(t.clone()),
                },
                pdfa_force: *pdfa_force,
                auto_position: *auto_position,
                emblem: emblem.clone(),
                logo_only_size_mm: *logo_only_size_mm,
                cms_profile: match cms_profile {
                    c::CmsProfileChoice::PdfasCompat => CmsProfileChoice::PdfasCompat,
                    c::CmsProfileChoice::PadesBb => CmsProfileChoice::PadesBb,
                },
            },
            placeholder: PlaceholderSettings {
                search: *search,
                kind: match kind {
                    c::PlaceholderKind::Fields => PlaceholderKind::Fields,
                    c::PlaceholderKind::Marker => PlaceholderKind::Marker,
                },
            },
            backend: BackendSettings { default: default.map(backend_choice) },
            id_austria: IdAustriaSettings {
                username: username.clone(),
                remember: *remember,
                fido2_default: *fido2_default,
            },
            keystore: KeystoreSettings {
                enabled: *enabled,
                file: file.clone(),
                declared_type: declared_type.clone(),
                alias: alias.clone(),
                password_storage: match password_storage {
                    c::PasswordStorage::None => PasswordStorage::None,
                    c::PasswordStorage::Memory => PasswordStorage::Memory,
                    c::PasswordStorage::Keychain => PasswordStorage::Keychain,
                },
            },
            output: OutputSettings { folder: folder.clone(), postfix: postfix.clone() },
            proxy: ProxySettings {
                mode: match mode {
                    c::ProxyMode::System => ProxyMode::System,
                    c::ProxyMode::None => ProxyMode::None,
                    c::ProxyMode::Manual => ProxyMode::Manual,
                },
                host: host.clone(),
                port: *port,
                user: user.clone(),
            },
            privacy: PrivacySettings { acknowledged: acknowledged.clone() },
            certificate_cache: CertificateCacheSettings { enabled: *cache_enabled },
            unknown: unknown
                .iter()
                .map(|(k, v)| (k.clone(), UnknownValue(serde_json::to_value(v).unwrap_or(serde_json::Value::Null))))
                .collect(),
        }
    }
}

fn backend_choice(v: pdfover_config::BackendChoice) -> BackendChoice {
    match v {
        pdfover_config::BackendChoice::IdAustria => BackendChoice::IdAustria,
        pdfover_config::BackendChoice::LocalBku => BackendChoice::LocalBku,
        pdfover_config::BackendChoice::Keystore => BackendChoice::Keystore,
    }
}

// ------------------------------------------------------------------------------------------------
// Erzeugung
// ------------------------------------------------------------------------------------------------

/// Kopf der erzeugten Datei.
pub const HEADER: &str = "\
// Automatisch erzeugt aus den Rust-Typen der Bridge (crates/core-api/src/view.rs und abhängige
// Crates) mit ts-rs – NICHT VON HAND ÄNDERN.
// Neu erzeugen: PDFOVER_BLESS=1 cargo test -p pdfover-core-api --test ts_bindings
//
// Abbildung (serde_json): Option<T> → T | null, PathBuf → string, u64/usize → number,
// Enums ohne serde-Attribut extern getaggt ({ \"Variante\": … } bzw. \"Variante\"),
// #[serde(tag = …)] intern getaggt.
";

/// Konfiguration: 64-Bit-Zahlen als `number` (Kennungen, Anfrage-IDs; serde_json schreibt Zahlen).
pub fn config() -> Config {
    Config::new().with_large_int("number")
}

/// Ein Eintrag der erzeugten Datei.
fn decl<T: TS>(cfg: &Config, out: &mut String) {
    out.push('\n');
    if let Some(docs) = T::docs() {
        out.push_str(&docs);
    }
    out.push_str("export ");
    out.push_str(&T::decl(cfg));
    out.push('\n');
}

macro_rules! declare_all {
    ($cfg:expr, $out:expr, $($t:ty),+ $(,)?) => {{
        let mut names: Vec<String> = Vec::new();
        $(
            decl::<$t>($cfg, $out);
            names.push(<$t as TS>::ident($cfg));
        )+
        names
    }};
}

/// Inhalt von `apps/app/src/core/generated.ts` und die Namen aller deklarierten Typen.
pub fn typescript_bindings_with_names() -> (String, Vec<String>) {
    use crate::view::*;
    use pdfover_backend_api as b;
    use pdfover_workflow as w;
    let cfg = config();
    let mut out = String::from(HEADER);
    let names = declare_all!(
        &cfg,
        &mut out,
        // pdfover-i18n, pdfover-workflow
        Lang,
        w::DocId,
        w::Step,
        w::MainBarStep,
        w::PlaceholderAnswer,
        w::ts_mirror::AcceptanceClass,
        w::ts_mirror::Invariant,
        w::ts_mirror::InvariantViolation,
        w::ts_mirror::OutputInfo,
        w::WorkflowError,
        w::DocResult,
        // pdfover-backend-api
        b::BackendId,
        b::ts_mirror::DigestAlg,
        b::SignInput,
        b::SignOutput,
        b::CertNeed,
        b::Interaction,
        b::ServerNeed,
        b::Qualification,
        b::FactorSet,
        b::Capabilities,
        b::FormProblem,
        b::FormContext,
        b::QrImage,
        b::NativeForm,
        b::SecretKind,
        b::SecretPrompt,
        b::NoticeKind,
        b::NoticeButtons,
        b::NoticeCode,
        b::Notice,
        b::NoticeAnswer,
        b::Progress,
        // pdfover-placement
        Placement,
        // pdfover-config
        SignatureProfile,
        NoteSetting,
        WindowSize,
        GeneralSettings,
        CmsProfileChoice,
        SignatureSettings,
        PlaceholderKind,
        PlaceholderSettings,
        BackendChoice,
        BackendSettings,
        IdAustriaSettings,
        PasswordStorage,
        KeystoreSettings,
        OutputSettings,
        ProxyMode,
        ProxySettings,
        PrivacySettings,
        CertificateCacheSettings,
        Settings,
        // pdfover-core-api (view.rs)
        NoticeView,
        SaveDialogView,
        MigrationOfferView,
        DoneItemView,
        DocInfo,
        QueueItemView,
        FieldInfo,
        QrInfo,
        BlockView,
        BackendOptionView,
        PrivacyNoticeView,
        InteractionView,
        OutputView,
        MainBarView,
        StepView,
        SettingsView,
        ViewState,
        FormAnswerView,
        UiEvent,
    );
    (out, names)
}

/// Inhalt von `apps/app/src/core/generated.ts`.
pub fn typescript_bindings() -> String {
    typescript_bindings_with_names().0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same<A: Serialize, B: Serialize>(a: &A, b: &B) {
        assert_eq!(serde_json::to_value(a).unwrap(), serde_json::to_value(b).unwrap());
    }

    #[test]
    fn placement_mirror_serializes_like_the_original() {
        let p = pdfover_placement::Placement { page_index: 3, x: 1.5, y: 700.0, width: 230.0, height: 49.0 };
        same(&p, &Placement::from(&p));
    }

    #[test]
    fn settings_mirror_serializes_like_the_original() {
        use pdfover_config as c;
        let mut s = c::Settings::default();
        same(&s, &Settings::from(&s));
        // alle Varianten und optionalen Werte belegt, dazu unbekannte Einträge (flatten)
        s.general.ui_language = Some(Lang::En);
        s.general.window_size = Some(c::WindowSize { width: 800, height: 600 });
        s.general.last_source_directory = Some("/tmp".into());
        s.signature.block_language = Some(Lang::De);
        s.signature.note = c::NoteSetting::Custom("Hinweis".into());
        s.signature.emblem = Some("/logo.png".into());
        s.signature.cms_profile = c::CmsProfileChoice::PadesBb;
        s.placeholder.kind = c::PlaceholderKind::Fields;
        s.backend.default = Some(c::BackendChoice::LocalBku);
        s.id_austria.username = Some("+43664".into());
        s.keystore.password_storage = c::PasswordStorage::Keychain;
        s.keystore.file = Some("/k.p12".into());
        s.proxy.mode = c::ProxyMode::Manual;
        s.proxy.port = Some(3128);
        s.privacy.acknowledged.insert("atrust_native".into(), 1);
        s.unknown.insert("future".into(), toml::Value::Integer(5));
        let mut table = toml::map::Map::new();
        table.insert("x".into(), toml::Value::String("y".into()));
        s.unknown.insert("zukunft".into(), toml::Value::Table(table));
        same(&s, &Settings::from(&s));
        for p in [
            c::SignatureProfile::SignaturblockSmall,
            c::SignatureProfile::Amtssignaturblock,
            c::SignatureProfile::BaseLogo,
            c::SignatureProfile::Invisible,
        ] {
            s.signature.profile = p;
            same(&s, &Settings::from(&s));
        }
        for n in [c::NoteSetting::Default, c::NoteSetting::None] {
            s.signature.note = n;
            same(&s, &Settings::from(&s));
        }
        for (b, m, ps) in [
            (c::BackendChoice::IdAustria, c::ProxyMode::System, c::PasswordStorage::None),
            (c::BackendChoice::Keystore, c::ProxyMode::None, c::PasswordStorage::Memory),
        ] {
            s.backend.default = Some(b);
            s.proxy.mode = m;
            s.keystore.password_storage = ps;
            same(&s, &Settings::from(&s));
        }
        // der Spiegel liest das Original-JSON und schreibt es unverändert zurück
        let json = serde_json::to_value(&s).unwrap();
        let mirror: Settings = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&mirror).unwrap(), json);
    }
}

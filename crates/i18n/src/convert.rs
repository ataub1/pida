//! Konverter `messages*.properties` → Fluent (Plan §7.5, docs/analyse/01 §6.3).
//!
//! Regeln:
//! 1. Java-Properties-Syntax über `pdfover-java-properties` (Escapes `\:`, `\!`, `\n`, `\\`).
//! 2. `&`-Mnemonic entfernen und als Attribut `.accesskey` ablegen (`&&` = literales `&`).
//! 3. `%s`/`%d` in Reihenfolge zu benannten Argumenten `{ $arg1 }`, `{ $arg2 }` … (EN und DE haben
//!    dieselbe Reihenfolge).
//! 4. `<a>…</a>` bleibt als Markup-Token im Text; die UI rendert es als Link.
//! 5. Mehrzeilige Werte (`\n`) als Fluent-Mehrzeiler; führende Leerzeichen über `{" "}`.
//! 6. B-09: DE-Duplikat `error.Initialization` wird zu `error.ConfigInitialization` korrigiert,
//!    fehlende Keys werden gemeldet.

use pdfover_java_properties::Properties;

/// Ergebnis einer Konvertierung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Conversion {
    /// Fertiger FTL-Text.
    pub ftl: String,
    /// Anzahl Nachrichten.
    pub messages: usize,
    /// Warnungen (z. B. nicht konvertierbare Platzhalter).
    pub warnings: Vec<String>,
}

/// Optionen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConvertOptions {
    /// Keys, die als ungenutzt markiert werden (Kommentar `# unused`).
    pub unused_keys: Vec<String>,
    /// Kopfkommentar (Lizenz/Herkunft: EUPL, PDF-Over).
    pub header: Option<String>,
}

/// Fehler.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    /// Properties nicht lesbar.
    #[error("Properties nicht lesbar: {0}")]
    Properties(String),
    /// Erzeugtes FTL ist ungültig.
    #[error("erzeugtes FTL ungültig: {0}")]
    InvalidFtl(String),
}

/// Konvertiert eine Properties-Map nach Fluent.
pub fn properties_to_ftl(props: &Properties, opts: &ConvertOptions) -> Result<Conversion, ConvertError> {
    todo!("Paket E")
}

/// Ein einzelner Wert: liefert (Fluent-Pattern, Access-Key).
pub fn convert_value(java_value: &str) -> (String, Option<char>) {
    todo!("Paket E")
}

/// Vergleicht zwei Sprachstände (fehlende/überzählige Keys, Platzhalter-Konsistenz).
pub fn compare_catalogs(base: &Properties, other: &Properties) -> Vec<String> {
    todo!("Paket E")
}

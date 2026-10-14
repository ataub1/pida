//! Ausdrücke `${…}` (OGNL-Teilmenge) und `SIG_DATE` (docs/analyse/04 §2.5).
//!
//! Unterstützt genau die belegte Teilmenge: `subject.X`, `issuer.X`, `sn`, `sbp.X`, `iui.pdfVersion`,
//! Ternary `a != null ? (b + " ") : ""`, Null-Vergleich, String-Konkatenation, Map-Zugriff und den
//! Listen-Literal-Trick (`{expr}` → `[wert]` → Klammern abschneiden). Alles andere ergibt
//! [`crate::SigBlockError::UnsupportedExpression`].

use std::collections::BTreeMap;

use time::OffsetDateTime;

use crate::Result;

/// Variablen für die Auflösung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExprContext {
    /// `subject.*` (inkl. Aliasse `title`/`T`, `DN`)
    pub subject: BTreeMap<String, String>,
    /// `issuer.*`
    pub issuer: BTreeMap<String, String>,
    /// `sn`
    pub sn: String,
    /// `sbp.*`
    pub sbp: BTreeMap<String, String>,
    /// `iui.pdfVersion`
    pub pdf_version: String,
}

/// Löst alle `${…}` in `template` auf.
pub fn resolve(template: &str, ctx: &ExprContext) -> Result<String> {
    todo!("Paket C")
}

/// Formatiert `SIG_DATE` mit einem `SimpleDateFormat`-Muster (Default
/// `yyyy-MM-dd'T'HH:mm:ssXXX`) in Zeitzone `timezone` bzw. lokal (Quirk Q9, Register).
pub fn format_sig_date(pattern: Option<&str>, time: OffsetDateTime, timezone: Option<&str>) -> Result<String> {
    todo!("Paket C")
}

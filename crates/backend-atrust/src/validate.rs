//! Eingabeprüfungen der Masken (docs/analyse/01 §4.6.1/§4.6.2). Die UI ruft sie über den Kern auf
//! (keine Fachlogik in TypeScript, Plan §2.2).

use pdfover_backend_api::FormProblem;

/// Passwortlänge 5–200 (`MobileBKUValidator`).
pub fn validate_password(password: &str) -> Result<(), FormProblem> {
    todo!("Paket D")
}

/// TAN: getrimmt nicht leer; darf kein Präfix des Vergleichswerts sein; höchstens 6 Zeichen.
pub fn validate_tan(tan: &str, reference_value: &str) -> Result<(), FormProblem> {
    todo!("Paket D")
}

/// Live-Warnung ab mehr als 3 Zeichen, wenn der Vergleichswert mit der Eingabe beginnt.
pub fn tan_live_warning(tan: &str, reference_value: &str) -> Option<FormProblem> {
    todo!("Paket D")
}

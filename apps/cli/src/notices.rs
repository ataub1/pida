//! EUPL-1.2-Text und eingebettete Drittlizenz-Hinweise (Feature `notices`, `build.rs`): für `pida-cli licenses` und
//! die Lizenzanzeige der App (Backlog Nr. 2 – auch auf Android, wo das Paket keine Ressourcendateien mitbringt).

/// EUPL-1.2-Text des Werks (Art. 5: jeder Kopie beilegen).
pub const EUPL: &str = include_str!("../../../LICENSE");

/// Drittlizenz-Hinweise (Dateiname, mit Deflate gepackter Inhalt aus `build.rs`; leer = nicht eingebettet).
pub const NOTICES: [(&str, &[u8]); 2] = [
    ("THIRD-PARTY-NOTICES.md", include_bytes!(concat!(env!("OUT_DIR"), "/THIRD-PARTY-NOTICES.md.deflate"))),
    ("RUST-STD-COPYRIGHT.html", include_bytes!(concat!(env!("OUT_DIR"), "/RUST-STD-COPYRIGHT.html.deflate"))),
];

/// Eingebettete Datei entpacken (`None`, wenn der Build ohne Hinweise lief).
pub fn notice(packed: &[u8]) -> Option<Vec<u8>> {
    if packed.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(packed), &mut out).ok()?;
    Some(out)
}

/// Eingebettete Datei nach Namen als Text (`THIRD-PARTY-NOTICES.md`, `RUST-STD-COPYRIGHT.html`); `None` ohne Hinweise.
pub fn text(name: &str) -> Option<String> {
    let (_, packed) = NOTICES.iter().find(|(n, _)| *n == name)?;
    notice(packed).map(|d| String::from_utf8_lossy(&d).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eupl_and_embedded_notices() {
        assert!(EUPL.trim_start().starts_with("EUROPEAN UNION PUBLIC LICENCE v. 1.2"));
        assert_eq!(text("unbekannt.txt"), None);
        // eingebettet oder nicht – dann für beide Dateien gleich (build.rs)
        let md = text("THIRD-PARTY-NOTICES.md");
        assert_eq!(md.is_some(), text("RUST-STD-COPYRIGHT.html").is_some());
        if let Some(md) = md {
            assert!(md.starts_with("# THIRD-PARTY-NOTICES – PIDA"), "{}", &md[..40]);
        }
    }
}

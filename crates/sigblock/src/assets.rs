//! Eingebettete PDF-AS-Konfiguration (`assets/`, übernommen aus `pdfas-config`, EUPL bzw.
//! Bitstream-Vera-Lizenz für DejaVu): `cfg/config.properties`, `cfg/advancedconfig.properties`,
//! `cfg/profiles/*.properties`, `fonts/DejaVu*.ttf`, `images/*.png`. `arial.ttf` wird bewusst nicht
//! ausgeliefert (analyse/05 §6.3). Dazu das PIDA-Siegel `images/pida-seal.png` ([`PIDA_SEAL`],
//! eigenes Werk des Projekts, EUPL-1.2, ADR 0005); die PDF-AS-Dateien bleiben unverändert.

/// `cfg/`-Dateien relativ zu `cfg/` (in fester Reihenfolge; Includes werden darüber aufgelöst).
pub(crate) const CFG_FILES: &[(&str, &[u8])] = &[
    ("config.properties", include_bytes!("../assets/cfg/config.properties")),
    ("advancedconfig.properties", include_bytes!("../assets/cfg/advancedconfig.properties")),
    (
        "profiles/AMTSSIGNATURBLOCK_BASE.properties",
        include_bytes!("../assets/cfg/profiles/AMTSSIGNATURBLOCK_BASE.properties"),
    ),
    (
        "profiles/AMTSSIGNATURBLOCK_BASE_EN.properties",
        include_bytes!("../assets/cfg/profiles/AMTSSIGNATURBLOCK_BASE_EN.properties"),
    ),
    (
        "profiles/AMTSSIGNATURBLOCK_DE_RECOMMENDED.properties",
        include_bytes!("../assets/cfg/profiles/AMTSSIGNATURBLOCK_DE_RECOMMENDED.properties"),
    ),
    (
        "profiles/AMTSSIGNATURBLOCK_EN_RECOMMENDED.properties",
        include_bytes!("../assets/cfg/profiles/AMTSSIGNATURBLOCK_EN_RECOMMENDED.properties"),
    ),
    ("profiles/BASE_LOGO.properties", include_bytes!("../assets/cfg/profiles/BASE_LOGO.properties")),
    ("profiles/BASE_NORMAL.properties", include_bytes!("../assets/cfg/profiles/BASE_NORMAL.properties")),
    ("profiles/BASE_NORMAL_EN.properties", include_bytes!("../assets/cfg/profiles/BASE_NORMAL_EN.properties")),
    ("profiles/BASE_SMALL.properties", include_bytes!("../assets/cfg/profiles/BASE_SMALL.properties")),
    ("profiles/BASE_SMALL_EN.properties", include_bytes!("../assets/cfg/profiles/BASE_SMALL_EN.properties")),
    ("profiles/BASE_SMALL_EN_NOTE.properties", include_bytes!("../assets/cfg/profiles/BASE_SMALL_EN_NOTE.properties")),
    ("profiles/BASE_SMALL_NOTE.properties", include_bytes!("../assets/cfg/profiles/BASE_SMALL_NOTE.properties")),
    ("profiles/INVISIBLE.properties", include_bytes!("../assets/cfg/profiles/INVISIBLE.properties")),
    (
        "profiles/SIGNATURBLOCK_SMALL_DE.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_DE.properties"),
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_DE_NOTE.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_DE_NOTE.properties"),
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_DE_NOTE_PDFA.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_DE_NOTE_PDFA.properties"),
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_DE_PDFA.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_DE_PDFA.properties"),
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_EN.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_EN.properties"),
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_EN_NOTE.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_EN_NOTE.properties"),
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_EN_NOTE_PDFA.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_EN_NOTE_PDFA.properties"),
    ),
    (
        "profiles/SIGNATURBLOCK_SMALL_EN_PDFA.properties",
        include_bytes!("../assets/cfg/profiles/SIGNATURBLOCK_SMALL_EN_PDFA.properties"),
    ),
];

/// `fonts/`
const FONTS: &[(&str, &[u8])] = &[
    ("DejaVuSansMono.ttf", include_bytes!("../assets/fonts/DejaVuSansMono.ttf")),
    ("DejaVuSansCondensed-Bold.ttf", include_bytes!("../assets/fonts/DejaVuSansCondensed-Bold.ttf")),
    ("DejaVuSansCondensed.ttf", include_bytes!("../assets/fonts/DejaVuSansCondensed.ttf")),
];

/// `images/` – die drei PDF-AS-Bildmarken (EUPL) und das PIDA-Siegel ([`PIDA_SEAL`], eigenes Werk
/// des Projekts, EUPL-1.2; Master `design/brand/pida-seal.svg`, ADR 0005).
pub(crate) const IMAGES: &[(&str, &[u8])] = &[
    ("signatur-logo_de.png", include_bytes!("../assets/images/signatur-logo_de.png")),
    ("signatur-logo_en.png", include_bytes!("../assets/images/signatur-logo_en.png")),
    ("amtsignatur-logo_demo.png", include_bytes!("../assets/images/amtsignatur-logo_demo.png")),
    ("pida-seal.png", include_bytes!("../assets/images/pida-seal.png")),
];

/// Profilreferenz des PIDA-Siegels (Standard-Bildmarke der Profile [`PIDA_SEAL_PROFILES`], ADR 0005).
///
/// 652 × 684 px RGBA = 2 × `signatur-logo_de/en.png` (326 × 342): gleiches Seitenverhältnis mit
/// Faktor 2, damit Zeilenhöhen und Blockmaße für jede Blockbreite unverändert bleiben (die Bildhöhe
/// folgt aus Spaltenbreite · Höhe/Breite; mit einer Zweierpotenz rechnet das Layout bitgleich), bei
/// doppelter Auflösung (Standardbreite 230 pt ≈ 1400 dpi, 495 pt ≈ 570 dpi, „nur Bildmarke“ 23 mm
/// ≈ 750 dpi); außerhalb des Kreises transparent (PDF/A-1: auf Weiß komponiert, sonst `SMask`).
pub const PIDA_SEAL: &str = "./images/pida-seal.png";

/// Profile, deren Standard-Bildmarke in [`crate::ProfileStore::builtin`] das PIDA-Siegel ist
/// (`value.SIG_LABEL` = [`PIDA_SEAL`]): die acht kleinen Standardblöcke (in PDF-AS
/// `./images/signatur-logo_de|en.png`) und `BASE_LOGO` („nur Bildmarke“ ohne eigene Bildmarke, in
/// PDF-AS `./images/signatur-logo_de.png`). Die Amtssignatur (`AMTSSIGNATURBLOCK_*`) behält die
/// Demo-Bildmarke der Behörde (`amtsignatur-logo_demo.png`).
pub const PIDA_SEAL_PROFILES: [&str; 9] = [
    "SIGNATURBLOCK_SMALL_DE",
    "SIGNATURBLOCK_SMALL_DE_NOTE",
    "SIGNATURBLOCK_SMALL_DE_PDFA",
    "SIGNATURBLOCK_SMALL_DE_NOTE_PDFA",
    "SIGNATURBLOCK_SMALL_EN",
    "SIGNATURBLOCK_SMALL_EN_NOTE",
    "SIGNATURBLOCK_SMALL_EN_PDFA",
    "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA",
    "BASE_LOGO",
];

/// Datei aus `cfg/` (Pfad relativ zu `cfg/`, `/`-getrennt).
pub(crate) fn cfg(path: &str) -> Option<&'static [u8]> {
    CFG_FILES.iter().find(|(p, _)| *p == path).map(|(_, d)| *d)
}

/// Font aus `fonts/`.
pub(crate) fn font(file: &str) -> Option<&'static [u8]> {
    FONTS.iter().find(|(p, _)| *p == file).map(|(_, d)| *d)
}

/// Bild aus `images/`.
pub(crate) fn image(file: &str) -> Option<&'static [u8]> {
    IMAGES.iter().find(|(p, _)| *p == file).map(|(_, d)| *d)
}

//! sRGB-ICC-Profil für `ICCBased`-Farbräume im PDF/A-Fall (Plan §6.5 „Logos als ICCBased (sRGB)“).
//!
//! ICC v2.1, Geräteklasse Monitor, RGB → XYZ (Matrix/TRC). Die Daten werden aus den Normwerten von
//! IEC 61966-2-1 berechnet (Bradford-adaptierte Primärvalenzen in s15Fixed16 wie im verbreiteten
//! sRGB-IEC61966-2.1-Profil, Weißpunkt D65, Übertragungsfunktion mit 1024 Stützstellen); es
//! werden keine fremden Profildateien eingebettet. Damit sind Bildmarken in PDF/A unabhängig vom
//! OutputIntent des Dokuments konform (DeviceRGB verlangt einen RGB-OutputIntent, bei CMYK-PDF/A
//! wie FOGRA39 also eine Verletzung von ISO 19005-1 §6.2.3.3).

use std::sync::OnceLock;

/// Anzahl der Stützstellen der Tonwertkurve.
const TRC_POINTS: usize = 1024;

/// Beschreibung (`desc`).
const DESCRIPTION: &str = "sRGB (IEC 61966-2-1), PDF-Over NG";
/// Hinweis (`cprt`): aus Normwerten berechnet, frei verwendbar.
const COPYRIGHT: &str = "No copyright, use freely";

/// `s15Fixed16Number`
fn s15f16(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(xyz: [f64; 3]) -> Vec<u8> {
    let mut t = b"XYZ \0\0\0\0".to_vec();
    for v in xyz {
        t.extend_from_slice(&s15f16(v));
    }
    t
}

/// sRGB-Übertragungsfunktion (IEC 61966-2-1): kodiert → linear.
fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn curve_tag() -> Vec<u8> {
    let mut t = b"curv\0\0\0\0".to_vec();
    t.extend_from_slice(&(TRC_POINTS as u32).to_be_bytes());
    for i in 0..TRC_POINTS {
        let y = srgb_to_linear(i as f64 / (TRC_POINTS - 1) as f64);
        t.extend_from_slice(&((y * 65535.0).round() as u16).to_be_bytes());
    }
    t
}

/// `textDescriptionType` (ICC v2): ASCII-Teil, leere Unicode- und ScriptCode-Teile.
fn desc_tag(text: &str) -> Vec<u8> {
    let mut t = b"desc\0\0\0\0".to_vec();
    t.extend_from_slice(&(text.len() as u32 + 1).to_be_bytes());
    t.extend_from_slice(text.as_bytes());
    t.push(0);
    t.extend_from_slice(&[0; 4]); // Unicode-Sprachcode
    t.extend_from_slice(&[0; 4]); // Unicode-Länge
    t.extend_from_slice(&[0; 2]); // ScriptCode-Code
    t.push(0); // ScriptCode-Länge
    t.extend_from_slice(&[0; 67]);
    t
}

fn text_tag(text: &str) -> Vec<u8> {
    let mut t = b"text\0\0\0\0".to_vec();
    t.extend_from_slice(text.as_bytes());
    t.push(0);
    t
}

fn build() -> Vec<u8> {
    // Primärvalenzen (D50-adaptiert) und Weißpunkt wie im sRGB-IEC61966-2.1-Profil
    let red = xyz_tag([0.436_066, 0.222_488, 0.013_916]);
    let green = xyz_tag([0.385_147, 0.716_873, 0.097_076]);
    let blue = xyz_tag([0.143_066, 0.060_608, 0.714_096]);
    let white = xyz_tag([0.950_455, 1.0, 1.089_050]);
    let trc = curve_tag();
    let blocks: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc_tag(DESCRIPTION)),
        (b"cprt", text_tag(COPYRIGHT)),
        (b"wtpt", white),
        (b"rXYZ", red),
        (b"gXYZ", green),
        (b"bXYZ", blue),
        (b"rTRC", trc),
    ];
    // gTRC/bTRC teilen sich die Daten von rTRC
    let tag_count = blocks.len() + 2;
    let mut offset = 128 + 4 + 12 * tag_count;
    let mut table = Vec::new();
    let mut data = Vec::new();
    let mut trc_entry = (0u32, 0u32);
    for (sig, bytes) in &blocks {
        table.extend_from_slice(*sig);
        table.extend_from_slice(&(offset as u32).to_be_bytes());
        table.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        if *sig == b"rTRC" {
            trc_entry = (offset as u32, bytes.len() as u32);
        }
        data.extend_from_slice(bytes);
        offset += bytes.len();
        while data.len() % 4 != 0 {
            data.push(0);
            offset += 1;
        }
    }
    for sig in [b"gTRC", b"bTRC"] {
        table.extend_from_slice(sig);
        table.extend_from_slice(&trc_entry.0.to_be_bytes());
        table.extend_from_slice(&trc_entry.1.to_be_bytes());
    }
    let size = 128 + 4 + table.len() + data.len();
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&(size as u32).to_be_bytes());
    out.extend_from_slice(&[0; 4]); // bevorzugtes CMM
    out.extend_from_slice(&[0x02, 0x10, 0x00, 0x00]); // Version 2.1.0
    out.extend_from_slice(b"mntr");
    out.extend_from_slice(b"RGB ");
    out.extend_from_slice(b"XYZ ");
    for v in [2026u16, 1, 1, 0, 0, 0] {
        out.extend_from_slice(&v.to_be_bytes()); // Erstellungsdatum (fest, deterministisch)
    }
    out.extend_from_slice(b"acsp");
    out.extend_from_slice(&[0; 4]); // Plattform
    out.extend_from_slice(&[0; 4]); // Flags
    out.extend_from_slice(&[0; 4]); // Hersteller
    out.extend_from_slice(&[0; 4]); // Modell
    out.extend_from_slice(&[0; 8]); // Attribute
    out.extend_from_slice(&[0; 4]); // Rendering-Intent: perzeptiv
    for v in [0.9642, 1.0, 0.8249] {
        out.extend_from_slice(&s15f16(v)); // PCS-Lichtart D50
    }
    out.extend_from_slice(&[0; 4]); // Ersteller
    out.resize(128, 0);
    out.extend_from_slice(&(tag_count as u32).to_be_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&data);
    debug_assert_eq!(out.len(), size);
    out
}

/// Das Profil (einmal berechnet).
pub(crate) fn srgb_profile() -> &'static [u8] {
    static PROFILE: OnceLock<Vec<u8>> = OnceLock::new();
    PROFILE.get_or_init(build)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
    }

    #[test]
    fn header_and_tag_table_are_consistent() {
        let p = srgb_profile();
        assert_eq!(u32_at(p, 0) as usize, p.len());
        assert_eq!(&p[8..12], &[2, 0x10, 0, 0]);
        assert_eq!(&p[12..24], b"mntrRGB XYZ ");
        assert_eq!(&p[36..40], b"acsp");
        let n = u32_at(p, 128) as usize;
        assert_eq!(n, 9);
        let mut sigs = Vec::new();
        for k in 0..n {
            let e = 132 + 12 * k;
            let (off, len) = (u32_at(p, e + 4) as usize, u32_at(p, e + 8) as usize);
            assert_eq!(off % 4, 0, "Tag-Daten 4-Byte-ausgerichtet");
            assert!(off + len <= p.len());
            let typ = &p[off..off + 4];
            let sig = &p[e..e + 4];
            let expected: &[u8] = match sig {
                b"desc" => b"desc",
                b"cprt" => b"text",
                b"rTRC" | b"gTRC" | b"bTRC" => b"curv",
                _ => b"XYZ ",
            };
            assert_eq!(typ, expected, "{}", String::from_utf8_lossy(sig));
            sigs.push(sig.to_vec());
        }
        for s in [b"desc", b"cprt", b"wtpt", b"rXYZ", b"gXYZ", b"bXYZ", b"rTRC", b"gTRC", b"bTRC"] {
            assert!(sigs.iter().any(|x| x == s));
        }
    }

    #[test]
    fn curve_matches_srgb() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-12);
        assert!((srgb_to_linear(0.5) - 0.214_041).abs() < 1e-6);
        let c = curve_tag();
        assert_eq!(c.len(), 12 + 2 * TRC_POINTS);
        assert_eq!(u16::from_be_bytes([c[c.len() - 2], c[c.len() - 1]]), 65535);
    }
}

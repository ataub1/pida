//! sRGB-ICC-Profil für den OutputIntent (Plan §6.9: „frei lizenziertes ICC-Profil“).
//!
//! Gleiche Berechnung wie `pdfover-sigblock` (`icc.rs`, dort für `ICCBased`-Farbräume; das Modul
//! ist dort privat): ICC v2.1, Geräteklasse Monitor, RGB → XYZ (Matrix/TRC) aus den Normwerten von
//! IEC 61966-2-1, Weißpunkt D65 (D50-adaptierte Primärvalenzen), Tonwertkurve mit 1024
//! Stützstellen. Keine fremden Profildateien; die Bytes sind deterministisch und identisch mit dem
//! Profil der Bildmarken.

use std::sync::OnceLock;

const TRC_POINTS: usize = 1024;
const DESCRIPTION: &str = "sRGB (IEC 61966-2-1), PDF-Over NG";
const COPYRIGHT: &str = "No copyright, use freely";

/// `OutputConditionIdentifier`/`Info` des OutputIntents (wie PDF-AS).
pub(crate) const OUTPUT_CONDITION: &str = "sRGB IEC61966-2.1";
/// `RegistryName` (wie PDF-AS).
pub(crate) const REGISTRY: &str = "http://www.color.org";

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

fn desc_tag(text: &str) -> Vec<u8> {
    let mut t = b"desc\0\0\0\0".to_vec();
    t.extend_from_slice(&(text.len() as u32 + 1).to_be_bytes());
    t.extend_from_slice(text.as_bytes());
    t.push(0);
    t.extend_from_slice(&[0; 4]);
    t.extend_from_slice(&[0; 4]);
    t.extend_from_slice(&[0; 2]);
    t.push(0);
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
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[0x02, 0x10, 0x00, 0x00]);
    out.extend_from_slice(b"mntr");
    out.extend_from_slice(b"RGB ");
    out.extend_from_slice(b"XYZ ");
    for v in [2026u16, 1, 1, 0, 0, 0] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(b"acsp");
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&[0; 4]);
    for v in [0.9642, 1.0, 0.8249] {
        out.extend_from_slice(&s15f16(v));
    }
    out.extend_from_slice(&[0; 4]);
    out.resize(128, 0);
    out.extend_from_slice(&(tag_count as u32).to_be_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&data);
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

    #[test]
    fn profile_header_is_consistent() {
        let p = srgb_profile();
        assert_eq!(u32::from_be_bytes([p[0], p[1], p[2], p[3]]) as usize, p.len());
        assert_eq!(&p[12..24], b"mntrRGB XYZ ");
        assert_eq!(&p[36..40], b"acsp");
        assert_eq!(u32::from_be_bytes([p[128], p[129], p[130], p[131]]), 9);
    }
}

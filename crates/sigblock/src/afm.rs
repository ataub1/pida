//! Standard-14-Metriken (AFM) und WinAnsiEncoding wie PDFBox 2.0 (`PDType1Font`, `WinAnsiEncoding`).

use crate::fonts::Std14;

/// Kopfwerte einer AFM-Datei (1/1000 em).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AfmMetrics {
    pub ascender: i16,
    pub descender: i16,
    pub cap_height: i16,
    pub x_height: i16,
    pub bbox: [i16; 4],
}

#[path = "afm_data.rs"]
mod data;

/// Breitentabelle (WinAnsi-Code → Breite, `0` = nicht belegt) und Kopfwerte.
pub(crate) fn tables(face: Std14) -> (&'static [u16; 256], &'static AfmMetrics) {
    match face {
        Std14::Helvetica => (&data::HELVETICA_WIDTHS, &data::HELVETICA_METRICS),
        Std14::HelveticaBold => (&data::HELVETICA_BOLD_WIDTHS, &data::HELVETICA_BOLD_METRICS),
        Std14::Courier => (&data::COURIER_WIDTHS, &data::COURIER_METRICS),
        Std14::CourierBold => (&data::COURIER_BOLD_WIDTHS, &data::COURIER_BOLD_METRICS),
        Std14::TimesRoman => (&data::TIMES_ROMAN_WIDTHS, &data::TIMES_ROMAN_METRICS),
        Std14::TimesBold => (&data::TIMES_BOLD_WIDTHS, &data::TIMES_BOLD_METRICS),
        Std14::TimesItalic => (&data::TIMES_ITALIC_WIDTHS, &data::TIMES_ITALIC_METRICS),
    }
}

/// Unicode → WinAnsi-Code, wie `PDType1Font.encode` mit `WinAnsiEncoding`: ASCII 0x20–0x7E,
/// Latin-1 0xA1–0xFF (ohne U+00A0 und U+00AD, deren AGL-Namen `nbspace`/`sfthyphen` nicht in der
/// Kodierung stehen) und die CP1252-Zeichen 0x80–0x9F.
pub(crate) fn winansi_code(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x20..=0x7E => Some(u as u8),
        0xA1..=0xFF if u != 0xAD => Some(u as u8),
        _ => data::WINANSI_HIGH.iter().find(|(ch, _)| *ch == c).map(|(_, code)| *code),
    }
}

/// CP1252-Rundreise (`StringUtils.convertStringToPDFFormat`): darstellbar sind ASCII inkl.
/// Steuerzeichen, Latin-1 0xA0–0xFF und die belegten Zeichen 0x80–0x9F.
pub(crate) fn cp1252_roundtrips(c: char) -> bool {
    let u = c as u32;
    u < 0x80 || (0xA0..=0xFF).contains(&u) || data::WINANSI_HIGH.iter().any(|(ch, _)| *ch == c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width(face: Std14, s: &str) -> u32 {
        let (w, _) = tables(face);
        s.chars().map(|c| u32::from(w[winansi_code(c).unwrap() as usize])).sum()
    }

    #[test]
    fn helvetica_bold_widths_match_pdfas_output() {
        // Rückgerechnet aus rechtsbündigen Captions in PDF-AS-signierten Referenz-PDFs.
        assert_eq!(width(Std14::HelveticaBold, "Unterzeichner"), 6668);
        assert_eq!(width(Std14::HelveticaBold, "Datum/Zeit-UTC"), 7555);
        assert_eq!(width(Std14::HelveticaBold, "Prüfinformation"), 7500);
    }

    #[test]
    fn descenders() {
        assert_eq!(tables(Std14::Helvetica).1.descender, -207);
        assert_eq!(tables(Std14::HelveticaBold).1.descender, -207);
        assert_eq!(tables(Std14::Courier).1.descender, -157);
        assert_eq!(tables(Std14::TimesRoman).1.descender, -217);
    }

    #[test]
    fn courier_is_monospace() {
        let (w, _) = tables(Std14::Courier);
        for c in (0x20u8..0x7F).chain(0xA1..=0xFF) {
            if c != 0xAD {
                assert_eq!(w[c as usize], 600, "code {c:#x}");
            }
        }
    }

    #[test]
    fn winansi_mapping() {
        assert_eq!(winansi_code('€'), Some(0x80));
        assert_eq!(winansi_code('ü'), Some(0xFC));
        assert_eq!(winansi_code('–'), Some(0x96));
        assert_eq!(winansi_code('\u{a0}'), None);
        assert_eq!(winansi_code('\n'), None);
        assert_eq!(winansi_code('Ł'), None);
        assert!(cp1252_roundtrips('\n'));
        assert!(cp1252_roundtrips('€'));
        assert!(!cp1252_roundtrips('Ł'));
        assert!(!cp1252_roundtrips('\u{81}'));
    }
}

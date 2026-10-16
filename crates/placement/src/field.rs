//! Signaturblock in einem vorhandenen leeren Signaturfeld (KR-F13; ADR 0004 Nachtrag 16).
//!
//! **PDF-AS 4.4.2** übernimmt beim Signieren in ein vorhandenes Feld dessen `/Rect` unverändert (PDFBox 2.0.32
//! `PDDocument.addSignature` → `assignSignatureRectangle`: „in case of an existing field keep the original rect“)
//! und hängt die Appearance des Blocks mit `/BBox [W+1 H+1]` an (`assignAppearanceDictionary`). Ein Viewer bildet
//! die transformierte BBox nach ISO 32000-1 §12.5.5 (Algorithmus „Appearance streams“, Matrix A) **nicht
//! proportional** auf das `/Rect` ab: Der Block wird auf die Feldgröße gestreckt bzw. gestaucht. Beleg:
//! `testdata/pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf` – PDF-AS schreibt dort einen 232 × 84 pt
//! großen Block in das 3,16 × 224 pt schmale Feld `Signature_4`; in einem seitengroßen Feld
//! (`manySignFields.pdf`, `Signature_7` mit `/Rect [0 0 612 792]`) füllte der Block die ganze Seite.
//!
//! **Korrigiert** ([`crate::CompatMode::Corrected`]): Der Block steht **proportional**, **höchstens in seiner
//! natürlichen Größe** und **mittig** im Feld ([`field_fit`]); `pida-pades` erweitert dazu nur die Holder-`/BBox`
//! (KR-P10), `/Rect` bleibt. Die Oberfläche zeigt den Block genau in [`FieldFit::rect`] (Vorschau = Ergebnis).
//!
//! **Als Ziel angeboten** (Einrasten beim Ziehen, „Hier signieren“, Hinweisleiste) wird ein Feld nur, wenn der
//! Block darin lesbar bleibt und es nicht viel größer ist als der Block ([`FieldFit::suitable`]):
//! Maßstab ≥ [`FIELD_MIN_SCALE`] und Anteil des Blocks an der Feldfläche ≥ [`FIELD_MIN_FILL`]. Ein 3 pt schmales
//! oder seitengroßes Feld ist kein plausibles Ziel. PDF-AS und PDF-Over prüfen die Geometrie nicht
//! (`SignatureFieldsAndPlaceHolderExtractor.findEmptySignatureFields` listet nur Namen).
//!
//! Koordinaten: beliebig, aber für Feld und Block **dieselben** – `pida-pades` rechnet im Benutzerraum der Seite
//! (Blockmaße dort bei 90°/270° vertauscht), die Oberfläche in `PAGE_ABS` der sichtbaren Seite; beide Wege ergeben
//! dasselbe Rechteck (Drehung und Verschiebung erhalten Maßstab und Mitte).

use pida_pdf_cos::{PageInfo, Rect, SigField};

use crate::position::{BlockGeometry, user_rect_to_visible};

/// Kleinster Maßstab, mit dem ein Feld als Ziel angeboten wird: Der Block wird höchstens auf ein Drittel
/// verkleinert – darunter ist die Schrift des Blocks kaum noch lesbar.
pub const FIELD_MIN_SCALE: f32 = 1.0 / 3.0;

/// Kleinster Anteil des (eingepassten) Blocks an der Feldfläche, damit ein Feld als Ziel angeboten wird: Ein Feld,
/// das mehr als fünfmal so groß ist wie der Block (etwa eine ganze Seite), ist kein plausibles Ziel.
pub const FIELD_MIN_FILL: f32 = 0.2;

/// Lage des Blocks in einem vorhandenen Signaturfeld ([`field_fit`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldFit {
    /// Rechteck, in dem der Block im Feld steht (proportional, höchstens natürliche Größe, mittig; im
    /// Koordinatenraum des Felds)
    pub rect: Rect,
    /// Maßstab gegenüber der natürlichen Blockgröße (`0 < scale ≤ 1`; `0` bei leerem Feld bzw. Block)
    pub scale: f32,
    /// Anteil des Blocks an der Feldfläche (`0 … 1`)
    pub fill: f32,
    /// als Ziel geeignet: `scale ≥` [`FIELD_MIN_SCALE`] und `fill ≥` [`FIELD_MIN_FILL`]
    pub suitable: bool,
}

/// Block der natürlichen Größe `block` in das Feld `field`: proportional verkleinert (nie vergrößert) und mittig.
/// Leeres bzw. nicht endliches Feld oder ein Block ohne Fläche (unsichtbar) → `rect` = Feld, `scale` = 0, nicht
/// geeignet.
pub fn field_fit(field: Rect, block: BlockGeometry) -> FieldFit {
    let f = field.normalized();
    let (fw, fh) = (f.width(), f.height());
    let (bw, bh) = (block.width, block.height);
    let usable = [f.llx, f.lly, fw, fh, bw, bh].iter().all(|v| v.is_finite()) && fw > 0.0 && fh > 0.0;
    if !usable || bw <= 0.0 || bh <= 0.0 {
        return FieldFit { rect: f, scale: 0.0, fill: 0.0, suitable: false };
    }
    let scale = (fw / bw).min(fh / bh).min(1.0);
    let (w, h) = (bw * scale, bh * scale);
    let llx = f.llx + (fw - w) / 2.0;
    let lly = f.lly + (fh - h) / 2.0;
    let rect = Rect::new(llx, lly, llx + w, lly + h);
    let fill = ((w * h) / (fw * fh)).min(1.0);
    // Rundungstoleranz (f32): genau ein Drittel bzw. ein Fünftel gilt noch als geeignet
    let suitable = scale + THRESHOLD_EPS >= FIELD_MIN_SCALE && fill + THRESHOLD_EPS >= FIELD_MIN_FILL;
    FieldFit { rect, scale, fill, suitable }
}

/// Toleranz der Schwellen gegen Rundung in `f32`.
const THRESHOLD_EPS: f32 = 1e-4;

/// Felder, die der Block nicht überdecken soll (Überdeckungsprüfung „überdeckt ein Formular- oder Signaturfeld“):
/// signierte und leere, die für den Block `block` (sichtbare Maße) als Ziel taugen ([`FieldFit::suitable`] für
/// mindestens ein Widget). Ein leeres Feld, das kein plausibles Ziel ist (seitengroß, 3 pt schmal), wird nicht
/// angeboten und ist deshalb auch kein Hindernis: Sonst meldete jede Lage auf einer Seite mit seitengroßem Feld eine
/// Überdeckung, die „Freie Fläche suchen“ nicht beheben kann. Widgets ohne Seite zählen nicht.
pub fn obstacle_fields(pages: &[PageInfo], fields: &[SigField], block: BlockGeometry) -> Vec<SigField> {
    fields
        .iter()
        .filter(|f| {
            f.signed
                || f.widgets.iter().any(|w| {
                    let info = w.page_index.and_then(|p| pages.get(p));
                    info.is_some_and(|info| field_fit(user_rect_to_visible(info, w.rect), block).suitable)
                })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use pida_pdf_cos::{PageInfo, Rotation};

    use super::*;
    use crate::user_rect_to_visible;

    /// Block der PDF-Over-Standardkonfiguration (`SIGNATURBLOCK_SMALL_DE`, w = 230).
    const BLOCK: BlockGeometry = BlockGeometry { width: 230.0, height: 82.0 };

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    /// Felder aus `testdata/pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf` (identisch mit dem Dokument der Meldung).
    #[test]
    fn many_sign_fields_only_plausible_targets() {
        let fields = [
            ("Signature_0", Rect::new(35.655, 694.467, 183.239, 766.262), true),
            ("Signature_1", Rect::new(236.496, 750.458, 251.389, 765.359), false),
            ("Signature_2", Rect::new(224.31, 621.77, 612.0, 635.767), false),
            ("Signature_3", Rect::new(177.372, 424.447, 190.912, 670.084), false),
            ("Signature_4", Rect::new(349.779, 357.168, 352.938, 581.131), false),
            ("Signature_5", Rect::new(426.053, 454.7, 596.655, 604.611), true),
            ("Signature_6", Rect::new(324.053, 708.014, 383.177, 758.135), false),
            ("Signature_7", Rect::new(0.0, 0.0, 612.0, 792.0), false),
        ];
        for (name, rect, suitable) in fields {
            let fit = field_fit(rect, BLOCK);
            assert_eq!(fit.suitable, suitable, "{name}: {fit:?}");
            // immer im Feld, proportional, nie größer als der Block
            assert!(fit.rect.llx >= rect.llx - 1e-3 && fit.rect.urx <= rect.urx + 1e-3, "{name}");
            assert!(fit.rect.lly >= rect.lly - 1e-3 && fit.rect.ury <= rect.ury + 1e-3, "{name}");
            assert!(fit.scale <= 1.0 && fit.rect.width() <= BLOCK.width + 1e-3, "{name}");
            assert!(
                close(fit.rect.width() / fit.rect.height(), BLOCK.width / BLOCK.height),
                "{name}: Seitenverhältnis"
            );
        }
    }

    /// Seitengroßes Feld: Block in natürlicher Größe mittig auf der Seite (nicht über die ganze Seite gestreckt).
    #[test]
    fn page_sized_field_keeps_the_natural_size() {
        let fit = field_fit(Rect::new(0.0, 0.0, 612.0, 792.0), BLOCK);
        assert_eq!(fit.scale, 1.0);
        assert!(close(fit.rect.width(), 230.0) && close(fit.rect.height(), 82.0));
        assert!(close(fit.rect.llx, 191.0) && close(fit.rect.lly, 355.0), "mittig: {:?}", fit.rect);
        assert!(fit.fill < FIELD_MIN_FILL && !fit.suitable);
    }

    /// Kleines, aber übliches Feld: proportional verkleinert und mittig; der knappe Rand bleibt frei.
    #[test]
    fn small_field_scales_down_proportionally() {
        let field = Rect::new(35.655, 694.467, 183.239, 766.262);
        let fit = field_fit(field, BLOCK);
        let s = (183.239f32 - 35.655) / 230.0;
        assert!(close(fit.scale, s), "{fit:?}");
        assert!(close(fit.rect.llx, field.llx) && close(fit.rect.urx, field.urx), "füllt die Breite");
        let h = 82.0 * s;
        assert!(close(fit.rect.lly, field.lly + (field.height() - h) / 2.0), "senkrecht mittig");
        assert!(fit.suitable);
    }

    /// Grenzen: genau ein Drittel bzw. ein Fünftel ist noch geeignet; verkehrte Ecken werden normalisiert.
    #[test]
    fn thresholds_and_normalisation() {
        let third = field_fit(Rect::new(0.0, 0.0, 230.0 / 3.0, 82.0 / 3.0), BLOCK);
        assert!(third.suitable, "{third:?}");
        let narrow = field_fit(Rect::new(0.0, 0.0, 230.0 * 0.3, 82.0), BLOCK);
        assert!(!narrow.suitable, "{narrow:?}");
        let large = field_fit(Rect::new(0.0, 0.0, 230.0 * 2.5, 82.0 * 2.0), BLOCK);
        assert!(close(large.fill, 0.2) && large.suitable, "{large:?}");
        let larger = field_fit(Rect::new(0.0, 0.0, 230.0 * 2.6, 82.0 * 2.0), BLOCK);
        assert!(!larger.suitable, "{larger:?}");
        let flipped = field_fit(Rect::new(300.0, 200.0, 0.0, 0.0), BLOCK);
        assert_eq!(flipped, field_fit(Rect::new(0.0, 0.0, 300.0, 200.0), BLOCK));
    }

    /// Hindernisse: signierte Felder immer, leere nur als plausibles Ziel (das seitengroße nicht).
    #[test]
    fn only_signed_or_plausible_fields_are_obstacles() {
        let page = |index| PageInfo {
            index,
            object_id: (1, 0),
            media_box: Rect::new(0.0, 0.0, 612.0, 792.0),
            crop_box: Rect::new(0.0, 0.0, 612.0, 792.0),
            rotation: Rotation::R0,
            user_unit: 1.0,
            annots_indirect: None,
        };
        let pages = [page(0), page(1)];
        let field = |name: &str, page: Option<usize>, rect: Rect, signed: bool| SigField {
            qualified_name: name.into(),
            partial_name: Some(name.into()),
            field_id: (10, 0),
            depth: 0,
            widgets: vec![pida_pdf_cos::SigWidget { id: (11, 0), page_index: page, rect, annot_flags: 0 }],
            signed,
            value_id: None,
            has_lock: false,
            has_seed_value: false,
            field_flags: 0,
        };
        let fields = [
            field("passend", Some(0), Rect::new(35.655, 694.467, 183.239, 766.262), false),
            field("seitengroß", Some(1), Rect::new(0.0, 0.0, 612.0, 792.0), false),
            field("schmal", Some(0), Rect::new(349.779, 357.168, 352.938, 581.131), false),
            field("signiert, schmal", Some(0), Rect::new(349.779, 357.168, 352.938, 581.131), true),
            field("ohne Seite", None, Rect::new(35.655, 694.467, 183.239, 766.262), false),
        ];
        let names: Vec<String> =
            obstacle_fields(&pages, &fields, BLOCK).into_iter().map(|f| f.qualified_name).collect();
        assert_eq!(names, ["passend", "signiert, schmal"]);
    }

    #[test]
    fn empty_field_or_invisible_block_is_never_suitable() {
        let empty = field_fit(Rect::new(10.0, 10.0, 10.0, 40.0), BLOCK);
        assert_eq!((empty.scale, empty.suitable), (0.0, false));
        let invisible = field_fit(Rect::new(0.0, 0.0, 300.0, 100.0), BlockGeometry { width: 0.0, height: 0.0 });
        assert_eq!((invisible.scale, invisible.suitable), (0.0, false));
        let nan = field_fit(Rect::new(0.0, 0.0, f32::NAN, 100.0), BLOCK);
        assert!(!nan.suitable);
    }

    /// Benutzerraum (pades, Blockmaße bei 90°/270° vertauscht) und sichtbare Seite (Oberfläche) ergeben dasselbe
    /// Rechteck – auch mit CropBox-Ursprung.
    #[test]
    fn user_space_and_visible_space_agree_for_all_rotations() {
        for rotation in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            let page = PageInfo {
                index: 0,
                object_id: (1, 0),
                media_box: Rect::new(0.0, 0.0, 612.0, 792.0),
                crop_box: Rect::new(20.0, 30.0, 600.0, 780.0),
                rotation,
                user_unit: 1.0,
                annots_indirect: None,
            };
            let field = Rect::new(100.0, 200.0, 400.0, 260.0);
            let user_block =
                if rotation.swaps_axes() { BlockGeometry { width: BLOCK.height, height: BLOCK.width } } else { BLOCK };
            let in_user = user_rect_to_visible(&page, field_fit(field, user_block).rect);
            let in_visible = field_fit(user_rect_to_visible(&page, field), BLOCK).rect;
            for (a, b) in [
                (in_user.llx, in_visible.llx),
                (in_user.lly, in_visible.lly),
                (in_user.urx, in_visible.urx),
                (in_user.ury, in_visible.ury),
            ] {
                assert!(close(a, b), "{rotation:?}: {in_user:?} ≠ {in_visible:?}");
            }
        }
    }
}

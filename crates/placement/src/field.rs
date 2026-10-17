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
//! **Als Ziel angeboten** (Einrasten beim Ziehen, „Hier signieren“, Hinweisleiste) wird ein Feld nur, wenn es den
//! Block plausibel aufnimmt ([`field_suitable`]): Der Block wird höchstens auf [`FIELD_MIN_SCALE`] verkleinert, die
//! Form des Felds weicht nicht zu stark von der des Blocks ab ([`FIELD_MIN_SHAPE`]), und das Feld bedeckt höchstens
//! [`FIELD_MAX_PAGE_SHARE`] der Seite. Ein 3 pt schmales, ein winziges oder ein seitengroßes
//! Feld ist kein plausibles Ziel; übliche Felder sind es für alle Profile (Signaturblock 230 × 82 pt, Amtssignatur
//! 495 × 120 pt, „Nur Bildmarke“ ab etwa 40 × 40 pt) – auch das Platzhalterfeld der PDF-AS-Testdaten
//! (`placeholder_sigfield_and_qr.pdf`, 63,7 × 61,1 pt). PDF-AS und PDF-Over prüfen die Geometrie nicht
//! (`SignatureFieldsAndPlaceHolderExtractor.findEmptySignatureFields` listet nur Namen).
//!
//! Koordinaten: beliebig, aber für Feld, Block und Seite **dieselben** – `pida-pades` rechnet im Benutzerraum der
//! Seite (Blockmaße dort bei 90°/270° vertauscht), die Oberfläche in `PAGE_ABS` der sichtbaren Seite; beide Wege
//! ergeben dasselbe Rechteck (Drehung und Verschiebung erhalten Maßstab und Mitte).

use pida_pdf_cos::{PageInfo, Rect, SigField};

use crate::position::{BlockGeometry, user_rect_to_visible};

/// Kleinster Maßstab, mit dem ein Feld als Ziel angeboten wird: Der Block wird höchstens auf ein Fünftel
/// verkleinert. Der Signaturblock (82 pt hoch) passt so noch in ein Feld von einer Formularzeile (≈ 16 pt), die
/// Amtssignatur (495 × 120 pt) in ein übliches 150 × 50-pt-Feld; kleinere Felder (Kästchen, Haarlinien) nehmen den
/// Block nur noch als Strich bzw. Punkt auf.
pub const FIELD_MIN_SCALE: f32 = 0.2;

/// Kleinste Formähnlichkeit von Feld und Block ([`FieldFit::shape`], Verhältnis der Seitenverhältnisse): Ein Feld,
/// das mehr als achtmal so lang gestreckt ist wie der Block (etwa eine 13 × 245-pt-Leiste oder eine seitenhohe
/// Spalte), ist kein plausibles Ziel. Bewusst nicht der Flächenanteil des Blocks: Ein großzügiges Feld bleibt ein Ziel
/// (z. B. „Nur Bildmarke“ in einem 300 × 100-pt-Feld, Anteil 0,13); zu große Felder begrenzt
/// [`FIELD_MAX_PAGE_SHARE`]. Muss der Block verkleinert werden, ist die Formähnlichkeit gleich seinem Anteil an der
/// Feldfläche ([`FieldFit::fill`]).
pub const FIELD_MIN_SHAPE: f32 = 0.125;

/// Größter Anteil des Felds an der (sichtbaren) Seitenfläche: Ein Feld, das mehr als die halbe Seite bedeckt (etwa
/// `Signature_7` mit `/Rect [0 0 612 792]`), ist kein Ziel – es finge den Block beim Ziehen überall auf der Seite
/// ein, und seine Markierung „Hier signieren“ verdeckte die Seite.
pub const FIELD_MAX_PAGE_SHARE: f32 = 0.5;

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
    /// Formähnlichkeit: kleineres der Verhältnisse der Seitenverhältnisse von Feld und Block (`0 … 1`, `1` =
    /// gleiche Form)
    pub shape: f32,
}

/// Block der natürlichen Größe `block` in das Feld `field`: proportional verkleinert (nie vergrößert) und mittig.
/// Leeres bzw. nicht endliches Feld oder ein Block ohne Fläche (unsichtbar) → `rect` = Feld, `scale` = 0.
pub fn field_fit(field: Rect, block: BlockGeometry) -> FieldFit {
    let f = field.normalized();
    let (fw, fh) = (f.width(), f.height());
    let (bw, bh) = (block.width, block.height);
    let usable = [f.llx, f.lly, fw, fh, bw, bh].iter().all(|v| v.is_finite()) && fw > 0.0 && fh > 0.0;
    if !usable || bw <= 0.0 || bh <= 0.0 {
        return FieldFit { rect: f, scale: 0.0, fill: 0.0, shape: 0.0 };
    }
    let scale = (fw / bw).min(fh / bh).min(1.0);
    let (w, h) = (bw * scale, bh * scale);
    let llx = f.llx + (fw - w) / 2.0;
    let lly = f.lly + (fh - h) / 2.0;
    let rect = Rect::new(llx, lly, llx + w, lly + h);
    let fill = ((w * h) / (fw * fh)).min(1.0);
    let ratio = (fw / fh) / (bw / bh);
    let shape = ratio.min(1.0 / ratio);
    FieldFit { rect, scale, fill, shape }
}

/// Als Ziel angeboten (Einrasten beim Ziehen, „Hier signieren“, Hinweisleiste; Hindernis der Überdeckungsprüfung)?
/// `field` und `page` (Breite × Höhe der Seite, auf der das Feld liegt) im selben Raum, `block` in natürlicher
/// Größe dort: Maßstab ≥ [`FIELD_MIN_SCALE`], Formähnlichkeit ≥ [`FIELD_MIN_SHAPE`], Feldfläche höchstens
/// [`FIELD_MAX_PAGE_SHARE`] der Seite. Leeres Feld oder unsichtbarer Block → nie.
pub fn field_suitable(field: Rect, block: BlockGeometry, page: (f32, f32)) -> bool {
    let fit = field_fit(field, block);
    if fit.scale <= 0.0 {
        return false;
    }
    let f = field.normalized();
    let page_area = page.0 * page.1;
    // Rundungstoleranz (f32): genau ein Fünftel bzw. Achtel bzw. die halbe Seite gilt noch als geeignet
    let share_ok = !(page_area.is_finite() && page_area > 0.0)
        || f.width() * f.height() <= page_area * (FIELD_MAX_PAGE_SHARE + THRESHOLD_EPS);
    fit.scale + THRESHOLD_EPS >= FIELD_MIN_SCALE && fit.shape + THRESHOLD_EPS >= FIELD_MIN_SHAPE && share_ok
}

/// Toleranz der Schwellen gegen Rundung in `f32`.
const THRESHOLD_EPS: f32 = 1e-4;

/// Felder, die der Block nicht überdecken soll (Überdeckungsprüfung „überdeckt ein Formular- oder Signaturfeld“):
/// signierte und leere, die für den Block `block` (sichtbare Maße) als Ziel taugen ([`field_suitable`] für
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
                    info.is_some_and(|info| {
                        field_suitable(user_rect_to_visible(info, w.rect), block, info.visible_size())
                    })
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

    /// Letter (Seite des Dokuments der Meldung) und A4.
    const LETTER: (f32, f32) = (612.0, 792.0);
    const A4: (f32, f32) = (595.28, 841.89);

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
            // 59 × 50 pt wie das Platzhalterfeld der PDF-AS-Testdaten: Block auf gut ein Viertel verkleinert
            ("Signature_6", Rect::new(324.053, 708.014, 383.177, 758.135), true),
            ("Signature_7", Rect::new(0.0, 0.0, 612.0, 792.0), false),
        ];
        for (name, rect, suitable) in fields {
            let fit = field_fit(rect, BLOCK);
            assert_eq!(field_suitable(rect, BLOCK, LETTER), suitable, "{name}: {fit:?}");
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

    /// Seitengroßes Feld: Block in natürlicher Größe mittig auf der Seite (nicht über die ganze Seite gestreckt);
    /// kein Ziel, weil es die ganze Seite bedeckt – für jeden Block.
    #[test]
    fn page_sized_field_keeps_the_natural_size() {
        let page = Rect::new(0.0, 0.0, 612.0, 792.0);
        let fit = field_fit(page, BLOCK);
        assert_eq!(fit.scale, 1.0);
        assert!(close(fit.rect.width(), 230.0) && close(fit.rect.height(), 82.0));
        assert!(close(fit.rect.llx, 191.0) && close(fit.rect.lly, 355.0), "mittig: {:?}", fit.rect);
        for block in [BLOCK, AMTS_LETTER, LOGO] {
            assert!(!field_suitable(page, block, LETTER), "{block:?}");
        }
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
        assert!(field_suitable(field, BLOCK, LETTER));
    }

    /// Amtssignatur auf Letter (`pw − 100`) und „Nur Bildmarke“ (23 mm, Siegel 326 × 342 px) bzw. 15 mm.
    const AMTS_LETTER: BlockGeometry = BlockGeometry { width: 512.0, height: 120.0 };
    const AMTS: BlockGeometry = BlockGeometry { width: 495.0, height: 120.0 };
    const LOGO: BlockGeometry = BlockGeometry { width: 62.118, height: 65.167 };
    const LOGO_15MM: BlockGeometry = BlockGeometry { width: 40.51, height: 42.52 };

    /// Übliche Felder sind für jedes Profil ein Ziel, absurde für keines (Prüfung „bricht keine normalen Felder“).
    #[test]
    fn common_fields_are_targets_for_every_profile() {
        let r = |w: f32, h: f32| Rect::new(100.0, 100.0, 100.0 + w, 100.0 + h);
        let common = [
            ("150 × 50", r(150.0, 50.0)),
            ("230 × 50 (certified-docmdp-p2.pdf)", r(230.0, 50.0)),
            ("Platzhalter PDF-AS 63,7 × 61,1 (placeholder_sigfield_and_qr.pdf)", r(63.7, 61.1)),
            ("200 × 60", r(200.0, 60.0)),
            ("300 × 100", r(300.0, 100.0)),
            ("400 × 150", r(400.0, 150.0)),
        ];
        for (block_name, block, skip) in [
            ("Signaturblock", BLOCK, &[][..]),
            // die Amtssignatur (495 pt breit) passt nicht sinnvoll in das 64-pt-Platzhalterfeld (Maßstab 0,13)
            ("Amtssignatur", AMTS, &["Platzhalter PDF-AS 63,7 × 61,1 (placeholder_sigfield_and_qr.pdf)"][..]),
            ("Bildmarke 23 mm", LOGO, &[][..]),
            ("Bildmarke 15 mm", LOGO_15MM, &[][..]),
        ] {
            for (name, field) in common {
                let want = !skip.contains(&name);
                assert_eq!(
                    field_suitable(field, block, A4),
                    want,
                    "{block_name} in {name}: {:?}",
                    field_fit(field, block)
                );
            }
        }
        // Formularzeile 200 × 20 nimmt den Signaturblock auf (Maßstab 0,24), die 14 pt hohe Leiste nicht (0,17)
        assert!(field_suitable(r(200.0, 20.0), BLOCK, A4));
        assert!(!field_suitable(r(388.0, 14.0), BLOCK, A4));
        let absurd = [r(3.2, 224.0), r(13.5, 245.6), r(50.0, 800.0)];
        for block in [BLOCK, AMTS, LOGO, LOGO_15MM] {
            for field in absurd {
                assert!(!field_suitable(field, block, A4), "{block:?} in {field:?}");
            }
        }
    }

    /// Grenzen: genau ein Fünftel (Maßstab), ein Achtel (Form) und die halbe Seite sind noch geeignet; der
    /// Flächenanteil des Blocks zählt nicht; verkehrte Ecken werden normalisiert.
    #[test]
    fn thresholds_and_normalisation() {
        let big = (5000.0, 5000.0);
        let r = |w: f32, h: f32| Rect::new(0.0, 0.0, w, h);
        assert!(field_suitable(r(230.0 / 5.0, 82.0 / 5.0), BLOCK, big), "Maßstab genau ⅕");
        assert!(!field_suitable(r(230.0 * 0.19, 82.0), BLOCK, big), "Maßstab 0,19");
        // halbe Höhe (Maßstab ½, Block 115 pt breit): bis zur achtfachen Breite des Blocks
        assert!(field_suitable(r(115.0 * 8.0, 41.0), BLOCK, big), "Form genau ⅛");
        assert!(close(field_fit(r(115.0 * 8.0, 41.0), BLOCK).fill, 0.125), "verkleinert: Form = Anteil");
        assert!(!field_suitable(r(115.0 * 8.2, 41.0), BLOCK, big), "Form unter ⅛");
        // in natürlicher Größe auch bei kleinem Flächenanteil (0,105): gleiche Form
        let roomy = r(600.0, 300.0);
        assert!(field_fit(roomy, BLOCK).fill < 0.125 && field_suitable(roomy, BLOCK, A4));
        // gestreckt, obwohl der Block in natürlicher Größe hineinpasst: kein Ziel
        assert!(!field_suitable(r(50.0, 800.0), LOGO_15MM, A4));
        // Seitenanteil: genau die halbe Seite ja, mehr nicht
        assert!(field_suitable(r(A4.0, A4.1 / 2.0), BLOCK, A4));
        assert!(!field_suitable(r(A4.0, A4.1 * 0.51), BLOCK, A4));
        let flipped = field_fit(Rect::new(300.0, 200.0, 0.0, 0.0), BLOCK);
        assert_eq!(flipped, field_fit(Rect::new(0.0, 0.0, 300.0, 200.0), BLOCK));
        assert!(field_suitable(Rect::new(300.0, 200.0, 0.0, 0.0), BLOCK, A4));
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
        let empty = Rect::new(10.0, 10.0, 10.0, 40.0);
        assert_eq!(field_fit(empty, BLOCK).scale, 0.0);
        assert!(!field_suitable(empty, BLOCK, A4));
        let field = Rect::new(0.0, 0.0, 300.0, 100.0);
        let invisible = BlockGeometry { width: 0.0, height: 0.0 };
        assert_eq!(field_fit(field, invisible).scale, 0.0);
        assert!(!field_suitable(field, invisible, A4));
        assert!(!field_suitable(Rect::new(0.0, 0.0, f32::NAN, 100.0), BLOCK, A4));
        // Seite ohne Fläche (unbekannt): nur Maßstab und Form zählen
        assert!(field_suitable(field, BLOCK, (0.0, 0.0)));
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

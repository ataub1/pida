//! Block in vorhandenen Signaturfeldern (KR-P10, KR-F13; ADR 0004 Nachtrag 16) am Dokument der Meldung
//! (`manySignFields.pdf` aus den PDF-AS-Testdaten): PDF-AS streckt den Block auf das Feld (Beleg
//! `manySignFields_signed4.pdf`), korrigiert steht er proportional, höchstens in natürlicher Größe und mittig;
//! `/Rect` bleibt in beiden Fällen. Die Lage wird so geprüft, wie ein Viewer sie zeichnet (ISO 32000-1 §12.5.5:
//! transformierte Holder-BBox → `/Rect`).

mod common;

use common::*;
use pida_pades::{FieldTarget, PlanOptions, Visibility, analyze, cert_fields, empty_fields, plan_signature, prepare};
use pida_pdf_cos::{Limits, Object, Rect, SourcePdf};
use pida_placement::{BlockGeometry, CompatMode, field_fit};
use pida_sigblock::{Appearance, SigBlockInput};

const MANY: &str = "pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf";

fn transform(m: [f32; 6], r: Rect) -> Rect {
    let pts = [(r.llx, r.lly), (r.urx, r.lly), (r.llx, r.ury), (r.urx, r.ury)]
        .map(|(x, y)| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]));
    Rect::new(
        pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
        pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
        pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max),
        pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max),
    )
}

/// Wohin ein Viewer das Inner-Form (`[0 0 W+1 H+1]`) zeichnet.
fn viewer_block(holder_bbox: Rect, matrix: [f32; 6], inner_bbox: Rect, rect: Rect) -> Rect {
    let tb = transform(matrix, holder_bbox);
    let t = transform(matrix, inner_bbox);
    let r = rect.normalized();
    let (sx, sy) = (r.width() / tb.width(), r.height() / tb.height());
    Rect::new(
        r.llx + (t.llx - tb.llx) * sx,
        r.lly + (t.lly - tb.lly) * sy,
        r.llx + (t.urx - tb.llx) * sx,
        r.lly + (t.ury - tb.lly) * sy,
    )
}

fn drawn(app: &Appearance, rect: Rect) -> Rect {
    viewer_block(
        app.holder_bbox().expect("Holder-BBox"),
        app.holder_matrix(),
        Rect::new(0.0, 0.0, app.width + 1.0, app.height + 1.0),
        rect,
    )
}

fn close(a: Rect, b: Rect, tol: f32) -> bool {
    [(a.llx, b.llx), (a.lly, b.lly), (a.urx, b.urx), (a.ury, b.ury)].iter().all(|(x, y)| (x - y).abs() <= tol)
}

fn rect_of(o: &Object) -> Rect {
    let v: Vec<f32> = o.as_array().unwrap().iter().map(|x| x.as_float().unwrap()).collect();
    Rect::new(v[0], v[1], v[2], v[3]).normalized()
}

/// Beleg PDF-AS 4.4.2: In `Signature_4` (3,16 × 224 pt) steht eine Appearance mit `/BBox [232 84]` und
/// Einheitsmatrix, `/Rect` unverändert – ein Viewer streckt den Block auf das schmale Feld.
#[test]
fn pdfas_stretches_the_block_onto_the_field() {
    let src = open(&testdata().join("pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf"));
    let f = src.signature_fields().unwrap().into_iter().find(|f| f.qualified_name == "Signature_4").unwrap();
    assert!(f.signed);
    let w = &f.widgets[0];
    let wd = src.get_object(w.id).unwrap().as_dict().unwrap();
    let Object::Reference(n) = wd.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap() else { panic!("/N") };
    let holder = src.get_object(*n).unwrap().as_stream().unwrap();
    let bbox = rect_of(holder.dict.get(b"BBox").unwrap());
    assert!((bbox.width() - 232.0).abs() < 0.01 && (bbox.height() - 84.0).abs() < 0.01, "{bbox:?}");
    let rect = w.rect.normalized();
    assert!((rect.width() - 3.159).abs() < 0.01 && (rect.height() - 223.963).abs() < 0.01, "{rect:?}");
    // gezeichnet wird über das ganze Feld: x um 1/73 gestaucht, y um das 2,7-Fache gestreckt
    let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    assert!(close(viewer_block(bbox, identity, bbox, rect), rect, 1e-3));
}

/// Korrigiert (Standard): in jedem Feld proportional, höchstens natürliche Größe, mittig – `/Rect` bleibt, die
/// Breite ist die des Profils (wie PDF-AS); mit `CompatMode::PdfasQuirks` wie PDF-AS über das ganze Feld.
#[test]
fn plans_fit_the_block_into_every_field_and_quirks_stretch_like_pdfas() {
    let src = open(&testdata().join(MANY));
    let report = analyze(&src).unwrap();
    let profile = store().profile("SIGNATURBLOCK_SMALL_DE").unwrap();
    let signer = KeystoreSigner::open("test-rsa2048");
    let time = pida_pades::local_signing_time();
    let input = SigBlockInput::new(cert_fields(&signer.chain.signer), time);
    let fields = empty_fields(&report);
    assert_eq!(fields.len(), 8);
    for cand in fields {
        let target = FieldTarget::Existing {
            field_id: cand.field_id,
            widget_id: cand.widget_id,
            qualified_name: cand.qualified_name.clone(),
        };
        for compat in [CompatMode::Corrected, CompatMode::PdfasQuirks] {
            let opts = PlanOptions { field: target.clone(), compat, ..PlanOptions::new(time) };
            let planned =
                plan_signature(&src, &report, profile, &input, &opts, fonts(), &**store().resources(), None).unwrap();
            let Visibility::Visible(bp) = &planned.plan.visibility else { panic!("sichtbar") };
            assert_eq!(bp.rect, cand.rect, "{}: /Rect bleibt", cand.qualified_name);
            let app = &bp.appearance;
            assert!((app.width - 230.0).abs() < 0.01, "{}: Profilbreite, nicht Feldbreite", cand.qualified_name);
            let got = drawn(app, bp.rect);
            let want = match compat {
                CompatMode::Corrected => {
                    let (w, h) = app.user_size();
                    field_fit(cand.rect, BlockGeometry { width: w, height: h }).rect
                }
                CompatMode::PdfasQuirks => cand.rect.normalized(),
            };
            assert!(close(got, want, 0.01), "{} {compat:?}: {got:?} ≠ {want:?}", cand.qualified_name);
            if compat == CompatMode::Corrected {
                assert!(got.width() <= app.width + 0.01 && got.height() <= app.height + 0.01, "nie größer");
            }
        }
    }
}

/// Seitengroßes Feld der Meldung: signiert (Keystore, Modell A), Selbstprüfung bestanden, Block in natürlicher
/// Größe mittig auf Seite 2.
#[test]
fn page_sized_field_signs_with_the_natural_block_size() {
    let src = SourcePdf::open(std::fs::read(testdata().join(MANY)).unwrap(), &Limits::default()).unwrap();
    let report = analyze(&src).unwrap();
    let cand = empty_fields(&report).into_iter().find(|c| c.qualified_name == "Signature_7").unwrap();
    let profile = store().profile("SIGNATURBLOCK_SMALL_DE").unwrap();
    let mut signer = KeystoreSigner::open("test-rsa2048");
    let time = pida_pades::local_signing_time();
    let input = SigBlockInput::new(cert_fields(&signer.chain.signer), time);
    let opts = PlanOptions {
        field: FieldTarget::Existing {
            field_id: cand.field_id,
            widget_id: cand.widget_id,
            qualified_name: cand.qualified_name.clone(),
        },
        ..PlanOptions::new(time)
    };
    let planned = plan_signature(&src, &report, profile, &input, &opts, fonts(), &**store().resources(), None).unwrap();
    let Visibility::Visible(bp) = &planned.plan.visibility else { panic!("sichtbar") };
    let block = drawn(&bp.appearance, bp.rect);
    let (w, h) = (bp.appearance.width, bp.appearance.height);
    let want = Rect::new((612.0 - w) / 2.0, (792.0 - h) / 2.0, (612.0 + w) / 2.0, (792.0 + h) / 2.0);
    assert!(close(block, want, 0.01), "{block:?} ≠ {want:?}");
    let chain = signer.chain.clone();
    let prepared = prepare(&src, &planned.plan, Some(&chain)).unwrap();
    let signed = signer.sign(prepared).unwrap();
    let out = SourcePdf::open(signed.bytes(), &Limits::default()).unwrap();
    let f = out.signature_fields().unwrap().into_iter().find(|f| f.qualified_name == "Signature_7").unwrap();
    assert!(f.signed);
    assert_eq!(f.widgets[0].rect, cand.rect, "/Rect bleibt");
}

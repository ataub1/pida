//! Pixelvergleich per PDFium (nur mit `PIDA_PDFIUM_DIR`, sonst übersprungen, ADR 0001 Regel 6):
//! eigenes Inner-Form gegen das Inner-Form der PDF-AS-Referenzen (gleiche Seite, gleiche
//! Auflösung), dazu ein Rendertest von [`Appearance::to_standalone_pdf`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pdfium_render::prelude::*;
use pida_pdf_cos::{Dictionary, Object, ObjectId, Stream, lopdf};
use pida_sigblock::{
    Appearance, BuildOptions, CertFields, DirResources, Emblem, FontProvider, PdfaMode, ProfileStore, Resources,
    SigBlockInput,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(rel)
}

fn pdfium() -> Option<Pdfium> {
    let dir = std::env::var("PIDA_PDFIUM_DIR").ok()?;
    match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir)) {
        Ok(b) => Some(Pdfium::new(b)),
        Err(e) => {
            eprintln!("PDFium nicht ladbar ({e}) – Test übersprungen");
            None
        }
    }
}

/// Kopiert `root` mit allen erreichbaren Objekten aus `src` in `dst`.
fn deep_copy(
    src: &lopdf::Document,
    root: ObjectId,
    dst: &mut lopdf::Document,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> ObjectId {
    if let Some(id) = map.get(&root) {
        return *id;
    }
    let new_id = dst.new_object_id();
    map.insert(root, new_id);
    let obj = src.get_object(root).unwrap().clone();
    let copied = copy_obj(src, &obj, dst, map);
    dst.objects.insert(new_id, copied);
    new_id
}

fn copy_obj(
    src: &lopdf::Document,
    o: &Object,
    dst: &mut lopdf::Document,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> Object {
    match o {
        Object::Reference(id) => Object::Reference(deep_copy(src, *id, dst, map)),
        Object::Array(a) => Object::Array(a.iter().map(|x| copy_obj(src, x, dst, map)).collect()),
        Object::Dictionary(d) => Object::Dictionary(copy_dict(src, d, dst, map)),
        Object::Stream(s) => {
            let mut s2 = s.clone();
            s2.dict = copy_dict(src, &s.dict, dst, map);
            Object::Stream(s2)
        }
        other => other.clone(),
    }
}

fn copy_dict(
    src: &lopdf::Document,
    d: &Dictionary,
    dst: &mut lopdf::Document,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> Dictionary {
    let mut out = Dictionary::new();
    for (k, v) in d.iter() {
        out.set(k.clone(), copy_obj(src, v, dst, map));
    }
    out
}

/// Mini-PDF: Seite `w × h`, Formular unskaliert mit x-Versatz `dx` gezeichnet.
fn page_with_form(src: &lopdf::Document, form: ObjectId, dx: f32, w: f32, h: f32) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.7");
    let mut map = HashMap::new();
    let fid = deep_copy(src, form, &mut doc, &mut map);
    let content = format!("q 1 0 0 1 {dx} 0 cm /X Do Q\n");
    let cid = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), content.into_bytes())));
    let pages_id = doc.new_object_id();
    let mut xo = Dictionary::new();
    xo.set("X", Object::Reference(fid));
    let mut res = Dictionary::new();
    res.set("XObject", Object::Dictionary(xo));
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages_id));
    page.set("MediaBox", Object::Array(vec![0.into(), 0.into(), Object::Real(w), Object::Real(h)]));
    page.set("Resources", Object::Dictionary(res));
    page.set("Contents", Object::Reference(cid));
    let page_id = doc.add_object(Object::Dictionary(page));
    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages.set("Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut cat = Dictionary::new();
    cat.set("Type", Object::Name(b"Catalog".to_vec()));
    cat.set("Pages", Object::Reference(pages_id));
    let cat_id = doc.add_object(Object::Dictionary(cat));
    doc.trailer.set("Root", Object::Reference(cat_id));
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

/// Eigene Appearance als lopdf-Dokument (Objekte materialisiert) + ID des Inner-Forms.
fn appearance_doc(app: &Appearance) -> (lopdf::Document, ObjectId) {
    let mut doc = lopdf::Document::with_version("1.7");
    let mut next = 1u32;
    let m = app.materialize(&mut || {
        let id = (next, 0);
        next += 1;
        id
    });
    doc.max_id = next;
    for (id, o) in m.objects {
        doc.objects.insert(id, o);
    }
    (doc, m.inner_id)
}

fn render(pdfium: &Pdfium, pdf: &[u8], scale: f32) -> (usize, usize, Vec<u8>) {
    let doc = pdfium.load_pdf_from_byte_slice(pdf, None).unwrap();
    let page = doc.pages().get(0).unwrap();
    let cfg = PdfRenderConfig::new().scale_page_by_factor(scale);
    let bmp = page.render_with_config(&cfg).unwrap();
    (bmp.width() as usize, bmp.height() as usize, bmp.as_raw_bytes())
}

/// (Anteil Pixel mit Abweichung > 32 je Kanal, mittlere Abweichung, Anteil nicht-weißer Pixel)
fn diff(a: &(usize, usize, Vec<u8>), b: &(usize, usize, Vec<u8>)) -> (f64, f64, f64) {
    assert_eq!((a.0, a.1), (b.0, b.1), "Bildgröße");
    let px = a.2.len() / 4;
    let mut bad = 0usize;
    let mut sum = 0u64;
    let mut ink = 0usize;
    for (p, q) in a.2.as_chunks::<4>().0.iter().zip(b.2.as_chunks::<4>().0) {
        let d = (0..3).map(|i| p[i].abs_diff(q[i])).max().unwrap();
        sum += u64::from(d);
        if d > 32 {
            bad += 1;
        }
        if p[..3].iter().any(|&c| c < 200) {
            ink += 1;
        }
    }
    (bad as f64 / px as f64, sum as f64 / px as f64, ink as f64 / px as f64)
}

struct Case {
    file: &'static str,
    bbox: (f32, f32),
    marker: Option<&'static [u8]>,
    profile: &'static str,
    width: f32,
    cn: &'static str,
    date: &'static str,
    note: Option<&'static str>,
    logo: &'static str,
    centered: bool,
}

fn find_gold(c: &Case) -> (lopdf::Document, ObjectId, f32) {
    let doc = lopdf::Document::load_mem(&std::fs::read(testdata(c.file)).unwrap()).unwrap();
    let mut found = None;
    for (id, o) in &doc.objects {
        let Object::Stream(s) = o else { continue };
        let Ok(bbox) = s.dict.get(b"BBox").and_then(|b| b.as_array()) else { continue };
        let b: Vec<f32> = bbox.iter().map(|v| v.as_float().unwrap_or(0.0)).collect();
        if (b[0] - c.bbox.0).abs() < 0.01
            && (b[1] - c.bbox.1).abs() < 0.01
            && s.dict.get(b"Resources").is_ok_and(|r| {
                let r = match r {
                    Object::Reference(rid) => doc.get_object(*rid).unwrap(),
                    o => o,
                };
                r.as_dict().is_ok_and(|d| d.get(b"Font").is_ok())
            })
            && c.marker.is_none_or(|m| s.content.windows(m.len()).any(|w| w == m))
        {
            found = Some((*id, b[0]));
        }
    }
    let (id, bw) = found.expect("Referenzformular");
    (doc, id, bw)
}

fn build(c: &Case) -> Appearance {
    let builtin = ProfileStore::builtin().unwrap();
    let def = ProfileStore::load_dir(&testdata("sigblock/pdfas-default/cfg")).unwrap();
    let def_res: Arc<dyn Resources> = Arc::new(DirResources { workdir: testdata("sigblock/pdfas-default") });
    let (mut profile, res) = match builtin.get(c.profile) {
        Some(p) => (p.clone(), builtin.resources().clone()),
        None => (def.get(c.profile).unwrap().clone(), def_res),
    };
    if c.centered {
        profile.tables.get_mut("main").unwrap().style.as_mut().unwrap().halign = Some(pida_sigblock::HAlign::Center);
    }
    let logo = std::fs::read(testdata(&format!("sigblock/pdfas-default/images/{}", c.logo))).unwrap();
    let mut cert = CertFields::example();
    cert.subject = vec![("CN".into(), c.cn.into())];
    let mut input = SigBlockInput::new(cert, OffsetDateTime::parse(c.date, &Rfc3339).unwrap());
    input.note = c.note.map(str::to_string);
    input.emblem = Some(Arc::new(Emblem::decode_original(&logo).unwrap()));
    let opts = BuildOptions { width: c.width, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    Appearance::build(&profile, &input, &opts, &FontProvider::from_resources(res.clone()), &*res).unwrap()
}

#[test]
fn pixels_match_pdfas_references() {
    let Some(pdfium) = pdfium() else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – Pixelvergleich übersprungen");
        return;
    };
    let cases = [
        Case {
            file: "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf",
            bbox: (231.99998, 64.0),
            marker: None,
            profile: "SIGNATURBLOCK_SMALL_DE_NOTE",
            width: 230.0,
            cn: "Stephan Keller",
            date: "2021-07-20T01:42:56-07:00",
            note: Some("!simple_config.Note_Default!"),
            logo: "signatur-logo_de.png",
            centered: false,
        },
        Case {
            file: "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf",
            bbox: (227.0, 51.0),
            marker: Some(b"(Signatory)"),
            profile: "SIGNATURBLOCK_SMALL_EN",
            width: 225.0,
            cn: "Stephan Keller",
            date: "2021-07-20T01:42:29-07:00",
            note: None,
            logo: "signatur-logo_en.png",
            centered: false,
        },
        Case {
            file: "pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf",
            bbox: (231.99998, 84.0),
            marker: None,
            profile: "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA",
            width: 230.0,
            cn: "Alexander Marsalek",
            date: "2021-07-22T15:53:55+02:00",
            note: None,
            logo: "signatur-logo_en.png",
            centered: true,
        },
        Case {
            file: "pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf",
            bbox: (496.30396, 74.0),
            marker: None,
            profile: "SIGNATURBLOCK_DE",
            width: 595.303_95 - 100.0,
            cn: "Andreas Fitzek ECC",
            date: "2024-03-08T16:18:06+01:00",
            note: None,
            logo: "signatur-logo_de.png",
            centered: false,
        },
    ];
    for c in &cases {
        let (gdoc, gid, gbw) = find_gold(c);
        let app = build(c);
        // alte Referenzen: Tabelle bei x = 1
        let dx_gold = if (gbw - app.width - 2.0).abs() < 0.01 { -1.0 } else { 0.0 };
        let (w, h) = (app.width, app.height + 1.0);
        let gold_pdf = page_with_form(&gdoc, gid, dx_gold, w, h);
        let (adoc, aid) = appearance_doc(&app);
        let own_pdf = page_with_form(&adoc, aid, 0.0, w, h);
        let g = render(&pdfium, &gold_pdf, 4.0);
        let o = render(&pdfium, &own_pdf, 4.0);
        let (bad, mean, ink) = diff(&g, &o);
        eprintln!(
            "{}: {}×{} px, abweichend {:.4} %, mittel {:.3}, Tinte {:.2} %",
            c.profile,
            g.0,
            g.1,
            bad * 100.0,
            mean,
            ink * 100.0
        );
        assert!(ink > 0.02, "{}: Referenz leer?", c.profile);
        assert!(bad < 0.002, "{}: {:.4} % Pixel weichen ab", c.profile, bad * 100.0);
        assert!(mean < 1.0, "{}: mittlere Abweichung {mean}", c.profile);
    }

    // Mini-PDF der Vorschau: Größe W×H, weißer Hintergrund, Rahmen sichtbar
    let s = ProfileStore::builtin().unwrap();
    let mut cert = CertFields::example();
    cert.subject = vec![("CN".into(), "Max Mustermann".into())];
    let input = SigBlockInput::new(cert, OffsetDateTime::parse("2026-10-14T12:34:56+02:00", &Rfc3339).unwrap());
    let opts = BuildOptions { width: 230.0, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    let app = Appearance::build(
        s.get("SIGNATURBLOCK_SMALL_DE").unwrap(),
        &input,
        &opts,
        &FontProvider::builtin(),
        &**s.resources(),
    )
    .unwrap();
    let (w, h, px) = render(&pdfium, &app.to_standalone_pdf().unwrap(), 4.0);
    assert_eq!((w, h), (920, 196));
    let at = |x: usize, y: usize| &px[(y * w + x) * 4..(y * w + x) * 4 + 3];
    let inked = |x0: usize, x1: usize, y0: usize, y1: usize| {
        (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| at(x, y).iter().any(|&c| c < 160)).count()
    };
    // Logo (≈ 4,4…37,4 pt × 7,4…40,7 pt von oben) enthält Farbe
    assert!(inked(20, 145, 32, 160) > 500, "Logo fehlt");
    // Bereich zwischen den Wertezeilen ist weiß (Hintergrund), kein Zufallsinhalt
    assert!(at(880, 190).iter().all(|&c| c > 200), "Hintergrund nicht weiß");
    // Rahmenlinie links (x = 0) über die ganze Höhe sichtbar
    assert!((10..180).all(|y| at(0, y).iter().any(|&c| c < 250)), "linker Rahmen fehlt");
}

/// PDF/A: Bildmarke als `ICCBased` (eingebettetes sRGB-Profil) statt DeviceRGB – PDFium muss
/// dieselben Farben zeigen (das Profil ist sRGB, also bis auf Rundung neutral).
#[test]
fn pdfa_icc_logo_renders_like_device_rgb() {
    let Some(pdfium) = pdfium() else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – Pixelvergleich übersprungen");
        return;
    };
    let s = ProfileStore::builtin().unwrap();
    let p = s.get("SIGNATURBLOCK_SMALL_DE_PDFA").unwrap();
    let mut cert = CertFields::example();
    cert.subject = vec![("CN".into(), "Max Mustermann".into())];
    let input = SigBlockInput::new(cert, OffsetDateTime::parse("2026-10-14T12:34:56+02:00", &Rfc3339).unwrap());
    let render_mode = |pdfa| {
        let opts = BuildOptions { width: 230.0, rotation: 0, pdfa, preview_only: false };
        let app = Appearance::build(p, &input, &opts, &FontProvider::builtin(), &**s.resources()).unwrap();
        render(&pdfium, &app.to_standalone_pdf().unwrap(), 4.0)
    };
    let rgb = render_mode(PdfaMode::None);
    let icc = render_mode(PdfaMode::Part2);
    let (bad, mean, ink) = diff(&rgb, &icc);
    let max = rgb.2.iter().zip(&icc.2).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    eprintln!("ICCBased vs. DeviceRGB: abweichend {:.4} %, mittel {mean:.4}, max {max}", bad * 100.0);
    assert!(ink > 0.02, "Block leer?");
    assert_eq!(bad, 0.0);
    assert!(max <= 4 && mean < 0.1, "max {max}, mittel {mean}");
}

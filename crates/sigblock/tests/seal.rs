//! PIDA-Siegel als Standard-Bildmarke der kleinen Standardblöcke (ADR 0005): Asset, Vorrang eigener
//! Bildmarken, unveränderte Blockmaße gegenüber dem PDF-AS-Logo, Transparenz (SMask, PDF/A-1 auf
//! Weiß) und Sichtbarkeit im PDFium-Render (nur mit `PIDA_PDFIUM_DIR`).

use std::path::Path;
use std::sync::Arc;

use pdfium_render::prelude::*;
use pida_pdf_cos::{Dictionary, Object};
use pida_sigblock::assets::{PIDA_SEAL, PIDA_SEAL_PROFILES};
use pida_sigblock::{
    Appearance, BuildOptions, CertFields, Emblem, FontProvider, PdfaMode, ProfileOverrides, ProfileStore,
    SigBlockInput, SourceColor,
};
use time::macros::datetime;

const SEAL_PNG: &[u8] = include_bytes!("../assets/images/pida-seal.png");
/// Pixelmaße des Siegels: 2 × 326 × 342 (PDF-AS-Logo).
const SEAL_PX: (u32, u32) = (652, 684);

fn seal() -> Emblem {
    Emblem::decode_original(SEAL_PNG).unwrap()
}

fn input(cn: &str) -> SigBlockInput {
    let mut cert = CertFields::example();
    cert.subject = vec![("CN".into(), cn.into())];
    SigBlockInput::new(cert, datetime!(2026-10-15 09:30:00 +2))
}

fn build_with(store: &ProfileStore, id: &str, width: f32, pdfa: PdfaMode, inp: &SigBlockInput) -> Appearance {
    let opts = BuildOptions { width, rotation: 0, pdfa, preview_only: false };
    let res = store.resources();
    Appearance::build(store.profile(id).unwrap(), inp, &opts, &FontProvider::builtin(), &**res)
        .unwrap_or_else(|e| panic!("{id} w={width}: {e}"))
}

fn builtin() -> ProfileStore {
    ProfileStore::builtin().unwrap()
}

fn pdfas() -> ProfileStore {
    ProfileStore::builtin_pdfas().unwrap()
}

fn stream_bytes(o: &Object) -> &[u8] {
    match o {
        Object::Stream(s) => &s.content,
        other => panic!("kein Stream: {other:?}"),
    }
}

/// Bild-XObjects (ohne SMasks) der Appearance.
fn images(a: &Appearance) -> Vec<&Dictionary> {
    let smasks: Vec<usize> = a
        .objects
        .iter()
        .filter_map(|o| match o {
            Object::Stream(s) => s.dict.get(b"SMask").ok().and_then(|r| r.as_reference().ok()).map(|(i, _)| i as usize),
            _ => None,
        })
        .collect();
    a.objects
        .iter()
        .enumerate()
        .filter(|(i, _)| !smasks.contains(i))
        .filter_map(|(_, o)| match o {
            Object::Stream(s) if s.dict.get(b"Subtype").ok().and_then(|n| n.as_name().ok()) == Some(b"Image") => {
                Some(&s.dict)
            }
            _ => None,
        })
        .collect()
}

fn dims(d: &Dictionary) -> (i64, i64) {
    (d.get(b"Width").unwrap().as_i64().unwrap(), d.get(b"Height").unwrap().as_i64().unwrap())
}

#[test]
fn seal_asset_matches_design_master() {
    // Generator design/brand/tools/build.mjs schreibt beide Dateien; sie müssen gleich bleiben.
    let design = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/brand/pida-seal-sigblock.png");
    let bytes = std::fs::read(&design).unwrap_or_else(|e| panic!("{}: {e}", design.display()));
    assert!(bytes == SEAL_PNG, "assets/images/pida-seal.png weicht von design/brand/pida-seal-sigblock.png ab");
}

#[test]
fn seal_asset_geometry_and_colors() {
    let s = seal();
    // doppelte Pixelmaße von signatur-logo_de/en.png (326 × 342): gleiches Seitenverhältnis mit Faktor 2
    let de = Emblem::decode_original(include_bytes!("../assets/images/signatur-logo_de.png")).unwrap();
    assert_eq!((s.width, s.height), (2 * de.width, 2 * de.height));
    assert_eq!((s.width, s.height), SEAL_PX);
    assert!(s.has_alpha, "außerhalb des Kreises transparent");
    assert_eq!(s.source_color, SourceColor::RgbAlpha);
    let px = |x: u32, y: u32| {
        let i = ((y * s.width + x) * 4) as usize;
        [s.rgba[i], s.rgba[i + 1], s.rgba[i + 2], s.rgba[i + 3]]
    };
    let near = |p: [u8; 4], rgb: [u8; 3]| p[3] == 255 && p.iter().zip(rgb).all(|(&c, e)| c.abs_diff(e) <= 12);
    // Ecken und Rand oben/unten (16 px) voll transparent, Farbe Weiß (kein schwarzer Grund ohne Alpha)
    for (x, y) in [(0, 0), (651, 0), (0, 683), (651, 683), (326, 6), (326, 677)] {
        assert_eq!(px(x, y), [255, 255, 255, 0], "({x}, {y})");
    }
    // Siegel: 652 px = 40 Einheiten (16,3 px je Einheit), um 16 px nach unten versetzt (viewBox 40 × 40
    // mittig in 652 × 684 wie `DefaultEmblem` im Prototyp)
    let at = |ux: f32, uy: f32| px((ux * 16.3) as u32, (16.0 + uy * 16.3) as u32);
    assert!(near(at(20.0, 20.0), [255, 255, 255]), "Mitte weiß: {:?}", at(20.0, 20.0));
    assert!(near(at(2.5, 20.0), [0x3a, 0x41, 0x4d]), "Ring: {:?}", at(2.5, 20.0));
    assert!(near(at(17.6, 25.6), [0x1f, 0x26, 0x30]), "Häkchen: {:?}", at(17.6, 25.6));
    assert!(near(at(20.0, 30.85), [0xc8, 0x10, 0x2e]), "Rot oben: {:?}", at(20.0, 30.85));
    assert!(near(at(20.0, 32.05), [255, 255, 255]), "Weiß: {:?}", at(20.0, 32.05));
    assert!(near(at(20.0, 33.25), [0xc8, 0x10, 0x2e]), "Rot unten: {:?}", at(20.0, 33.25));
    // innerhalb des Kreises deckend
    assert_eq!(at(20.0, 4.0)[3], 255);
}

#[test]
fn seal_is_the_default_of_exactly_the_small_profiles_and_base_logo() {
    let b = builtin();
    let p = pdfas();
    assert!(b.problems().is_empty(), "{:?}", b.problems());
    assert_eq!(b.ids().collect::<Vec<_>>(), p.ids().collect::<Vec<_>>());
    for id in b.ids() {
        let (mut ours, theirs) = (b.get(id).unwrap().clone(), p.get(id).unwrap());
        if PIDA_SEAL_PROFILES.contains(&id) {
            assert_eq!(ours.values["SIG_LABEL"], PIDA_SEAL, "{id}");
            assert_eq!(ours.raw["value.SIG_LABEL"], PIDA_SEAL, "{id}");
            ours.values.insert("SIG_LABEL".into(), theirs.values["SIG_LABEL"].clone());
            ours.raw.insert("value.SIG_LABEL".into(), theirs.raw["value.SIG_LABEL"].clone());
        }
        // Amtssignatur: Platzhalter statt Demo-Bildmarke (Backlog Nr. 10); `_RECOMMENDED` erbt ihn
        if id.starts_with("AMTSSIGNATURBLOCK") {
            assert_eq!(ours.values["SIG_LABEL"], pida_sigblock::assets::AMTS_PLACEHOLDER, "{id}");
            ours.values.insert("SIG_LABEL".into(), theirs.values["SIG_LABEL"].clone());
            if let Some(raw) = theirs.raw.get("value.SIG_LABEL") {
                assert_eq!(ours.raw["value.SIG_LABEL"], pida_sigblock::assets::AMTS_PLACEHOLDER, "{id}");
                ours.raw.insert("value.SIG_LABEL".into(), raw.clone());
            }
        }
        // sonst alles gleich (Basisprofile)
        assert_eq!(&ours, theirs, "{id}");
    }
    assert_eq!(PIDA_SEAL_PROFILES.len(), 9);
    for id in PIDA_SEAL_PROFILES {
        assert!(id.starts_with("SIGNATURBLOCK_SMALL_") || id == "BASE_LOGO", "{id}");
        let theirs = &p.get(id).unwrap().values["SIG_LABEL"];
        assert!(theirs.starts_with("./images/signatur-logo_"), "{id}: PDF-AS {theirs}");
    }
    // Amtssignatur: neutrale Platzhalter-Bildmarke statt der Demo-Bildmarke der Behörde (Backlog Nr. 10)
    for id in ["AMTSSIGNATURBLOCK_DE_RECOMMENDED", "AMTSSIGNATURBLOCK_EN_RECOMMENDED"] {
        assert_eq!(b.get(id).unwrap().values["SIG_LABEL"], pida_sigblock::assets::AMTS_PLACEHOLDER, "{id}");
        assert_eq!(p.get(id).unwrap().values["SIG_LABEL"], "./images/amtsignatur-logo_demo.png", "{id}: PDF-AS");
    }
    assert_eq!(b.get("BASE_LOGO").unwrap().values["SIG_LABEL"], PIDA_SEAL);
    assert_eq!(p.get("BASE_LOGO").unwrap().values["SIG_LABEL"], "./images/signatur-logo_de.png");
    // Ressource auflösbar (auch über DirResources-Rückfall)
    let img = b.resources().image(PIDA_SEAL).unwrap();
    assert_eq!(*img, seal());
}

/// Gleiches Seitenverhältnis wie das PDF-AS-Logo (doppelte Pixelmaße) ⇒ derselbe Content-Stream des
/// Inner-Forms (Maße, Bildmatrix, Texte, Linien) für alle acht kleinen Profile, viele Breiten, mit/ohne
/// Hinweis und lange Namen, und für `BASE_LOGO` mit dem PDF-Over-Overlay (1 × 1 px ohne eigene
/// Bildmarke) von 5 bis 120 mm.
#[test]
fn block_dimensions_unchanged_against_pdfas_logo() {
    let (b, p) = (builtin(), pdfas());
    let long = "Univ.-Prof. Dipl.-Ing. Dr. techn. Maximiliane Mustermann-Langname von und zu Beispielhausen";
    let mut checked = 0;
    let same = |ours: &Appearance, theirs: &Appearance, what: &str| {
        assert_eq!((ours.width, ours.height), (theirs.width, theirs.height), "{what}");
        assert_eq!(
            stream_bytes(&ours.objects[ours.inner]),
            stream_bytes(&theirs.objects[theirs.inner]),
            "{what}: Inner-Form"
        );
        let (oi, ti) = (images(ours), images(theirs));
        assert_eq!((oi.len(), ti.len()), (1, 1), "{what}");
        assert_eq!(dims(oi[0]), (i64::from(SEAL_PX.0), i64::from(SEAL_PX.1)), "{what}");
        assert_eq!(dims(ti[0]), (326, 342), "{what}");
    };
    for id in PIDA_SEAL_PROFILES.into_iter().filter(|id| *id != "BASE_LOGO") {
        let pdfa = if id.ends_with("PDFA") { PdfaMode::Part1 } else { PdfaMode::None };
        for cn in ["Max Mustermann", long] {
            for note in [None, Some("Hinweis: ".repeat(30))] {
                let mut inp = input(cn);
                inp.note = note.clone();
                for width in [90.0, 150.0, 225.0, 230.0, 300.0, 495.30396, 595.0] {
                    let ours = build_with(&b, id, width, pdfa, &inp);
                    let theirs = build_with(&p, id, width, pdfa, &inp);
                    same(&ours, &theirs, &format!("{id} w={width} cn={cn}"));
                    checked += 1;
                }
            }
        }
    }
    // „nur Bildmarke“ wie PDF-Over ohne eigene Bildmarke (1 × 1 px ⇒ quadratisch), auch PDF/A
    for mm in [5.0, 23.0, 47.5, 120.0] {
        let ov = ProfileOverrides::base_logo(None, (1, 1), mm);
        let (ours_p, theirs_p) =
            (b.get("BASE_LOGO").unwrap().with_overrides(&ov), p.get("BASE_LOGO").unwrap().with_overrides(&ov));
        let width = ov.imagescaletofit.unwrap().0;
        for pdfa in [PdfaMode::None, PdfaMode::Part1, PdfaMode::Part3] {
            let opts = BuildOptions { width, rotation: 0, pdfa, preview_only: false };
            let inp = input("Max Mustermann");
            let ours = Appearance::build(&ours_p, &inp, &opts, &FontProvider::builtin(), &**b.resources()).unwrap();
            let theirs = Appearance::build(&theirs_p, &inp, &opts, &FontProvider::builtin(), &**p.resources()).unwrap();
            same(&ours, &theirs, &format!("BASE_LOGO {mm} mm {pdfa:?}"));
            checked += 1;
        }
    }
    assert_eq!(checked, 8 * 2 * 2 * 7 + 4 * 3);
}

fn stream_bytes_of<'a>(a: &'a Appearance, d: &Dictionary) -> &'a [u8] {
    a.objects
        .iter()
        .find_map(|o| match o {
            Object::Stream(s) if std::ptr::eq(&s.dict, d) => Some(&s.content[..]),
            _ => None,
        })
        .unwrap()
}

fn tiny_emblem() -> Arc<Emblem> {
    let rgba = [10u8, 120, 200, 255].repeat(24 * 12);
    Arc::new(Emblem::from_rgba(24, 12, rgba, [7; 32]).unwrap())
}

#[test]
fn own_emblem_takes_precedence_over_seal() {
    let b = builtin();
    // Einstellung/CLI `-e` über SigBlockInput::emblem
    let mut inp = input("Max Mustermann");
    inp.emblem = Some(tiny_emblem());
    let a = build_with(&b, "SIGNATURBLOCK_SMALL_DE_NOTE", 230.0, PdfaMode::None, &inp);
    let imgs = images(&a);
    assert_eq!(imgs.len(), 1);
    assert_eq!(dims(imgs[0]), (24, 12));
    // PDF-Over-Overlay `value.SIG_LABEL` (ProfileOverrides::sig_label)
    let ov = ProfileOverrides { sig_label: Some(tiny_emblem()), ..Default::default() };
    let prof = b.get("SIGNATURBLOCK_SMALL_EN").unwrap().with_overrides(&ov);
    let opts = BuildOptions { width: 230.0, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    let a =
        Appearance::build(&prof, &input("Max Mustermann"), &opts, &FontProvider::builtin(), &**b.resources()).unwrap();
    assert_eq!(dims(images(&a)[0]), (24, 12));
    // „nur Bildmarke“: eigene Bildmarke über das PDF-Over-Overlay geht vor, ohne sie das Siegel
    let own = ProfileOverrides::base_logo(Some(tiny_emblem()), (24, 12), 23.0);
    let prof = b.get("BASE_LOGO").unwrap().with_overrides(&own);
    let a = Appearance::build(&prof, &input("Max"), &opts, &FontProvider::builtin(), &**b.resources()).unwrap();
    assert_eq!(dims(images(&a)[0]), (24, 12));
    let logo = build_with(&b, "BASE_LOGO", 230.0, PdfaMode::None, &input("Max"));
    assert_eq!(dims(images(&logo)[0]), (i64::from(SEAL_PX.0), i64::from(SEAL_PX.1)));
    assert!(images(&logo)[0].get(b"SMask").is_ok(), "BASE_LOGO: Siegel mit SMask");
    // Amtssignatur: Platzhalter in doppelter Auflösung der Demo-Bildmarke (472 × 470 = 2 × 236 × 235) – gleiche
    // Blockmaße wie mit der Demo-Bildmarke von PDF-AS
    for id in ["AMTSSIGNATURBLOCK_DE_RECOMMENDED", "AMTSSIGNATURBLOCK_EN_RECOMMENDED"] {
        let amts = build_with(&b, id, 400.0, PdfaMode::None, &input("Max"));
        assert_eq!(dims(images(&amts)[0]), (472, 470), "{id}");
        let theirs = build_with(&pdfas(), id, 400.0, PdfaMode::None, &input("Max"));
        assert_eq!(dims(images(&theirs)[0]), (236, 235), "{id}: PDF-AS");
        assert_eq!((amts.width, amts.height), (theirs.width, theirs.height), "{id}: gleiche Blockmaße");
    }
}

/// Auch wenn ein Programm die Transparenz verwirft (PDF-AS `removeAlphaChannel`, hier mit
/// `pdfas_quirks` bei PDF/A-1), bleibt der Grund außerhalb des Kreises weiß.
#[test]
fn seal_without_alpha_shows_white_corners() {
    let b = builtin();
    let mut inp = input("Max Mustermann");
    inp.pdfas_quirks = true;
    let a = build_with(&b, "SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, PdfaMode::Part1, &inp);
    let imgs = images(&a);
    assert_eq!(imgs.len(), 1);
    assert!(imgs[0].get(b"SMask").is_err());
    let rgb = unpredict(&inflate(stream_bytes_of(&a, imgs[0])), SEAL_PX.0 as usize, 3);
    // gespeicherte RGB-Werte unverändert übernommen, Ecken weiß
    assert_eq!(rgb, seal().rgb());
    assert_eq!(&rgb[0..3], &[255, 255, 255]);
    assert_eq!(&rgb[rgb.len() - 3..], &[255, 255, 255]);
}

fn inflate(data: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).unwrap();
    out
}

/// PNG-Prädiktor (`/Predictor 15`) rückgängig machen.
fn unpredict(data: &[u8], width: usize, bpp: usize) -> Vec<u8> {
    let stride = width * bpp;
    let rows = data.len() / (stride + 1);
    let mut out = vec![0u8; rows * stride];
    for r in 0..rows {
        let f = data[r * (stride + 1)];
        for i in 0..stride {
            let x = data[r * (stride + 1) + 1 + i];
            let a = if i >= bpp { out[r * stride + i - bpp] } else { 0 };
            let b = if r > 0 { out[(r - 1) * stride + i] } else { 0 };
            let c = if r > 0 && i >= bpp { out[(r - 1) * stride + i - bpp] } else { 0 };
            let paeth = || {
                let p = i16::from(a) + i16::from(b) - i16::from(c);
                let (pa, pb, pc) = ((p - i16::from(a)).abs(), (p - i16::from(b)).abs(), (p - i16::from(c)).abs());
                if pa <= pb && pa <= pc {
                    a
                } else if pb <= pc {
                    b
                } else {
                    c
                }
            };
            out[r * stride + i] = match f {
                0 => x,
                1 => x.wrapping_add(a),
                2 => x.wrapping_add(b),
                3 => x.wrapping_add(((u16::from(a) + u16::from(b)) / 2) as u8),
                4 => x.wrapping_add(paeth()),
                _ => panic!("Filter {f}"),
            };
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------
// PDFium
// ------------------------------------------------------------------------------------------------

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

/// Rendert die Appearance (4 ×) und zählt im Bildbereich Pixel in Siegelfarben: Rot des Akzents,
/// dunkler Ring/Häkchen; außerhalb des Kreises (Ecke der Bildzelle) bleibt es weiß.
#[test]
fn seal_visible_in_pdfium_render() {
    let Some(pdfium) = pdfium() else {
        eprintln!("PIDA_PDFIUM_DIR nicht gesetzt – Rendertest übersprungen");
        return;
    };
    let b = builtin();
    for (id, pdfa) in [
        ("SIGNATURBLOCK_SMALL_DE", PdfaMode::None),
        ("SIGNATURBLOCK_SMALL_EN_NOTE", PdfaMode::None),
        ("SIGNATURBLOCK_SMALL_DE_PDFA", PdfaMode::Part1),
        ("SIGNATURBLOCK_SMALL_EN_NOTE_PDFA", PdfaMode::Part2),
    ] {
        let a = build_with(&b, id, 230.0, pdfa, &input("Max Mustermann"));
        let pdf = a.to_standalone_pdf().unwrap();
        let doc = pdfium.load_pdf_from_byte_slice(&pdf, None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let bmp = page.render_with_config(&PdfRenderConfig::new().scale_page_by_factor(4.0)).unwrap();
        let (w, h, px) = (bmp.width() as usize, bmp.height() as usize, bmp.as_raw_bytes());
        let at = |x: usize, y: usize| [px[(y * w + x) * 4], px[(y * w + x) * 4 + 1], px[(y * w + x) * 4 + 2]];
        // Bildspalte: 230/5,5 pt breit, Polsterung 4 pt ⇒ Bild ≈ x 4…37,8 pt
        let (x0, x1) = (16usize, 151usize);
        let mut red = 0;
        let mut dark = 0;
        for y in 0..h {
            for x in x0..x1 {
                let p = at(x, y);
                let (hi, lo) = (*p.iter().max().unwrap(), *p.iter().min().unwrap());
                // Rot (#c8102e): ein Kanal hoch, die anderen niedrig (Reihenfolge BGRA/RGBA egal)
                if hi > 150 && p.iter().filter(|&&c| c < 90).count() == 2 {
                    red += 1;
                }
                if hi < 100 && lo < 70 {
                    dark += 1;
                }
            }
        }
        eprintln!("{id} {pdfa:?}: {w}×{h} px, Rot {red}, dunkel {dark}");
        // Akzent 10 × 2,6 Einheiten bei ≈ 3,3 px je Einheit ≈ 280 px; Ring und Häkchen ≈ 2500 px
        assert!(red > 150, "{id}: roter Akzent fehlt ({red})");
        assert!(dark > 1500, "{id}: Ring/Häkchen fehlen ({dark})");
        // Bild ≈ 4,4…37,4 pt × 7,4…40,7 pt von oben: Ecke des Bilds (außerhalb des Kreises) weiß,
        // auch bei PDF/A-1 (auf Weiß komponiert) – kein schwarzer Grund
        let corner = at(22, 34);
        assert!(corner.iter().all(|&c| c > 245), "{id}: Bildecke nicht weiß {corner:?}");
    }
}

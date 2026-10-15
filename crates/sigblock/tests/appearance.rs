//! Appearance-Objektgraph (docs/analyse/03 §4.7): Holder/Inner, BBox, Matrix, Fonts (Type1,
//! Type0-Subset), Bilder (Predictor, SMask, PDF/A), lokale Referenzen, Mini-PDF (qpdf als Orakel).

use std::collections::HashMap;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;

use pida_pdf_cos::{Dictionary, Object, lopdf};
use pida_sigblock::{
    Appearance, BuildOptions, CertFields, Emblem, FontProvider, LOCAL_GENERATION, PdfaMode, ProfileStore,
    SigBlockInput, expr,
};
use skrifa::raw::{FontRef, TableProvider};
use time::macros::datetime;

fn input() -> SigBlockInput {
    let mut cert = CertFields::example();
    cert.subject = vec![("CN".into(), "Max Mustermann".into())];
    SigBlockInput::new(cert, datetime!(2026-10-14 12:34:56 +2))
}

fn build(id: &str, width: f32, rotation: i32, pdfa: PdfaMode, inp: &SigBlockInput) -> Appearance {
    let s = ProfileStore::builtin().unwrap();
    let opts = BuildOptions { width, rotation, pdfa, preview_only: false };
    Appearance::build(s.get(id).unwrap(), inp, &opts, &FontProvider::builtin(), &**s.resources()).unwrap()
}

fn dict(o: &Object) -> &Dictionary {
    match o {
        Object::Stream(s) => &s.dict,
        Object::Dictionary(d) => d,
        other => panic!("kein Dictionary: {other:?}"),
    }
}

fn local_index(o: &Object) -> usize {
    match o {
        Object::Reference((i, g)) if *g == LOCAL_GENERATION => *i as usize,
        other => panic!("keine lokale Referenz: {other:?}"),
    }
}

fn reals(o: &Object) -> Vec<f32> {
    o.as_array().unwrap().iter().map(|v| v.as_float().unwrap()).collect()
}

fn inflate(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).unwrap();
    out
}

fn stream_data(o: &Object) -> Vec<u8> {
    let Object::Stream(s) = o else { panic!() };
    if s.dict.get(b"Filter").is_ok() { inflate(&s.content) } else { s.content.clone() }
}

#[test]
fn holder_and_inner_form() {
    let a = build("SIGNATURBLOCK_SMALL_DE", 230.0, 0, PdfaMode::None, &input());
    assert_eq!((a.holder, a.inner, a.inner_name.as_str()), (0, 1, "FRM1"));
    let h = dict(&a.objects[0]);
    assert_eq!(stream_data(&a.objects[0]), b"q 1.0 0.0 0.0 1.0 0.0 0.0 cm /FRM1 Do Q");
    let bbox = reals(h.get(b"BBox").unwrap());
    assert!((bbox[0] - 230.99998).abs() < 1e-4 && bbox[1] == 50.0 && bbox[2] == 0.0 && bbox[3] == 0.0, "{bbox:?}");
    assert_eq!(reals(h.get(b"Matrix").unwrap()), vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let hres = h.get(b"Resources").unwrap().as_dict().unwrap();
    assert_eq!(local_index(hres.get(b"XObject").unwrap().as_dict().unwrap().get(b"FRM1").unwrap()), 1);
    let procset: Vec<&[u8]> =
        hres.get(b"ProcSet").unwrap().as_array().unwrap().iter().map(|n| n.as_name().unwrap()).collect();
    assert_eq!(procset, vec![&b"PDF"[..], b"Text", b"ImageC", b"ImageB", b"ImageI"]);
    let inner = dict(&a.objects[1]);
    assert_eq!(reals(inner.get(b"BBox").unwrap()), bbox);
    assert!(inner.get(b"Filter").is_err(), "Inner-Form unkomprimiert");
    assert!(inner.get(b"Matrix").is_err());
    let ires = inner.get(b"Resources").unwrap().as_dict().unwrap();
    assert!(ires.get(b"XObject").unwrap().as_dict().unwrap().get(b"Im1").is_ok());
    let fonts = ires.get(b"Font").unwrap().as_dict().unwrap();
    // F1 = Caption (Helvetica-Bold, nicht eingebettet), F2 = Werte (DejaVuSansMono, Type0)
    let f1 = dict(&a.objects[local_index(fonts.get(b"F1").unwrap())]);
    assert_eq!(f1.get(b"Subtype").unwrap().as_name().unwrap(), b"Type1");
    assert_eq!(f1.get(b"BaseFont").unwrap().as_name().unwrap(), b"Helvetica-Bold");
    assert_eq!(f1.get(b"Encoding").unwrap().as_name().unwrap(), b"WinAnsiEncoding");
    assert!(f1.get(b"FontDescriptor").is_err());
    let f2 = dict(&a.objects[local_index(fonts.get(b"F2").unwrap())]);
    assert_eq!(f2.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0");
    assert_eq!(f2.get(b"Encoding").unwrap().as_name().unwrap(), b"Identity-H");
    let base = String::from_utf8_lossy(f2.get(b"BaseFont").unwrap().as_name().unwrap()).to_string();
    assert!(base.ends_with("+DejaVuSansMono") && base.len() == "AAAAAA+DejaVuSansMono".len(), "{base}");
    assert!(a.tu_text.contains("Max Mustermann"));
}

#[test]
fn type0_subset_is_consistent() {
    let a = build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, PdfaMode::Part1, &input());
    let ires = dict(&a.objects[1]).get(b"Resources").unwrap().as_dict().unwrap();
    let fonts = ires.get(b"Font").unwrap().as_dict().unwrap();
    let mut decoded_text = String::new();
    let content = stream_data(&a.objects[1]);
    for (name, r) in fonts.iter() {
        let t0 = dict(&a.objects[local_index(r)]);
        assert_eq!(t0.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0", "PDF/A: alle Fonts eingebettet");
        let cid = dict(&a.objects[local_index(&t0.get(b"DescendantFonts").unwrap().as_array().unwrap()[0])]);
        assert_eq!(cid.get(b"Subtype").unwrap().as_name().unwrap(), b"CIDFontType2");
        let fd = dict(&a.objects[local_index(cid.get(b"FontDescriptor").unwrap())]);
        assert!((fd.get(b"Descent").unwrap().as_float().unwrap() + 235.839_84).abs() < 1e-3);
        assert!(fd.get(b"CIDSet").is_ok(), "CIDSet bei PDF/A-1");
        // FontFile2 ist ein gültiger TrueType-Font
        let ff = stream_data(&a.objects[local_index(fd.get(b"FontFile2").unwrap())]);
        let font = FontRef::new(&ff).expect("Subset parsebar");
        let n = font.maxp().unwrap().num_glyphs();
        assert!(n > 1 && n < 120, "Subset klein: {n}");
        // CIDToGIDMap: jede verwendete CID zeigt auf eine gültige neue GID
        let map = stream_data(&a.objects[local_index(cid.get(b"CIDToGIDMap").unwrap())]);
        // ToUnicode dekodiert den Text
        let tu = String::from_utf8(stream_data(&a.objects[local_index(t0.get(b"ToUnicode").unwrap())])).unwrap();
        let mut cmap: HashMap<u16, char> = HashMap::new();
        for line in tu.lines().filter(|l| l.starts_with('<') && l.matches('<').count() == 3) {
            let parts: Vec<u32> = line
                .split(['<', '>', ' '])
                .filter(|s| !s.is_empty())
                .map(|h| u32::from_str_radix(h, 16).unwrap())
                .collect();
            for (k, cidv) in (parts[0]..=parts[1]).enumerate() {
                cmap.insert(cidv as u16, char::from_u32(parts[2] + k as u32).unwrap());
                let ng = u16::from_be_bytes([map[cidv as usize * 2], map[cidv as usize * 2 + 1]]);
                assert!(ng > 0 && ng < n, "CID {cidv} → GID {ng}");
            }
        }
        // W-Array: DejaVuSansMono 602 je Zeichen
        let w = cid.get(b"W").unwrap().as_array().unwrap();
        assert!(!w.is_empty());
        let name = String::from_utf8_lossy(name).to_string();
        // Strings des Fonts aus dem Content-Stream dekodieren
        let text = String::from_utf8_lossy(&content).to_string();
        let mut cur = String::new();
        for line in text.lines() {
            if line.ends_with(" Tf") {
                cur = line.split(' ').next().unwrap().trim_start_matches('/').to_string();
            }
            if line.ends_with(" Tj") && cur == name && line.starts_with('<') {
                let hex = &line[1..line.find('>').unwrap()];
                for i in (0..hex.len()).step_by(4) {
                    let c = u16::from_str_radix(&hex[i..i + 4], 16).unwrap();
                    decoded_text.push(cmap[&c]);
                }
                decoded_text.push('|');
            }
        }
    }
    assert!(decoded_text.contains("Prüfinformation"), "{decoded_text}");
    assert!(decoded_text.contains("Informationen zur Prüfung der"), "{decoded_text}");
}

#[test]
fn cidset_removed_for_pdfa3() {
    let a = build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, PdfaMode::Part3, &input());
    for o in &a.objects {
        if let Ok(d) = o.as_dict()
            && d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"FontDescriptor")
        {
            assert!(d.get(b"CIDSet").is_err());
        }
    }
    // keine verwaisten Objekte: jedes Objekt außer dem Holder wird referenziert
    let mut referenced = vec![false; a.objects.len()];
    referenced[a.holder] = true;
    fn mark(o: &Object, r: &mut Vec<bool>) {
        match o {
            Object::Reference((i, g)) if *g == LOCAL_GENERATION => r[*i as usize] = true,
            Object::Array(v) => v.iter().for_each(|x| mark(x, r)),
            Object::Dictionary(d) => d.iter().for_each(|(_, v)| mark(v, r)),
            Object::Stream(s) => s.dict.iter().for_each(|(_, v)| mark(v, r)),
            _ => {}
        }
    }
    for o in &a.objects {
        mark(o, &mut referenced);
    }
    assert!(referenced.iter().all(|r| *r), "{referenced:?}");
}

fn images(a: &Appearance) -> Vec<&Dictionary> {
    a.objects
        .iter()
        .filter_map(|o| if let Object::Stream(s) = o { Some(&s.dict) } else { None })
        .filter(|d| d.get(b"Subtype").and_then(|t| t.as_name()).ok() == Some(b"Image"))
        .collect()
}

fn unpredict(data: &[u8], width: usize, bpp: usize) -> Vec<u8> {
    let stride = width * bpp;
    let mut out = vec![0u8; data.len() / (stride + 1) * stride];
    for r in 0..data.len() / (stride + 1) {
        let f = data[r * (stride + 1)];
        for i in 0..stride {
            let x = data[r * (stride + 1) + 1 + i];
            let a = if i >= bpp { out[r * stride + i - bpp] } else { 0 };
            let b = if r > 0 { out[(r - 1) * stride + i] } else { 0 };
            let c = if r > 0 && i >= bpp { out[(r - 1) * stride + i - bpp] } else { 0 };
            let p = |a: u8, b: u8, c: u8| {
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
                4 => x.wrapping_add(p(a, b, c)),
                _ => panic!("Filter {f}"),
            };
        }
    }
    out
}

#[test]
fn logo_image_is_lossless() {
    let a = build("SIGNATURBLOCK_SMALL_DE", 230.0, 0, PdfaMode::None, &input());
    let imgs = images(&a);
    assert_eq!(imgs.len(), 1, "Standardlogo ohne Alpha ⇒ keine SMask");
    let d = imgs[0];
    assert_eq!(d.get(b"Width").unwrap().as_i64().unwrap(), 326);
    assert_eq!(d.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceRGB");
    let parms = d.get(b"DecodeParms").unwrap().as_dict().unwrap();
    assert_eq!(parms.get(b"Predictor").unwrap().as_i64().unwrap(), 15);
    let obj = a.objects.iter().find(|o| matches!(o, Object::Stream(s) if std::ptr::eq(&s.dict, d))).unwrap();
    let rgb = unpredict(&stream_data(obj), 326, 3);
    let logo = Emblem::decode_original(include_bytes!("../assets/images/signatur-logo_de.png")).unwrap();
    assert_eq!(rgb, logo.rgb());
}

fn alpha_emblem() -> Arc<Emblem> {
    let mut rgba = Vec::new();
    for i in 0..(20 * 10) {
        rgba.extend_from_slice(&[200, 30, 30, if i % 2 == 0 { 0 } else { 255 }]);
    }
    Arc::new(Emblem::from_rgba(20, 10, rgba, [3; 32]).unwrap())
}

#[test]
fn alpha_emblem_smask_and_pdfa1_flattening() {
    let mut inp = input();
    inp.emblem = Some(alpha_emblem());
    let a = build("SIGNATURBLOCK_SMALL_DE", 230.0, 0, PdfaMode::None, &inp);
    let imgs = images(&a);
    assert_eq!(imgs.len(), 2);
    let main = imgs.iter().find(|d| d.get(b"SMask").is_ok()).expect("SMask");
    let smask = dict(&a.objects[local_index(main.get(b"SMask").unwrap())]);
    assert_eq!(smask.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceGray");
    // PDF/A-2/3: Transparenz erlaubt
    assert_eq!(images(&build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, PdfaMode::Part2, &inp)).len(), 2);
    // PDF/A-1: auf Weiß komponiert, keine SMask
    let p1 = build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, PdfaMode::Part1, &inp);
    let imgs = images(&p1);
    assert_eq!(imgs.len(), 1);
    assert!(imgs[0].get(b"SMask").is_err());
    let obj = p1.objects.iter().find(|o| matches!(o, Object::Stream(s) if std::ptr::eq(&s.dict, imgs[0]))).unwrap();
    let rgb = unpredict(&stream_data(obj), 20, 3);
    assert_eq!(&rgb[0..6], &[255, 255, 255, 200, 30, 30]);
}

fn icc_of<'a>(a: &'a Appearance, space: &Object) -> &'a Object {
    let arr = space.as_array().expect("[/ICCBased …]");
    assert_eq!(arr[0].as_name().unwrap(), b"ICCBased");
    &a.objects[local_index(&arr[1])]
}

/// PDF/A: Bildmarke und farbige Füllungen im eingebetteten sRGB-Profil (Plan §6.5), damit der Block
/// auch bei CMYK-OutputIntent konform ist; ohne PDF/A und mit Quirks DeviceRGB wie PDF-AS.
#[test]
fn pdfa_uses_icc_based_srgb() {
    let mut inp = input();
    inp.emblem = Some(alpha_emblem());
    for pdfa in [PdfaMode::Part1, PdfaMode::Part2, PdfaMode::Part3] {
        let a = build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, pdfa, &inp);
        let mut icc_objects = Vec::new();
        for d in images(&a) {
            let cs = d.get(b"ColorSpace").unwrap();
            if cs.as_name().ok() == Some(b"DeviceGray") {
                assert_ne!(pdfa, PdfaMode::Part1, "PDF/A-1 ohne SMask");
                continue;
            }
            let icc = icc_of(&a, cs);
            let id = dict(icc);
            assert_eq!(id.get(b"N").unwrap().as_i64().unwrap(), 3);
            assert_eq!(id.get(b"Alternate").unwrap().as_name().unwrap(), b"DeviceRGB");
            let data = stream_data(icc);
            assert_eq!(&data[36..40], b"acsp");
            assert_eq!(&data[12..20], b"mntrRGB ");
            assert_eq!(data[8], 2, "ICC v2 (PDF/A-1-tauglich)");
            icc_objects.push(std::ptr::from_ref(icc));
        }
        assert_eq!(icc_objects.len(), 1, "{pdfa:?}");
    }
    // ohne PDF/A und mit Quirks: DeviceRGB
    let none = build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, PdfaMode::None, &inp);
    assert!(images(&none).iter().all(|d| d.get(b"ColorSpace").unwrap().as_name().is_ok()));
    inp.pdfas_quirks = true;
    let q = build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, PdfaMode::Part2, &inp);
    assert!(images(&q).iter().any(|d| d.get(b"ColorSpace").unwrap().as_name().ok() == Some(b"DeviceRGB")));
}

#[test]
fn pdfa_colored_background_uses_icc_color_space() {
    use pida_java_properties::Properties;
    let mut props = Properties::new();
    for (k, v) in [
        ("sig_obj.types.T", "on"),
        ("sig_obj.T.table.main.1", "SIG_NOTE-v"),
        ("sig_obj.T.table.main.Style.bgcolor", "255 0 0"),
        ("sig_obj.T.table.main.Style.font", "TTF:DejaVuSansCondensed-Bold.ttf,6"),
        ("sig_obj.T.table.main.Style.valuefont", "TTF:DejaVuSansMono.ttf,6"),
        ("sig_obj.T.value.SIG_NOTE", "Hinweis"),
    ] {
        props.insert(k, v);
    }
    let s = ProfileStore::from_properties(&props, Arc::new(pida_sigblock::BuiltinResources::default())).unwrap();
    let opts = BuildOptions { width: 100.0, rotation: 0, pdfa: PdfaMode::Part1, preview_only: false };
    let a =
        Appearance::build(s.get("T").unwrap(), &input(), &opts, &FontProvider::builtin(), &**s.resources()).unwrap();
    let content = String::from_utf8(stream_data(&a.objects[a.inner])).unwrap();
    assert!(content.starts_with("/CS1 cs\n1 0 0 sc\n0 1 100 8 re\nf\n0 g\n"), "{content}");
    let res = dict(&a.objects[a.inner]).get(b"Resources").unwrap().as_dict().unwrap();
    let cs = res.get(b"ColorSpace").unwrap().as_dict().unwrap().get(b"CS1").unwrap();
    assert_eq!(&stream_data(icc_of(&a, cs))[36..40], b"acsp");
    // ohne PDF/A: rg wie bisher, kein Farbraum-Eintrag
    let opts = BuildOptions { pdfa: PdfaMode::None, ..opts };
    let n =
        Appearance::build(s.get("T").unwrap(), &input(), &opts, &FontProvider::builtin(), &**s.resources()).unwrap();
    assert!(String::from_utf8(stream_data(&n.objects[n.inner])).unwrap().starts_with("1 0 0 rg\n"));
    assert!(dict(&n.objects[n.inner]).get(b"Resources").unwrap().as_dict().unwrap().get(b"ColorSpace").is_err());
}

/// Externes Orakel LittleCMS (Pillow `ImageCms`): Das eingebettete Profil ist gültig und bildet
/// sRGB-Werte auf sich selbst ab. Ohne `python3` mit Pillow übersprungen.
#[test]
fn icc_profile_is_srgb_according_to_littlecms() {
    let probe = Command::new("python3").args(["-I", "-c", "import PIL.ImageCms"]).output();
    if !probe.is_ok_and(|o| o.status.success()) {
        eprintln!("python3 mit Pillow fehlt – ICC-Orakel übersprungen");
        return;
    }
    let mut inp = input();
    inp.emblem = Some(alpha_emblem());
    let a = build("SIGNATURBLOCK_SMALL_DE_PDFA", 230.0, 0, PdfaMode::Part2, &inp);
    let img = images(&a).into_iter().find(|d| d.get(b"ColorSpace").unwrap().as_array().is_ok()).unwrap();
    let data = stream_data(icc_of(&a, img.get(b"ColorSpace").unwrap()));
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("srgb.icc");
    std::fs::write(&path, &data).unwrap();
    let script = r#"
import sys
from PIL import Image, ImageCms
p = ImageCms.getOpenProfile(sys.argv[1])
t = ImageCms.buildTransform(p, ImageCms.createProfile("sRGB"), "RGB", "RGB")
cols = [(0, 0, 0), (255, 255, 255), (255, 0, 0), (0, 255, 0), (0, 0, 255), (128, 128, 128), (200, 30, 30)]
img = Image.new("RGB", (len(cols), 1))
for i, c in enumerate(cols):
    img.putpixel((i, 0), c)
out = ImageCms.applyTransform(img, t)
print(max(abs(a - b) for i, c in enumerate(cols) for a, b in zip(c, out.getpixel((i, 0)))))
"#;
    let out = Command::new("python3").arg("-I").arg("-c").arg(script).arg(&path).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let max: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
    assert!(max <= 2, "sRGB-Abweichung {max}");
}

#[test]
fn rotation_and_preview_only() {
    let inp = input();
    let r = build("SIGNATURBLOCK_SMALL_DE", 230.0, 90, PdfaMode::None, &inp);
    assert_eq!(reals(dict(&r.objects[0]).get(b"Matrix").unwrap()), vec![0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);
    let s = ProfileStore::builtin().unwrap();
    let opts = BuildOptions { width: 230.0, rotation: 0, pdfa: PdfaMode::None, preview_only: true };
    let p = Appearance::build(
        s.get("SIGNATURBLOCK_SMALL_DE").unwrap(),
        &inp,
        &opts,
        &FontProvider::builtin(),
        &**s.resources(),
    )
    .unwrap();
    assert!(stream_data(&p.objects[0]).is_empty(), "Vorschau: Holder ohne Do");
    // Mini-PDF der Vorschau zeigt trotzdem die Tabelle (Inner-Form direkt)
    let pdf = p.to_standalone_pdf().unwrap();
    let doc = lopdf::Document::load_mem(&pdf).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
}

#[test]
fn invisible_profile_and_signer_name() {
    let s = ProfileStore::builtin().unwrap();
    let inv = s.get("INVISIBLE").unwrap();
    let opts = BuildOptions { width: 230.0, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    let a = Appearance::build(inv, &input(), &opts, &FontProvider::builtin(), &**s.resources()).unwrap();
    assert_eq!(a, Appearance::invisible());
    let bbox = dict(&a.objects[0]).get(b"BBox").unwrap().as_array().unwrap().clone();
    assert_eq!(bbox.iter().map(|o| o.as_i64().unwrap()).collect::<Vec<_>>(), vec![0, 0, 0, 0]);
    assert_eq!(expr::signer_name(inv, &input()).unwrap(), None);
    let mut inp = input();
    inp.cert.subject.push(("title".into(), "Mag.".into()));
    assert_eq!(
        expr::signer_name(s.get("SIGNATURBLOCK_SMALL_DE").unwrap(), &inp).unwrap().as_deref(),
        Some("Mag. Max Mustermann")
    );
}

#[test]
fn materialize_replaces_all_local_references() {
    let a = build("AMTSSIGNATURBLOCK_DE_RECOMMENDED", 495.0, 0, PdfaMode::None, &input());
    let mut next = 100u32;
    let m = a.materialize(&mut || {
        next += 1;
        (next, 0)
    });
    assert_eq!(m.objects.len(), a.objects.len());
    assert_eq!(m.holder_id, (101, 0));
    assert_eq!(m.inner_id, (102, 0));
    assert_eq!(m.inner_name, "FRM1");
    fn walk(o: &Object, ids: &[u32]) {
        match o {
            Object::Reference((i, g)) => {
                assert_ne!(*g, LOCAL_GENERATION);
                assert!(ids.contains(i), "Referenz {i} zeigt ins Leere");
            }
            Object::Array(a) => a.iter().for_each(|x| walk(x, ids)),
            Object::Dictionary(d) => d.iter().for_each(|(_, v)| walk(v, ids)),
            Object::Stream(s) => s.dict.iter().for_each(|(_, v)| walk(v, ids)),
            _ => {}
        }
    }
    let ids: Vec<u32> = m.objects.iter().map(|(id, _)| id.0).collect();
    for (_, o) in &m.objects {
        walk(o, &ids);
    }
}

#[test]
fn all_pdfover_profiles_build() {
    let s = ProfileStore::builtin().unwrap();
    for id in pida_sigblock::PDFOVER_PROFILE_IDS {
        let mut p = s.get(id).unwrap().clone();
        let width = if id.starts_with("AMTS") {
            495.0
        } else if id == "BASE_LOGO" {
            let ov = pida_sigblock::ProfileOverrides::base_logo(None, (326, 342), 23.0);
            p = p.with_overrides(&ov);
            62.117_935
        } else {
            230.0
        };
        for pdfa in [PdfaMode::None, PdfaMode::Part1, PdfaMode::Part3] {
            let opts = BuildOptions { width, rotation: 0, pdfa, preview_only: false };
            let a = Appearance::build(&p, &input(), &opts, &FontProvider::builtin(), &**s.resources())
                .unwrap_or_else(|e| panic!("{id}/{pdfa:?}: {e}"));
            if id == "INVISIBLE" {
                assert!(a.layout.is_none());
                continue;
            }
            assert!(a.width > 0.0 && a.height > 0.0, "{id}");
            if pdfa != PdfaMode::None {
                // PDF/A: kein nicht eingebetteter Font
                for o in &a.objects {
                    if let Ok(d) = o.as_dict() {
                        assert_ne!(
                            d.get(b"Subtype").and_then(|t| t.as_name()).ok(),
                            Some(&b"Type1"[..]),
                            "{id}/{pdfa:?}"
                        );
                    }
                }
            }
            let pdf = a.to_standalone_pdf().unwrap();
            assert!(lopdf::Document::load_mem(&pdf).is_ok(), "{id}");
        }
    }
}

#[test]
fn debug_output_redacts_personal_data() {
    let mut inp = input();
    inp.cert.subject = vec![("CN".into(), "Erika Geheim".into())];
    inp.note = Some("Vertraulicher Hinweis".into());
    inp.sbp.insert("Akt".into(), "GZ-4711".into());
    let a = build("SIGNATURBLOCK_SMALL_DE_NOTE", 230.0, 0, PdfaMode::None, &inp);
    let mut next = 0u32;
    let m = a.materialize(&mut || {
        next += 1;
        (next, 0)
    });
    let ov = pida_sigblock::ProfileOverrides { sig_note: inp.note.clone(), ..Default::default() };
    let d = format!("{a:?} {:?} {inp:?} {:?} {m:?} {ov:?}", a.layout, expr::ExprContext::from_input(&inp));
    for secret in ["Erika", "Geheim", "Vertraulich", "GZ-4711"] {
        assert!(!d.contains(secret), "{secret} in Debug-Ausgabe: {d}");
    }
    assert!(d.contains("Akt"), "Schlüsselnamen bleiben sichtbar");
}

fn qpdf_available() -> bool {
    Command::new("qpdf").arg("--version").output().is_ok_and(|o| o.status.success())
}

#[test]
fn standalone_pdf_is_valid() {
    let tmp = tempfile::tempdir().unwrap();
    for (id, width, rot) in [
        ("SIGNATURBLOCK_SMALL_DE_NOTE", 230.0, 0),
        ("SIGNATURBLOCK_SMALL_EN_PDFA", 230.0, 270),
        ("AMTSSIGNATURBLOCK_EN_RECOMMENDED", 495.0, 90),
    ] {
        let a = build(id, width, rot, PdfaMode::None, &input());
        let pdf = a.to_standalone_pdf().unwrap();
        let doc = lopdf::Document::load_mem(&pdf).unwrap();
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        let page = doc.get_dictionary(page_id).unwrap();
        let mb = reals(page.get(b"MediaBox").unwrap());
        let (w, h) = if rot % 180 == 0 { (a.width, a.height) } else { (a.height, a.width) };
        assert!((mb[2] - w).abs() < 1e-3 && (mb[3] - h).abs() < 1e-3, "{id}: {mb:?}");
        if qpdf_available() {
            let path = tmp.path().join(format!("{id}.pdf"));
            std::fs::write(&path, &pdf).unwrap();
            let out = Command::new("qpdf").arg("--check").arg(&path).output().unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(out.status.success(), "{id}: qpdf --check: {stdout} {}", String::from_utf8_lossy(&out.stderr));
            assert!(stdout.contains("No syntax or stream encoding errors"), "{stdout}");
        } else {
            eprintln!("qpdf fehlt – Orakel übersprungen");
        }
    }
}

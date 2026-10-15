//! Golden-Vergleich gegen von PDF-AS signierte Referenz-PDFs (pdf-as-4 Testressourcen,
//! Kopien unter `testdata/pdf/pdfas/` bzw. `testdata/pdf/pdfas-suites/`; Ergebnisse und
//! Abweichungen in `testdata/sigblock/RESULTS.md`).
//!
//! Verglichen wird der Tokenstrom des Inner-Forms (`/FRM1`): Rechtecke, Rahmenlinien,
//! Linienbreite, Bildmatrix, Textblöcke (Font, Größe, Startpunkt, Zeilenvorschub, kodierte
//! Strings) – Zahlen mit ±0,01 pt. Farboperatoren werden ignoriert (PDF-AS: DeviceRGB, hier im
//! Standard DeviceGray; mit `pdfas_quirks` identisch, eigener Test). Ältere PDF-AS-Versionen
//! zeichneten die Tabelle bei x = 1 mit `/BBox [W+2 H+2]` (heute x = 0, `[W+1 H+1]`); dieser
//! Versatz wird aus der BBox erkannt und herausgerechnet.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pida_pdf_cos::{Dictionary, Object, lopdf};
use pida_sigblock::{
    Appearance, BuildOptions, CertFields, DirResources, Emblem, FontProvider, PdfaMode, Profile, ProfileStore,
    Resources, SigBlockInput,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const TOL: f32 = 0.01;

fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(rel)
}

// ------------------------------------------------------------------------------------------------
// Content-Stream-Tokenizer und Normalisierung
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f32),
    Name(String),
    Str(Vec<u8>),
    Op(String),
}

fn tokenize(s: &[u8]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut i = 0;
    let delim = |b: u8| b.is_ascii_whitespace() || b"/[]<>()".contains(&b);
    while i < s.len() {
        let b = s[i];
        if b.is_ascii_whitespace() {
            i += 1;
        } else if b == b'(' {
            let mut depth = 1;
            let mut j = i + 1;
            let mut v = Vec::new();
            while j < s.len() {
                let c = s[j];
                if c == b'\\' {
                    let n = s[j + 1];
                    v.push(match n {
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        b'b' => 8,
                        b'f' => 12,
                        other => other,
                    });
                    j += 2;
                    continue;
                }
                if c == b'(' {
                    depth += 1;
                }
                if c == b')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                v.push(c);
                j += 1;
            }
            out.push(Tok::Str(v));
            i = j + 1;
        } else if b == b'<' {
            let j = i + s[i..].iter().position(|&c| c == b'>').unwrap();
            let hex: Vec<u8> = s[i + 1..j].iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
            let bytes =
                hex.chunks(2).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect();
            out.push(Tok::Str(bytes));
            i = j + 1;
        } else if b == b'/' {
            let mut j = i + 1;
            while j < s.len() && !delim(s[j]) {
                j += 1;
            }
            out.push(Tok::Name(String::from_utf8_lossy(&s[i + 1..j]).into()));
            i = j;
        } else {
            let mut j = i;
            while j < s.len() && !delim(s[j]) {
                j += 1;
            }
            let t = String::from_utf8_lossy(&s[i..j]).to_string();
            match t.parse::<f32>() {
                Ok(n) => out.push(Tok::Num(n)),
                Err(_) => out.push(Tok::Op(t)),
            }
            i = j.max(i + 1);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
enum G {
    Rect([f32; 4]),
    Line([f32; 4]),
    Width(f32),
    Image([f32; 6]),
    Text { font: String, size: f32, start: (f32, f32), steps: Vec<(f32, f32)>, strings: Vec<Vec<u8>> },
}

/// Textblock im Aufbau: Font, Größe, Start, Vorschübe, Strings.
type TextAcc = (String, f32, Option<(f32, f32)>, Vec<(f32, f32)>, Vec<Vec<u8>>);

/// Normalisiert den Tokenstrom; `fonts`: Ressourcenname → BaseFont ohne Subset-Präfix.
fn normalize(content: &[u8], fonts: &HashMap<String, String>) -> Vec<G> {
    let mut out = Vec::new();
    let mut nums: Vec<f32> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut strs: Vec<Vec<u8>> = Vec::new();
    let mut pending_move: Option<(f32, f32)> = None;
    let mut cm: Option<[f32; 6]> = None;
    let mut text: Option<TextAcc> = None;
    for t in tokenize(content) {
        match t {
            Tok::Num(n) => nums.push(n),
            Tok::Name(n) => names.push(n),
            Tok::Str(s) => strs.push(s),
            Tok::Op(op) => {
                match op.as_str() {
                    "re" => out.push(G::Rect([nums[0], nums[1], nums[2], nums[3]])),
                    "m" => pending_move = Some((nums[0], nums[1])),
                    "l" => {
                        let (x, y) = pending_move.take().expect("l ohne m");
                        out.push(G::Line([x, y, nums[0], nums[1]]));
                    }
                    "w" => out.push(G::Width(nums[0])),
                    "cm" => cm = Some([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]),
                    "Do" => {
                        if let Some(m) = cm.take() {
                            out.push(G::Image(m));
                        }
                    }
                    "BT" => text = Some((String::new(), 0.0, None, Vec::new(), Vec::new())),
                    "Tf" => {
                        let t = text.as_mut().unwrap();
                        t.0 = fonts.get(&names[0]).cloned().unwrap_or_else(|| names[0].clone());
                        t.1 = nums[0];
                    }
                    "Td" => {
                        let t = text.as_mut().unwrap();
                        if t.2.is_none() {
                            t.2 = Some((nums[0], nums[1]));
                        } else {
                            t.3.push((nums[0], nums[1]));
                        }
                    }
                    "Tj" => text.as_mut().unwrap().4.push(strs[0].clone()),
                    "ET" => {
                        let (font, size, start, steps, strings) = text.take().unwrap();
                        out.push(G::Text { font, size, start: start.unwrap_or((0.0, 0.0)), steps, strings });
                    }
                    // Farben, Füllen, Strich, q/Q: nicht verglichen
                    _ => {}
                }
                nums.clear();
                names.clear();
                strs.clear();
            }
        }
    }
    out
}

fn shifted(g: &G, dx: f32) -> G {
    match g {
        G::Rect([x, y, w, h]) => G::Rect([x + dx, *y, *w, *h]),
        G::Line([a, b, c, d]) => G::Line([a + dx, *b, c + dx, *d]),
        G::Image([a, b, c, d, e, f]) => G::Image([*a, *b, *c, *d, e + dx, *f]),
        G::Text { font, size, start, steps, strings } => G::Text {
            font: font.clone(),
            size: *size,
            start: (start.0 + dx, start.1),
            steps: steps.clone(),
            strings: strings.clone(),
        },
        G::Width(w) => G::Width(*w),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOL
}

fn same(a: &G, b: &G) -> bool {
    let all = |x: &[f32], y: &[f32]| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| close(*p, *q));
    match (a, b) {
        (G::Rect(x), G::Rect(y)) | (G::Line(x), G::Line(y)) => all(x, y),
        (G::Width(x), G::Width(y)) => close(*x, *y),
        (G::Image(x), G::Image(y)) => all(x, y),
        (
            G::Text { font: f1, size: s1, start: p1, steps: st1, strings: t1 },
            G::Text { font: f2, size: s2, start: p2, steps: st2, strings: t2 },
        ) => {
            f1 == f2
                && close(*s1, *s2)
                && close(p1.0, p2.0)
                && close(p1.1, p2.1)
                && st1.len() == st2.len()
                && st1.iter().zip(st2).all(|(p, q)| close(p.0, q.0) && close(p.1, q.1))
                && t1 == t2
        }
        _ => false,
    }
}

// ------------------------------------------------------------------------------------------------
// Inner-Forms aus Referenz-PDFs
// ------------------------------------------------------------------------------------------------

struct Form {
    bbox: Vec<f32>,
    content: Vec<u8>,
    fonts: HashMap<String, String>,
}

fn strip_subset(name: &str) -> String {
    match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.bytes().all(|b| b.is_ascii_uppercase()) => rest.to_string(),
        _ => name.to_string(),
    }
}

fn deref<'a>(doc: &'a lopdf::Document, o: &'a Object) -> &'a Object {
    match o {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(o),
        _ => o,
    }
}

fn font_names(doc: &lopdf::Document, res: &Dictionary) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(f) = res.get(b"Font")
        && let Ok(fd) = deref(doc, f).as_dict()
    {
        for (k, v) in fd.iter() {
            if let Ok(d) = deref(doc, v).as_dict() {
                let base = d.get(b"BaseFont").and_then(|b| b.as_name()).map(|b| String::from_utf8_lossy(b).to_string());
                map.insert(String::from_utf8_lossy(k).to_string(), strip_subset(&base.unwrap_or_default()));
            }
        }
    }
    map
}

fn inner_forms(path: &Path) -> Vec<Form> {
    let doc = lopdf::Document::load_mem(&std::fs::read(path).unwrap()).unwrap();
    let mut forms = Vec::new();
    for obj in doc.objects.values() {
        let Object::Stream(s) = obj else { continue };
        if s.dict.get(b"Subtype").and_then(|o| o.as_name()).ok() != Some(b"Form") {
            continue;
        }
        let Ok(res) = s.dict.get(b"Resources") else { continue };
        let Ok(res) = deref(&doc, res).as_dict() else { continue };
        if res.get(b"Font").is_err() {
            continue;
        }
        let bbox = s
            .dict
            .get(b"BBox")
            .and_then(|b| b.as_array())
            .map(|a| a.iter().map(|v| v.as_float().unwrap_or(0.0)).collect())
            .unwrap_or_default();
        let content = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
        forms.push(Form { bbox, content, fonts: font_names(&doc, res) });
    }
    forms
}

fn own_form(app: &Appearance) -> Form {
    let Object::Stream(s) = &app.objects[app.inner] else { panic!("Inner-Form fehlt") };
    let res = s.dict.get(b"Resources").unwrap().as_dict().unwrap();
    let mut fonts = HashMap::new();
    if let Ok(fd) = res.get(b"Font").and_then(|f| f.as_dict()) {
        for (k, v) in fd.iter() {
            let Object::Reference((i, _)) = v else { panic!() };
            let d = app.objects[*i as usize].as_dict().unwrap();
            let base = d.get(b"BaseFont").unwrap().as_name().unwrap();
            fonts.insert(String::from_utf8_lossy(k).to_string(), strip_subset(&String::from_utf8_lossy(base)));
        }
    }
    Form { bbox: Vec::new(), content: s.content.clone(), fonts }
}

// ------------------------------------------------------------------------------------------------
// Fälle
// ------------------------------------------------------------------------------------------------

struct Case {
    name: &'static str,
    file: &'static str,
    /// BBox-Breite und -Höhe der Referenz (identifiziert das Formular)
    bbox: (f32, f32),
    profile: &'static str,
    width: f32,
    cn: &'static str,
    date: &'static str,
    note: Option<&'static str>,
    logo: &'static str,
    /// zusätzliches Erkennungsmerkmal im Content-Stream (mehrere Formulare gleicher Größe)
    marker: Option<&'static [u8]>,
    /// Abweichung der damals verwendeten Profilversion (siehe RESULTS.md)
    tweak: Option<fn(&mut Profile)>,
}

/// G4 entstand mit einer älteren Profilversion: Captions zentriert statt rechtsbündig.
fn captions_centered(p: &mut Profile) {
    let st = p.tables.get_mut("main").and_then(|t| t.style.as_mut()).unwrap();
    st.halign = Some(pida_sigblock::HAlign::Center);
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "G1 SMALL_DE_NOTE (w=230)",
            file: "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf",
            bbox: (231.99998, 64.0),
            profile: "SIGNATURBLOCK_SMALL_DE_NOTE",
            width: 230.0,
            cn: "Stephan Keller",
            date: "2021-07-20T01:42:56-07:00",
            note: Some("!simple_config.Note_Default!"),
            logo: "signatur-logo_de.png",
            marker: None,
            tweak: None,
        },
        Case {
            name: "G2 SMALL_DE (QR-Platzhalter, w=225)",
            file: "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf",
            bbox: (227.0, 51.0),
            profile: "SIGNATURBLOCK_SMALL_DE",
            width: 225.0,
            cn: "Stephan Keller",
            date: "2021-07-20T01:39:53-07:00",
            note: None,
            logo: "signatur-logo_de.png",
            marker: None,
            tweak: None,
        },
        Case {
            name: "G3 SMALL_EN (QR-Platzhalter, w=225)",
            file: "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf",
            bbox: (227.0, 51.0),
            profile: "SIGNATURBLOCK_SMALL_EN",
            width: 225.0,
            cn: "Stephan Keller",
            date: "2021-07-20T01:42:29-07:00",
            note: None,
            logo: "signatur-logo_en.png",
            marker: Some(b"(Signatory)"),
            tweak: None,
        },
        Case {
            name: "G4 SMALL_EN_NOTE_PDFA (w=230)",
            file: "pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf",
            bbox: (231.99998, 84.0),
            profile: "SIGNATURBLOCK_SMALL_EN_NOTE_PDFA",
            width: 230.0,
            cn: "Alexander Marsalek",
            date: "2021-07-22T15:53:55+02:00",
            note: None,
            logo: "signatur-logo_en.png",
            marker: None,
            tweak: Some(captions_centered),
        },
        Case {
            name: "G5 SIGNATURBLOCK_DE (PDF-AS-Standardkonfiguration, A4 auto)",
            file: "pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf",
            bbox: (496.30396, 74.0),
            profile: "SIGNATURBLOCK_DE",
            // Seitenbreite 595.303937… (f32) − 2·50
            width: 595.303_95 - 100.0,
            cn: "Andreas Fitzek ECC",
            date: "2024-03-08T16:18:06+01:00",
            note: None,
            logo: "signatur-logo_de.png",
            marker: None,
            tweak: None,
        },
    ]
}

fn pdfas_default() -> (ProfileStore, Arc<dyn Resources>) {
    let cfg = testdata("sigblock/pdfas-default/cfg");
    let store = ProfileStore::load_dir(&cfg).unwrap();
    let res: Arc<dyn Resources> = Arc::new(DirResources { workdir: testdata("sigblock/pdfas-default") });
    (store, res)
}

fn build_case(c: &Case, quirks: bool) -> (Appearance, Profile) {
    let builtin = ProfileStore::builtin().unwrap();
    let (default_store, default_res) = pdfas_default();
    let (mut profile, resources): (Profile, Arc<dyn Resources>) = match builtin.get(c.profile) {
        Some(p) => (p.clone(), builtin.resources().clone()),
        None => (default_store.get(c.profile).unwrap().clone(), default_res.clone()),
    };
    if let Some(t) = c.tweak {
        t(&mut profile);
    }
    // Die Referenzen wurden mit dem 200×210-px-Logo der PDF-AS-Standardkonfiguration erzeugt.
    let logo = std::fs::read(testdata(&format!("sigblock/pdfas-default/images/{}", c.logo))).unwrap();
    let mut cert = CertFields::example();
    cert.subject = vec![("CN".into(), c.cn.into())];
    let t = OffsetDateTime::parse(c.date, &Rfc3339).unwrap();
    let mut input = SigBlockInput::new(cert, t);
    input.note = c.note.map(str::to_string);
    input.emblem = Some(Arc::new(Emblem::decode_original(&logo).unwrap()));
    input.pdfas_quirks = quirks;
    let opts = BuildOptions { width: c.width, rotation: 0, pdfa: PdfaMode::None, preview_only: false };
    let app = Appearance::build(&profile, &input, &opts, &FontProvider::from_resources(resources.clone()), &*resources)
        .unwrap_or_else(|e| panic!("{}: {e}", c.name));
    (app, profile)
}

fn find_form(c: &Case) -> Form {
    let forms = inner_forms(&testdata(c.file));
    forms
        .into_iter()
        .find(|f| {
            f.bbox.len() == 4
                && close(f.bbox[0], c.bbox.0)
                && close(f.bbox[1], c.bbox.1)
                && c.marker.is_none_or(|m| f.content.windows(m.len()).any(|w| w == m))
        })
        .unwrap_or_else(|| panic!("{}: Formular mit BBox {:?} nicht gefunden", c.name, c.bbox))
}

#[test]
fn golden_inner_forms_match_pdfas() {
    let mut report = String::new();
    for c in cases() {
        let gold = find_form(&c);
        let (app, _) = build_case(&c, false);
        // alt: Tabelle bei x = 1, BBox W+2; neu: x = 0, BBox W+1
        let dx_gold = if close(gold.bbox[0] - app.width, 2.0) { -1.0 } else { 0.0 };
        assert!(close(gold.bbox[0] + dx_gold, app.width + 1.0), "{}: BBox {} vs W {}", c.name, gold.bbox[0], app.width);
        assert!(close(gold.bbox[1] - app.height, if dx_gold != 0.0 { 2.0 } else { 1.0 }), "{}: H", c.name);
        let g: Vec<G> = normalize(&gold.content, &gold.fonts).iter().map(|x| shifted(x, dx_gold)).collect();
        let ours = normalize(&own_form(&app).content, &own_form(&app).fonts);
        assert_eq!(g.len(), ours.len(), "{}: Anzahl Operationen\nPDF-AS: {g:#?}\neigen: {ours:#?}", c.name);
        for (k, (a, b)) in g.iter().zip(&ours).enumerate() {
            assert!(same(a, b), "{}: Operation {k} weicht ab\nPDF-AS: {a:?}\neigen:  {b:?}", c.name);
        }
        let texts = g.iter().filter(|x| matches!(x, G::Text { .. })).count();
        let max = g.iter().zip(&ours).map(|(a, b)| max_dev(a, b)).fold(0f32, f32::max);
        // gleiche 5-Nachkommastellen-Ausgabe ⇒ nur Rundung beim Parsen bzw. Verschieben
        assert!(max < 1e-4, "{}: maximale Abweichung {max}", c.name);
        report.push_str(&format!(
            "{}: {} Operationen gleich ({} Textblöcke), dx = {}, max. Abweichung {max:e} pt\n",
            c.name,
            g.len(),
            texts,
            dx_gold
        ));
    }
    println!("{report}");
}

fn nums(g: &G) -> Vec<f32> {
    match g {
        G::Rect(v) | G::Line(v) => v.to_vec(),
        G::Width(w) => vec![*w],
        G::Image(m) => m.to_vec(),
        G::Text { size, start, steps, .. } => {
            let mut v = vec![*size, start.0, start.1];
            for s in steps {
                v.extend([s.0, s.1]);
            }
            v
        }
    }
}

fn max_dev(a: &G, b: &G) -> f32 {
    nums(a).iter().zip(nums(b)).map(|(x, y)| (x - y).abs()).fold(0f32, f32::max)
}

/// Mit `pdfas_quirks` ist auch der Content-Stream des neuen Formats (G5) byte-gleich – inklusive
/// Farboperatoren `/DeviceRGB cs … sc` und Zahlenformat.
#[test]
fn golden_g5_bytes_identical_with_quirks() {
    let c = cases().into_iter().find(|c| c.name.starts_with("G5")).unwrap();
    let gold = find_form(&c);
    let (app, _) = build_case(&c, true);
    let ours = own_form(&app);
    let gold_text = String::from_utf8_lossy(&gold.content).replace("\r\n", "\n");
    let ours_text = String::from_utf8_lossy(&ours.content).to_string();
    assert_eq!(gold_text.trim_end(), ours_text.trim_end());
}

/// Alte Referenzen (x = 1): nach Abzug des Versatzes ebenfalls byte-gleich bis auf die Zahlen
/// der x-Koordinaten.
#[test]
fn golden_old_format_tokens_with_quirks() {
    for c in cases().into_iter().filter(|c| !c.name.starts_with("G5")) {
        let gold = find_form(&c);
        let (app, _) = build_case(&c, true);
        let gold_ops: Vec<String> = tokenize(&gold.content)
            .into_iter()
            .filter_map(|t| if let Tok::Op(o) = t { Some(o) } else { None })
            .collect();
        let our_ops: Vec<String> = tokenize(&own_form(&app).content)
            .into_iter()
            .filter_map(|t| if let Tok::Op(o) = t { Some(o) } else { None })
            .collect();
        assert_eq!(gold_ops, our_ops, "{}: Operatorfolge", c.name);
    }
}

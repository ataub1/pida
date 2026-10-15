//! Golden-Vergleich gegen einen frischen PDF-AS-4.4.2-Lauf (Plan §8.2): alle 12 Profil-IDs von
//! PDF-Over × Eingaben × Positionen, Hinweis- und Bildmarken-Varianten.
//!
//! Die Referenzen erzeugt `tools/golden/run-pdfas.sh` (offizielle PDF-AS-CLI 4.4.2, Test-PKCS#12,
//! Keystore-Option `-c ks`); die Tests lesen nur `testdata/golden/pdfas-4.4.2/` (Manifeste mit den
//! Operationen des Inner-Forms, ausgewählte signierte PDFs). Bericht:
//! `docs/integration/golden-report.md`.
//!
//! Verglichen wird je Fall:
//! - Blockmaße (`/BBox` des Inner- und Holder-Forms, `/Matrix`), ±0,01 pt,
//! - Operationen des Inner-Forms: Rechtecke, Linien, Linienbreiten, Bildmatrizen, Textblöcke
//!   (Font ohne Subset-Präfix, Größe, Startpunkt jeder Zeile ±0,01 pt, kodierte Bytes exakt),
//! - Fonts (Typ, Einbettung, Descent, Breiten der verwendeten Glyphen), Bilder (Pixelmaße,
//!   Farbraum, SHA-256 der dekodierten Samples, SMask),
//! - mit `pdfas_quirks`: Content-Stream byte-gleich, `/TU` gleich, Holder-Inhalt gleich,
//! - Pixel (PDFium, 288 dpi): Block aus der PDF-AS-Ausgabe gegen `Appearance::to_standalone_pdf`.
//!
//! Bewusste Abweichungen (Kompatibilitätsregister) sind unten in [`known_deviation`] aufgeführt
//! und werden als solche geprüft; jede andere Abweichung lässt den Test scheitern.

mod pdfas442;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use pdfas442::*;
use pdfover_sigblock::PDFOVER_PROFILE_IDS;

/// Ergebnis eines Falls.
#[derive(Debug, Default, Clone, serde::Serialize)]
struct Outcome {
    id: String,
    profile: String,
    ops: usize,
    texts: usize,
    max_dev: f32,
    /// (Aspekt, Detail) unerwarteter Abweichungen
    unexpected: Vec<(String, String)>,
    /// (Aspekt, Begründung) dokumentierter Abweichungen
    expected: Vec<(String, String)>,
}

/// PDF/A-Profil (`SIG_PDFA1B_VALID`)?
fn is_pdfa_profile(case: &CaseInfo) -> bool {
    case.profile.ends_with("_PDFA")
}

/// Bewusste, dokumentierte Abweichungen (Kompatibilitätsregister, docs/integration/golden-report.md
/// Abschnitt „Signaturblock“, IDs SB-G1 … SB-G6). Aspekte: `bbox`, `ops`, `fonts`, `image`,
/// `image-colorspace`, `image-samples`, `image-smask` (Standardmodus), `image-colorspace-quirks`,
/// `image-samples-quirks`, `image-smask-quirks` (mit `pdfas_quirks`), `quirks` (Bytes), `pixels`
/// (Standardmodus/Mini-PDF), `pixels-quirks`.
///
/// Seit Welle 3 trägt `Emblem` den Farbtyp der Quelle (`SourceColor`): mit `pdfas_quirks` sind
/// SB-G2 (SMask für jeden Alphakanal), SB-G3 (Palettentransparenz verworfen) und SB-G4 (DeviceGray,
/// aufgehellt) exakt wie PDF-AS – die `-quirks`-Aspekte dürfen dort nicht mehr abweichen. Der
/// Standardmodus bleibt korrigiert (Entscheidung des Auftraggebers offen, Default dokumentiert).
fn known_deviation(case: &CaseInfo, aspect: &str) -> Option<&'static str> {
    let em = case.emblem.as_deref().unwrap_or("");
    let pdfa = is_pdfa_profile(case);
    match aspect {
        // SB-G1 120-mm-Grenze: PDF-Over klemmt targetLogoSize nur in der Vorschau, PDF-Over NG
        // auch beim Signieren (ProfileOverrides::base_logo, Register „Logo-Größe > 120 mm“).
        _ if case.profile == "BASE_LOGO" && case.base_logo_mm.is_some_and(|mm| mm > 120.0) => {
            Some("SB-G1 120-mm-Grenze: Logo auch beim Signieren auf 120 mm geklemmt")
        }
        // SB-G3 indizierte Transparenz (PNG-Palette mit tRNS, GIF): PDF-AS verwirft sie
        // (`hasAlpha` ohne Alpha-Raster), PDF-Over NG behält sie als SMask.
        "image-smask" | "pixels" if em == "emblem-indexed.png" && !pdfa => {
            Some("SB-G3 indizierte Transparenz bleibt als SMask erhalten")
        }
        // SB-G4 Graustufen-Bildmarken: PDF-AS schreibt DeviceGray mit Java-Umrechnung linear →
        // sRGB (aufgehellt, z. B. 20 → 79); PDF-Over NG übernimmt die Originalwerte (DeviceRGB).
        "image-colorspace" | "image-samples" | "pixels" if em == "emblem-tall.png" && !pdfa => {
            Some("SB-G4 Graustufen-Bildmarke ohne Java-Aufhellung, DeviceRGB")
        }
        // SB-G5 JPEG-Dekoder: zune-jpeg statt libjpeg (ImageIO), Abweichung ≤ 2 Stufen je Kanal.
        "image-samples" | "image-samples-quirks" | "pixels" | "pixels-quirks" if em == "emblem-photo.jpg" => {
            Some("SB-G5 JPEG-Dekoder (zune-jpeg) weicht um wenige Stufen ab")
        }
        // SB-G6 PDF/A ohne Alpha: Standard = Komposition auf Weiß; PDF-AS (removeAlphaChannel)
        // behält die gespeicherten RGB-Werte – mit pdfas_quirks exakt nachgebildet.
        "image-samples" | "pixels" if em == "emblem-alpha.png" && pdfa => {
            Some("SB-G6 PDF/A: Transparenz auf Weiß komponiert (PDF-AS: gespeicherte Farbe, meist Schwarz)")
        }
        _ => None,
    }
}

/// SHA-256 einer voll deckenden Alpha-Ebene `w × h` (SB-G2).
fn opaque_smask_sha(w: u32, h: u32) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(vec![255u8; (w * h) as usize]).iter().map(|b| format!("{b:02x}")).collect()
}

fn check_case(m: &Manifest) -> Outcome {
    let case = &m.case;
    let mut o = Outcome { id: case.id.clone(), profile: case.profile.clone(), ..Default::default() };
    let flag = |o: &mut Outcome, aspect: &str, detail: String| match known_deviation(case, aspect) {
        Some(reason) => {
            if !o.expected.iter().any(|(a, _)| a == aspect) {
                o.expected.push((aspect.to_string(), reason.to_string()));
            }
        }
        None => o.unexpected.push((aspect.to_string(), detail)),
    };
    let inner = m.inner.as_ref().expect("Inner-Form im Manifest");
    let holder = m.holder.as_ref().expect("Holder-Form im Manifest");
    let app = build(m, false);
    let own = own_form(&app);

    // Blockmaße
    let bb = &inner.bbox;
    if !(close(bb[0], app.width + 1.0) && close(bb[1], app.height + 1.0) && bb[2] == 0.0 && bb[3] == 0.0) {
        flag(&mut o, "bbox", format!("PDF-AS {bb:?}, eigen [{} {} 0 0]", app.width + 1.0, app.height + 1.0));
    }
    if own.bbox.len() != 4 || own.bbox.iter().zip(bb).any(|(a, b)| !close(*a, *b)) {
        flag(&mut o, "bbox", format!("Inner-BBox PDF-AS {bb:?}, eigen {:?}", own.bbox));
    }
    if holder.bbox.iter().zip(bb).any(|(a, b)| !close(*a, *b)) {
        flag(&mut o, "holder", format!("Holder-BBox {:?} ≠ Inner {:?}", holder.bbox, bb));
    }

    // Operationen
    let gold = manifest_ops(inner);
    let ours = parse_content(&own.content, &own.font_names);
    o.ops = gold.len();
    o.texts = gold.iter().filter(|g| matches!(g, G::Text { .. })).count();
    if gold.len() != ours.len() {
        let gk: Vec<&str> = gold.iter().map(G::kind).collect();
        let ok: Vec<&str> = ours.iter().map(G::kind).collect();
        flag(&mut o, "ops", format!("Anzahl {} vs {}: {gk:?} / {ok:?}", gold.len(), ours.len()));
    } else {
        for (k, (a, b)) in gold.iter().zip(&ours).enumerate() {
            match deviation(a, b) {
                Some(d) => {
                    o.max_dev = o.max_dev.max(d);
                    if d > TOL {
                        flag(&mut o, "ops", format!("Op {k}: Abweichung {d}: PDF-AS {a:?} / eigen {b:?}"));
                    }
                }
                None => {
                    let text = inner
                        .texts
                        .iter()
                        .flat_map(|t| t.lines.iter())
                        .filter_map(|l| l.text.clone())
                        .collect::<Vec<_>>()
                        .join(" | ");
                    flag(&mut o, "ops", format!("Op {k} verschieden: PDF-AS {a:?} / eigen {b:?} (Texte: {text})"));
                }
            }
        }
    }

    // Fonts
    let gold_fonts: BTreeMap<String, &FontInfo> =
        inner.fonts.values().map(|f| (f.base_font.clone().unwrap_or_default(), f)).collect();
    let own_fonts: BTreeMap<String, &OwnFont> = own.fonts.values().map(|f| (f.base_font.clone(), f)).collect();
    if gold_fonts.keys().collect::<Vec<_>>() != own_fonts.keys().collect::<Vec<_>>() {
        flag(
            &mut o,
            "fonts",
            format!("Fonts {:?} vs {:?}", gold_fonts.keys().collect::<Vec<_>>(), own_fonts.keys().collect::<Vec<_>>()),
        );
    }
    let used: HashMap<String, BTreeSet<u32>> = {
        let mut u: HashMap<String, BTreeSet<u32>> = HashMap::new();
        for g in &gold {
            if let G::Text { font, lines, .. } = g {
                let two = gold_fonts.get(font).is_some_and(|f| f.subtype.as_deref() == Some("Type0"));
                for (_, _, b) in lines {
                    let e = u.entry(font.clone()).or_default();
                    if two {
                        e.extend(b.chunks(2).map(|c| u32::from(c[0]) << 8 | u32::from(c[1])));
                    }
                }
            }
        }
        u
    };
    for (name, gf) in &gold_fonts {
        let Some(of) = own_fonts.get(name) else { continue };
        if gf.subtype.as_deref() != Some(of.subtype.as_str()) {
            flag(&mut o, "fonts", format!("{name}: Subtype {:?} vs {}", gf.subtype, of.subtype));
        }
        if gf.embedded != of.embedded {
            flag(&mut o, "fonts", format!("{name}: Einbettung {:?} vs {:?}", gf.embedded, of.embedded));
        }
        if gf.encoding != of.encoding {
            flag(&mut o, "fonts", format!("{name}: Encoding {:?} vs {:?}", gf.encoding, of.encoding));
        }
        let gd = gf.descriptor.get("Descent").and_then(|v| v.as_f64()).map(|v| v as f32);
        if let (Some(a), Some(b)) = (gd, of.descent)
            && !close(a, b)
        {
            flag(&mut o, "fonts", format!("{name}: Descent {a} vs {b}"));
        }
        if gf.cid_set.unwrap_or(false) != of.cid_set {
            flag(&mut o, "fonts", format!("{name}: CIDSet {:?} vs {}", gf.cid_set, of.cid_set));
        }
        for cid in used.get(name).into_iter().flatten() {
            let a = gf.w.get(&cid.to_string()).copied();
            let b = of.widths.get(cid).copied();
            if a != b {
                flag(&mut o, "fonts", format!("{name}: /W für CID {cid}: {a:?} vs {b:?}"));
            }
        }
    }

    // Bilder (Standardmodus und mit pdfas_quirks)
    let own_quirks = own_form(&build(m, true));
    if inner.images.len() != own.images.len() {
        flag(&mut o, "image", format!("Anzahl Bilder {} vs {}", inner.images.len(), own.images.len()));
    }
    for (name, gi) in &inner.images {
        let (Some(oi), Some(qi)) = (own.images.get(name), own_quirks.images.get(name)) else {
            flag(&mut o, "image", format!("{name} fehlt"));
            continue;
        };
        if (gi.width, gi.height) != (oi.width, oi.height) {
            flag(&mut o, "image", format!("{name}: {}×{} vs {}×{}", gi.width, gi.height, oi.width, oi.height));
        }
        let gcs = gi.color_space.clone().unwrap_or_default();
        // PDF/A ohne Quirks: ICCBased (sRGB) statt DeviceRGB, gleiche Samples (Register)
        if gcs != oi.color_space && !(gcs == "DeviceRGB" && oi.color_space == "ICCBased") {
            flag(&mut o, "image-colorspace", format!("{name}: {gcs} vs {}", oi.color_space));
        } else {
            if gi.sha256 != oi.sha256 {
                flag(&mut o, "image-samples", format!("{name}: Samples verschieden ({gcs})"));
            }
        }
        // mit pdfas_quirks: Farbraum, Samples und SMask exakt wie PDF-AS (SB-G2 … SB-G4 nachgebildet)
        if gcs != qi.color_space {
            flag(&mut o, "image-colorspace-quirks", format!("{name}: {gcs} vs {} (Quirks)", qi.color_space));
        } else if gi.sha256 != qi.sha256 {
            flag(&mut o, "image-samples-quirks", format!("{name}: Samples mit Quirks verschieden ({gcs})"));
        }
        match (&gi.smask, &qi.smask) {
            (None, None) => {}
            (Some(a), Some(b)) if a.sha256 == *b => {}
            (a, b) => flag(
                &mut o,
                "image-smask-quirks",
                format!("{name}: SMask mit Quirks {:?} vs {:?}", a.is_some(), b.is_some()),
            ),
        }
        match (&gi.smask, &oi.smask) {
            (None, None) => {}
            (Some(a), Some(b)) if a.sha256 == *b => {}
            // SB-G2: PDFBox (LosslessFactory) schreibt für jeden Alphakanal eine SMask, auch wenn
            // er voll deckend ist (z. B. amtsignatur-logo_demo.png); PDF-Over NG lässt sie weg
            // (Emblem::has_alpha = „hat Transparenz“), pixelgleich.
            (Some(a), None) if a.sha256 == opaque_smask_sha(a.width, a.height) => {
                if !o.expected.iter().any(|(k, _)| k == "image-smask") {
                    o.expected.push((
                        "image-smask".into(),
                        "SB-G2 voll deckender Alphakanal ohne SMask (pixelgleich)".into(),
                    ));
                }
            }
            (a, b) => flag(&mut o, "image-smask", format!("{name}: SMask {:?} vs {:?}", a.is_some(), b.is_some())),
        }
    }
    o
}

fn visible_cases() -> Vec<IndexCase> {
    index().cases.into_iter().filter(|c| c.profile != "INVISIBLE" && c.status == "ok").collect()
}

fn run_parallel<T: Send>(cases: &[IndexCase], f: impl Fn(&IndexCase) -> T + Sync) -> Vec<T> {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunk = cases.len().div_ceil(n).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> =
            cases.chunks(chunk).map(|part| s.spawn(|| part.iter().map(&f).collect::<Vec<T>>())).collect();
        handles.into_iter().flat_map(|h| h.join().expect("Thread")).collect()
    })
}

fn write_report(name: &str, value: &impl serde::Serialize) {
    if let Some(dir) = std::env::var_os("PDFOVER_GOLDEN_REPORT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }
}

#[test]
fn index_covers_all_profiles_and_oracles_pass() {
    let idx = index();
    assert!(idx.cases.len() >= 500, "Fallzahl {}", idx.cases.len());
    let profiles: BTreeSet<&str> = idx.cases.iter().map(|c| c.profile.as_str()).collect();
    assert_eq!(profiles, PDFOVER_PROFILE_IDS.iter().copied().collect::<BTreeSet<_>>(), "alle 12 Profil-IDs");
    let inputs: BTreeSet<&str> = idx.cases.iter().map(|c| c.input.as_str()).collect();
    for i in [
        "a4",
        "a4-landscape",
        "a4-rot90",
        "a4-rot180",
        "a4-rot270",
        "a4-cropbox",
        "content-half",
        "presigned",
        "qr",
        "qr-id200",
        "landscape-rot90",
        "landscape-rot270",
        "wide-rot180",
        "a4-rot270-low",
        "rot90-842-low",
        "half-rot90",
    ] {
        assert!(inputs.contains(i), "Eingabe {i}");
    }
    let mut bad = Vec::new();
    for c in &idx.cases {
        if c.status != "ok" {
            bad.push(format!("{}: Status {}", c.id, c.status));
            continue;
        }
        let m = manifest(&c.id);
        let o = m.oracles.as_ref().expect("Orakel");
        let ph = o.pyhanko.as_ref().expect("pyHanko-Ergebnis");
        for k in ["intact", "valid", "trusted"] {
            if ph.get(k).and_then(|v| v.as_bool()) != Some(true) {
                bad.push(format!("{}: pyHanko {k} = {:?}", c.id, ph.get(k)));
            }
        }
        if ph.get("coverage").and_then(|v| v.as_str()) != Some("ENTIRE_FILE") {
            bad.push(format!("{}: pyHanko coverage {:?}", c.id, ph.get("coverage")));
        }
        let q = o.qpdf_check.as_ref().expect("qpdf --check");
        if q.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            bad.push(format!("{}: qpdf --check {q}", c.id));
        }
        if c.profile != "INVISIBLE" {
            assert!(m.inner.is_some(), "{}: Inner-Form", c.id);
            assert!(m.sig_date.is_some() || c.profile == "BASE_LOGO", "{}: SIG_DATE", c.id);
        }
        if let Some(p) = &c.pdf {
            assert!(golden_dir().join(p).is_file(), "{}: {p}", c.id);
        }
    }
    assert!(bad.is_empty(), "Orakel-Befunde:\n{}", bad.join("\n"));
}

/// Externes Orakel für die eigenen Blöcke: `qpdf --check` auf `to_standalone_pdf` je Profil und
/// Bildmarken-Variante (Standard und `pdfas_quirks`); ohne qpdf übersprungen.
#[test]
fn own_blocks_pass_qpdf_check() {
    let qpdf_ok = std::process::Command::new("qpdf").arg("--version").output().is_ok_and(|o| o.status.success());
    if !qpdf_ok {
        eprintln!("qpdf fehlt – Orakel übersprungen");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let cases: Vec<IndexCase> =
        visible_cases().into_iter().filter(|c| c.input == "a4" && c.position_key == "manual").collect();
    assert!(cases.len() >= 60, "{}", cases.len());
    let mut bad = Vec::new();
    for c in &cases {
        let m = manifest(&c.id);
        for quirks in [false, true] {
            let path = tmp.path().join(format!("{}-{quirks}.pdf", c.id));
            std::fs::write(&path, build(&m, quirks).to_standalone_pdf().unwrap()).unwrap();
            let out = std::process::Command::new("qpdf").arg("--check").arg(&path).output().unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !out.status.success() || !stdout.contains("No syntax or stream encoding errors") {
                bad.push(format!("{} (quirks={quirks}): {stdout} {}", c.id, String::from_utf8_lossy(&out.stderr)));
            }
        }
    }
    assert!(bad.is_empty(), "qpdf --check:\n{}", bad.join("\n"));
}

#[test]
fn blocks_match_pdfas_442() {
    let cases = visible_cases();
    let outcomes = run_parallel(&cases, |c| check_case(&manifest(&c.id)));
    // Zusammenfassung je Profil
    let mut by_profile: BTreeMap<&str, (usize, usize, f32, usize)> = BTreeMap::new();
    for o in &outcomes {
        let e = by_profile.entry(o.profile.as_str()).or_default();
        e.0 += 1;
        if o.unexpected.is_empty() && o.expected.is_empty() {
            e.1 += 1;
        }
        // maximale Koordinatenabweichung nur über Fälle ohne dokumentierte Abweichung der Operationen
        if !o.expected.iter().any(|(a, _)| a == "ops" || a == "bbox") {
            e.2 = e.2.max(o.max_dev);
        }
        e.3 += usize::from(!o.expected.is_empty());
    }
    let mut report =
        String::from("Profil | Fälle | gleich | max. Abweichung (pt) | Fälle mit dokumentierter Abweichung\n");
    for (p, (n, same, dev, exp)) in &by_profile {
        writeln!(report, "{p} | {n} | {same} | {dev:.6} | {exp}").unwrap();
    }
    println!("{report}");
    write_report("sigblock-structure.json", &outcomes);
    let failures: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.unexpected.is_empty())
        .map(|o| {
            format!(
                "{}:\n  {}",
                o.id,
                o.unexpected.iter().map(|(a, d)| format!("[{a}] {d}")).collect::<Vec<_>>().join("\n  ")
            )
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} von {} Fällen weichen ab:\n{}",
        failures.len(),
        outcomes.len(),
        failures.join("\n")
    );
}

// ------------------------------------------------------------------------------------------------
// Pixelvergleich (PDFium, 288 dpi)
// ------------------------------------------------------------------------------------------------

mod pixels {
    use std::collections::HashMap;

    use pdfium_render::prelude::*;
    use pdfover_pdf_cos::{Dictionary, Object, ObjectId, Stream, lopdf};

    pub fn pdfium() -> Option<Pdfium> {
        let dir = std::env::var("PDFOVER_PDFIUM_DIR").ok().filter(|d| !d.is_empty())?;
        match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir)) {
            Ok(b) => Some(Pdfium::new(b)),
            Err(e) => {
                eprintln!("PDFium nicht ladbar ({e}) – Pixelvergleich übersprungen");
                None
            }
        }
    }

    fn deref<'a>(doc: &'a lopdf::Document, o: &'a Object) -> &'a Object {
        match o {
            Object::Reference(id) => doc.get_object(*id).unwrap_or(o),
            _ => o,
        }
    }

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

    fn floats(doc: &lopdf::Document, o: &Object) -> Vec<f32> {
        deref(doc, o)
            .as_array()
            .map(|a| a.iter().map(|v| deref(doc, v).as_float().unwrap_or(0.0)).collect())
            .unwrap_or_default()
    }

    /// Widget der Signatur über die ganze Datei: (Holder-Form, `/Rect`).
    fn newest_widget(doc: &lopdf::Document, file_len: usize) -> (ObjectId, Vec<f32>) {
        for o in doc.objects.values() {
            let Ok(d) = o.as_dict() else { continue };
            if d.get(b"Subtype").and_then(|s| s.as_name()).ok() != Some(b"Widget") {
                continue;
            }
            let Ok(v) = d.get(b"V") else { continue };
            let Ok(vd) = deref(doc, v).as_dict() else { continue };
            let br = floats(doc, vd.get(b"ByteRange").unwrap());
            if br.len() == 4 && (br[2] + br[3]) as usize == file_len {
                let ap = deref(doc, d.get(b"AP").unwrap()).as_dict().unwrap();
                let Object::Reference(n) = ap.get(b"N").unwrap() else { panic!("/AP /N direkt") };
                return (*n, floats(doc, d.get(b"Rect").unwrap()));
            }
        }
        panic!("Signatur-Widget nicht gefunden")
    }

    /// Mini-PDF mit dem Erscheinungsbild der PDF-AS-Ausgabe: Holder-BBox (mit `/Matrix`) wird wie im
    /// Viewer auf das Rect abgebildet (ISO 32000-1 §12.5.5). Die Seitengröße ist – wie bei
    /// `Appearance::to_standalone_pdf` – die transformierte BBox minus 1 (= W × H des Layouts);
    /// das geschriebene `/Rect` (float-gerundet, z. B. 230 statt 229,99998) würde die Abbildung um
    /// Bruchteile eines Gerätepixels verschieben und die Bildinterpolation verändern.
    pub fn pdfas_block(bytes: &[u8]) -> Vec<u8> {
        let src = lopdf::Document::load_mem(bytes).expect("signierte Datei lesbar");
        let (holder, rect) = newest_widget(&src, bytes.len());
        assert_eq!(rect.len(), 4, "/Rect");
        holder_page(&src, holder, Some(&rect))
    }

    /// Eigener Block über denselben Weg wie [`pdfas_block`] (Seite → Holder → Inner-Form).
    pub fn own_block(app: &pdfover_sigblock::Appearance) -> Vec<u8> {
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
        holder_page(&doc, m.holder_id, None)
    }

    fn holder_page(src: &lopdf::Document, holder: ObjectId, rect: Option<&[f32]>) -> Vec<u8> {
        let hd = &src.get_object(holder).unwrap().as_stream().unwrap().dict;
        let bbox = floats(src, hd.get(b"BBox").unwrap());
        let m = hd.get(b"Matrix").map(|o| floats(src, o)).unwrap_or_else(|_| vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let (bx0, by0, bx1, by1) =
            (bbox[0].min(bbox[2]), bbox[1].min(bbox[3]), bbox[0].max(bbox[2]), bbox[1].max(bbox[3]));
        let pts = [(bx0, by0), (bx1, by0), (bx0, by1), (bx1, by1)];
        let tp: Vec<(f32, f32)> =
            pts.iter().map(|&(x, y)| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])).collect();
        let x0 = tp.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let x1 = tp.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let y0 = tp.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let y1 = tp.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        let pw = x1 - x0 - 1.0;
        let ph = y1 - y0 - 1.0;
        // Plausibilität gegen das Rect (gleiche Maße bis auf die float-Rundung)
        if let Some(rect) = rect {
            assert!(((rect[2] - rect[0]).abs() - pw).abs() < 0.01 && ((rect[3] - rect[1]).abs() - ph).abs() < 0.01);
        }
        let (sx, sy) = (pw / (x1 - x0), ph / (y1 - y0));
        let mut doc = lopdf::Document::with_version("1.7");
        let mut map = HashMap::new();
        let hid = deep_copy(src, holder, &mut doc, &mut map);
        let content = format!("q {sx} 0 0 {sy} {} {} cm /H Do Q\n", -x0 * sx, -y0 * sy);
        page_doc(doc, hid, content, pw, ph)
    }

    fn page_doc(mut doc: lopdf::Document, xobj: ObjectId, content: String, w: f32, h: f32) -> Vec<u8> {
        let cid = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), content.into_bytes())));
        let pages_id = doc.new_object_id();
        let mut xo = Dictionary::new();
        xo.set("H", Object::Reference(xobj));
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

    pub fn render(pdfium: &Pdfium, pdf: &[u8], scale: f32) -> (usize, usize, Vec<u8>) {
        let doc = pdfium.load_pdf_from_byte_slice(pdf, None).expect("PDFium lädt Mini-PDF");
        let page = doc.pages().get(0).unwrap();
        let cfg = PdfRenderConfig::new().scale_page_by_factor(scale);
        let bmp = page.render_with_config(&cfg).unwrap();
        (bmp.width() as usize, bmp.height() as usize, bmp.as_raw_bytes())
    }

    /// Diagnose: BGRA-Bitmap als PNG (nur mit `PDFOVER_GOLDEN_DEBUG_DIR`).
    pub fn write_png(path: &std::path::Path, img: &(usize, usize, Vec<u8>)) {
        let f = std::fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), img.0 as u32, img.1 as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let rgba: Vec<u8> = img.2.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    }

    /// (Anteil Pixel mit Abweichung > 0, Anteil > 32, maximale Kanalabweichung); `None` bei
    /// unterschiedlicher Bildgröße.
    pub fn diff(a: &(usize, usize, Vec<u8>), b: &(usize, usize, Vec<u8>)) -> Option<(f64, f64, u8)> {
        if (a.0, a.1) != (b.0, b.1) {
            return None;
        }
        let px = (a.2.len() / 4).max(1);
        let (mut any, mut big, mut max) = (0usize, 0usize, 0u8);
        for (p, q) in a.2.as_chunks::<4>().0.iter().zip(b.2.as_chunks::<4>().0) {
            let d = (0..3).map(|i| p[i].abs_diff(q[i])).max().unwrap_or(0);
            max = max.max(d);
            if d > 0 {
                any += 1;
            }
            if d > 32 {
                big += 1;
            }
        }
        Some((any as f64 / px as f64, big as f64 / px as f64, max))
    }
}

/// Pixelabweichung: (Anteil Pixel mit Abweichung > 0, Anteil > 32, maximale Kanalabweichung).
type PixelDiff = (f64, f64, u8);

#[derive(Debug, Clone, serde::Serialize)]
struct PixelOutcome {
    id: String,
    profile: String,
    size: (usize, usize),
    /// gleicher Renderweg (Seite → Holder → Inner-Form), Standardmodus
    default: Option<PixelDiff>,
    /// gleicher Renderweg, mit `pdfas_quirks`
    quirks: Option<PixelDiff>,
    /// eigener Block über `Appearance::to_standalone_pdf` (Seite → Inner-Form), Standardmodus
    standalone: Option<PixelDiff>,
}

/// Block der PDF-AS-Ausgabe gegen den eigenen Block, 288 dpi (Plan §8.2):
/// 1. gleicher Renderweg (Holder-Form mit BBox → Rect wie im Viewer), Standard und Quirks,
/// 2. `Appearance::to_standalone_pdf` (Blockvorschau, zeichnet ebenfalls über das Holder-Form).
///
/// Erwartung: pixelgleich (Diff-Quote 0); PDFium rastert Bilder je Formularebene mit anderer
/// Rundung, deshalb zeichnet auch die Vorschau über das Holder-Form.
///
/// Dokumentierte Ausnahmen siehe [`known_deviation`] (Aspekt `pixels`).
#[test]
fn pixels_match_pdfas_442() {
    let Some(pdfium) = pixels::pdfium() else {
        eprintln!("PDFOVER_PDFIUM_DIR nicht gesetzt – Pixelvergleich übersprungen");
        return;
    };
    let cases: Vec<IndexCase> = visible_cases().into_iter().filter(|c| c.pdf.is_some()).collect();
    assert!(cases.len() >= 60, "zu wenige signierte Referenz-PDFs: {}", cases.len());
    let debug = std::env::var_os("PDFOVER_GOLDEN_DEBUG_DIR").map(std::path::PathBuf::from);
    let mut outcomes = Vec::new();
    for c in &cases {
        let m = manifest(&c.id);
        let gold = pixels::render(&pdfium, &pixels::pdfas_block(&read(c.pdf.as_deref().unwrap())), 4.0);
        let own = |quirks: bool| pixels::render(&pdfium, &pixels::own_block(&build(&m, quirks)), 4.0);
        let (own_default, own_quirks) = (own(false), own(true));
        let standalone = pixels::render(&pdfium, &build(&m, false).to_standalone_pdf().unwrap(), 4.0);
        let o = PixelOutcome {
            id: c.id.clone(),
            profile: c.profile.clone(),
            size: (gold.0, gold.1),
            default: pixels::diff(&gold, &own_default),
            quirks: pixels::diff(&gold, &own_quirks),
            standalone: pixels::diff(&gold, &standalone),
        };
        if let Some(dir) = &debug
            && o.default.is_none_or(|d| d.0 > 0.0)
        {
            std::fs::create_dir_all(dir).unwrap();
            for (name, img) in [("pdfas", &gold), ("eigen", &own_default), ("standalone", &standalone)] {
                pixels::write_png(&dir.join(format!("{}-{name}.png", c.id)), img);
            }
        }
        outcomes.push(o);
    }
    // Zusammenfassung je Profil: Fälle ohne dokumentierte Abweichung getrennt von SB-G1 … SB-G6
    #[derive(Default)]
    struct Agg {
        n: usize,
        identical: [usize; 3],
        undocumented: [f64; 3],
        documented: [f64; 3],
        documented_cases: BTreeSet<String>,
    }
    let ratio = |d: Option<PixelDiff>| d.map_or(1.0, |d| d.0);
    let mut by_profile: BTreeMap<&str, Agg> = BTreeMap::new();
    for o in &outcomes {
        let case = manifest(&o.id).case;
        let e = by_profile.entry(o.profile.as_str()).or_default();
        e.n += 1;
        for (k, (d, aspect)) in
            [(o.default, "pixels"), (o.quirks, "pixels-quirks"), (o.standalone, "pixels")].iter().enumerate()
        {
            let r = ratio(*d);
            if r == 0.0 {
                e.identical[k] += 1;
            }
            if let Some(reason) = known_deviation(&case, aspect) {
                e.documented[k] = e.documented[k].max(r);
                if r > 0.0 {
                    e.documented_cases.insert(reason.split_whitespace().next().unwrap_or("").to_string());
                }
            } else {
                e.undocumented[k] = e.undocumented[k].max(r);
            }
        }
    }
    let mut report = String::from(
        "Profil | PDFs | pixelgleich Standard/Quirks/Mini-PDF | Diff-Quote ohne dok. Abweichung (S/Q/M) | \
         Diff-Quote mit dok. Abweichung (S/Q/M) | Register\n",
    );
    for (p, e) in &by_profile {
        let pct = |v: [f64; 3]| format!("{:.4} / {:.4} / {:.4} %", v[0] * 100.0, v[1] * 100.0, v[2] * 100.0);
        writeln!(
            report,
            "{p} | {} | {}/{}/{} | {} | {} | {}",
            e.n,
            e.identical[0],
            e.identical[1],
            e.identical[2],
            pct(e.undocumented),
            pct(e.documented),
            e.documented_cases.iter().cloned().collect::<Vec<_>>().join(", ")
        )
        .unwrap();
    }
    println!("{report}");
    for o in &outcomes {
        println!("{:70} {:?} Standard {:?} Quirks {:?} Mini-PDF {:?}", o.id, o.size, o.default, o.quirks, o.standalone);
    }
    write_report("sigblock-pixels.json", &outcomes);
    let mut failures = Vec::new();
    for o in &outcomes {
        let case = manifest(&o.id).case;
        // SB-G5 (JPEG): erlaubt sind wenige Stufen, keine sichtbaren Unterschiede
        let jpeg = case.emblem.as_deref() == Some("emblem-photo.jpg");
        let bad = |d: Option<PixelDiff>, aspect: &str| match d {
            None => known_deviation(&case, aspect).is_none(),
            Some(_) if jpeg => d.is_some_and(|d| d.2 > 4),
            Some(d) => d.0 > 0.0 && known_deviation(&case, aspect).is_none(),
        };
        if bad(o.default, "pixels") || bad(o.quirks, "pixels-quirks") || bad(o.standalone, "pixels") {
            failures.push(format!(
                "{}: Standard {:?}, Quirks {:?}, Mini-PDF {:?}",
                o.id, o.default, o.quirks, o.standalone
            ));
        }
    }
    assert!(failures.is_empty(), "nicht pixelgleich:\n{}", failures.join("\n"));
}

/// Mit `pdfas_quirks` ist der Inner-Form-Inhalt byte-gleich (inklusive `/DeviceRGB cs … sc`,
/// Zahlenformat und kodierter Strings), `/TU` und der Holder stimmen überein.
#[test]
fn content_bytes_identical_with_quirks() {
    let cases = visible_cases();
    let results = run_parallel(&cases, |c| {
        let m = manifest(&c.id);
        let inner = m.inner.as_ref().unwrap();
        let holder = m.holder.as_ref().unwrap();
        let app = build(&m, true);
        let own = own_form(&app);
        let mut errs = Vec::new();
        let gold = latin1(&inner.content);
        let trim = |b: &[u8]| {
            let mut v = b.to_vec();
            while v.last().is_some_and(|c| c.is_ascii_whitespace()) {
                v.pop();
            }
            v
        };
        if trim(&gold) != trim(&own.content) {
            let g = String::from_utf8_lossy(&gold).to_string();
            let o = String::from_utf8_lossy(&own.content).to_string();
            let first = g.lines().zip(o.lines()).position(|(a, b)| a != b);
            errs.push(format!(
                "Inhalt verschieden ({} vs {} Bytes), erste abweichende Zeile {:?}: {:?} / {:?}",
                gold.len(),
                own.content.len(),
                first,
                first.and_then(|i| g.lines().nth(i)),
                first.and_then(|i| o.lines().nth(i))
            ));
        }
        let tu = m.widget.as_ref().and_then(|w| w.tu.clone()).unwrap_or_default();
        if tu != app.tu_text {
            errs.push(format!("/TU {tu:?} vs {:?}", app.tu_text));
        }
        let pdfover_pdf_cos::Object::Stream(hs) = &app.objects[app.holder] else { unreachable!() };
        let hc = String::from_utf8_lossy(&hs.content).to_string();
        if hc != holder.content {
            errs.push(format!("Holder-Inhalt {:?} vs {hc:?}", holder.content));
        }
        let hm: Vec<f32> = hs
            .dict
            .get(b"Matrix")
            .map(|a| a.as_array().unwrap().iter().map(|v| v.as_float().unwrap_or(0.0)).collect())
            .unwrap_or_default();
        if let Some(gm) = &holder.matrix
            && gm.iter().zip(&hm).any(|(a, b)| !close(*a, *b))
        {
            errs.push(format!("Holder-Matrix {gm:?} vs {hm:?}"));
        }
        if holder.xobjects != vec![app.inner_name.clone()] {
            errs.push(format!("Holder-XObjects {:?} vs {}", holder.xobjects, app.inner_name));
        }
        (c.id.clone(), errs)
    });
    let failures: Vec<String> = results
        .iter()
        .filter(|(id, e)| !e.is_empty() && known_deviation(&manifest(id).case, "quirks").is_none())
        .map(|(id, e)| format!("{id}:\n  {}", e.join("\n  ")))
        .collect();
    write_report("sigblock-quirks.json", &results);
    println!(
        "byte-gleich mit pdfas_quirks: {} von {}",
        results.iter().filter(|(_, e)| e.is_empty()).count(),
        results.len()
    );
    assert!(failures.is_empty(), "{} Fälle nicht byte-gleich:\n{}", failures.len(), failures.join("\n"));
}

/// Register Q10 (`/TU`) im Standardmodus: PDF-AS schreibt nur die Texte der äußersten Tabelle
/// (SMALL ohne Hinweis und BASE_LOGO: leer), PDF-Over NG alle Texte in Leserichtung. Geprüft wird
/// die dokumentierte Form: Die PDF-AS-Zeilen sind in derselben Reihenfolge im eigenen `/TU`
/// enthalten; ein leeres PDF-AS-`/TU` ist im Standardmodus nur bei Blöcken ohne Text erlaubt.
#[test]
fn tu_default_mode_contains_pdfas_tu() {
    let cases = visible_cases();
    let results = run_parallel(&cases, |c| {
        let m = manifest(&c.id);
        let gold = m.widget.as_ref().and_then(|w| w.tu.clone()).unwrap_or_default();
        let app = build(&m, false);
        let own: Vec<&str> = app.tu_text.lines().collect();
        let mut rest = own.iter();
        let missing: Vec<&str> = gold.lines().filter(|l| !rest.any(|o| o == l)).collect();
        let has_text = m.inner.as_ref().is_some_and(|i| !i.texts.is_empty());
        let mut errs = Vec::new();
        if !missing.is_empty() {
            errs.push(format!("PDF-AS-Zeilen fehlen oder in anderer Reihenfolge: {missing:?}"));
        }
        if has_text && app.tu_text.trim().is_empty() {
            errs.push("eigenes /TU leer trotz Text im Block".to_string());
        }
        if !has_text && !app.tu_text.is_empty() {
            errs.push(format!("eigenes /TU ohne Text im Block: {:?}", app.tu_text));
        }
        (c.id.clone(), gold.is_empty() && has_text, errs)
    });
    let q10 = results.iter().filter(|(_, empty, e)| *empty && e.is_empty()).count();
    println!("Q10: {q10} Fälle mit leerem PDF-AS-/TU und eigenem Text-/TU");
    assert!(q10 > 0, "Q10-Fälle (SMALL ohne Hinweis) fehlen");
    let failures: Vec<String> = results
        .iter()
        .filter(|(id, _, e)| !e.is_empty() && known_deviation(&manifest(id).case, "tu").is_none())
        .map(|(id, _, e)| format!("{id}: {}", e.join("; ")))
        .collect();
    assert!(failures.is_empty(), "{} Fälle:\n{}", failures.len(), failures.join("\n"));
}

/// Die eingecheckten Golden-Daten enthalten keine lokalen Pfade (Benutzer-, Arbeits- oder
/// Worktree-Ordner des erzeugenden Rechners); Pfade in die Konfigurationskopie heißen `<cfg>/…`.
#[test]
fn golden_data_has_no_local_paths() {
    let mut files = vec!["index.json".to_string()];
    files.extend(index().cases.iter().map(|c| format!("cases/{}/manifest.json", c.id)));
    let needles = ["/Users/", "/home/", "/private/", "/tmp/", ":\\\\Users", "worktrees", "target/golden"];
    let bad: Vec<String> = files
        .iter()
        .filter_map(|f| {
            let text = String::from_utf8_lossy(&read(f)).to_string();
            needles.iter().find(|n| text.contains(**n)).map(|n| format!("{f}: enthält {n:?}"))
        })
        .collect();
    assert!(bad.is_empty(), "lokale Pfade in den Golden-Daten:\n{}", bad.join("\n"));
}

//! PDF/A-Orakel (Welle 3, T2 Punkt 3): signierte Ausgaben der PDF/A-Profile (`SIGNATURBLOCK_*_PDFA`,
//! mit Bildmarken: Alphakanal, Palette mit Transparenz, Graustufen), AMTS-Block und unsichtbare
//! Signatur auf PDF/A-1b-, PDF/A-2b- und PDF/A-3b-Eingaben.
//!
//! - [`pdfa_outputs_pass_verapdf`]: veraPDF (Greenfield-CLI, `tools/verapdf/fetch.sh`) prüft Eingabe
//!   und Ausgabe; jede Regel, die in der Ausgabe, aber nicht in der Eingabe verletzt ist, ist ein
//!   Fehler der Engine. Ohne veraPDF-JAR (`PDFOVER_VERAPDF_JAR`, sonst `tools/verapdf/dist/` im
//!   Worktree bzw. unter `PDFOVER_MAIN`) oder ohne Java (`PDFOVER_JAVA`, `JAVA_HOME`, `java` im PATH)
//!   übersprungen. `PDFOVER_VERAPDF_KEEP=<dir>` legt alle geprüften Dateien und die Berichte ab.
//! - [`pdfa_outputs_structural_checks`]: ohne externe Werkzeuge die wichtigsten PDF/A-Regeln für das
//!   Increment (ISO 19005-1 §6.1.3 `/ID`, §6.1.4/§6.1.8 Syntax, §6.4 Transparenz in PDF/A-1, §6.3.4
//!   eingebettete Fonts, §6.2.3 Gerätefarbräume, §6.5.3 Widget-Flags und `/AP` nur mit `/N`, §6.9
//!   kein `NeedAppearances`).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use common::*;
use pdfover_pades::{PlanOptions, SignPlan, analyze, cert_fields, plan_signature, prepare};
use pdfover_pdf_cos::{Dictionary, Limits, Object, ObjectId, ObjectLocation, SourcePdf};
use pdfover_sigblock::{Emblem, SigBlockInput};

/// Eingaben: (Flavour, Bezeichnung, Datei, optionaler Byte-Patch).
fn inputs() -> Vec<(&'static str, &'static str, Vec<u8>)> {
    let read = |rel: &str| std::fs::read(testdata().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let a3 = read("pdf/pdfas-suites/POS_LAST/example.pdf");
    // PDF/A-2b: dieselbe Datei (keine eingebetteten Dateien) mit `pdfaid:part` 2 – gleiche Länge,
    // unkomprimiertes XMP, Querverweis bleibt gültig
    let mut a2 = a3.clone();
    let from = b"pdfaid:part>3<";
    let pos = a2.windows(from.len()).position(|w| w == from).expect("pdfaid:part im XMP");
    a2[pos + 12] = b'2';
    vec![
        ("1b", "ghostscript-1b", read("pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf")),
        ("2b", "word-2b", a2),
        ("3b", "word-3b", a3),
    ]
}

/// Bildmarken aus dem Golden-Korpus (PNG RGBA, Palette mit tRNS, Graustufen).
fn emblem(name: &str) -> Arc<Emblem> {
    let p = testdata().join("golden/pdfas-4.4.2/inputs").join(name);
    Arc::new(Emblem::decode(&std::fs::read(&p).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}")))
}

/// Varianten: (Bezeichnung, Profil, Bildmarke); `None` als Profil = unsichtbar.
fn variants() -> Vec<(&'static str, Option<&'static str>, Option<&'static str>)> {
    vec![
        ("small-pdfa", Some("SIGNATURBLOCK_SMALL_DE_PDFA"), None),
        ("small-note-pdfa", Some("SIGNATURBLOCK_SMALL_EN_NOTE_PDFA"), None),
        ("small-pdfa-alpha", Some("SIGNATURBLOCK_SMALL_DE_PDFA"), Some("emblem-alpha.png")),
        ("small-pdfa-indexed", Some("SIGNATURBLOCK_SMALL_DE_PDFA"), Some("emblem-indexed.png")),
        ("small-pdfa-gray", Some("SIGNATURBLOCK_SMALL_DE_PDFA"), Some("emblem-tall.png")),
        ("amts", Some("AMTSSIGNATURBLOCK_DE_RECOMMENDED"), None),
        ("small-plain-alpha", Some("SIGNATURBLOCK_SMALL_DE"), Some("emblem-alpha.png")),
        ("invisible", None, None),
    ]
}

/// Eine signierte Ausgabe.
struct Signed {
    flavour: &'static str,
    label: String,
    source: Arc<SourcePdf>,
    bytes: Vec<u8>,
}

fn sign_all() -> Vec<Signed> {
    let mut signer = KeystoreSigner::open("test-ecp256");
    let chain = signer.chain.clone();
    let mut out = Vec::new();
    for (flavour, name, data) in inputs() {
        let src = Arc::new(SourcePdf::open(data, &Limits::default()).unwrap());
        let report = analyze(&src).unwrap();
        assert!(report.pdfa.is_some(), "{name}: PDF/A erkannt");
        for (variant, profile, em) in variants() {
            let time = pdfover_pades::local_signing_time();
            let plan = match profile {
                None => SignPlan::invisible(time),
                Some(id) => {
                    let p = store().profile(id).expect("Profil");
                    let opts = PlanOptions::new(time);
                    let mut input = SigBlockInput::new(cert_fields(&chain.signer), time);
                    input.emblem = em.map(emblem);
                    plan_signature(&src, &report, p, &input, &opts, fonts(), &**store().resources(), None)
                        .unwrap_or_else(|e| panic!("{name}/{variant}: planen: {e}"))
                        .plan
                }
            };
            let prepared = prepare(&src, &plan, Some(&chain)).unwrap_or_else(|e| panic!("{name}/{variant}: {e}"));
            let signed = signer.sign(prepared).unwrap_or_else(|e| panic!("{name}/{variant}: {e}"));
            out.push(Signed {
                flavour,
                label: format!("{name}__{variant}"),
                source: Arc::clone(&src),
                bytes: signed.bytes().to_vec(),
            });
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------
// Strukturprüfung ohne externe Werkzeuge
// ------------------------------------------------------------------------------------------------

fn dict_of<'a>(pdf: &'a SourcePdf, o: &'a Object) -> Option<&'a Dictionary> {
    match pdf.resolve(o).ok()? {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

fn name_of<'a>(pdf: &'a SourcePdf, d: &'a Dictionary, key: &[u8]) -> Option<&'a [u8]> {
    d.get(key).ok().and_then(|o| pdf.resolve(o).ok()).and_then(|o| o.as_name().ok())
}

/// Farbkomponenten des Ausgabeprofils der PDF/A-OutputIntent (`/N` des ICC-Streams).
fn output_intent_components(pdf: &SourcePdf) -> Option<i64> {
    let cat = pdf.catalog().ok()?;
    let intents = pdf.resolve(cat.get(b"OutputIntents").ok()?).ok()?.as_array().ok()?;
    for oi in intents {
        let d = dict_of(pdf, oi)?;
        if name_of(pdf, d, b"S") != Some(b"GTS_PDFA1") {
            continue;
        }
        let profile = dict_of(pdf, d.get(b"DestOutputProfile").ok()?)?;
        return profile.get(b"N").ok().and_then(|n| pdf.resolve(n).ok()).and_then(|n| n.as_i64().ok());
    }
    None
}

fn font_embedded(pdf: &SourcePdf, font: &Dictionary) -> bool {
    let descriptor = |f: &Dictionary| {
        f.get(b"FontDescriptor")
            .ok()
            .and_then(|d| dict_of(pdf, d))
            .is_some_and(|fd| fd.has(b"FontFile") || fd.has(b"FontFile2") || fd.has(b"FontFile3"))
    };
    match name_of(pdf, font, b"Subtype") {
        Some(b"Type0") => font
            .get(b"DescendantFonts")
            .ok()
            .and_then(|o| pdf.resolve(o).ok())
            .and_then(|o| o.as_array().ok())
            .is_some_and(|a| a.iter().all(|f| dict_of(pdf, f).is_some_and(descriptor))),
        Some(b"Type3") => true,
        _ => descriptor(font),
    }
}

/// Geprüfte neue Objekte je Art (gegen eine leere Prüfung).
#[derive(Debug, Default)]
struct Checked {
    objects: usize,
    fonts: usize,
    images: usize,
    widgets: usize,
}

/// Probleme des Increments (nur neue bzw. fortgeschriebene Objekte) nach ISO 19005.
fn structural_problems(s: &Signed, checked: &mut Checked) -> Vec<String> {
    let mut p = Vec::new();
    let out = SourcePdf::open(s.bytes.clone(), &Limits::default()).expect("Ausgabe lesbar");
    let part1 = s.flavour == "1b";
    let base = s.source.len();
    // §6.1.3: Trailer mit /ID; nach %%EOF höchstens ein EOL
    if out.trailer_id().is_none() {
        p.push("letzter Trailer ohne /ID".into());
    }
    let tail = &s.bytes[s.bytes.len().saturating_sub(8)..];
    if !(tail.ends_with(b"%%EOF") || tail.ends_with(b"%%EOF\n") || tail.ends_with(b"%%EOF\r\n")) {
        p.push(format!("Dateiende {:?}", String::from_utf8_lossy(tail)));
    }
    // §6.1.4: „xref“ + EOL, Unterabschnitte „a b“ mit genau einem Leerzeichen
    let inc = &s.bytes[base as usize..];
    if let Some(x) = inc.windows(4).position(|w| w == b"xref").filter(|&x| !inc[..x].ends_with(b"start")) {
        let after = &inc[x + 4..];
        if !(after.starts_with(b"\n") || after.starts_with(b"\r\n") || after.starts_with(b"\r")) {
            p.push("xref ohne EOL".into());
        }
        let header = after.split(|b| *b == b'\n').nth(1).unwrap_or_default();
        let header = String::from_utf8_lossy(header).trim_end_matches('\r').to_string();
        if header.split(' ').count() != 2
            || header.split(' ').any(|t| t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()))
        {
            p.push(format!("Unterabschnittskopf {header:?}"));
        }
    }
    // §6.1.8: „n g obj“ + EOL, EOL + „endobj“
    for (i, w) in inc.windows(4).enumerate() {
        if w == b" obj" && !matches!(inc.get(i + 4), Some(b'\n' | b'\r')) {
            p.push(format!("„obj“ ohne EOL bei +{i}"));
            break;
        }
    }
    let new_ids: BTreeSet<ObjectId> = out
        .object_locations()
        .into_iter()
        .filter(|(_, loc)| matches!(loc, ObjectLocation::Offset(o) if *o >= base))
        .map(|(id, _)| id)
        .collect();
    let oi_n = output_intent_components(&out);
    for id in &new_ids {
        let Ok(obj) = out.get_object(*id) else { continue };
        let d = match obj {
            Object::Dictionary(d) => d,
            Object::Stream(st) => &st.dict,
            _ => continue,
        };
        let what = format!("Objekt {} {}", id.0, id.1);
        checked.objects += 1;
        // §6.1.7: keine externen Streams
        if matches!(obj, Object::Stream(_)) && (d.has(b"F") || d.has(b"FFilter") || d.has(b"FDecodeParms")) {
            p.push(format!("{what}: externer Stream"));
        }
        if part1 {
            // §6.4: keine SMask, keine Transparenzgruppen
            if d.has(b"SMask") && name_of(&out, d, b"Subtype") == Some(b"Image") {
                p.push(format!("{what}: Bild mit /SMask (PDF/A-1)"));
            }
            if let Some(g) = d.get(b"Group").ok().and_then(|g| dict_of(&out, g))
                && name_of(&out, g, b"S") == Some(b"Transparency")
            {
                p.push(format!("{what}: Transparenzgruppe (PDF/A-1)"));
            }
        }
        // §6.2.3: DeviceRGB/DeviceCMYK nur passend zur OutputIntent, DeviceGray nur mit OutputIntent
        if name_of(&out, d, b"Subtype") == Some(b"Image") {
            checked.images += 1;
            match name_of(&out, d, b"ColorSpace") {
                Some(b"DeviceRGB") if oi_n != Some(3) => p.push(format!("{what}: DeviceRGB ohne RGB-OutputIntent")),
                Some(b"DeviceCMYK") if oi_n != Some(4) => p.push(format!("{what}: DeviceCMYK ohne CMYK-OutputIntent")),
                Some(b"DeviceGray") if oi_n.is_none() => p.push(format!("{what}: DeviceGray ohne OutputIntent")),
                _ => {}
            }
        }
        // §6.3.4: Fonts eingebettet
        checked.fonts += usize::from(name_of(&out, d, b"Type") == Some(b"Font"));
        if name_of(&out, d, b"Type") == Some(b"Font") && !font_embedded(&out, d) {
            p.push(format!(
                "{what}: Font nicht eingebettet ({:?})",
                name_of(&out, d, b"BaseFont").map(String::from_utf8_lossy)
            ));
        }
        // §6.5.3: Annotationen druckbar, nicht verborgen; /AP nur mit /N
        if name_of(&out, d, b"Type") == Some(b"Annot") || name_of(&out, d, b"Subtype") == Some(b"Widget") {
            checked.widgets += 1;
            let f = d.get(b"F").ok().and_then(|f| out.resolve(f).ok()).and_then(|f| f.as_i64().ok()).unwrap_or(0);
            if f & 4 == 0 || f & (1 | 2 | 32) != 0 {
                p.push(format!("{what}: Annotation /F {f}"));
            }
            if let Some(ap) = d.get(b"AP").ok().and_then(|a| dict_of(&out, a))
                && ap.iter().any(|(k, _)| k.as_slice() != b"N")
            {
                p.push(format!("{what}: /AP mit mehr als /N"));
            }
        }
        // §6.9: kein NeedAppearances true
        if let Ok(Object::Boolean(true)) = d.get(b"NeedAppearances") {
            p.push(format!("{what}: NeedAppearances true"));
        }
    }
    p
}

#[test]
fn pdfa_outputs_structural_checks() {
    let signed = sign_all();
    assert_eq!(signed.len(), inputs().len() * variants().len());
    let mut bad = Vec::new();
    let mut checked = Checked::default();
    for s in &signed {
        for problem in structural_problems(s, &mut checked) {
            bad.push(format!("{} ({}): {problem}", s.label, s.flavour));
        }
    }
    eprintln!("geprüft: {checked:?}");
    // jede Ausgabe hat mindestens ein Widget; sichtbare Blöcke Fonts und Bilder
    assert!(checked.widgets >= signed.len() && checked.fonts >= 3 * 7 && checked.images >= 3 * 4, "{checked:?}");
    assert!(bad.is_empty(), "PDF/A-Strukturbefunde:\n{}", bad.join("\n"));
}

// ------------------------------------------------------------------------------------------------
// veraPDF
// ------------------------------------------------------------------------------------------------

/// Gepinnte Version (tools/verapdf/fetch.sh).
const VERAPDF_JAR: &str = "greenfield-apps-1.28.2.jar";

struct VeraPdf {
    java: PathBuf,
    jar: PathBuf,
}

fn java_works(java: &Path) -> bool {
    Command::new(java).arg("-version").output().is_ok_and(|o| o.status.success())
}

fn verapdf() -> Option<VeraPdf> {
    let mut jars: Vec<PathBuf> = std::env::var_os("PDFOVER_VERAPDF_JAR").map(PathBuf::from).into_iter().collect();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    jars.push(workspace.join("tools/verapdf/dist").join(VERAPDF_JAR));
    if let Some(main) = std::env::var_os("PDFOVER_MAIN") {
        jars.push(PathBuf::from(main).join("tools/verapdf/dist").join(VERAPDF_JAR));
    }
    let jar = jars.into_iter().find(|p| p.is_file())?;
    let mut javas: Vec<PathBuf> = std::env::var_os("PDFOVER_JAVA").map(PathBuf::from).into_iter().collect();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        javas.push(PathBuf::from(home).join("bin/java"));
    }
    javas.push(PathBuf::from("java"));
    let java = javas.into_iter().find(|j| java_works(j))?;
    Some(VeraPdf { java, jar })
}

/// Ergebnis einer Datei im veraPDF-Bericht (MRR).
#[derive(Debug, Default)]
struct VeraReport {
    compliant: bool,
    /// verletzte Regeln: „Spezifikation Klausel-Test: Beschreibung“
    failed: BTreeMap<String, String>,
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(xml_unescape(&tag[start..end]))
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(&s[a..b])
}

/// Wertet einen MRR-Bericht aus: je `<job>` Dateiname, `isCompliant` und verletzte Regeln.
fn parse_mrr(xml: &str) -> Result<BTreeMap<String, VeraReport>, String> {
    let mut out = BTreeMap::new();
    for job in xml.split("<job>").skip(1) {
        let job = job.split("</job>").next().unwrap_or(job);
        let name = between(job, "<name>", "</name>").map(xml_unescape).ok_or("Job ohne <name>")?;
        let Some(vr_start) = job.find("<validationReport") else {
            let msg = between(job, "<exceptionMessage>", "</exceptionMessage>").unwrap_or("kein validationReport");
            return Err(format!("{name}: {}", xml_unescape(msg)));
        };
        let vr_tag = &job[vr_start..job[vr_start..].find('>').map_or(job.len(), |e| vr_start + e)];
        let mut report =
            VeraReport { compliant: attr(vr_tag, "isCompliant").as_deref() == Some("true"), ..Default::default() };
        for rule in job.split("<rule ").skip(1) {
            let tag = &rule[..rule.find('>').unwrap_or(rule.len())];
            let tag = format!(" {tag}");
            if attr(&tag, "status").as_deref() != Some("failed") {
                continue;
            }
            let key = format!(
                "{} {}-{}",
                attr(&tag, "specification").unwrap_or_default(),
                attr(&tag, "clause").unwrap_or_default(),
                attr(&tag, "testNumber").unwrap_or_default()
            );
            let description = between(rule, "<description>", "</description>").map(xml_unescape).unwrap_or_default();
            report.failed.insert(key, description);
        }
        out.insert(name, report);
    }
    if out.is_empty() {
        return Err("kein <job> im Bericht".into());
    }
    Ok(out)
}

impl VeraPdf {
    fn validate(&self, flavour: &str, files: &[PathBuf]) -> Result<(String, BTreeMap<String, VeraReport>), String> {
        let output = Command::new(&self.java)
            .arg("-Xmx1g")
            .arg("-cp")
            .arg(&self.jar)
            .arg("org.verapdf.apps.GreenfieldCliWrapper")
            .args(["--format", "mrr", "--flavour", flavour])
            .args(files)
            .output()
            .map_err(|e| format!("veraPDF startet nicht: {e}"))?;
        let xml = String::from_utf8_lossy(&output.stdout).into_owned();
        let reports = parse_mrr(&xml).map_err(|e| {
            format!("{e}\nExit {:?}\nstderr: {}", output.status.code(), String::from_utf8_lossy(&output.stderr))
        })?;
        Ok((xml, reports))
    }
}

#[test]
fn mrr_parser_reads_compliance_and_failed_rules() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?><report><jobs>
      <job><item size="10"><name>/tmp/a&amp;b.pdf</name></item>
        <validationReport jobEndStatus="normal" profileName="PDF/A-1B validation profile" statement="x" isCompliant="false">
          <details passedRules="1" failedRules="1" passedChecks="1" failedChecks="1">
            <rule specification="ISO 19005-1:2005" clause="6.1.3" testNumber="1" status="failed" failedChecks="1">
              <description>The file trailer dictionary shall contain the ID keyword</description><object>CosTrailer</object>
            </rule>
            <rule specification="ISO 19005-1:2005" clause="6.1.4" testNumber="2" status="passed" passedChecks="3"><description>ok</description></rule>
          </details></validationReport></job>
      <job><item size="10"><name>/tmp/c.pdf</name></item>
        <validationReport jobEndStatus="normal" profileName="p" statement="y" isCompliant="true"><details/></validationReport></job>
      </jobs></report>"#;
    let r = parse_mrr(xml).unwrap();
    assert_eq!(r.len(), 2);
    let a = &r["/tmp/a&b.pdf"];
    assert!(!a.compliant);
    assert_eq!(a.failed.keys().collect::<Vec<_>>(), ["ISO 19005-1:2005 6.1.3-1"]);
    assert!(a.failed.values().next().unwrap().contains("ID keyword"));
    assert!(r["/tmp/c.pdf"].compliant && r["/tmp/c.pdf"].failed.is_empty());
    let broken = "<report><jobs><job><item><name>x.pdf</name></item><taskException><exceptionMessage>kaputt</exceptionMessage></taskException></job></jobs></report>";
    assert!(parse_mrr(broken).unwrap_err().contains("kaputt"));
    assert!(parse_mrr("").is_err());
}

#[test]
fn pdfa_outputs_pass_verapdf() {
    let Some(vera) = verapdf() else {
        eprintln!(
            "veraPDF oder Java fehlt – PDF/A-Orakel übersprungen (tools/verapdf/fetch.sh, PDFOVER_VERAPDF_JAR, PDFOVER_JAVA)"
        );
        return;
    };
    let keep = std::env::var_os("PDFOVER_VERAPDF_KEEP").map(PathBuf::from);
    let tmp = tempfile::tempdir().unwrap();
    let dir = keep.clone().unwrap_or_else(|| tmp.path().to_path_buf());
    std::fs::create_dir_all(&dir).unwrap();
    let signed = sign_all();
    let mut regressions = Vec::new();
    let mut summary = Vec::new();
    for flavour in ["1b", "2b", "3b"] {
        let group: Vec<&Signed> = signed.iter().filter(|s| s.flavour == flavour).collect();
        let mut files = Vec::new();
        let source_path = dir.join(format!("source-{flavour}.pdf"));
        std::fs::write(&source_path, group[0].source.bytes()).unwrap();
        files.push(source_path.clone());
        for s in &group {
            let p = dir.join(format!("{}.pdf", s.label));
            std::fs::write(&p, &s.bytes).unwrap();
            files.push(p);
        }
        let (xml, reports) = vera.validate(flavour, &files).unwrap_or_else(|e| panic!("veraPDF ({flavour}): {e}"));
        if keep.is_some() {
            std::fs::write(dir.join(format!("verapdf-{flavour}.xml")), &xml).unwrap();
        }
        let find = |p: &Path| {
            let canon = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            reports
                .iter()
                .find(|(name, _)| {
                    Path::new(name) == p
                        || Path::new(name) == canon
                        || name.ends_with(&*p.file_name().unwrap().to_string_lossy())
                })
                .map(|(_, r)| r)
        };
        let source = find(&source_path).unwrap_or_else(|| panic!("kein Bericht für die Eingabe {flavour}"));
        summary.push(format!(
            "Eingabe {flavour}: konform {} ({} Regeln verletzt)",
            source.compliant,
            source.failed.len()
        ));
        for (s, path) in group.iter().zip(&files[1..]) {
            let r = find(path).unwrap_or_else(|| panic!("kein Bericht für {}", s.label));
            let new: Vec<String> = r
                .failed
                .iter()
                .filter(|(k, _)| !source.failed.contains_key(*k))
                .map(|(k, d)| format!("{k}: {d}"))
                .collect();
            summary.push(format!("{} ({flavour}): konform {}, neu verletzt {}", s.label, r.compliant, new.len()));
            if !new.is_empty() {
                regressions.push(format!("{} ({flavour}):\n    {}", s.label, new.join("\n    ")));
            }
        }
    }
    eprintln!("{}", summary.join("\n"));
    assert!(regressions.is_empty(), "veraPDF: durch das Signieren verletzte PDF/A-Regeln:\n{}", regressions.join("\n"));
}

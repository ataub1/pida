//! Robustheit: mutierte Korpusdateien durch `analyze` → `plan_signature` (ohne Renderer) →
//! `prepare` → `finish_raw` → `self_check`. Erwartet wird **kein Panic** (Fehler sind erlaubt);
//! gelingt das Signieren, muss die Selbstprüfung bestehen oder eine Verletzung melden.
//!
//! Standard: 25 Mutationen je Datei; mehr mit `PDFOVER_FUZZ_ITERS=<n>`, anderer Startwert mit
//! `PDFOVER_FUZZ_SEED=<u64>`.

mod common;

use common::*;
use pdfover_backend_keystore::LoadedKey;
use pdfover_cades::{RawSigner, SignedAttrsBuilder, SigningTime};
use pdfover_pades::{CheckContext, PlanOptions, SignPlan, analyze, prepare, reject_open_error};
use pdfover_pdf_cos::{Limits, SourcePdf};

const FILES: [&str; 7] = [
    "pdf/pdfas/pdf-as-legacy/simple.pdf",
    "pdf/generated/simple-xref-stream.pdf",
    "pdf/generated/simple-xref-hybrid.pdf",
    "pdf/generated/simple-aes256-owner.pdf",
    "pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf",
    "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf",
    "pdf/pdfas/pdf-as-cli/rotate_090.pdf",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate(rng: &mut Rng, data: &[u8]) -> Vec<u8> {
    let mut d = data.to_vec();
    match rng.below(5) {
        0 => {
            for _ in 0..1 + rng.below(8) {
                let i = rng.below(d.len());
                d[i] = rng.next() as u8;
            }
        }
        1 => {
            // Ziffern in Zahlen (Offsets, Längen, /Count) verändern
            for _ in 0..1 + rng.below(4) {
                let i = rng.below(d.len());
                if let Some(j) = d[i..].iter().position(u8::is_ascii_digit) {
                    d[i + j] = b'0' + (rng.next() % 10) as u8;
                }
            }
        }
        2 => d.truncate(d.len() * (50 + rng.below(50)) / 100),
        3 => {
            let i = rng.below(d.len());
            let junk: Vec<u8> = (0..1 + rng.below(64)).map(|_| rng.next() as u8).collect();
            d.splice(i..i, junk);
        }
        _ => {
            // Schlüsselwörter verfälschen
            for kw in [&b"/Annots"[..], b"/Kids", b"/Fields", b"/AcroForm", b"/Count", b"/Rotate", b"/StructTreeRoot"] {
                if rng.below(3) == 0
                    && let Some(p) = d.windows(kw.len()).position(|w| w == kw)
                {
                    d[p + 1] = b'X';
                }
            }
        }
    }
    d
}

fn sign(src: &SourcePdf, plan: &SignPlan, key: &LoadedKey) {
    let chain = key.chain().clone();
    let Ok(prepared) = prepare(src, plan, Some(&chain)) else { return };
    let alg = key.algorithm().digest();
    let digest = prepared.digest(alg);
    let attrs =
        SignedAttrsBuilder::new(prepared.cms_profile(), &digest, &chain.signer, SigningTime(prepared.signing_time()))
            .build()
            .unwrap();
    let sig = key.sign_digest(&attrs.digest(alg)).unwrap();
    let signed = prepared.finish_raw(&attrs, &sig, &chain).expect("eigene Signatur passt");
    // Verletzungen sind bei beschädigten Originalen (K2) erlaubt (dann wird nichts gespeichert),
    // bei intaktem Querverweis nicht; Panics nie
    if let Err(v) =
        signed.self_check(&CheckContext { source: src, expected_signer: Some(&chain.signer), renderer: None })
    {
        if std::env::var_os("PDFOVER_FUZZ_VERBOSE").is_some() {
            eprintln!("Verletzung ({:?}): {v}", src.xref_health());
        }
        if let Some(dir) = std::env::var_os("PDFOVER_FUZZ_DUMP") {
            let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let name = format!("{n}-{:?}-{:?}.pdf", src.xref_health(), v.invariant);
            let _ = std::fs::write(std::path::PathBuf::from(dir).join(name), src.bytes());
        }
        // Bei intaktem Original darf keine Invariante scheitern (das wäre ein Fehler der Engine),
        // auch I5 nicht: Querverweiseinträge, die auf ein anderes Objekt zeigen, korrigiert
        // `pdf-cos` beim Öffnen von Quelle **und** Ausgabe gleich (`testdata/pades/
        // repro-xref-misassigned.pdf`).
        assert!(src.xref_health() != pdfover_pdf_cos::XrefHealth::Intact, "Selbstprüfung bei intaktem Original: {v}");
    }
}

/// Signiert unsichtbar und sichtbar; bei intaktem Querverweis muss die Selbstprüfung bestehen.
fn sign_both(src: &SourcePdf, key: &LoadedKey) {
    let report = analyze(src).unwrap();
    let opts = PlanOptions::new(pdfover_pades::local_signing_time());
    let planned = plan_for(src, &report, "SIGNATURBLOCK_SMALL_DE", &key.chain().signer, &opts, None).unwrap();
    sign(src, &planned.plan, key);
    sign(src, &SignPlan::invisible(pdfover_pades::local_signing_time()), key);
}

fn test_key() -> LoadedKey {
    let p12 = std::fs::read(key_p12("test-ecp256")).unwrap();
    LoadedKey::open_pkcs12(&p12, "test", None).unwrap()
}

/// Reproduktion aus dem Mutationstest (`rotate_090.pdf`, xref-Unterabschnitt `0 2` → `1 2`: der
/// Katalog gilt in der letzten Revision als frei, Objekt 2 zeigt auf den Katalog). Früher sah der
/// Re-Parse der Ausgabe die alte Katalogfassung (I5); `pdf-cos` liest Quelle und Ausgabe jetzt
/// gleich, die Selbstprüfung besteht.
#[test]
fn misassigned_xref_entries_fail_safe() {
    let data = std::fs::read(testdata().join("pades/repro-xref-misassigned.pdf")).unwrap();
    let src = SourcePdf::open(data, &Limits::default()).unwrap();
    let key = test_key();
    sign_both(&src, &key);
    // ausdrücklich: auch die Selbstprüfung besteht (unabhängig von der Einstufung der Quelle)
    let chain = key.chain().clone();
    let prepared = prepare(&src, &SignPlan::invisible(pdfover_pades::local_signing_time()), Some(&chain)).unwrap();
    let alg = key.algorithm().digest();
    let attrs = SignedAttrsBuilder::new(
        prepared.cms_profile(),
        &prepared.digest(alg),
        &chain.signer,
        SigningTime(prepared.signing_time()),
    )
    .build()
    .unwrap();
    let sig = key.sign_digest(&attrs.digest(alg)).unwrap();
    let signed = prepared.finish_raw(&attrs, &sig, &chain).unwrap();
    signed.self_check(&CheckContext { source: &src, expected_signer: Some(&chain.signer), renderer: None }).unwrap();
}

/// Weitere Funde des Mutationstests mit intaktem Querverweis (als Byte-Patch der Korpusdateien):
/// - `/DR /Font 29 h R` (kein Dictionary): der PDFBox-Fixup ersetzt es durch ein direktes
///   Dictionary, I6 lässt genau das zu;
/// - `/MediaBox [0 0.595.32 841.92]`: die Originalschreibweise des fehlerhaften Reals wird beim
///   Fortschreiben der Seite nicht übernommen (sonst fünf statt vier Werte, I6).
#[test]
fn known_mutations_with_intact_xref_sign_cleanly() {
    let key = test_key();
    for (rel, from, to) in [
        ("pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf", &b"/Font 29 0 R"[..], &b"/Font 29 h R"[..]),
        ("pdf/pdfas/pdf-as-cli/rotate_090.pdf", b"/MediaBox[0 0 595.32 841.92]", b"/MediaBox[0 0.595.32 841.92]"),
    ] {
        let mut data = std::fs::read(testdata().join(rel)).unwrap();
        let pos = data.windows(from.len()).position(|w| w == from).unwrap_or_else(|| panic!("{rel}: Muster fehlt"));
        data[pos..pos + to.len()].copy_from_slice(to);
        let src = SourcePdf::open(data, &Limits::default()).unwrap();
        assert_eq!(src.xref_health(), pdfover_pdf_cos::XrefHealth::Intact, "{rel}");
        sign_both(&src, &key);
    }
}

#[test]
fn mutated_inputs_do_not_panic() {
    let iters: usize = std::env::var("PDFOVER_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(25);
    let key = test_key();
    let seed =
        std::env::var("PDFOVER_FUZZ_SEED").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0x5eed_1234_abcd_ef01);
    let mut rng = Rng(seed.max(1));
    let mut signed = 0usize;
    for rel in FILES {
        let data = std::fs::read(testdata().join(rel)).unwrap();
        for i in 0..iters {
            let m = mutate(&mut rng, &data);
            let label = format!("{rel} Mutation {i}");
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let src = match SourcePdf::open(m.clone(), &Limits::default()) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = reject_open_error(&e);
                        return false;
                    }
                };
                let Ok(report) = analyze(&src) else { return false };
                let _ = pdfover_pades::empty_fields(&report);
                sign(&src, &SignPlan::invisible(pdfover_pades::local_signing_time()), &key);
                let opts = PlanOptions::new(pdfover_pades::local_signing_time());
                if let Ok(planned) = plan_for(&src, &report, "SIGNATURBLOCK_SMALL_DE", &key.chain().signer, &opts, None)
                {
                    sign(&src, &planned.plan, &key);
                    return true;
                }
                false
            }));
            match r {
                Ok(true) => signed += 1,
                Ok(false) => {}
                Err(_) => panic!("Panic bei {label}"),
            }
        }
    }
    eprintln!("{signed} mutierte Dokumente sichtbar signiert");
}

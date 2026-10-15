//! Robustheit: mutierte Korpusdateien durch `analyze` → `plan_signature` (ohne Renderer) →
//! `prepare` → `finish_raw` → `self_check`. Erwartet wird **kein Panic** (Fehler sind erlaubt);
//! gelingt das Signieren, muss die Selbstprüfung bestehen oder eine Verletzung melden.
//!
//! Standard: 25 Mutationen je Datei; mehr mit `PDFOVER_FUZZ_ITERS=<n>`.

mod common;

use common::*;
use pdfover_backend_keystore::LoadedKey;
use pdfover_cades::{RawSigner, SignedAttrsBuilder, SigningTime};
use pdfover_pades::{CheckContext, Invariant, PlanOptions, SignPlan, analyze, prepare, reject_open_error};
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
            let _ =
                std::fs::write(std::path::PathBuf::from(dir).join(format!("{n}-{:?}.pdf", v.invariant)), src.bytes());
        }
        // I5 (Re-Parse) darf auch bei „intaktem“ Original scheitern: Querverweise, deren Einträge
        // auf ein anderes Objekt zeigen, korrigiert `pdf-cos` beim Öffnen, liest die Ausgabe aber
        // teils mit der alten Objektfassung (offener Punkt Paket A, `testdata/pades/
        // repro-xref-misassigned.pdf`) – dann wird nichts gespeichert. I1–I4 und I6 dürfen bei
        // intaktem Original nie scheitern (das wären Fehler der Engine).
        assert!(
            src.xref_health() != pdfover_pdf_cos::XrefHealth::Intact || v.invariant == Invariant::I5Reparse,
            "Selbstprüfung bei intaktem Original: {v}"
        );
    }
}

/// Reproduktion aus dem Mutationstest (`rotate_090.pdf`, xref-Unterabschnitt `0 2` → `1 2`: der
/// Katalog gilt in der letzten Revision als frei, Objekt 2 zeigt auf den Katalog): Signieren
/// gelingt, die Selbstprüfung erkennt den fehlerhaften Re-Parse (I5) bzw. besteht – nie eine
/// andere Verletzung, nie ein Panic.
#[test]
fn misassigned_xref_entries_fail_safe() {
    let data = std::fs::read(testdata().join("pades/repro-xref-misassigned.pdf")).unwrap();
    let src = SourcePdf::open(data, &Limits::default()).unwrap();
    let p12 = std::fs::read(key_p12("test-ecp256")).unwrap();
    let key = LoadedKey::open_pkcs12(&p12, "test", None).unwrap();
    let report = analyze(&src).unwrap();
    let opts = PlanOptions::new(pdfover_pades::local_signing_time());
    let planned = plan_for(&src, &report, "SIGNATURBLOCK_SMALL_DE", &key.chain().signer, &opts, None).unwrap();
    sign(&src, &planned.plan, &key);
    sign(&src, &SignPlan::invisible(pdfover_pades::local_signing_time()), &key);
}

#[test]
fn mutated_inputs_do_not_panic() {
    let iters: usize = std::env::var("PDFOVER_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(25);
    let p12 = std::fs::read(key_p12("test-ecp256")).unwrap();
    let key = LoadedKey::open_pkcs12(&p12, "test", None).unwrap();
    let mut rng = Rng(0x5eed_1234_abcd_ef01);
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

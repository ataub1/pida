//! Robustheit des Writers (Plan §7.7, §8.4): feindliche Trailer-Werte und mutierte Eingaben dürfen
//! weder paniken noch ein Increment liefern, das die Invarianten I1/I2 verletzt.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use pida_pdf_cos::{Limits, Object, SourcePdf};
use pida_pdf_incremental::{IncError, IncrementBuilder, IncrementOptions, RESERVE_MIN_MODEL_A, XrefFormat};

/// Einfaches Dokument mit frei wählbarem Trailer-Zusatz (klassische Tabelle).
fn simple_with_trailer(extra: &str) -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>",
    ];
    let mut buf = b"%PDF-1.4\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(buf.len());
        buf.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = buf.len();
    buf.extend_from_slice(b"xref\n0 4\n0000000000 65535 f\r\n");
    for o in offs {
        buf.extend_from_slice(format!("{o:010} 00000 n\r\n").as_bytes());
    }
    buf.extend_from_slice(format!("trailer\n<< /Root 1 0 R {extra} >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    buf
}

/// `/Size` am Rand des Objektnummernraums: kein Überlauf, sondern ein Fehler.
#[test]
fn object_number_space_exhausted_is_an_error() {
    for size in ["/Size 4294967295", "/Size 4294967294", "/Size 99999999999"] {
        let src = SourcePdf::open(simple_with_trailer(size), &Limits::default()).unwrap();
        for format in [XrefFormat::Table, XrefFormat::Stream, XrefFormat::Hybrid] {
            let r = catch_unwind(AssertUnwindSafe(|| {
                let opts = IncrementOptions { xref: format, ..IncrementOptions::default() };
                let mut b = IncrementBuilder::new(&src, opts);
                let _ = b.add_signature_dictionary(Default::default());
                let _ = b.add_object(Object::Null);
                b.build()
            }));
            match r {
                Ok(Err(IncError::Serialize(_) | IncError::DuplicateObject(_))) => {}
                Ok(Ok(inc)) => panic!("{size} {format:?}: Increment trotz erschöpfter Objektnummern ({inc:?})"),
                Ok(Err(e)) => panic!("{size} {format:?}: unerwarteter Fehler {e}"),
                Err(_) => panic!("{size} {format:?}: Panic"),
            }
        }
    }
    // Normale Größe: weiterhin ein gültiges Increment
    let src = SourcePdf::open(simple_with_trailer("/Size 4"), &Limits::default()).unwrap();
    let inc = common::add_dummy_signature(&src, IncrementOptions::default(), RESERVE_MIN_MODEL_A).unwrap();
    assert!(inc.byte_range.is_consistent_with(inc.total_len()));
}

/// Mutierte Korpusdateien: öffnen → Increment mit Dummy-Signatur → erneut öffnen. Panics sind
/// verboten; gelingt das Increment, gelten I1 (Präfix) und I2 (ByteRange).
#[test]
fn mutated_inputs_never_break_the_writer() {
    let iters: usize = std::env::var("PIDA_FUZZ_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let mut state: u64 = std::env::var("PIDA_FUZZ_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0xC0FF_EE11);
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut built = 0usize;
    let mut failures = Vec::new();
    for path in common::corpus() {
        let data = std::fs::read(&path).unwrap();
        for it in 0..iters {
            let mut m = data.clone();
            for _ in 0..1 + next() % 6 {
                let i = (next() as usize) % m.len();
                match next() % 3 {
                    0 => m[i] ^= 1 << (next() % 8),
                    1 if m[i].is_ascii_digit() => m[i] = b'0' + (next() % 10) as u8,
                    _ => {
                        let end = (i + 1 + (next() as usize) % 64).min(m.len());
                        m.drain(i..end);
                    }
                }
                if m.is_empty() {
                    m.extend_from_slice(b"%PDF-1.4\n");
                }
            }
            let r = catch_unwind(AssertUnwindSafe(|| -> Option<bool> {
                let src = SourcePdf::open(m.clone(), &Limits::default()).ok()?;
                let inc = common::add_dummy_signature(&src, IncrementOptions::default(), 64).ok()?;
                let out = inc.to_vec(src.bytes());
                let ok = out.starts_with(src.bytes()) && inc.byte_range.is_consistent_with(out.len() as u64);
                let _ = SourcePdf::open(out, &Limits::default());
                Some(ok)
            }));
            match r {
                Err(_) => failures.push(format!("{} #{it}: Panic", common::rel(&path))),
                Ok(Some(false)) => failures.push(format!("{} #{it}: Invariante verletzt", common::rel(&path))),
                Ok(Some(true)) => built += 1,
                Ok(None) => {}
            }
        }
    }
    eprintln!("Writer-Fuzzing: {built} Increments gebaut");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(built > 0);
}

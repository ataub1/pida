//! Gemeinsame Hilfen der Integrationstests (Testdaten nur aus `testdata/`, nie aus `reference/`).
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use pdfover_cades::{CertChain, Certificate, LocalSigner};

pub fn testdata(rel: &str) -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(rel)
}

pub fn read(rel: &str) -> Vec<u8> {
    std::fs::read(testdata(rel)).unwrap_or_else(|e| panic!("Testdatei {rel}: {e}"))
}

pub fn cert(rel: &str) -> Certificate {
    Certificate::from_der(&read(rel)).expect("Zertifikat")
}

/// Signer aus `testdata/cades/<name>.key.p8.der` und dem Zertifikat `cert_rel`.
pub fn signer(key_name: &str, cert_rel: &str) -> LocalSigner {
    let key = read(&format!("cades/{key_name}.key.p8.der"));
    LocalSigner::from_pkcs8_der(&key, CertChain::new(cert(cert_rel))).expect("LocalSigner")
}

pub fn rsa2048() -> LocalSigner {
    signer("test-rsa2048", "keystores/test-rsa2048.crt.der")
}
pub fn ecp256() -> LocalSigner {
    signer("test-ecp256", "keystores/test-ecp256.crt.der")
}
pub fn ecp384() -> LocalSigner {
    signer("test-ecp384-title", "keystores/test-ecp384-title.crt.der")
}
pub fn p521() -> LocalSigner {
    signer("leaf-p521", "cades/leaf-p521.crt.der")
}
pub fn rsa3072() -> LocalSigner {
    signer("leaf-rsa3072", "cades/leaf-rsa3072.crt.der")
}
/// RSA-4608, selbstsigniert: über der 4096-Bit-Grenze, die `rsa` beim DER-Import setzt (Review).
pub fn rsa4608() -> LocalSigner {
    signer("selfsigned-rsa4608", "cades/selfsigned-rsa4608.crt.der")
}

pub fn content() -> Vec<u8> {
    read("cades/content.bin")
}

/// `openssl` ≥ 3.2 mit `cms -cades` verfügbar? Sonst werden Live-Orakeltests übersprungen.
pub fn openssl() -> Option<PathBuf> {
    let out = Command::new("openssl").args(["cms", "-help"]).output().ok()?;
    let help = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    if help.contains("-cades") { Some(PathBuf::from("openssl")) } else { None }
}

/// Signatur aus einem PDF: (ByteRange-Inhalt, `/Contents`-Bytes inkl. Padding). Nimmt alle
/// Signaturen in Dateireihenfolge.
pub fn pdf_signatures(pdf: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    let needle = b"/ByteRange";
    let mut pos = 0;
    while let Some(i) = find(&pdf[pos..], needle) {
        let start = pos + i + needle.len();
        pos = start;
        let rest = &pdf[start..];
        let open = rest.iter().position(|&b| b == b'[').unwrap();
        let close = rest.iter().position(|&b| b == b']').unwrap();
        let nums: Vec<usize> = std::str::from_utf8(&rest[open + 1..close])
            .unwrap()
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        if nums.len() != 4 || nums[1] == 0 || nums[2] > pdf.len() {
            continue; // Platzhalter o. ä.
        }
        let (a, b, c, d) = (nums[0], nums[1], nums[2], nums[3]);
        if pdf[b] != b'<' || pdf[c - 1] != b'>' {
            continue;
        }
        let hex = &pdf[b + 1..c - 1];
        let contents = hex::decode(hex).expect("Hex in /Contents");
        let mut data = pdf[a..a + b].to_vec();
        data.extend_from_slice(&pdf[c..c + d]);
        out.push((data, contents));
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Erster abweichender Offset (ohne Bytes auszugeben – Zertifikate können Personendaten tragen).
pub fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    if a == b {
        return None;
    }
    Some(a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len())))
}

pub fn assert_same_bytes(actual: &[u8], expected: &[u8], what: &str) {
    if let Some(off) = first_diff(actual, expected) {
        panic!("{what}: erste Abweichung bei Offset {off} (Längen {} / {})", actual.len(), expected.len());
    }
}

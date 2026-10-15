//! Regressionstests aus dem adversarialen Review von Paket B: bösartige signing-time-Kodierungen
//! (früher Panic), RSA-Schlüssel über 4096 Bit (früher von `rsa` beim DER-Import abgelehnt),
//! PKCS#8 und Zertifikat mit verschiedenen Schlüsseltypen, redigierte `Debug`-Ausgaben und ein
//! kleiner Mutations-Durchlauf über den Parser (kein Panic bei kaputten Eingaben).

mod common;

use std::process::Command;

use pdfover_cades::{
    AssembleOptions, CertChain, CmsError, CmsProfile, DigestAlg, Expect, ExpectedSigner, KeyType, LocalSigner,
    RawSigner, SignatureAlg, SignedAttrs, SignedAttrsBuilder, SigningTime, assemble, estimate_cms_len, parse_cms,
    sign_detached, verify_detached, verify_raw,
};
use time::OffsetDateTime;

use common::*;

fn golden_time() -> SigningTime {
    SigningTime(OffsetDateTime::from_unix_timestamp(1_791_979_200).unwrap())
}

fn verify(cms: &[u8], content: &[u8], expect: &Expect) -> pdfover_cades::Result<pdfover_cades::CmsReport> {
    verify_detached(cms, &mut &content[..], expect)
}

fn tlv(tag: u8, c: &[u8]) -> Vec<u8> {
    let mut v = vec![tag];
    match c.len() {
        n if n < 0x80 => v.push(n as u8),
        n if n < 0x100 => v.extend([0x81, n as u8]),
        n => v.extend([0x82, (n >> 8) as u8, n as u8]),
    }
    v.extend_from_slice(c);
    v
}

/// Elemente eines konstruierten DER-Inhalts (Testhilfe, Längen < 64 KiB).
fn split(content: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < content.len() {
        let (len, hl) = match content[pos + 1] {
            l if l < 0x80 => (l as usize, 2),
            0x81 => (content[pos + 2] as usize, 3),
            0x82 => (((content[pos + 2] as usize) << 8) | content[pos + 3] as usize, 4),
            _ => panic!("Länge"),
        };
        out.push(content[pos..pos + hl + len].to_vec());
        pos += hl + len;
    }
    out
}

/// Ersetzt den Wert von signing-time (UTCTime) in den Attributen und montiert ein gültig signiertes CMS.
fn cms_with_signing_time(raw_time: &[u8]) -> Vec<u8> {
    let s = ecp256();
    let md = DigestAlg::Sha256.digest(&content());
    let attrs =
        SignedAttrsBuilder::new(CmsProfile::PdfasCompat, &md, &s.chain().signer, golden_time()).build().unwrap();
    let hl = if attrs.der[1] & 0x80 != 0 { 2 + (attrs.der[1] & 0x7F) as usize } else { 2 };
    let signing_time_oid = [0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x05];
    let items: Vec<Vec<u8>> = split(&attrs.der[hl..])
        .into_iter()
        .map(|it| {
            if it.windows(signing_time_oid.len()).any(|w| w == signing_time_oid) {
                let mut c = signing_time_oid.to_vec();
                c.extend(tlv(0x31, &tlv(0x17, raw_time)));
                tlv(0x30, &c)
            } else {
                it
            }
        })
        .collect();
    let attrs = SignedAttrs { der: tlv(0x31, &items.concat()), ..attrs };
    let sig = s.sign_digest(&attrs.digest(DigestAlg::Sha256)).unwrap();
    assemble(&attrs, &sig, s.chain(), &AssembleOptions::default()).unwrap()
}

#[test]
fn hostile_signing_time_is_an_error_not_a_panic() {
    let content = content();
    // Kontrolle: gültige Offset-Form wird gelesen (signiert, also auch Signatur gültig)
    let ok = cms_with_signing_time(b"261014140000+0200");
    let r = verify(&ok, &content, &Expect::default()).unwrap();
    assert_eq!(r.signing_time.unwrap().unix_timestamp(), 1_791_979_200);
    // Nicht-ASCII im Offset bzw. Datum: früher Panic beim Slicing an einer Nicht-Zeichengrenze
    for raw in ["2610141200+1\u{e9}1", "261014120000+0\u{20ac}", "2610141200\u{e9}", "26101412+1+1Z"] {
        let cms = cms_with_signing_time(raw.as_bytes());
        let r = std::panic::catch_unwind(|| verify(&cms, &content, &Expect::default()));
        assert!(matches!(r, Ok(Err(CmsError::Der(_)))), "{raw:?}: {r:?}");
    }
}

#[test]
fn rsa_keys_above_4096_bits_sign_and_verify() {
    let s = rsa4608();
    assert_eq!(s.key_type(), KeyType::Rsa { bits: 4608 });
    assert_eq!(s.chain().signer.key_type().unwrap(), KeyType::Rsa { bits: 4608 });
    assert_eq!(s.algorithm(), SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256));
    let content = content();
    for profile in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
        let cms = sign_detached(&s, &DigestAlg::Sha256.digest(&content), profile, golden_time()).unwrap();
        let expect = Expect { signer: ExpectedSigner::Certificate(s.chain().signer.clone()), ..Expect::default() };
        let r = verify(&cms, &content, &expect).unwrap();
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        let est = estimate_cms_len(s.chain(), profile, s.algorithm(), false).unwrap();
        assert!(est >= cms.len() && est - cms.len() <= 8);
    }
    let d = DigestAlg::Sha512.digest(b"x");
    let sig = s.sign_digest(&d).unwrap();
    assert_eq!(sig.value.len(), 576);
    verify_raw(&s.chain().signer, &d, &sig).unwrap();
    // OpenSSL-CMS (Fixture) mit demselben Zertifikat
    let fixture = read("cades/openssl-rsa4608-sha256-cades.cms.der");
    let expect = Expect { signer: ExpectedSigner::Certificate(s.chain().signer.clone()), ..Expect::default() };
    let r = verify(&fixture, &content, &expect).unwrap();
    assert_eq!(r.signature_alg, SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256));
    let mut bad = content.clone();
    bad[0] ^= 1;
    assert!(matches!(verify(&fixture, &bad, &expect), Err(CmsError::DigestMismatch)));
}

#[test]
fn rsa4608_cms_verifies_with_openssl() {
    if openssl().is_none() {
        eprintln!("openssl ≥ 3.2 nicht gefunden – Orakeltest übersprungen");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let s = rsa4608();
    let content = content();
    let cms = sign_detached(&s, &DigestAlg::Sha256.digest(&content), CmsProfile::PadesBB, golden_time()).unwrap();
    let (c, m) = (dir.path().join("c.bin"), dir.path().join("m.der"));
    std::fs::write(&c, &content).unwrap();
    std::fs::write(&m, &cms).unwrap();
    let out = Command::new("openssl")
        .args(["cms", "-verify", "-binary", "-inform", "DER", "-noverify", "-out"])
        .arg(dir.path().join("o.bin"))
        .arg("-in")
        .arg(&m)
        .arg("-content")
        .arg(&c)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn pkcs8_and_certificate_of_different_key_types_are_a_mismatch() {
    let rsa_key = read("cades/test-rsa2048.key.p8.der");
    let p256_key = read("cades/test-ecp256.key.p8.der");
    let p384_key = read("cades/test-ecp384-title.key.p8.der");
    let rsa_cert = cert("keystores/test-rsa2048.crt.der");
    let p256_cert = cert("keystores/test-ecp256.crt.der");
    let p384_cert = cert("keystores/test-ecp384-title.crt.der");
    let p521_cert = cert("cades/leaf-p521.crt.der");
    for (key, c) in [
        (&p256_key, &rsa_cert),
        (&rsa_key, &p256_cert),
        (&p256_key, &p384_cert),
        (&p384_key, &p256_cert),
        (&p384_key, &p521_cert),
    ] {
        let r = LocalSigner::from_pkcs8_der(key, CertChain::new(c.clone()));
        assert!(matches!(r, Err(CmsError::KeyMismatch)), "{r:?}");
    }
    // passende Paare weiterhin in Ordnung
    LocalSigner::from_pkcs8_der(&rsa_key, CertChain::new(rsa_cert)).unwrap();
    LocalSigner::from_pkcs8_der(&p384_key, CertChain::new(p384_cert)).unwrap();
    // kaputtes PKCS#8 bleibt ein Schlüsselfehler
    for junk in [&b""[..], b"\x30\x00", b"\x30\x03\x02\x01\x00", b"\x04\x02ab"] {
        assert!(matches!(LocalSigner::from_pkcs8_der(junk, CertChain::new(p256_cert.clone())), Err(CmsError::Key(_))));
    }
}

#[test]
fn rdn_attributes_debug_hides_values() {
    let c = cert("keystores/test-ecp384-title.crt.der");
    let rdns = c.subject_rdns();
    assert!(rdns.iter().any(|a| a.value.contains("NUR TEST")));
    let dbg = format!("{rdns:?}");
    assert!(!dbg.contains("NUR TEST"), "{dbg}");
    assert!(dbg.contains("2.5.4.3") && dbg.contains("CN"), "{dbg}");
}

#[test]
fn parser_survives_mutations_and_truncations() {
    // Kurzfassung des Mutations-Durchlaufs aus dem Review (der ausführliche lief mit > 370 000
    // Varianten ohne Panic): jede Position einmal verändert bzw. abgeschnitten → nur parse_cms.
    let content = content();
    let base =
        sign_detached(&rsa2048(), &DigestAlg::Sha256.digest(&content), CmsProfile::PdfasCompat, golden_time()).unwrap();
    for i in 0..base.len() {
        for v in [0x00u8, 0x80, 0xFF] {
            let mut m = base.clone();
            m[i] = v;
            let _ = parse_cms(&m);
        }
        let _ = parse_cms(&base[..i]);
        let _ = verify(&base[..i], &content, &Expect::default());
    }
    // tief verschachtelte BER-Struktur und riesige Längenangaben
    let mut deep = vec![0x30, 0x80];
    for _ in 0..10_000 {
        deep.extend_from_slice(&[0x30, 0x80]);
    }
    assert!(parse_cms(&deep).is_err());
    assert!(parse_cms(&[0x30, 0x88, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_err());
    assert!(parse_cms(&[0x30, 0x84, 0xFF, 0xFF, 0xFF, 0xFF]).is_err());
}

#[test]
fn trailing_elements_inside_signed_data_are_rejected() {
    // ContentInfo { OID, [0] { SignedData { …, signerInfos, <Zusatz> } } } – ASN.1 kennt dort keine
    // Erweiterung; der Verifier akzeptierte das bisher stillschweigend.
    let content = content();
    let cms =
        sign_detached(&ecp256(), &DigestAlg::Sha256.digest(&content), CmsProfile::PadesBB, golden_time()).unwrap();
    verify(&cms, &content, &Expect::default()).unwrap();
    let hl = |b: &[u8]| if b[1] & 0x80 != 0 { 2 + (b[1] & 0x7F) as usize } else { 2 };
    let ci = &cms[hl(&cms)..];
    let parts = split(ci);
    assert_eq!(parts.len(), 2);
    let explicit = &parts[1];
    let sd = &explicit[hl(explicit)..];
    let sd_inner = &sd[hl(sd)..];
    let extra = [0x05, 0x00];
    let rebuild = |sd_content: &[u8], after_sd: &[u8]| {
        let mut inner = tlv(0x30, sd_content);
        inner.extend_from_slice(after_sd);
        tlv(0x30, &[parts[0].clone(), tlv(0xA0, &inner)].concat())
    };
    assert_eq!(rebuild(sd_inner, &[]), cms, "Testhilfe baut das Original nach");
    let in_sd = rebuild(&[sd_inner, &extra].concat(), &[]);
    assert!(matches!(verify(&in_sd, &content, &Expect::default()), Err(CmsError::Der(_))));
    assert!(parse_cms(&in_sd).is_err());
    let after_sd = rebuild(sd_inner, &extra);
    assert!(matches!(verify(&after_sd, &content, &Expect::default()), Err(CmsError::Der(_))));
}

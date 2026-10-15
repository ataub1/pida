//! Kreuzvalidierung mit OpenSSL in beiden Richtungen (Plan §8.4):
//!
//! - eigene CMS → `openssl cms -verify -binary -content … -inform DER -noverify -cades`
//!   (prüft auch ESS signing-certificate);
//! - OpenSSL-CMS → eigenes [`verify_detached`]: eingecheckte Fixtures (`testdata/cades/openssl-*`,
//!   laufen immer) und frisch erzeugte (nur wenn `openssl` ≥ 3.2 vorhanden, sonst übersprungen).

mod common;

use std::path::Path;
use std::process::Command;

use pdfover_cades::{
    AlgorithmPolicy, AssembleOptions, CertChain, Certificate, CmsError, CmsProfile, DigestAlg, Expect, ExpectedSigner,
    LocalSigner, RawSigner, SignatureAlg, SignedAttrsBuilder, SigningTime, assemble, sign_detached, verify_detached,
};
use time::OffsetDateTime;

use common::*;

fn now() -> SigningTime {
    SigningTime(OffsetDateTime::now_utc())
}

fn verify(cms: &[u8], content: &[u8], expect: &Expect) -> pdfover_cades::Result<pdfover_cades::CmsReport> {
    verify_detached(cms, &mut &content[..], expect)
}

// ---------------------------------------------------------------------------------------------
// OpenSSL → eigener Verifier (Fixtures, immer)
// ---------------------------------------------------------------------------------------------

#[test]
fn fixtures_from_openssl_verify() {
    let content = content();
    let rsa = cert("keystores/test-rsa2048.crt.der");
    let cases: [(&str, &Certificate, SignatureAlg); 5] = [
        ("openssl-rsa2048-sha256-cades", &rsa, SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256)),
        ("openssl-rsa2048-sha512-cades", &rsa, SignatureAlg::RsaPkcs1v15(DigestAlg::Sha512)),
        ("openssl-ecp256-sha256-cades", &cert("keystores/test-ecp256.crt.der"), SignatureAlg::Ecdsa(DigestAlg::Sha256)),
        (
            "openssl-ecp384-sha384-cades",
            &cert("keystores/test-ecp384-title.crt.der"),
            SignatureAlg::Ecdsa(DigestAlg::Sha384),
        ),
        ("openssl-p521-sha512-cades-keyid", &cert("cades/leaf-p521.crt.der"), SignatureAlg::Ecdsa(DigestAlg::Sha512)),
    ];
    for (name, signer, alg) in cases {
        let cms = read(&format!("cades/{name}.cms.der"));
        let expect = Expect { signer: ExpectedSigner::Certificate(signer.clone()), ..Expect::default() };
        let r = verify(&cms, &content, &expect).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(r.signature_alg, alg, "{name}");
        assert!(r.has_signing_certificate_v2, "{name}");
        assert!(r.signing_time.is_some(), "{name}");
        // Manipulation erkannt
        let mut bad = content.clone();
        bad[0] ^= 0x20;
        assert!(matches!(verify(&cms, &bad, &expect), Err(CmsError::DigestMismatch)), "{name}");
    }
    // SubjectKeyIdentifier als SignerIdentifier, Zwischenzertifikat mitgeliefert
    let r = verify(&read("cades/openssl-p521-sha512-cades-keyid.cms.der"), &content, &Expect::default()).unwrap();
    assert_eq!(r.certificates.len(), 2);
    assert_eq!(r.signer, cert("cades/leaf-p521.crt.der"));
}

#[test]
fn fixture_without_ess_and_without_attributes() {
    let content = content();
    let lax = Expect { require_signing_certificate: false, ..Expect::default() };
    // ohne -cades: kein ESS-Attribut (PDF-AS 11008)
    let plain = read("cades/openssl-rsa2048-sha256-plain.cms.der");
    assert!(matches!(
        verify(&plain, &content, &Expect::default()),
        Err(CmsError::MissingAttribute("signing-certificate"))
    ));
    let r = verify(&plain, &content, &lax).unwrap();
    assert!(!r.has_signing_certificate_v1 && !r.has_signing_certificate_v2);
    // SMIMECapabilities u. ä. unbekannte Attribute stören nicht
    assert!(r.signed_attr_oids.contains(&"1.2.840.113549.1.9.15".to_string()));

    // -noattr: Signatur direkt über den Inhalt
    let noattr = read("cades/openssl-rsa2048-sha256-noattr.cms.der");
    assert!(matches!(verify(&noattr, &content, &Expect::default()), Err(CmsError::MissingAttribute(_))));
    let r = verify(&noattr, &content, &lax).unwrap();
    assert!(r.signed_attr_oids.is_empty());
    let mut bad = content.clone();
    bad.pop();
    assert!(matches!(verify(&noattr, &bad, &lax), Err(CmsError::InvalidSignature)));
}

#[test]
fn fixture_pss_and_sha1_only_as_prior_signatures() {
    let content = content();
    let prior = Expect { policy: AlgorithmPolicy::PriorSignature, ..Expect::default() };
    for name in ["openssl-rsa3072-pss-sha256-cades", "openssl-rsa2048-sha1-cades"] {
        let cms = read(&format!("cades/{name}.cms.der"));
        assert!(matches!(verify(&cms, &content, &Expect::default()), Err(CmsError::UnsupportedAlgorithm(_))), "{name}");
        verify(&cms, &content, &prior).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

// ---------------------------------------------------------------------------------------------
// Live-Orakel (übersprungen ohne openssl ≥ 3.2)
// ---------------------------------------------------------------------------------------------

fn write(dir: &Path, name: &str, data: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, data).unwrap();
    p
}

/// `openssl cms -verify -noverify` (Signatur, message-digest; ohne Kettenprüfung) und zusätzlich
/// `openssl cms -verify -cades` mit dem Testzertifikat bzw. der Test-Root als Vertrauensanker
/// (prüft ESS signing-certificate; `-cades` verlangt eine Kettenprüfung, daher nicht mit
/// `-noverify` kombinierbar). Liefert (beide erfolgreich, stderr).
fn openssl_verify(dir: &Path, cms: &[u8], content: &[u8], anchor: &[u8]) -> (bool, String) {
    let cms_p = write(dir, "our.cms.der", cms);
    let content_p = write(dir, "content.bin", content);
    let anchor_der = write(dir, "anchor.der", anchor);
    let anchor_pem = dir.join("anchor.pem");
    assert!(
        Command::new("openssl")
            .args(["x509", "-inform", "DER", "-in"])
            .arg(&anchor_der)
            .arg("-out")
            .arg(&anchor_pem)
            .status()
            .unwrap()
            .success()
    );
    let run = |extra: &[&str]| {
        let out = Command::new("openssl")
            .args(["cms", "-verify", "-binary", "-inform", "DER", "-out"])
            .arg(dir.join("out.bin"))
            .args(extra)
            .arg("-in")
            .arg(&cms_p)
            .arg("-content")
            .arg(&content_p)
            .output()
            .expect("openssl starten");
        (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
    };
    let (ok1, e1) = run(&["-noverify"]);
    let anchor_s = anchor_pem.to_str().unwrap().to_owned();
    let (ok2, e2) = run(&["-cades", "-purpose", "any", "-partial_chain", "-CAfile", &anchor_s]);
    (ok1 && ok2, format!("{e1}{e2}"))
}

#[test]
fn our_cms_verifies_with_openssl() {
    if openssl().is_none() {
        eprintln!("openssl ≥ 3.2 nicht gefunden – Orakeltest übersprungen");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let content = content();
    for profile in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
        for s in [rsa2048(), ecp256(), ecp384(), p521(), rsa3072()] {
            let cms = sign_detached(&s, &s.algorithm().digest().digest(&content), profile, now()).unwrap();
            let anchor = s.chain().signer.as_der().to_vec();
            let (ok, err) = openssl_verify(dir.path(), &cms, &content, &anchor);
            assert!(ok, "openssl lehnt {profile:?}/{:?} ab: {err}", s.algorithm());
            assert!(err.contains("CAdES Verification successful"), "{err}");
            // Gegenprobe: Orakel erkennt Manipulation
            let mut bad = content.clone();
            bad[10] ^= 1;
            assert!(!openssl_verify(dir.path(), &cms, &bad, &anchor).0);
        }
    }
    // mit Zwischenzertifikaten
    let all = Certificate::from_pem_all(&String::from_utf8(read("cades/chain-p521.pem")).unwrap()).unwrap();
    let chain = CertChain { signer: all[0].clone(), intermediates: all[1..].to_vec() };
    let s = LocalSigner::from_pkcs8_der(&read("cades/leaf-p521.key.p8.der"), chain.clone()).unwrap();
    let md = DigestAlg::Sha512.digest(&content);
    let attrs = SignedAttrsBuilder::new(CmsProfile::PadesBB, &md, &chain.signer, now()).build().unwrap();
    let sig = s.sign_digest(&attrs.digest(DigestAlg::Sha512)).unwrap();
    let cms =
        assemble(&attrs, &sig, &chain, &AssembleOptions { profile: CmsProfile::PadesBB, include_intermediates: true })
            .unwrap();
    let (ok, err) = openssl_verify(dir.path(), &cms, &content, &read("cades/ca-root.crt.der"));
    assert!(ok, "{err}");
}

/// PEM-Dateien für OpenSSL aus unseren DER-Testdaten.
fn pem_files(dir: &Path, key_name: &str, cert_rel: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let key = write(dir, &format!("{key_name}.key.der"), &read(&format!("cades/{key_name}.key.p8.der")));
    let crt_der = write(dir, &format!("{key_name}.crt.der"), &read(cert_rel));
    let crt_pem = dir.join(format!("{key_name}.crt.pem"));
    let st = Command::new("openssl")
        .args(["x509", "-inform", "DER", "-in"])
        .arg(&crt_der)
        .arg("-out")
        .arg(&crt_pem)
        .status()
        .unwrap();
    assert!(st.success());
    (key, crt_pem)
}

#[test]
fn fresh_openssl_cms_verify_with_ours() {
    if openssl().is_none() {
        eprintln!("openssl ≥ 3.2 nicht gefunden – Orakeltest übersprungen");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let content = content();
    let content_p = write(dir.path(), "content.bin", &content);
    let keys = [
        ("test-rsa2048", "keystores/test-rsa2048.crt.der"),
        ("test-ecp256", "keystores/test-ecp256.crt.der"),
        ("test-ecp384-title", "keystores/test-ecp384-title.crt.der"),
        ("leaf-p521", "cades/leaf-p521.crt.der"),
        ("leaf-rsa3072", "cades/leaf-rsa3072.crt.der"),
    ];
    for (key_name, cert_rel) in keys {
        let (key, crt) = pem_files(dir.path(), key_name, cert_rel);
        for md in ["sha256", "sha384", "sha512"] {
            for extra in [&[][..], &["-keyid"][..], &["-nosmimecap"][..]] {
                let out = dir.path().join("o.cms.der");
                let st = Command::new("openssl")
                    .args(["cms", "-sign", "-binary", "-outform", "DER", "-cades", "-md", md, "-keyform", "DER"])
                    .args(extra)
                    .arg("-in")
                    .arg(&content_p)
                    .arg("-signer")
                    .arg(&crt)
                    .arg("-inkey")
                    .arg(&key)
                    .arg("-out")
                    .arg(&out)
                    .output()
                    .unwrap();
                assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
                let cms = std::fs::read(&out).unwrap();
                let expect = Expect { signer: ExpectedSigner::Certificate(cert(cert_rel)), ..Expect::default() };
                let r = verify(&cms, &content, &expect).unwrap_or_else(|e| panic!("{key_name}/{md}/{extra:?}: {e}"));
                assert_eq!(r.digest_alg.name().replace('-', "").to_lowercase(), md);
                assert!(r.has_signing_certificate_v2);
            }
        }
        // RSA-PSS (nur Vorsignaturen)
        if key_name.contains("rsa") {
            let out = dir.path().join("pss.cms.der");
            let st = Command::new("openssl")
                .args(["cms", "-sign", "-binary", "-outform", "DER", "-cades", "-md", "sha384", "-keyform", "DER"])
                .arg("-in")
                .arg(&content_p)
                .arg("-signer")
                .arg(&crt)
                .arg("-inkey")
                .arg(&key)
                // -keyopt gilt für den zuvor genannten Signer
                .args(["-keyopt", "rsa_padding_mode:pss", "-keyopt", "rsa_pss_saltlen:-1"])
                .arg("-out")
                .arg(&out)
                .output()
                .unwrap();
            assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
            let cms = std::fs::read(&out).unwrap();
            let prior = Expect { policy: AlgorithmPolicy::PriorSignature, ..Expect::default() };
            let r = verify(&cms, &content, &prior).unwrap_or_else(|e| panic!("PSS {key_name}: {e}"));
            assert!(matches!(r.signature_alg, SignatureAlg::RsaPss { digest: DigestAlg::Sha384, .. }));
        }
    }
}

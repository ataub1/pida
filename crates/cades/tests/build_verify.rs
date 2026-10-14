//! Builder, Montage, Signierer und Verifier im Zusammenspiel: Golden-Vergleich der SignedAttributes
//! (unabhängig mit asn1crypto erzeugt), Rundläufe für RSA/P-256/P-384/P-521 in beiden Profilen und
//! Negativtests (falscher Digest, falsches Zertifikat, zwei SignerInfos, fehlendes ESS …).

mod common;

use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::{Decode, Encode};
use pdfover_cades::{
    AlgorithmPolicy, AssembleOptions, CertChain, Certificate, CmsError, CmsProfile, CmsWarning, Digest, DigestAlg,
    EcCurve, Expect, ExpectedSigner, KeyType, LocalSigner, RawSignature, RawSigner, SignatureAlg, SignedAttrsBuilder,
    SigningTime, assemble, estimate_cms_len, parse_cms, select_algorithm, sign_detached, trim_contents_padding,
    verify_detached, verify_raw,
};
use time::OffsetDateTime;

use common::*;

const GOLDEN_TIME: i64 = 1_791_979_200; // 2026-10-14T12:00:00Z

fn golden_time() -> SigningTime {
    SigningTime(OffsetDateTime::from_unix_timestamp(GOLDEN_TIME).unwrap())
}

fn expect_signer(c: &Certificate) -> Expect {
    Expect { signer: ExpectedSigner::Certificate(c.clone()), ..Expect::default() }
}

fn verify(cms: &[u8], content: &[u8], expect: &Expect) -> pdfover_cades::Result<pdfover_cades::CmsReport> {
    verify_detached(cms, &mut &content[..], expect)
}

#[test]
fn signed_attrs_match_independent_asn1crypto_goldens() {
    let content = content();
    let cases = [
        (
            "golden-attrs-pdfas-ecp384-sha256.der",
            "keystores/test-ecp384-title.crt.der",
            DigestAlg::Sha256,
            CmsProfile::PdfasCompat,
        ),
        (
            "golden-attrs-padesbb-ecp384-sha256.der",
            "keystores/test-ecp384-title.crt.der",
            DigestAlg::Sha256,
            CmsProfile::PadesBB,
        ),
        ("golden-attrs-pdfas-p521-sha512.der", "cades/leaf-p521.crt.der", DigestAlg::Sha512, CmsProfile::PdfasCompat),
        ("golden-attrs-padesbb-p521-sha512.der", "cades/leaf-p521.crt.der", DigestAlg::Sha512, CmsProfile::PadesBB),
        (
            "golden-attrs-padesbb-rsa2048-sha384.der",
            "keystores/test-rsa2048.crt.der",
            DigestAlg::Sha384,
            CmsProfile::PadesBB,
        ),
    ];
    for (golden, cert_rel, alg, profile) in cases {
        let c = cert(cert_rel);
        let attrs = SignedAttrsBuilder::new(profile, &alg.digest(&content), &c, golden_time()).build().unwrap();
        assert_same_bytes(&attrs.der, &read(&format!("cades/{golden}")), golden);
        assert_eq!(attrs.digest_alg, alg);
        assert_eq!(attrs.profile, profile);
        let tagged = attrs.implicit_tagged_der();
        assert_eq!(tagged[0], 0xA0);
        assert_eq!(&tagged[1..], &attrs.der[1..]);
        assert_eq!(attrs.digest(alg), alg.digest(&attrs.der));
    }
}

#[test]
fn signed_attrs_with_sha1_use_signing_certificate_v1() {
    let c = cert("keystores/test-rsa2048.crt.der");
    let attrs = SignedAttrsBuilder::new(CmsProfile::PdfasCompat, &DigestAlg::Sha1.digest(b"x"), &c, golden_time())
        .build()
        .unwrap();
    // ESSCertID: SHA-1-Hash des Zertifikats, OID id-aa-signingCertificate
    let oid_v1 = [0x06, 0x0B, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x10, 0x02, 0x0C];
    assert!(attrs.der.windows(oid_v1.len()).any(|w| w == oid_v1));
    let fp = c.fingerprint(DigestAlg::Sha1).value;
    assert!(attrs.der.windows(fp.len()).any(|w| w == fp));
    // Länge des message-digest muss zum Verfahren passen
    let bad = Digest { alg: DigestAlg::Sha256, value: vec![0; 20] };
    assert!(SignedAttrsBuilder::new(CmsProfile::PdfasCompat, &bad, &c, golden_time()).build().is_err());
}

fn roundtrip(signer: &LocalSigner, profile: CmsProfile) -> Vec<u8> {
    let content = content();
    let alg = signer.algorithm();
    let cms = sign_detached(signer, &alg.digest().digest(&content), profile, golden_time()).unwrap();

    let c = &signer.chain().signer;
    let report = verify(&cms, &content, &expect_signer(c)).expect("eigenes CMS gültig");
    assert_eq!(report.signature_alg, alg);
    assert_eq!(report.digest_alg, alg.digest());
    assert_eq!(&report.signer, c);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert!(report.has_signing_certificate_v2);
    match profile {
        CmsProfile::PdfasCompat => {
            assert_eq!(report.signed_attr_oids.len(), 5);
            assert_eq!(report.signing_time.unwrap().unix_timestamp(), GOLDEN_TIME);
        }
        CmsProfile::PadesBB => {
            assert_eq!(
                report.signed_attr_oids,
                ["1.2.840.113549.1.9.3", "1.2.840.113549.1.9.4", "1.2.840.113549.1.9.16.2.47"]
            );
            assert!(report.signing_time.is_none());
        }
    }
    let est = estimate_cms_len(signer.chain(), profile, alg, false).unwrap();
    assert!(est >= cms.len() && est - cms.len() <= 8, "Schätzung {est} vs. {}", cms.len());

    // Struktur über den strikten `cms`-Crate (DER) – SignedData v1, detached, 1 Zertifikat, SignerInfo v1
    let ci = ContentInfo::from_der(&cms).expect("DER-konform");
    let sd: SignedData = ci.content.decode_as().unwrap();
    assert_eq!(sd.version, cms::content_info::CmsVersion::V1);
    assert!(sd.encap_content_info.econtent.is_none());
    assert_eq!(sd.certificates.as_ref().unwrap().0.len(), 1);
    assert!(sd.crls.is_none());
    let si = sd.signer_infos.0.get(0).unwrap();
    assert_eq!(si.version, cms::content_info::CmsVersion::V1);
    assert!(matches!(si.sid, cms::signed_data::SignerIdentifier::IssuerAndSerialNumber(_)));
    assert!(si.unsigned_attrs.is_none());
    // Parameterkodierung je Profil
    let null = der::asn1::Null.to_der().unwrap();
    let has_null = |p: &Option<der::Any>| p.as_ref().map(|a| a.to_der().unwrap()) == Some(null.clone());
    let digest_null = has_null(&si.digest_alg.parameters);
    assert_eq!(digest_null, profile == CmsProfile::PdfasCompat, "digestAlgorithm-Parameter");
    let sig_null = has_null(&si.signature_algorithm.parameters);
    match alg {
        SignatureAlg::RsaPkcs1v15(_) => assert!(sig_null, "RSA immer mit NULL"),
        _ => assert_eq!(sig_null, profile == CmsProfile::PdfasCompat, "ECDSA-Parameter"),
    }
    assert_eq!(si.signature_algorithm.oid.to_string(), alg.oid());
    cms
}

#[test]
fn roundtrip_all_keys_and_profiles() {
    for profile in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
        for s in [rsa2048(), ecp256(), ecp384(), p521(), rsa3072()] {
            roundtrip(&s, profile);
        }
    }
}

#[test]
fn algorithm_selection_per_test_key() {
    assert_eq!(rsa2048().algorithm(), SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256));
    assert_eq!(rsa3072().algorithm(), SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256));
    assert_eq!(ecp256().algorithm(), SignatureAlg::Ecdsa(DigestAlg::Sha256));
    // P-384 → SHA-256 wie PDF-AS (Feldgröße ≥ 256, < 512)
    assert_eq!(ecp384().algorithm(), SignatureAlg::Ecdsa(DigestAlg::Sha256));
    assert_eq!(p521().algorithm(), SignatureAlg::Ecdsa(DigestAlg::Sha512));
    assert_eq!(select_algorithm(&p521().key_type()).unwrap(), p521().algorithm());
}

#[test]
fn key_types_from_certificates() {
    assert_eq!(cert("keystores/test-rsa2048.crt.der").key_type().unwrap(), KeyType::Rsa { bits: 2048 });
    assert_eq!(cert("cades/leaf-rsa3072.crt.der").key_type().unwrap(), KeyType::Rsa { bits: 3072 });
    assert_eq!(cert("keystores/test-ecp256.crt.der").key_type().unwrap(), KeyType::Ec { curve: EcCurve::P256 });
    assert_eq!(cert("keystores/test-ecp384-title.crt.der").key_type().unwrap(), KeyType::Ec { curve: EcCurve::P384 });
    assert_eq!(cert("cades/leaf-p521.crt.der").key_type().unwrap(), KeyType::Ec { curve: EcCurve::P521 });
    assert_eq!(cert("cades/ca-root.crt.der").key_type().unwrap(), KeyType::Ec { curve: EcCurve::P384 });
    assert_eq!(p521().key_type(), KeyType::Ec { curve: EcCurve::P521 });
    assert_eq!(rsa2048().key_type(), KeyType::Rsa { bits: 2048 });
}

#[test]
fn pem_parsing() {
    for n in ["test-rsa2048", "test-ecp256", "test-ecp384-title"] {
        let pem = String::from_utf8(read(&format!("keystores/{n}.crt.pem"))).unwrap();
        let der = read(&format!("keystores/{n}.crt.der"));
        assert_eq!(Certificate::from_pem(&pem).unwrap().as_der(), der.as_slice());
        // Text vor dem Block (openssl x509 -text) und CRLF werden toleriert
        let noisy = format!("Certificate:\n    Data: …\n{}", pem.replace('\n', "\r\n"));
        assert_eq!(Certificate::from_pem(&noisy).unwrap().as_der(), der.as_slice());
    }
    let chain = String::from_utf8(read("cades/chain-p521.pem")).unwrap();
    let all = Certificate::from_pem_all(&chain).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0], cert("cades/leaf-p521.crt.der"));
    assert_eq!(all[1], cert("cades/ca-intermediate.crt.der"));
    assert_eq!(all[2], cert("cades/ca-root.crt.der"));
    assert!(Certificate::from_pem("kein PEM").is_err());
    assert!(Certificate::from_pem("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n").is_err());
    assert!(Certificate::from_pem("-----BEGIN CERTIFICATE-----\n!!!!\n-----END CERTIFICATE-----\n").is_err());
    assert!(Certificate::from_pem("-----BEGIN CERTIFICATE-----\nMIIB\n").is_err());
    // anderer PEM-Typ wird übersprungen
    let key_then_cert = format!("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n{chain}");
    assert_eq!(Certificate::from_pem(&key_then_cert).unwrap(), all[0]);
}

#[test]
fn intermediates_are_embedded_only_on_request() {
    let chain_pem = String::from_utf8(read("cades/chain-p521.pem")).unwrap();
    let all = Certificate::from_pem_all(&chain_pem).unwrap();
    let chain = CertChain { signer: all[0].clone(), intermediates: all[1..].to_vec() };
    let key = read("cades/leaf-p521.key.p8.der");
    let signer = LocalSigner::from_pkcs8_der(&key, chain.clone()).unwrap();
    let content = content();
    let md = DigestAlg::Sha512.digest(&content);
    for (include, n) in [(false, 1), (true, 3)] {
        let attrs = SignedAttrsBuilder::new(CmsProfile::PadesBB, &md, &chain.signer, golden_time()).build().unwrap();
        let sig = signer.sign_digest(&attrs.digest(DigestAlg::Sha512)).unwrap();
        let opts = AssembleOptions { profile: CmsProfile::PadesBB, include_intermediates: include };
        let cms = assemble(&attrs, &sig, &chain, &opts).unwrap();
        let report = verify(&cms, &content, &expect_signer(&chain.signer)).unwrap();
        assert_eq!(report.certificates.len(), n);
        assert_eq!(report.signer, chain.signer);
        let est = estimate_cms_len(&chain, CmsProfile::PadesBB, sig.alg, include).unwrap();
        assert!(est >= cms.len() && est - cms.len() <= 8);
        // CertificateSet DER-sortiert (strikter Decoder akzeptiert)
        ContentInfo::from_der(&cms).unwrap().content.decode_as::<SignedData>().unwrap();
    }
}

#[test]
fn padding_and_ber_outer_encoding_are_accepted() {
    let s = ecp256();
    let content = content();
    let cms = sign_detached(&s, &DigestAlg::Sha256.digest(&content), CmsProfile::PdfasCompat, golden_time()).unwrap();
    let mut padded = cms.clone();
    padded.resize(8192, 0);
    assert_eq!(trim_contents_padding(&padded).unwrap(), cms.as_slice());
    let report = verify(&padded, &content, &Expect::default()).unwrap();
    assert_eq!(report.der_len, cms.len());
    padded.push(0x01);
    assert!(matches!(verify(&padded, &content, &Expect::default()), Err(CmsError::Der(_))));

    // äußere SEQUENCE mit unbestimmter Länge (BER) – die signierten Bytes bleiben unverändert
    let t_len = if cms[1] & 0x80 != 0 { 2 + (cms[1] & 0x7F) as usize } else { 2 };
    let mut ber = vec![0x30, 0x80];
    ber.extend_from_slice(&cms[t_len..]);
    ber.extend_from_slice(&[0, 0]);
    let report = verify(&ber, &content, &Expect::default()).unwrap();
    assert_eq!(report.der_len, ber.len());
}

#[test]
fn negative_wrong_digest_wrong_certificate_tampering() {
    let s = rsa2048();
    let content = content();
    let cms = sign_detached(&s, &DigestAlg::Sha256.digest(&content), CmsProfile::PdfasCompat, golden_time()).unwrap();
    // falscher Inhalt
    let mut other = content.clone();
    other.push(b'!');
    assert!(matches!(verify(&cms, &other, &Expect::default()), Err(CmsError::DigestMismatch)));
    // falsches erwartetes Zertifikat
    let wrong = cert("keystores/test-ecp256.crt.der");
    assert!(matches!(verify(&cms, &content, &expect_signer(&wrong)), Err(CmsError::UnexpectedSigner)));
    // Signaturwert manipuliert (letztes Byte)
    let mut bad = cms.clone();
    *bad.last_mut().unwrap() ^= 0x01;
    assert!(matches!(verify(&bad, &content, &Expect::default()), Err(CmsError::InvalidSignature)));
    // signierte Attribute manipuliert (Byte im message-digest-Wert ist durch DigestMismatch
    // abgedeckt; hier die Zeitangabe in signing-time)
    let pos = cms.windows(13).position(|w| w == b"261014120000Z").unwrap();
    let mut bad = cms.clone();
    bad[pos + 11] = b'1';
    assert!(matches!(verify(&bad, &content, &Expect::default()), Err(CmsError::InvalidSignature)));
}

#[test]
fn negative_signing_certificate_points_to_other_certificate() {
    // Attribute mit ESS auf Zertifikat A, signiert und montiert mit Schlüssel/Zertifikat B
    let a = cert("keystores/test-ecp256.crt.der");
    let b = rsa2048();
    let content = content();
    let attrs = SignedAttrsBuilder::new(CmsProfile::PadesBB, &DigestAlg::Sha256.digest(&content), &a, golden_time())
        .build()
        .unwrap();
    let sig = b.sign_digest(&attrs.digest(DigestAlg::Sha256)).unwrap();
    let cms = assemble(
        &attrs,
        &sig,
        b.chain(),
        &AssembleOptions { profile: CmsProfile::PadesBB, include_intermediates: false },
    )
    .unwrap();
    assert!(matches!(verify(&cms, &content, &Expect::default()), Err(CmsError::SigningCertificateMismatch)));
}

#[test]
fn negative_signer_certificate_not_in_cms() {
    // Zertifikat A im CMS, aber SignerInfo/ESS/Signatur von B: Signer nicht auffindbar
    let s = ecp256();
    let content = content();
    let cms = sign_detached(&s, &DigestAlg::Sha256.digest(&content), CmsProfile::PadesBB, golden_time()).unwrap();
    let own = s.chain().signer.as_der();
    let other = read("keystores/test-ecp384-title.crt.der");
    let pos = cms.windows(own.len()).position(|w| w == own).unwrap();
    // Zertifikat ersetzen und alle umgebenden Längen anpassen ist aufwendig; stattdessen das
    // openssl-CMS ohne Zertifikate verwenden (siehe openssl_oracle.rs) und hier eine Kopie mit
    // gleich langem, fremdem Zertifikat prüfen, wenn die Längen zufällig passen.
    if other.len() == own.len() {
        let mut swapped = cms.clone();
        swapped[pos..pos + own.len()].copy_from_slice(&other);
        assert!(matches!(verify(&swapped, &content, &Expect::default()), Err(CmsError::SignerCertificateMissing)));
    }
    let nocerts = read("cades/openssl-rsa2048-sha256-nocerts.cms.der");
    assert!(matches!(verify(&nocerts, &content, &Expect::default()), Err(CmsError::SignerCertificateMissing)));
}

#[test]
fn negative_structure() {
    let content = content();
    // kein SignedData (ContentInfo mit id-data)
    let data_ci = [0x30, 0x0B, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x01];
    assert!(matches!(verify(&data_ci, &content, &Expect::default()), Err(CmsError::NotSignedData)));
    assert!(matches!(parse_cms(&data_ci), Err(CmsError::NotSignedData)));
    assert!(verify(b"garbage", &content, &Expect::default()).is_err());
    assert!(verify(&[], &content, &Expect::default()).is_err());
    assert!(parse_cms(&[0x30, 0x03, 0x02, 0x01, 0x01]).is_err());
    // abgeschnittenes CMS
    let s = ecp256();
    let cms = sign_detached(&s, &DigestAlg::Sha256.digest(&content), CmsProfile::PadesBB, golden_time()).unwrap();
    assert!(matches!(verify(&cms[..cms.len() - 10], &content, &Expect::default()), Err(CmsError::Der(_))));
    // zwei SignerInfos
    let two = read("cades/openssl-two-signers.cms.der");
    assert!(matches!(verify(&two, &content, &Expect::default()), Err(CmsError::SignerCount(2))));
    assert_eq!(parse_cms(&two).unwrap().signer_count, 2);
}

#[test]
fn assemble_rejects_inconsistent_inputs() {
    let content = content();
    let rsa = rsa2048();
    let ec = ecp256();
    let md = DigestAlg::Sha256.digest(&content);
    let attrs =
        SignedAttrsBuilder::new(CmsProfile::PdfasCompat, &md, &rsa.chain().signer, golden_time()).build().unwrap();
    let sig = rsa.sign_digest(&attrs.digest(DigestAlg::Sha256)).unwrap();
    let opts = AssembleOptions::default();
    // Profil der Attribute ≠ Optionen
    let opts_bb = AssembleOptions { profile: CmsProfile::PadesBB, include_intermediates: false };
    assert!(assemble(&attrs, &sig, rsa.chain(), &opts_bb).is_err());
    // Hashverfahren inkonsistent
    let sig512 = rsa.sign_digest(&attrs.digest(DigestAlg::Sha512)).unwrap();
    assert!(assemble(&attrs, &sig512, rsa.chain(), &opts).is_err());
    // RSA-Signatur mit EC-Zertifikat
    assert!(matches!(assemble(&attrs, &sig, ec.chain(), &opts), Err(CmsError::UnsupportedAlgorithm(_))));
    // PSS wird nicht montiert
    let pss = RawSignature {
        alg: SignatureAlg::RsaPss { digest: DigestAlg::Sha256, salt_len: 32 },
        value: sig.value.clone(),
    };
    assert!(assemble(&attrs, &pss, rsa.chain(), &opts).is_err());
    // ECDSA-Wert kein DER
    let ec_attrs =
        SignedAttrsBuilder::new(CmsProfile::PdfasCompat, &md, &ec.chain().signer, golden_time()).build().unwrap();
    let raw = RawSignature { alg: SignatureAlg::Ecdsa(DigestAlg::Sha256), value: vec![1; 64] };
    assert!(matches!(assemble(&ec_attrs, &raw, ec.chain(), &opts), Err(CmsError::InvalidSignature)));
    // RSA-Wert zu lang
    let long = RawSignature { alg: sig.alg, value: vec![1; 257] };
    assert!(matches!(assemble(&attrs, &long, rsa.chain(), &opts), Err(CmsError::InvalidSignature)));
    // RSA-Wert mit fehlender führender Null wird aufgefüllt (I2OSP)
    let short = RawSignature { alg: sig.alg, value: sig.value[1..].to_vec() };
    if sig.value[0] == 0 {
        let cms = assemble(&attrs, &short, rsa.chain(), &opts).unwrap();
        verify(&cms, &content, &Expect::default()).unwrap();
    }
}

#[test]
fn local_signer_rejects_mismatched_key_and_sha1() {
    let key = read("cades/test-rsa2048.key.p8.der");
    let other_rsa = cert("cades/leaf-rsa3072.crt.der");
    assert!(matches!(LocalSigner::from_pkcs8_der(&key, CertChain::new(other_rsa)), Err(CmsError::KeyMismatch)));
    let ec_key = read("cades/test-ecp256.key.p8.der");
    let ec_cert = cert("keystores/test-ecp256.crt.der");
    // EC-Schlüssel mit RSA-Zertifikat
    assert!(LocalSigner::from_pkcs8_der(&ec_key, CertChain::new(cert("keystores/test-rsa2048.crt.der"))).is_err());
    // P-256-Schlüssel mit P-384-Zertifikat
    assert!(LocalSigner::from_pkcs8_der(&ec_key, CertChain::new(cert("keystores/test-ecp384-title.crt.der"))).is_err());
    // P-384-Schlüssel des Titel-Zertifikats mit dem P-384-Root-Zertifikat der Test-PKI
    let p384_key = read("cades/test-ecp384-title.key.p8.der");
    assert!(matches!(
        LocalSigner::from_pkcs8_der(&p384_key, CertChain::new(cert("cades/ca-root.crt.der"))),
        Err(CmsError::KeyMismatch)
    ));
    // fremder P-256-Schlüssel? (nur ein P-256-Testschlüssel vorhanden) – Müll statt PKCS#8
    assert!(matches!(LocalSigner::from_pkcs8_der(b"\x30\x00", CertChain::new(ec_cert.clone())), Err(CmsError::Key(_))));
    let s = LocalSigner::from_pkcs8_der(&ec_key, CertChain::new(ec_cert)).unwrap();
    assert!(matches!(s.sign_digest(&DigestAlg::Sha1.digest(b"x")), Err(CmsError::UnsupportedAlgorithm(_))));
    let short = Digest { alg: DigestAlg::Sha256, value: vec![0; 31] };
    assert!(s.sign_digest(&short).is_err());
    // Debug ohne Personendaten
    let dbg = format!("{s:?}");
    assert!(!dbg.contains("NUR TEST"), "{dbg}");
    assert!(dbg.contains("P-256"));
}

#[test]
fn raw_signatures_any_sha2_on_any_curve() {
    for s in [rsa2048(), ecp256(), ecp384(), p521()] {
        for alg in [DigestAlg::Sha256, DigestAlg::Sha384, DigestAlg::Sha512] {
            let d = alg.digest(b"PDF-Over NG");
            let sig = s.sign_digest(&d).unwrap();
            assert_eq!(sig.alg.digest(), alg);
            verify_raw(&s.chain().signer, &d, &sig).unwrap();
            let other = alg.digest(b"anders");
            assert!(matches!(verify_raw(&s.chain().signer, &other, &sig), Err(CmsError::InvalidSignature)));
            let wrong_alg = DigestAlg::Sha256.digest(b"x");
            if alg != DigestAlg::Sha256 {
                assert!(verify_raw(&s.chain().signer, &wrong_alg, &sig).is_err());
            }
        }
    }
    // RSA PKCS#1 v1.5 ist deterministisch (Blinding ändert das Ergebnis nicht)
    let d = DigestAlg::Sha256.digest(b"det");
    assert_eq!(rsa2048().sign_digest(&d).unwrap(), rsa2048().sign_digest(&d).unwrap());
    // ECDSA nach RFC 6979 ebenfalls
    assert_eq!(ecp256().sign_digest(&d).unwrap(), ecp256().sign_digest(&d).unwrap());
}

#[test]
fn ecdsa_fixed_to_der_roundtrip() {
    let s = p521();
    let d = DigestAlg::Sha512.digest(b"fixed");
    let sig = s.sign_digest(&d).unwrap();
    // DER → r||s (66 Bytes je Hälfte) → DER
    let v = &sig.value;
    let mut pos = if v[1] & 0x80 != 0 { 2 + (v[1] & 0x7F) as usize } else { 2 };
    let mut fixed = Vec::new();
    for _ in 0..2 {
        assert_eq!(v[pos], 0x02);
        let len = v[pos + 1] as usize;
        let int = &v[pos + 2..pos + 2 + len];
        let m: Vec<u8> = int.iter().copied().skip_while(|&b| b == 0).collect();
        fixed.extend(std::iter::repeat_n(0u8, 66 - m.len()));
        fixed.extend_from_slice(&m);
        pos += 2 + len;
    }
    let again = RawSignature::ecdsa_from_fixed(sig.alg, &fixed).unwrap();
    assert_eq!(again, sig);
    verify_raw(&s.chain().signer, &d, &again).unwrap();
}

#[test]
fn policy_new_vs_prior_signature() {
    let content = content();
    let sha1 = read("cades/openssl-rsa2048-sha1-cades.cms.der");
    assert!(matches!(verify(&sha1, &content, &Expect::default()), Err(CmsError::UnsupportedAlgorithm(_))));
    let prior = Expect { policy: AlgorithmPolicy::PriorSignature, ..Expect::default() };
    let r = verify(&sha1, &content, &prior).unwrap();
    assert_eq!(r.digest_alg, DigestAlg::Sha1);
    assert!(r.has_signing_certificate_v1);
    assert!(r.warnings.contains(&CmsWarning::Sha1Digest));
    assert!(r.warnings.contains(&CmsWarning::SigningCertificateV1));

    let pss = read("cades/openssl-rsa3072-pss-sha256-cades.cms.der");
    assert!(matches!(verify(&pss, &content, &Expect::default()), Err(CmsError::UnsupportedAlgorithm(_))));
    let r = verify(&pss, &content, &prior).unwrap();
    assert_eq!(r.signature_alg, SignatureAlg::RsaPss { digest: DigestAlg::Sha256, salt_len: 32 });
}

/// Zerlegt den Inhalt eines konstruierten DER-Werts in seine Elemente (Testhilfe, Längen < 64 KiB).
fn split_tlvs(content: &[u8]) -> Vec<Vec<u8>> {
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

#[test]
fn verifier_uses_original_attribute_bytes_even_if_not_der_sorted() {
    // Fremde Signer kodieren SET OF mitunter unsortiert; signiert sind genau diese Bytes.
    let s = ecp256();
    let content = content();
    let md = DigestAlg::Sha256.digest(&content);
    let attrs = SignedAttrsBuilder::new(CmsProfile::PadesBB, &md, &s.chain().signer, golden_time()).build().unwrap();
    let hl = if attrs.der[1] & 0x80 != 0 { 2 + (attrs.der[1] & 0x7F) as usize } else { 2 };
    let mut items = split_tlvs(&attrs.der[hl..]);
    assert_eq!(items.len(), 3);
    items.reverse();
    let mut unsorted = attrs.der[..hl].to_vec();
    for i in &items {
        unsorted.extend_from_slice(i);
    }
    assert_ne!(unsorted, attrs.der);
    let attrs = pdfover_cades::SignedAttrs { der: unsorted, ..attrs };
    let sig = s.sign_digest(&attrs.digest(DigestAlg::Sha256)).unwrap();
    let opts = AssembleOptions { profile: CmsProfile::PadesBB, include_intermediates: false };
    let cms = assemble(&attrs, &sig, s.chain(), &opts).unwrap();
    // Ein Decoder, der dekodiert und neu kodiert, verliert die signierten Bytes (oder lehnt ab) …
    if let Ok(sd) = ContentInfo::from_der(&cms).and_then(|ci| ci.content.decode_as::<SignedData>()) {
        let reencoded = sd.signer_infos.0.get(0).unwrap().signed_attrs.as_ref().unwrap().to_der().unwrap();
        assert_ne!(reencoded, attrs.der, "Neukodierung sortiert das SET OF");
    }
    // … unser Verifier prüft die Originalbytes und meldet nur einen Hinweis
    let report = verify(&cms, &content, &expect_signer(&s.chain().signer)).unwrap();
    assert_eq!(report.warnings, [CmsWarning::NonDerSignedAttributes]);
    assert_eq!(report.signed_attr_oids[0], "1.2.840.113549.1.9.16.2.47", "Reihenfolge wie übertragen");
}

#[test]
fn debug_output_is_redacted() {
    // Regel 9: keine Zertifikats-Personendaten (Subject, Aussteller + Seriennummer im DER) in Debug
    let s = ecp384();
    let content = content();
    let md = DigestAlg::Sha256.digest(&content);
    let attrs =
        SignedAttrsBuilder::new(CmsProfile::PdfasCompat, &md, &s.chain().signer, golden_time()).build().unwrap();
    let dbg = format!("{attrs:?}");
    assert!(dbg.contains("der_len") && !dbg.contains("48, 130"), "{dbg}");
    let cms = sign_detached(&s, &md, CmsProfile::PdfasCompat, golden_time()).unwrap();
    let report = verify(&cms, &content, &Expect::default()).unwrap();
    let dbg = format!("{report:?} {:?} {:?}", parse_cms(&cms).unwrap(), s);
    assert!(!dbg.contains("NUR TEST") && !dbg.contains("Dipl.-Ing."), "{dbg}");
}

#[test]
fn cms_error_codes_are_stable_and_distinct() {
    let errs = [
        CmsError::Der(String::new()),
        CmsError::Certificate(String::new()),
        CmsError::UnsupportedAlgorithm(String::new()),
        CmsError::NotSignedData,
        CmsError::SignerCount(2),
        CmsError::MissingAttribute("x"),
        CmsError::DigestMismatch,
        CmsError::InvalidSignature,
        CmsError::SignerCertificateMissing,
        CmsError::SigningCertificateMismatch,
        CmsError::UnexpectedSigner,
        CmsError::Key(String::new()),
        CmsError::KeyMismatch,
        CmsError::TooLarge { len: 1, max: 0 },
        CmsError::Io(std::io::Error::other("x")),
    ];
    let codes: std::collections::BTreeSet<_> = errs.iter().map(CmsError::code).collect();
    assert_eq!(codes.len(), errs.len());
    assert!(codes.iter().all(|c| c.starts_with("cms-")));
}

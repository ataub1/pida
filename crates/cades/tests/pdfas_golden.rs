//! Golden-Tests gegen echte PDF-AS/IAIK-Signaturen (Proben P1–P3 aus docs/analyse/03 §0.1, in
//! `testdata/pdf/pdfas/`): Unser Verifier akzeptiert sie, und unser Builder reproduziert sowohl die
//! SignedAttributes als auch das komplette CMS **byte-genau** (Profil `PdfasCompat`).
//!
//! Unabhängiges Orakel für die Struktur ist der `cms`-Crate (RustCrypto, strikter DER-Decoder).
//! Ausgaben enthalten keine Zertifikatsinhalte (P2/P3 tragen echte Zertifikate).

mod common;

use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::{Decode, Encode};
use pida_cades::{
    AlgorithmPolicy, AssembleOptions, CertChain, CmsError, CmsProfile, DigestAlg, EcCurve, Expect, ExpectedSigner,
    KeyType, RawSignature, SignatureAlg, SignedAttrsBuilder, SigningTime, assemble, estimate_cms_len, parse_cms,
    trim_contents_padding, verify_detached,
};

use common::*;

const P1: &str = "pdf/pdfas/pdf-as-web/data/dummy-pdf-signed.pdf";
const P2: &str = "pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf";
const P3: &str = "pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf";

const PDFAS_ATTR_OIDS: [&str; 5] = [
    "1.2.840.113549.1.9.3",
    "0.4.0.1733.2.1",
    "1.2.840.113549.1.9.5",
    "1.2.840.113549.1.9.4",
    "1.2.840.113549.1.9.16.2.47",
];

struct Sample {
    data: Vec<u8>,
    contents: Vec<u8>,
}

fn samples(rel: &str) -> Vec<Sample> {
    pdf_signatures(&read(rel)).into_iter().map(|(data, contents)| Sample { data, contents }).collect()
}

/// Prüft eine Probe und rekonstruiert Attribute und CMS.
fn golden(s: &Sample, expected_alg: SignatureAlg) {
    let cms_der = trim_contents_padding(&s.contents).unwrap();
    let report = verify_detached(&s.contents, &mut s.data.as_slice(), &Expect::default()).expect("PDF-AS-CMS gültig");
    assert_eq!(report.signature_alg, expected_alg);
    assert_eq!(report.digest_alg, DigestAlg::Sha256);
    assert_eq!(report.signed_attr_oids, PDFAS_ATTR_OIDS);
    assert!(report.has_signing_certificate_v2 && !report.has_signing_certificate_v1);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(report.certificates.len(), 1, "PDF-AS bettet nur das Signerzertifikat ein");
    assert_eq!(report.der_len, cms_der.len());

    // Orakel: RustCrypto `cms` dekodiert dasselbe CMS
    let ci = ContentInfo::from_der(cms_der).expect("cms-Crate dekodiert P-Probe");
    let sd: SignedData = ci.content.decode_as().unwrap();
    let si = sd.signer_infos.0.get(0).unwrap();
    let oracle_attrs = si.signed_attrs.as_ref().unwrap().to_der().unwrap();

    // 1. SignedAttributes byte-genau
    let digest = DigestAlg::Sha256.digest(&s.data);
    let time = report.signing_time.expect("signing-time");
    let attrs =
        SignedAttrsBuilder::new(CmsProfile::PdfasCompat, &digest, &report.signer, SigningTime(time)).build().unwrap();
    assert_same_bytes(&attrs.der, &oracle_attrs, "SignedAttributes vs. PDF-AS");

    // 2. Komplettes CMS byte-genau (gleiche Rohsignatur eingesetzt)
    let sig = RawSignature { alg: report.signature_alg, value: si.signature.as_bytes().to_vec() };
    let chain = CertChain::new(report.signer.clone());
    let rebuilt = assemble(
        &attrs,
        &sig,
        &chain,
        &AssembleOptions { profile: CmsProfile::PdfasCompat, include_intermediates: false },
    )
    .unwrap();
    assert_same_bytes(&rebuilt, cms_der, "CMS vs. PDF-AS");

    // 3. Schätzung ist obere Schranke und eng
    let est = estimate_cms_len(&chain, CmsProfile::PdfasCompat, report.signature_alg, false).unwrap();
    assert!(est >= cms_der.len() && est - cms_der.len() <= 8, "Schätzung {est} vs. {}", cms_der.len());

    // 4. parse_cms ohne Prüfung
    let info = parse_cms(&s.contents).unwrap();
    assert_eq!(info.signer_count, 1);
    assert_eq!(info.digest_alg, Some(DigestAlg::Sha256));
    assert_eq!(info.signature_alg, Some(expected_alg));
    assert_eq!(info.der_len, cms_der.len());
    assert_eq!(info.certificates, report.certificates);

    // 5. Negativ: ein Byte im signierten Bereich geändert
    let mut tampered = s.data.clone();
    let mid = tampered.len() / 2;
    tampered[mid] ^= 0x01;
    assert!(matches!(
        verify_detached(&s.contents, &mut tampered.as_slice(), &Expect::default()),
        Err(CmsError::DigestMismatch)
    ));
    // 6. Negativ: anderes erwartetes Zertifikat
    let other = cert("keystores/test-rsa2048.crt.der");
    let expect = Expect { signer: ExpectedSigner::Certificate(other), ..Expect::default() };
    assert!(matches!(verify_detached(&s.contents, &mut s.data.as_slice(), &expect), Err(CmsError::UnexpectedSigner)));
    let expect = Expect { signer: ExpectedSigner::Certificate(report.signer.clone()), ..Expect::default() };
    verify_detached(&s.contents, &mut s.data.as_slice(), &expect).unwrap();
    // 7. Negativ: Reserve zu klein
    let expect = Expect { max_len: Some(cms_der.len() - 1), ..Expect::default() };
    assert!(matches!(verify_detached(&s.contents, &mut s.data.as_slice(), &expect), Err(CmsError::TooLarge { .. })));
}

#[test]
fn p1_rsa_keystore_signature_is_reproduced_byte_exact() {
    let s = samples(P1);
    assert_eq!(s.len(), 1);
    golden(&s[0], SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256));
    let report = verify_detached(&s[0].contents, &mut s[0].data.as_slice(), &Expect::default()).unwrap();
    assert_eq!(report.signer.key_type().unwrap(), KeyType::Rsa { bits: 3072 });
    // signing-time 2024-03-15 18:33:33Z (UTCTime) = /M 19:33:33+01'00'
    assert_eq!(report.signing_time.unwrap().unix_timestamp(), 1_710_527_613);
}

#[test]
fn p2_ecdsa_signature_with_null_parameters_is_reproduced_byte_exact() {
    let s = samples(P2);
    assert!(!s.is_empty());
    for sample in &s {
        golden(sample, SignatureAlg::Ecdsa(DigestAlg::Sha256));
    }
    let report = verify_detached(&s[0].contents, &mut s[0].data.as_slice(), &Expect::default()).unwrap();
    assert_eq!(report.signer.key_type().unwrap(), KeyType::Ec { curve: EcCurve::P256 });
}

#[test]
fn p3_three_consecutive_signatures_are_reproduced_byte_exact() {
    let s = samples(P3);
    assert_eq!(s.len(), 3);
    for sample in &s {
        golden(sample, SignatureAlg::Ecdsa(DigestAlg::Sha256));
    }
}

#[test]
fn prior_signature_policy_accepts_pdfas_samples_too() {
    for rel in [P1, P2, P3] {
        for s in samples(rel) {
            let expect =
                Expect { policy: AlgorithmPolicy::PriorSignature, signer: ExpectedSigner::Any, ..Expect::default() };
            verify_detached(&s.contents, &mut s.data.as_slice(), &expect).unwrap();
        }
    }
}

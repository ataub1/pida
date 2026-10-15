//! PKCS#12-Laden über alle Test-Keystores: moderne (PBES2/AES) und Legacy-Verschlüsselung
//! (3DES, RC2-40), Java-Exporte, Ein-Passwort-Abbildung, Alias-Auswahl, JKS/JCEKS-Erkennung und
//! Fehlerfälle. Danach Modell A Ende-zu-Ende (Signatur → CMS → `verify_detached`, optional OpenSSL).

use std::path::PathBuf;
use std::process::Command;

use pdfover_backend_keystore::{KeystoreError, KeystoreFormat, LoadedKey, list_aliases};
use pdfover_cades::{
    CmsProfile, DigestAlg, EcCurve, Expect, ExpectedSigner, KeyType, RawSigner, SignatureAlg, SigningTime,
    sign_detached, verify_detached, verify_raw,
};

fn td(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(td(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn open(rel: &str, pw: &str, alias: Option<&str>) -> Result<LoadedKey, KeystoreError> {
    LoadedKey::open_pkcs12(&read(rel), pw, alias)
}

/// Signiert und prüft eine Rohsignatur (Selbstprüfung).
fn sign_check(key: &LoadedKey) {
    let alg = key.algorithm().digest();
    let d = alg.digest(b"PDF-Over NG Keystore-Test");
    let sig = key.sign_digest(&d).unwrap();
    assert_eq!(sig.alg, key.algorithm());
    verify_raw(&key.chain().signer, &d, &sig).unwrap();
}

#[test]
fn shared_test_keystores_load_with_expected_algorithms() {
    let cases = [
        (
            "keystores/test-rsa2048.p12",
            "test-rsa2048",
            KeyType::Rsa { bits: 2048 },
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256),
        ),
        (
            "keystores/test-ecp256.p12",
            "test-ecp256",
            KeyType::Ec { curve: EcCurve::P256 },
            SignatureAlg::Ecdsa(DigestAlg::Sha256),
        ),
        // P-384 mit Titel-RDN → SHA-256 wie PDF-AS
        (
            "keystores/test-ecp384-title.p12",
            "test-ecp384-title",
            KeyType::Ec { curve: EcCurve::P384 },
            SignatureAlg::Ecdsa(DigestAlg::Sha256),
        ),
        // Legacy pbeWithSHA1And3-KeyTripleDES-CBC, MAC SHA-1
        (
            "keystores/test-rsa2048-legacy-3des.p12",
            "test-rsa2048",
            KeyType::Rsa { bits: 2048 },
            SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256),
        ),
    ];
    for (rel, alias, kt, alg) in cases {
        for a in [None, Some(alias)] {
            let key = open(rel, "test", a).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(key.alias(), alias);
            assert_eq!(key.key_type(), kt);
            assert_eq!(key.algorithm(), alg);
            let cert_der = read(&rel.replace("-legacy-3des", "").replace(".p12", ".crt.der"));
            assert_eq!(key.chain().signer.as_der(), cert_der.as_slice());
            assert!(key.chain().intermediates.is_empty());
            sign_check(&key);
        }
        let list = list_aliases(&read(rel), "test").unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].is_key_entry);
        assert!(list[0].subject.as_deref().unwrap().contains("NUR TEST"));
    }
    // Titel-RDN bleibt im Zertifikat erhalten
    let key = open("keystores/test-ecp384-title.p12", "test", None).unwrap();
    assert!(key.chain().signer.subject_rdns().iter().any(|a| a.short_name.as_deref() == Some("title")));
}

#[test]
fn encryption_variants_from_openssl_and_java() {
    for rel in [
        "backend-keystore/openssl-legacy-rc2.p12",        // RC2-40 + 3DES, MAC SHA-1
        "backend-keystore/openssl-pbes2-3des.p12",        // PBES2 mit DES-EDE3-CBC (pkcs5 `3des`)
        "backend-keystore/openssl-unencrypted-nomac.p12", // unverschlüsselt, ohne MAC
        "backend-keystore/openssl-nomac.p12",             // verschlüsselt, ohne MAC
        "backend-keystore/java-modern.p12",               // JDK 21: PBES2/AES-256, HmacPBESHA256
        "backend-keystore/java-legacy.p12",               // JDK legacy: RC2-40/3DES, 50 000 Iterationen
        "backend-keystore/java-sha1-prf.p12",             // PBES2 mit PRF HMAC-SHA-1 (pkcs5 `sha1-insecure`)
    ] {
        let key = open(rel, "test", None).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(key.key_type(), KeyType::Rsa { bits: 2048 }, "{rel}");
        sign_check(&key);
        // falsches Passwort → WrongPassword (ohne MAC über die Entschlüsselung erkannt)
        if !rel.contains("unencrypted") {
            assert!(matches!(open(rel, "falsch", None), Err(KeystoreError::WrongPassword)), "{rel}");
        }
    }
    // leeres Passwort und Umlaute (UTF-8)
    open("backend-keystore/openssl-empty-password.p12", "", None).unwrap();
    assert!(matches!(
        open("backend-keystore/openssl-empty-password.p12", "test", None),
        Err(KeystoreError::WrongPassword)
    ));
    open("backend-keystore/openssl-umlaut-password.p12", "Pässwörd €!", None).unwrap();
    assert!(matches!(
        open("backend-keystore/openssl-umlaut-password.p12", "Passwoerd €!", None),
        Err(KeystoreError::WrongPassword)
    ));
}

#[test]
fn unsupported_schemes_are_reported_before_any_password_check() {
    let e = open("backend-keystore/openssl-md5-des.p12", "test", None).unwrap_err();
    assert!(matches!(&e, KeystoreError::UnsupportedPbe(m) if m.contains("pbeWithMD5AndDES-CBC")), "{e}");
    // auch mit falschem Passwort derselbe Fehler (Vorprüfung ohne Passwort)
    assert!(matches!(
        open("backend-keystore/openssl-md5-des.p12", "falsch", None),
        Err(KeystoreError::UnsupportedPbe(_))
    ));
    let e = open("backend-keystore/openssl-pbmac1.p12", "test", None).unwrap_err();
    assert!(matches!(&e, KeystoreError::UnsupportedPbe(m) if m.contains("PBMAC1")), "{e}");
    assert_eq!(e.code(), "keystore-unsupported-pbe");
}

#[test]
fn one_password_mapping_for_separate_store_and_key_passwords() {
    let rsa = read("keystores/test-rsa2048.p12");
    // abweichendes Schlüsselpasswort bei Ein-Passwort-Datei: Store-Passwort genügt
    LoadedKey::open_pkcs12_with(&rsa, "test", Some("anders"), None).unwrap();
    // vertauscht/veraltet: Store-Passwort falsch, Schlüsselpasswort ist das einzige Passwort
    LoadedKey::open_pkcs12_with(&rsa, "falsch", Some("test"), None).unwrap();
    // leeres Schlüsselpasswort = keins
    LoadedKey::open_pkcs12_with(&rsa, "test", Some(""), None).unwrap();
    assert!(matches!(
        LoadedKey::open_pkcs12_with(&rsa, "falsch", Some("auch falsch"), None),
        Err(KeystoreError::WrongPassword)
    ));

    // Java-Keystore mit getrennten Passwörtern (Store "test", Schlüssel "schluessel")
    let sep = read("backend-keystore/java-separate-keypass.p12");
    for key_pw in [None, Some("schluessel"), Some("test")] {
        let r = LoadedKey::open_pkcs12_with(&sep, "test", key_pw, None);
        assert!(matches!(r, Err(KeystoreError::DifferentKeyPassword)), "{key_pw:?}: {r:?}");
    }
    // Schlüsselpasswort allein besteht den MAC nicht
    assert!(matches!(LoadedKey::open_pkcs12_with(&sep, "schluessel", None, None), Err(KeystoreError::WrongPassword)));
    assert!(matches!(LoadedKey::open_pkcs12_with(&sep, "falsch", None, None), Err(KeystoreError::WrongPassword)));
    let e = LoadedKey::open_pkcs12_with(&sep, "test", None, None).unwrap_err();
    assert_eq!(e.code(), "keystore-different-key-password");
    assert!(e.to_string().contains("einheitlichem Passwort"));
}

#[test]
fn alias_selection() {
    let two = read("backend-keystore/java-two-keys.p12");
    assert!(matches!(LoadedKey::open_pkcs12(&two, "test", None), Err(KeystoreError::AliasRequired(2))));
    let k = LoadedKey::open_pkcs12(&two, "test", Some("erster")).unwrap();
    assert_eq!(k.key_type(), KeyType::Rsa { bits: 2048 });
    // ohne Groß-/Kleinschreibung, gemeldet wird der gespeicherte Alias
    let k = LoadedKey::open_pkcs12(&two, "test", Some("ZWEITER")).unwrap();
    assert_eq!(k.alias(), "zweiter");
    assert_eq!(k.key_type(), KeyType::Ec { curve: EcCurve::P256 });
    sign_check(&k);
    assert!(matches!(
        LoadedKey::open_pkcs12(&two, "test", Some("vertrauensanker")),
        Err(KeystoreError::NotAKeyEntry(a)) if a == "vertrauensanker"
    ));
    assert!(matches!(
        LoadedKey::open_pkcs12(&two, "test", Some("fehlt")),
        Err(KeystoreError::AliasNotFound(a)) if a == "fehlt"
    ));
    let list = list_aliases(&two, "test").unwrap();
    let names: Vec<_> = list.iter().map(|a| (a.alias.as_str(), a.is_key_entry)).collect();
    assert_eq!(names, [("erster", true), ("vertrauensanker", false), ("zweiter", true)]);
    assert!(matches!(list_aliases(&two, "falsch"), Err(KeystoreError::WrongPassword)));

    // Java speichert Aliasse klein; PDF-Over-Konfigurationen können die Originalschreibweise tragen
    let mixed = read("backend-keystore/java-mixed-case-alias.p12");
    let k = LoadedKey::open_pkcs12(&mixed, "test", Some("Mein Signaturschluessel")).unwrap();
    assert_eq!(k.alias(), "mein signaturschluessel");
    // ohne friendlyName: Alias = Subject-Text (p12-keystore), einziger Eintrag ohne Alias wählbar
    let noname = read("backend-keystore/openssl-no-name.p12");
    let k = LoadedKey::open_pkcs12(&noname, "test", None).unwrap();
    assert!(!k.alias().is_empty());
    let k2 = LoadedKey::open_pkcs12(&noname, "test", Some(k.alias())).unwrap();
    assert_eq!(k2.alias(), k.alias());
}

#[test]
fn p521_with_chain() {
    let key = open("backend-keystore/openssl-p521-chain.p12", "test", Some("p521")).unwrap();
    assert_eq!(key.key_type(), KeyType::Ec { curve: EcCurve::P521 });
    assert_eq!(key.algorithm(), SignatureAlg::Ecdsa(DigestAlg::Sha512));
    assert_eq!(key.chain().signer.as_der(), read("cades/leaf-p521.crt.der").as_slice());
    // Zwischen-CA ja, selbstsignierte Wurzel nein
    assert_eq!(key.chain().intermediates.len(), 1);
    assert_eq!(key.chain().intermediates[0].as_der(), read("cades/ca-intermediate.crt.der").as_slice());
    sign_check(&key);
}

#[test]
fn java_formats_need_conversion() {
    for (rel, fmt) in [
        ("backend-keystore/java-rsa2048.jks", KeystoreFormat::Jks),
        ("backend-keystore/java-rsa2048.jceks", KeystoreFormat::Jceks),
    ] {
        let bytes = read(rel);
        assert_eq!(pdfover_backend_keystore::detect_format(&bytes), fmt);
        assert!(
            matches!(LoadedKey::open_pkcs12(&bytes, "test", None), Err(KeystoreError::ConversionNeeded(f)) if f == fmt)
        );
        assert!(matches!(list_aliases(&bytes, "test"), Err(KeystoreError::ConversionNeeded(_))));
        let hint = pdfover_backend_keystore::conversion_hint(fmt, &td(rel)).unwrap();
        assert!(hint.contains("-deststoretype PKCS12"));
    }
}

#[test]
fn unsupported_keys_and_mismatch() {
    for name in ["brainpoolP256r1", "ed25519", "dsa", "secp224r1"] {
        let r = open(&format!("backend-keystore/openssl-{name}.p12"), "test", None);
        assert!(matches!(r, Err(KeystoreError::UnsupportedKey(_))), "{name}: {r:?}");
    }
    assert!(matches!(
        open("backend-keystore/java-key-mismatch.p12", "test", None),
        Err(KeystoreError::KeyCertificateMismatch)
    ));
    // Zertifikatsseitig: Schlüsseltypen werden erkannt
    let kt = |n: &str| {
        pdfover_cades::Certificate::from_der(&read(&format!("backend-keystore/{n}.crt.der")))
            .unwrap()
            .key_type()
            .unwrap()
    };
    assert_eq!(kt("brainpoolP256r1"), KeyType::Ec { curve: EcCurve::BrainpoolP256r1 });
    assert_eq!(kt("ed25519"), KeyType::Other("Ed25519".into()));
    assert_eq!(kt("dsa"), KeyType::Dsa);
    assert_eq!(kt("secp224r1"), KeyType::Ec { curve: EcCurve::Other("1.3.132.0.33".into()) });
}

#[test]
fn malformed_input() {
    for bytes in [&b""[..], b"garbage", &[0x30, 0x03, 0x02, 0x01, 0x03], &[0x30, 0x80, 0x00, 0x00]] {
        let r = LoadedKey::open_pkcs12(bytes, "test", None);
        assert!(matches!(r, Err(KeystoreError::Malformed(_))), "{bytes:?}: {r:?}");
    }
    let full = read("keystores/test-rsa2048.p12");
    assert!(matches!(LoadedKey::open_pkcs12(&full[..full.len() / 2], "test", None), Err(KeystoreError::Malformed(_))));
    // ein Byte im verschlüsselten Bereich verändert → MAC schlägt fehl → wie falsches Passwort
    let mut bad = full.clone();
    let n = bad.len();
    bad[n - 100] ^= 0x01;
    assert!(LoadedKey::open_pkcs12(&bad, "test", None).is_err());
}

#[test]
fn debug_output_has_no_secrets_or_personal_data() {
    let key = open("keystores/test-rsa2048.p12", "test", None).unwrap();
    let dbg = format!("{key:?}");
    assert!(!dbg.contains("NUR TEST"), "{dbg}");
    let list = list_aliases(&read("keystores/test-rsa2048.p12"), "test").unwrap();
    assert!(!format!("{list:?}").contains("NUR TEST"));
}

/// Modell A Ende-zu-Ende: Hash der ByteRange-Daten → SignedAttrs → Rohsignatur des Keystores →
/// `assemble` → eigener Verifier und (falls vorhanden) OpenSSL.
#[test]
fn model_a_end_to_end_with_all_keys() {
    let content = b"%PDF-1.7 ByteRange-Daten (NUR TEST)".repeat(100);
    let dir = tempfile::tempdir().unwrap();
    let have_openssl = Command::new("openssl").arg("version").output().is_ok_and(|o| o.status.success());
    for (rel, alias) in [
        ("keystores/test-rsa2048.p12", None),
        ("keystores/test-ecp256.p12", None),
        ("keystores/test-ecp384-title.p12", None),
        ("keystores/test-rsa2048-legacy-3des.p12", None),
        ("backend-keystore/openssl-p521-chain.p12", Some("p521")),
        ("backend-keystore/java-legacy.p12", None),
    ] {
        let key = open(rel, "test", alias).unwrap();
        for profile in [CmsProfile::PdfasCompat, CmsProfile::PadesBB] {
            let md = key.algorithm().digest().digest(&content);
            let cms = sign_detached(&key, &md, profile, SigningTime(time::OffsetDateTime::now_utc())).unwrap();
            let expect =
                Expect { signer: ExpectedSigner::Certificate(key.chain().signer.clone()), ..Expect::default() };
            let r = verify_detached(&cms, &mut content.as_slice(), &expect).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(r.signature_alg, key.algorithm());
            if have_openssl {
                let c = dir.path().join("c.bin");
                let s = dir.path().join("s.der");
                std::fs::write(&c, &content).unwrap();
                std::fs::write(&s, &cms).unwrap();
                let out = Command::new("openssl")
                    .args(["cms", "-verify", "-binary", "-inform", "DER", "-noverify", "-out"])
                    .arg(dir.path().join("o.bin"))
                    .arg("-in")
                    .arg(&s)
                    .arg("-content")
                    .arg(&c)
                    .output()
                    .unwrap();
                assert!(out.status.success(), "{rel}/{profile:?}: {}", String::from_utf8_lossy(&out.stderr));
            }
        }
    }
}

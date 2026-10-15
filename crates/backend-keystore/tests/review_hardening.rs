//! Regressionstests aus dem adversarialen Review von Paket B: PKCS#12 ohne `localKeyId` (Java bzw.
//! PDF-Over lädt sie, `p12-keystore` `Strict` verwarf den Schlüssel), Redaktion von Alias/Pfad in
//! `Debug`, Abbruch während des Entsperrens und Größengrenze der Keystore-Datei.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use pdfover_backend_api::{
    BackendError, BackendEvent, CancelToken, FormAnswer, InteractionBroker, InteractionError, NativeForm, Notice,
    NoticeAnswer, Progress, SecretPrompt, SessionContext, SignatureBackend,
};
use pdfover_backend_keystore::{KeystoreBackend, KeystoreConfig, KeystoreError, LoadedKey, list_aliases};
use pdfover_cades::{
    AssembleOptions, CmsProfile, DigestAlg, EcCurve, Expect, ExpectedSigner, KeyType, RawSigner, SignatureAlg,
    SignedAttrsBuilder, SigningTime, assemble, verify_detached, verify_raw,
};
use zeroize::Zeroizing;

fn td(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(td(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn ctx(cancel: CancelToken) -> SessionContext {
    SessionContext { language: Default::default(), batch_size: 1, cancel, headless: false }
}

#[test]
fn keys_without_local_key_id_are_linked_like_java() {
    // friendlyName am Schlüssel, keine localKeyId: Verknüpfung über den öffentlichen Schlüssel
    let bytes = read("backend-keystore/openssl-no-localkeyid.p12");
    let key = LoadedKey::open_pkcs12(&bytes, "test", None).unwrap();
    assert_eq!(key.alias(), "test-rsa2048");
    assert_eq!(key.key_type(), KeyType::Rsa { bits: 2048 });
    assert_eq!(key.chain().signer.as_der(), read("keystores/test-rsa2048.crt.der").as_slice());
    let d = DigestAlg::Sha256.digest(b"ohne localKeyId");
    verify_raw(&key.chain().signer, &d, &key.sign_digest(&d).unwrap()).unwrap();
    LoadedKey::open_pkcs12(&bytes, "test", Some("TEST-RSA2048")).unwrap();
    assert!(matches!(LoadedKey::open_pkcs12(&bytes, "falsch", None), Err(KeystoreError::WrongPassword)));
    let list = list_aliases(&bytes, "test").unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].is_key_entry && list[0].alias == "test-rsa2048");

    // ganz ohne Attribute, Kette in fremder Reihenfolge (Root vor Zwischen-CA)
    let bytes = read("backend-keystore/openssl-no-localkeyid-chain.p12");
    let key = LoadedKey::open_pkcs12(&bytes, "test", None).unwrap();
    assert_eq!(key.key_type(), KeyType::Ec { curve: EcCurve::P521 });
    assert_eq!(key.algorithm(), SignatureAlg::Ecdsa(DigestAlg::Sha512));
    assert_eq!(key.chain().signer.as_der(), read("cades/leaf-p521.crt.der").as_slice());
    assert_eq!(key.chain().intermediates.len(), 1, "Zwischen-CA ja, selbstsignierte Wurzel nein");
    assert_eq!(key.chain().intermediates[0].as_der(), read("cades/ca-intermediate.crt.der").as_slice());
    // Alias = Subject-Text (wie p12-keystore ohne friendlyName), damit wieder wählbar
    assert!(key.alias().contains("P-521"), "{}", key.alias());
    let again = LoadedKey::open_pkcs12(&bytes, "test", Some(key.alias())).unwrap();
    assert_eq!(again.chain(), key.chain());
    let list = list_aliases(&bytes, "test").unwrap();
    assert_eq!(
        list.iter().filter(|a| a.is_key_entry).count(),
        1,
        "nur der Schlüssel, CA-Zertifikate sind keine Einträge"
    );

    // Modell A mit Kette: eigener Verifier und – falls vorhanden – OpenSSL
    let content = b"ByteRange-Daten ohne localKeyId (NUR TEST)".repeat(20);
    let md = DigestAlg::Sha512.digest(&content);
    let now = SigningTime(time::OffsetDateTime::now_utc());
    let attrs = SignedAttrsBuilder::new(CmsProfile::PadesBB, &md, &key.chain().signer, now).build().unwrap();
    let sig = key.sign_digest(&attrs.digest(DigestAlg::Sha512)).unwrap();
    let opts = AssembleOptions { profile: CmsProfile::PadesBB, include_intermediates: true };
    let cms = assemble(&attrs, &sig, key.chain(), &opts).unwrap();
    let expect = Expect { signer: ExpectedSigner::Certificate(key.chain().signer.clone()), ..Expect::default() };
    let r = verify_detached(&cms, &mut content.as_slice(), &expect).unwrap();
    assert_eq!(r.certificates.len(), 2);
    if Command::new("openssl").arg("version").output().is_ok_and(|o| o.status.success()) {
        let dir = tempfile::tempdir().unwrap();
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
}

#[test]
fn debug_output_hides_alias_and_directories() {
    // ohne friendlyName ist der Alias der Subject-Text (Personendaten)
    let bytes = read("backend-keystore/openssl-no-name.p12");
    let key = LoadedKey::open_pkcs12(&bytes, "test", None).unwrap();
    assert!(key.alias().contains("NUR TEST"));
    let dbg = format!("{key:?}");
    assert!(!dbg.contains("NUR TEST"), "{dbg}");
    let list = list_aliases(&bytes, "test").unwrap();
    let dbg = format!("{list:?}");
    assert!(!dbg.contains("NUR TEST"), "{dbg}");

    let mut cfg = KeystoreConfig::new("/home/max.mustermann/Schluessel/signatur.p12");
    cfg.alias = Some("Max Mustermann".into());
    let dbg = format!("{:?}", KeystoreBackend::new(cfg));
    assert!(dbg.contains("signatur.p12"), "{dbg}");
    assert!(!dbg.contains("mustermann") && !dbg.contains("Mustermann"), "{dbg}");
}

/// Broker, der beim Liefern des Passworts zugleich abbricht (Nutzerin schließt das Fenster, während
/// der Keystore entschlüsselt wird).
#[derive(Debug)]
struct CancelOnSecret(CancelToken);

#[async_trait]
impl InteractionBroker for CancelOnSecret {
    async fn native_form(&self, _: NativeForm) -> Result<FormAnswer, InteractionError> {
        Err(InteractionError::Unsupported)
    }
    async fn secret(&self, _: SecretPrompt) -> Result<Zeroizing<String>, InteractionError> {
        self.0.cancel();
        Ok(Zeroizing::new("test".into()))
    }
    async fn notice(&self, _: Notice) -> Result<NoticeAnswer, InteractionError> {
        Err(InteractionError::Unsupported)
    }
    fn progress(&self, _: Progress) {}
    fn event(&self, _: BackendEvent) {}
    fn open_external(&self, _: &str) -> Result<(), InteractionError> {
        Ok(())
    }
}

#[tokio::test]
async fn cancel_during_unlock_wins_over_the_key_derivation() {
    let b = KeystoreBackend::new(KeystoreConfig::new(td("keystores/test-rsa2048.p12")));
    let token = CancelToken::new();
    let r = b.open(ctx(token.clone()), Arc::new(CancelOnSecret(token))).await;
    assert_eq!(r.err(), Some(BackendError::Cancelled));
}

#[tokio::test]
async fn oversized_file_is_rejected_without_reading_it_completely() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("riesig.p12");
    let f = std::fs::File::create(&path).unwrap();
    f.set_len(17 << 20).unwrap(); // dünn besetzt, schnell
    drop(f);
    let mut cfg = KeystoreConfig::new(&path);
    cfg.store_password = Some(Zeroizing::new("test".into()));
    let b = KeystoreBackend::new(cfg);
    let e = b.open(ctx(CancelToken::new()), Arc::new(CancelOnSecret(CancelToken::new()))).await.err().unwrap();
    assert!(matches!(&e, BackendError::Keystore(m) if m.contains("MiB")), "{e:?}");
}

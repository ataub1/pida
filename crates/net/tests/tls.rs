//! TLS über rustls + rustls-platform-verifier: ein selbstsigniertes Zertifikat wird vom
//! OS-Truststore abgelehnt und als [`NetError::Tls`] gemeldet (nicht als Verbindungsfehler).
//! Das Zertifikat erzeugt `openssl` zur Laufzeit; ohne `openssl` wird der Test übersprungen.

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use pida_net::{HttpClient, NetConfig, NetError, ProxyConfig, Purpose, Url};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn self_signed(dir: &std::path::Path) -> Option<(Vec<u8>, Vec<u8>)> {
    let key_pem = dir.join("key.pem");
    let cert_der = dir.join("cert.der");
    let key_der = dir.join("key.der");
    let ok = Command::new("openssl")
        .args(["req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1", "-nodes", "-keyout"])
        .arg(&key_pem)
        .args(["-out"])
        .arg(&cert_der)
        .args([
            "-outform",
            "DER",
            "-subj",
            "/CN=127.0.0.1 (NUR TEST)",
            "-days",
            "2",
            "-addext",
            "subjectAltName=IP:127.0.0.1",
        ])
        .output()
        .ok()?
        .status
        .success()
        && Command::new("openssl")
            .args(["pkcs8", "-topk8", "-nocrypt", "-in"])
            .arg(&key_pem)
            .args(["-outform", "DER", "-out"])
            .arg(&key_der)
            .output()
            .ok()?
            .status
            .success();
    ok.then(|| (std::fs::read(&cert_der).unwrap(), std::fs::read(&key_der).unwrap()))
}

#[tokio::test]
async fn self_signed_certificate_is_rejected_as_tls_error() {
    let dir = tempfile::tempdir().unwrap();
    let Some((cert, key)) = self_signed(dir.path()) else {
        eprintln!("openssl fehlt – TLS-Test übersprungen");
        return;
    };
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(cert)],
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(key)),
        )
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                if let Ok(mut tls) = acceptor.accept(stream).await {
                    let mut buf = [0u8; 1024];
                    let _ = tls.read(&mut buf).await;
                    let _ = tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await;
                }
            });
        }
    });

    let cfg =
        NetConfig { proxy: ProxyConfig::None, request_timeout: Some(Duration::from_secs(20)), ..NetConfig::default() };
    let client = HttpClient::new(&cfg, Purpose::AtrustSession).unwrap();
    let err = client.get(&Url::parse(&format!("https://{addr}/")).unwrap(), &[]).await.unwrap_err();
    assert!(matches!(err, NetError::Tls(_)), "{err:?}");
    server.abort();
}

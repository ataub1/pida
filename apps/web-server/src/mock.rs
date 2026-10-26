//! **Testgegenstelle statt A-Trust** (nur mit `--mock-atrust`, nie im Betrieb): verhält sich für den Rückkanal wie
//! das SL-HTTPS-Binding von A-Trust mit `DataURL` – nimmt den Formular-POST der Seite an (`XMLRequest`, `DataURL`),
//! zeigt eine eigene, deutlich gekennzeichnete Bestätigungsseite im Overlay, führt den Befehl mit einem
//! Test-Keystore aus und schickt die SL-Antwort **per HTTP** (`multipart/form-data`, `XMLResponse`) an die DataURL.
//! Antwortet die DataURL mit `text/xml`, ist das der nächste Befehl derselben Sitzung (ohne neue „Anmeldung“);
//! sonst zeigt die Testgegenstelle die Antwort der DataURL (Abschlussseite).
//!
//! So lässt sich die ganze Kette lokal prüfen, ohne Anfragen an A-Trust. Die DataURL muss auf diesen Server zeigen
//! (`http://127.0.0.1:<port>/…` bzw. `localhost`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::Engine;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use pida_backend_keystore::LoadedKey;
use pida_cades::{CmsProfile, Digest, RawSigner, SigningTime};

const SL: &str = "http://www.buergerkarte.at/namespaces/securitylayer/1.2#";

/// Ausgeführter Befehl → SL-Antwort.
fn execute(key: &LoadedKey, request: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    if request.contains("InfoboxReadRequest") {
        return format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><sl:InfoboxReadResponse xmlns:sl="{SL}"><sl:AssocArrayData><sl:Pair Key="SecureSignatureKeypair"><sl:Base64Content>{}</sl:Base64Content></sl:Pair></sl:AssocArrayData></sl:InfoboxReadResponse>"#,
            b64.encode(key.chain().signer.as_der())
        );
    }
    if request.contains("CreateCMSSignatureRequest") {
        let text = |tag: &str| {
            let open = format!("<{tag}>");
            let start = request.find(&open)? + open.len();
            let end = request[start..].find(&format!("</{tag}>"))? + start;
            Some(request[start..end].trim().to_owned())
        };
        let result = (|| {
            let data = b64.decode(text("Base64Content")?.split_whitespace().collect::<String>()).ok()?;
            let from: usize = text("From")?.parse().ok()?;
            let to: usize = text("To")?.parse().ok()?;
            if from > to || to >= data.len() {
                return None;
            }
            let alg = key.algorithm().digest();
            let mut hasher = alg.hasher();
            hasher.update(&data[..from]);
            hasher.update(&data[to + 1..]);
            let digest: Digest = hasher.finalize();
            pida_cades::sign_detached(
                key,
                &digest,
                CmsProfile::PdfasCompat,
                SigningTime(time::OffsetDateTime::now_utc()),
            )
            .ok()
        })();
        return match result {
            Some(cms) => format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><sl:CreateCMSSignatureResponse xmlns:sl="{SL}"><sl:CMSSignature>{}</sl:CMSSignature></sl:CreateCMSSignatureResponse>"#,
                b64.encode(cms)
            ),
            None => error(4000, "Befehl nicht lesbar (Testgegenstelle)"),
        };
    }
    error(4000, "Befehl nicht unterstützt (Testgegenstelle)")
}

fn error(code: u32, info: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><sl:ErrorResponse xmlns:sl="{SL}"><sl:ErrorCode>{code}</sl:ErrorCode><sl:Info>{info}</sl:Info></sl:ErrorResponse>"#
    )
}

fn page(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title}</title>
<style>body{{font:15px/1.5 system-ui,sans-serif;margin:0;padding:24px;color:#14171c;background:#fff8e1}}.tag{{display:inline-block;padding:2px 8px;border-radius:6px;background:#b0102a;color:#fff;font-weight:600;font-size:12px}}h1{{font-size:20px}}button{{font:inherit;padding:10px 16px;border-radius:8px;border:1px solid #16191f;margin-right:8px;cursor:pointer}}.primary{{background:#16191f;color:#fff}}</style></head>
<body><p class="tag">PIDA-Testgegenstelle – kein A-Trust, keine echte Signatur</p><h1>{title}</h1>{body}</body></html>"#
    )
}

struct Pending {
    data_url: String,
    request: String,
    step: usize,
}

/// Testgegenstelle (Sitzungen im Speicher).
#[derive(Clone)]
pub struct MockAtrust {
    key: Arc<LoadedKey>,
    port: u16,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
}

/// Antwort an das Overlay.
pub struct Html(pub String);

impl MockAtrust {
    /// Mit Test-Keystore.
    pub fn new(key: LoadedKey, port: u16) -> Self {
        Self { key: Arc::new(key), port, pending: Arc::default() }
    }

    fn allowed_data_url(&self, url: &str) -> bool {
        [format!("http://127.0.0.1:{}/", self.port), format!("http://localhost:{}/", self.port)]
            .iter()
            .any(|p| url.starts_with(p.as_str()))
    }

    fn confirm_page(&self, id: &str, p: &Pending) -> Html {
        let what = if p.request.contains("InfoboxReadRequest") {
            "Anmeldung simulieren und Zertifikat freigeben (InfoboxRead)".to_owned()
        } else if p.request.contains("CreateCMSSignatureRequest") {
            format!("Signatur bestätigen (CreateCMSSignature, Befehl {} dieser Sitzung)", p.step)
        } else {
            "Unbekannter Befehl".to_owned()
        };
        Html(page(
            "ID Austria (Test)",
            &format!(
                r#"<p>{what}</p><form method="post" action="/mock-atrust/confirm/{id}" style="display:inline"><button class="primary" type="submit">Bestätigen</button></form><form method="post" action="/mock-atrust/cancel/{id}" style="display:inline"><button type="submit">Ablehnen</button></form>"#
            ),
        ))
    }

    /// Formular-POST der Seite.
    pub fn start(&self, xml_request: &str, data_url: &str) -> Html {
        if !self.allowed_data_url(data_url) {
            return Html(page("Abgelehnt", "<p>Die DataURL zeigt nicht auf diesen Server.</p>"));
        }
        let id = {
            let mut buf = [0u8; 12];
            rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut buf);
            buf.iter().map(|b| format!("{b:02x}")).collect::<String>()
        };
        let p = Pending { data_url: data_url.to_owned(), request: xml_request.to_owned(), step: 1 };
        let html = self.confirm_page(&id, &p);
        self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id, p);
        html
    }

    /// „Bestätigen“ bzw. „Ablehnen“ (SL 6001).
    pub async fn answer(&self, id: &str, confirm: bool) -> Html {
        let Some(mut p) = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(id) else {
            return Html(page("Sitzung unbekannt", "<p>Diese Testsitzung ist abgelaufen.</p>"));
        };
        let response = if confirm {
            execute(&self.key, &p.request)
        } else {
            error(6001, "Abbruch durch die Nutzerin (Testgegenstelle)")
        };
        match post_data_url(&p.data_url, &response).await {
            Ok((content_type, body)) if content_type.to_ascii_lowercase().starts_with("text/xml") => {
                p.request = String::from_utf8_lossy(&body).into_owned();
                p.step += 1;
                let html = self.confirm_page(id, &p);
                self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id.to_owned(), p);
                html
            }
            Ok((_, body)) => Html(String::from_utf8_lossy(&body).into_owned()),
            Err(e) => Html(page("DataURL nicht erreichbar", &format!("<p>{e}</p>"))),
        }
    }
}

/// POST der SL-Antwort an die DataURL wie A-Trust (`multipart/form-data`: `ResponseType`, `XMLResponse`).
async fn post_data_url(url: &str, xml: &str) -> Result<(String, Vec<u8>), String> {
    let rest = url.strip_prefix("http://").ok_or("nur http")?;
    let (authority, path) = rest.split_once('/').map(|(a, p)| (a, format!("/{p}"))).unwrap_or((rest, "/".into()));
    let stream = tokio::net::TcpStream::connect(authority).await.map_err(|e| e.to_string())?;
    let (mut sender, conn) =
        hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(stream)).await.map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let boundary = "----pida-mock-atrust";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"ResponseType\"\r\n\r\nHTTP-Security-Layer-RESPONSE\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"XMLResponse\"\r\nContent-Type: text/xml; charset=UTF-8\r\n\r\n{xml}\r\n--{boundary}--\r\n"
    );
    let request = Request::post(path)
        .header("Host", authority)
        .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
        .body(Full::new(Bytes::from(body)))
        .map_err(|e| e.to_string())?;
    let response = sender.send_request(request).await.map_err(|e| e.to_string())?;
    let content_type =
        response.headers().get(hyper::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    let body = response.into_body().collect().await.map_err(|e| e.to_string())?.to_bytes().to_vec();
    Ok((content_type, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> LoadedKey {
        let bytes =
            std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.p12")).unwrap();
        LoadedKey::open_pkcs12(&bytes, "test", None).unwrap()
    }

    #[test]
    fn executes_infobox_and_cms() {
        let key = key();
        let infobox = execute(&key, "<InfoboxReadRequest/>");
        assert!(infobox.contains("SecureSignatureKeypair"));
        let data = b"%PDF-1.4 <0000> rest";
        let req = format!(
            "<CreateCMSSignatureRequest><Base64Content>{}</Base64Content><From>9</From><To>14</To></CreateCMSSignatureRequest>",
            base64::engine::general_purpose::STANDARD.encode(data)
        );
        assert!(execute(&key, &req).contains("CMSSignature"));
        assert!(
            execute(&key, "<CreateCMSSignatureRequest><From>9</From></CreateCMSSignatureRequest>")
                .contains("ErrorResponse")
        );
    }

    #[test]
    fn data_url_must_point_here() {
        let m = MockAtrust::new(key(), 1442);
        assert!(m.allowed_data_url("http://127.0.0.1:1442/api/sl/data/x"));
        assert!(!m.allowed_data_url("http://127.0.0.1:9999/api/sl/data/x"));
        assert!(!m.allowed_data_url("https://example.org/"));
    }
}

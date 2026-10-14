//! Test-Hilfen (Feature `testing`): ein minimaler HTTP/1.1-Server auf `127.0.0.1` mit frei
//! programmierbarem Handler und Aufrufprotokoll. Nur für Tests – kein TLS, keine Pipelining-
//! Unterstützung, jede Antwort schließt die Verbindung (`Connection: close`).
//!
//! Der Server kann auch als Klartext-Proxy dienen (Request-Ziel in absoluter Form, `CONNECT`).

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::Url;

/// Aufgezeichneter Request.
#[derive(Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    /// Methode (`GET`, `POST`, `CONNECT` …)
    pub method: String,
    /// Request-Ziel wie gesendet (Pfad mit Abfrage bzw. absolute URL beim Proxy)
    pub target: String,
    /// Header in Empfangsreihenfolge, Namen kleingeschrieben
    pub headers: Vec<(String, String)>,
    /// Körper
    pub body: Vec<u8>,
}

impl std::fmt::Debug for RecordedRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordedRequest")
            .field("method", &self.method)
            .field("target", &self.target)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Teil eines empfangenen Multipart-Körpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedPart {
    /// `name`
    pub name: String,
    /// `filename`
    pub filename: Option<String>,
    /// `Content-Type`
    pub content_type: Option<String>,
    /// Inhalt
    pub data: Vec<u8>,
}

impl ReceivedPart {
    /// Inhalt als UTF-8.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }
}

impl RecordedRequest {
    /// Erster Header (ohne Groß-/Kleinschreibung).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// Pfad ohne Abfrage.
    pub fn path(&self) -> &str {
        let t = self.target.as_str();
        let t = t.split_once("://").map(|(_, rest)| rest.find('/').map_or("/", |i| &rest[i..])).unwrap_or(t);
        t.split('?').next().unwrap_or(t)
    }

    /// Abfrage ohne `?`.
    pub fn query(&self) -> Option<&str> {
        self.target.split_once('?').map(|(_, q)| q)
    }

    /// Felder eines urlencoded Körpers.
    pub fn form_fields(&self) -> Vec<(String, String)> {
        url::form_urlencoded::parse(&self.body).into_owned().collect()
    }

    /// Erstes urlencoded Feld mit diesem Namen.
    pub fn form_field(&self, name: &str) -> Option<String> {
        self.form_fields().into_iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    /// Teile eines `multipart/form-data`-Körpers (leer, wenn kein Multipart).
    pub fn multipart(&self) -> Vec<ReceivedPart> {
        let Some(ct) = self.header("content-type") else { return Vec::new() };
        let Some(boundary) = ct.split(';').find_map(|p| p.trim().strip_prefix("boundary=")) else {
            return Vec::new();
        };
        parse_multipart(&self.body, boundary.trim_matches('"'))
    }

    /// Erster Multipart-Teil mit diesem Namen.
    pub fn part(&self, name: &str) -> Option<ReceivedPart> {
        self.multipart().into_iter().find(|p| p.name == name)
    }
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > haystack.len() {
        return None;
    }
    haystack[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

fn parse_multipart(body: &[u8], boundary: &str) -> Vec<ReceivedPart> {
    let delimiter = format!("--{boundary}");
    let mut parts = Vec::new();
    let Some(mut pos) = find(body, delimiter.as_bytes(), 0) else { return parts };
    loop {
        pos += delimiter.len();
        if body[pos..].starts_with(b"--") {
            break;
        }
        pos += 2; // CRLF
        let Some(header_end) = find(body, b"\r\n\r\n", pos) else { break };
        let headers = String::from_utf8_lossy(&body[pos..header_end]).into_owned();
        let data_start = header_end + 4;
        let next_delim = format!("\r\n{delimiter}");
        let Some(data_end) = find(body, next_delim.as_bytes(), data_start) else { break };
        let mut part = ReceivedPart { name: String::new(), filename: None, content_type: None, data: Vec::new() };
        for line in headers.split("\r\n") {
            let Some((name, value)) = line.split_once(':') else { continue };
            if name.eq_ignore_ascii_case("content-disposition") {
                for param in value.split(';') {
                    let param = param.trim();
                    if let Some(v) = param.strip_prefix("name=") {
                        part.name = v.trim_matches('"').to_owned();
                    } else if let Some(v) = param.strip_prefix("filename=") {
                        part.filename = Some(v.trim_matches('"').to_owned());
                    }
                }
            } else if name.eq_ignore_ascii_case("content-type") {
                part.content_type = Some(value.trim().to_owned());
            }
        }
        part.data = body[data_start..data_end].to_vec();
        parts.push(part);
        pos = data_end + 2;
    }
    parts
}

/// Antwort des Mock-Servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockResponse {
    /// Status
    pub status: u16,
    /// Reason-Phrase (`None` = Standardtext)
    pub reason: Option<String>,
    /// Header
    pub headers: Vec<(String, String)>,
    /// Körper
    pub body: Vec<u8>,
    /// Verzögerung vor der Antwort
    pub delay: Option<Duration>,
    /// Verbindung ohne Antwort schließen (Apache `NoHttpResponseException`)
    pub close_without_response: bool,
}

impl MockResponse {
    /// Antwort mit Status und leerem Körper.
    pub fn status(status: u16) -> Self {
        Self { status, reason: None, headers: Vec::new(), body: Vec::new(), delay: None, close_without_response: false }
    }

    /// 200 mit Körper und Content-Type.
    pub fn body(content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self::status(200).with_header("Content-Type", content_type).with_body(body)
    }

    /// 200 `text/html; charset=utf-8`.
    pub fn html(body: impl Into<String>) -> Self {
        Self::body("text/html; charset=utf-8", body.into().into_bytes())
    }

    /// 200 `text/xml; charset=utf-8`.
    pub fn xml(body: impl Into<String>) -> Self {
        Self::body("text/xml; charset=utf-8", body.into().into_bytes())
    }

    /// 200 `application/json`.
    pub fn json(body: impl Into<String>) -> Self {
        Self::body("application/json; charset=utf-8", body.into().into_bytes())
    }

    /// Redirect (`Location`).
    pub fn redirect(status: u16, location: &str) -> Self {
        Self::status(status).with_header("Location", location)
    }

    /// Verbindung ohne Antwort schließen.
    pub fn no_response() -> Self {
        Self { close_without_response: true, ..Self::status(0) }
    }

    /// Header ergänzen.
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Körper setzen.
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// Reason-Phrase setzen.
    pub fn with_reason(mut self, reason: &str) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Verzögerung setzen.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    fn encode(&self) -> Vec<u8> {
        let reason = self.reason.clone().unwrap_or_else(|| http_reason(self.status).unwrap_or("Status").to_owned());
        let mut out = format!("HTTP/1.1 {} {}\r\n", self.status, reason).into_bytes();
        let mut has_length = false;
        for (name, value) in &self.headers {
            if name.eq_ignore_ascii_case("content-length") {
                has_length = true;
            }
            out.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
        if !has_length {
            out.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
        }
        out.extend_from_slice(b"Connection: close\r\n\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}

fn http_reason(status: u16) -> Option<&'static str> {
    Some(match status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        404 => "Not Found",
        407 => "Proxy Authentication Required",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => return None,
    })
}

/// Handler: bekommt jeden Request und liefert die Antwort.
pub type Handler = Arc<dyn Fn(&RecordedRequest) -> MockResponse + Send + Sync>;

/// Mock-Server auf `127.0.0.1:<zufälliger Port>`; stoppt beim Drop.
pub struct MockServer {
    addr: SocketAddr,
    log: Arc<Mutex<Vec<RecordedRequest>>>,
    task: tokio::task::JoinHandle<()>,
}

impl std::fmt::Debug for MockServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockServer").field("addr", &self.addr).finish_non_exhaustive()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl MockServer {
    /// Startet den Server (innerhalb einer Tokio-Laufzeit).
    pub async fn start(handler: impl Fn(&RecordedRequest) -> MockResponse + Send + Sync + 'static) -> MockServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("Mock-Server: bind");
        let addr = listener.local_addr().expect("Mock-Server: Adresse");
        let log = Arc::new(Mutex::new(Vec::new()));
        let handler: Handler = Arc::new(handler);
        let task_log = Arc::clone(&log);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { break };
                let handler = Arc::clone(&handler);
                let log = Arc::clone(&task_log);
                tokio::spawn(async move {
                    let _ = serve_connection(stream, handler, log).await;
                });
            }
        });
        MockServer { addr, log, task }
    }

    /// Adresse.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http://127.0.0.1:<port><path>`.
    pub fn url(&self, path: &str) -> Url {
        Url::parse(&format!("http://{}{}", self.addr, path)).expect("Mock-URL")
    }

    /// Bisher empfangene Requests.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.log.lock().expect("poisoned").clone()
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    handler: Handler,
    log: Arc<Mutex<Vec<RecordedRequest>>>,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n", 0) {
            break i;
        }
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut rl = request_line.split(' ');
    let method = rl.next().unwrap_or_default().to_owned();
    let target = rl.next().unwrap_or_default().to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let content_length: usize =
        headers.iter().find(|(k, _)| k == "content-length").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    let request = RecordedRequest { method, target, headers, body };
    log.lock().expect("poisoned").push(request.clone());
    let response = handler(&request);
    if let Some(delay) = response.delay {
        tokio::time::sleep(delay).await;
    }
    if response.close_without_response {
        drop(stream);
        return Ok(());
    }
    stream.write_all(&response.encode()).await?;
    stream.flush().await?;
    let _ = stream.shutdown().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_parser_roundtrip() {
        let parts = vec![
            crate::FormPart::file("fileupload", "sign.pdf", "application/pdf", b"%PDF\r\n--x".to_vec()),
            crate::FormPart::text("XMLRequest", "<x/>"),
        ];
        let (ct, body) = crate::multipart::encode(&parts);
        let req = RecordedRequest {
            method: "POST".into(),
            target: "/a?b=c".into(),
            headers: vec![("content-type".into(), ct)],
            body,
        };
        let got = req.multipart();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].name, "fileupload");
        assert_eq!(got[0].filename.as_deref(), Some("sign.pdf"));
        assert_eq!(got[0].data, b"%PDF\r\n--x");
        assert_eq!(got[1].text(), "<x/>");
        assert_eq!(req.path(), "/a");
        assert_eq!(req.query(), Some("b=c"));
        let proxied = RecordedRequest { target: "http://example.invalid/x/y?z".into(), ..req };
        assert_eq!(proxied.path(), "/x/y");
    }
}

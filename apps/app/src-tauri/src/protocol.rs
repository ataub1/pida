//! Custom Protocol `pdfover-render://` für Seitenkacheln und die Blockvorschau (Plan §3.1, §6.6).
//!
//! | URL (macOS/Linux/iOS) | Windows/Android | Inhalt |
//! |---|---|---|
//! | `pdfover-render://localhost/page/<doc>/<seite>?scale=<f>` | `http://pdfover-render.localhost/page/…` | PNG der Seite (0-basiert) |
//! | `pdfover-render://localhost/block?scale=<f>&rev=<n>` | `http://pdfover-render.localhost/block?…` | PNG der Blockvorschau |
//!
//! `scale` ist der Faktor gegenüber 72 dpi (1 pt = 1 px); `rev` dient nur dem Cache-Busting
//! (`BlockView::preview_revision`). Alle Eingaben werden streng geprüft; Antworten tragen nie
//! Dokumentinhalte in Fehlertexten (Regel 9).

use pdfover_workflow::DocId;

/// Name des Schemas.
pub const SCHEME: &str = "pdfover-render";
/// Kleinster erlaubter Maßstab.
pub const MIN_SCALE: f32 = 0.05;
/// Größter erlaubter Maßstab (Schutz vor riesigen Bitmaps; der Renderer begrenzt zusätzlich).
pub const MAX_SCALE: f32 = 8.0;

/// Geprüfte Anfrage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderRequest {
    /// Seitenkachel
    Page {
        /// Dokument
        doc: DocId,
        /// Seite (0-basiert)
        page: usize,
        /// Maßstab
        scale: f32,
    },
    /// Blockvorschau
    Block {
        /// Maßstab
        scale: f32,
    },
}

/// Ungültige Anfrage (→ HTTP 400/404).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderUriError {
    /// Unbekannter Pfad (404)
    NotFound,
    /// Ungültiger Parameter (400)
    BadRequest(&'static str),
}

/// Zerlegt Pfad und Query einer `pdfover-render`-URL.
pub fn parse(path: &str, query: Option<&str>) -> Result<RenderRequest, RenderUriError> {
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    let scale = scale_param(query)?;
    match segments.as_slice() {
        ["page", doc, page] => {
            let doc = doc.parse::<u64>().map_err(|_| RenderUriError::BadRequest("doc"))?;
            let page = page.parse::<usize>().map_err(|_| RenderUriError::BadRequest("page"))?;
            Ok(RenderRequest::Page { doc: DocId(doc), page, scale })
        }
        ["block"] => Ok(RenderRequest::Block { scale }),
        _ => Err(RenderUriError::NotFound),
    }
}

fn scale_param(query: Option<&str>) -> Result<f32, RenderUriError> {
    let mut scale = None;
    for pair in query.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == "scale" {
            let v = value.parse::<f32>().map_err(|_| RenderUriError::BadRequest("scale"))?;
            if !v.is_finite() || !(MIN_SCALE..=MAX_SCALE).contains(&v) {
                return Err(RenderUriError::BadRequest("scale"));
            }
            scale = Some(v);
        }
    }
    Ok(scale.unwrap_or(1.0))
}

/// Antwort als PNG.
pub fn png(bytes: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(200)
        .header(tauri::http::header::CONTENT_TYPE, "image/png")
        .header(tauri::http::header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(bytes)
        .unwrap_or_else(|_| error(500, "response"))
}

/// Fehlerantwort (nur stabiler Code als Text, keine Pfade oder Dokumentdaten).
pub fn error(status: u16, code: &str) -> tauri::http::Response<Vec<u8>> {
    let mut response = tauri::http::Response::new(code.as_bytes().to_vec());
    *response.status_mut() = tauri::http::StatusCode::from_u16(status).unwrap_or(tauri::http::StatusCode::BAD_REQUEST);
    response
        .headers_mut()
        .insert(tauri::http::header::CONTENT_TYPE, tauri::http::HeaderValue::from_static("text/plain; charset=utf-8"));
    response
        .headers_mut()
        .insert(tauri::http::header::CACHE_CONTROL, tauri::http::HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_tiles() {
        assert_eq!(
            parse("/page/7/2", Some("scale=1.5")),
            Ok(RenderRequest::Page { doc: DocId(7), page: 2, scale: 1.5 })
        );
        // ohne Maßstab: 72 dpi
        assert_eq!(parse("page/1/0", None), Ok(RenderRequest::Page { doc: DocId(1), page: 0, scale: 1.0 }));
    }

    #[test]
    fn parses_block_preview_with_revision() {
        assert_eq!(parse("/block", Some("rev=12&scale=2")), Ok(RenderRequest::Block { scale: 2.0 }));
    }

    #[test]
    fn rejects_invalid_input() {
        assert_eq!(parse("/page/x/1", None), Err(RenderUriError::BadRequest("doc")));
        assert_eq!(parse("/page/1/-1", None), Err(RenderUriError::BadRequest("page")));
        assert_eq!(parse("/page/1/1", Some("scale=NaN")), Err(RenderUriError::BadRequest("scale")));
        assert_eq!(parse("/page/1/1", Some("scale=inf")), Err(RenderUriError::BadRequest("scale")));
        assert_eq!(parse("/page/1/1", Some("scale=100")), Err(RenderUriError::BadRequest("scale")));
        assert_eq!(parse("/page/1/1", Some("scale=0")), Err(RenderUriError::BadRequest("scale")));
        assert_eq!(parse("/page/1", None), Err(RenderUriError::NotFound));
        assert_eq!(parse("/../etc/passwd", None), Err(RenderUriError::NotFound));
        assert_eq!(parse("/", None), Err(RenderUriError::NotFound));
    }

    #[test]
    fn responses_have_types_and_no_cache() {
        let ok = png(vec![1, 2, 3]);
        assert_eq!(ok.status(), 200);
        assert_eq!(ok.headers()[tauri::http::header::CONTENT_TYPE], "image/png");
        let bad = error(404, "not-found");
        assert_eq!(bad.status(), 404);
        assert_eq!(bad.body(), b"not-found");
        assert_eq!(bad.headers()[tauri::http::header::CACHE_CONTROL], "no-store");
    }
}

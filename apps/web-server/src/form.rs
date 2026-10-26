//! Formularfelder aus `application/x-www-form-urlencoded` bzw. `multipart/form-data` (A-Trust schickt die
//! SL-Antwort an die DataURL als Formular mit `XMLResponse`, SL-Bindings §3.2.2; die Seite schickt den ersten
//! Befehl als Formular mit `XMLRequest`/`DataURL`).

/// Felder als `(Name, Wert)`; unbekannte Kodierung → leer.
pub fn fields(content_type: &str, body: &[u8]) -> Vec<(String, Vec<u8>)> {
    let lower = content_type.to_ascii_lowercase();
    if lower.starts_with("application/x-www-form-urlencoded") {
        urlencoded(body)
    } else if lower.starts_with("multipart/form-data") {
        boundary(content_type).map(|b| multipart(body, &b)).unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Erstes Feld `name`.
pub fn field<'a>(fields: &'a [(String, Vec<u8>)], name: &str) -> Option<&'a [u8]> {
    fields.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_slice())
}

fn unhex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// `%XX` und `+` dekodieren.
pub fn percent_decode(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < s.len() => match (unhex(s[i + 1]), unhex(s[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    out
}

fn urlencoded(body: &[u8]) -> Vec<(String, Vec<u8>)> {
    body.split(|&b| b == b'&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let mut it = pair.splitn(2, |&b| b == b'=');
            let k = percent_decode(it.next().unwrap_or_default());
            let v = percent_decode(it.next().unwrap_or_default());
            (String::from_utf8_lossy(&k).into_owned(), v)
        })
        .collect()
}

fn boundary(content_type: &str) -> Option<String> {
    content_type.split(';').map(str::trim).find_map(|p| {
        let (k, v) = p.split_once('=')?;
        k.trim().eq_ignore_ascii_case("boundary").then(|| v.trim().trim_matches('"').to_owned())
    })
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn multipart(body: &[u8], boundary: &str) -> Vec<(String, Vec<u8>)> {
    let delim = format!("--{boundary}").into_bytes();
    let mut out = Vec::new();
    let Some(mut pos) = find(body, &delim, 0) else { return out };
    loop {
        let start = pos + delim.len();
        if body.get(start..start + 2) == Some(b"--") {
            break;
        }
        let Some(next) = find(body, &delim, start) else { break };
        let part = &body[start..next];
        let part = part.strip_prefix(b"\r\n").unwrap_or(part);
        let part = part.strip_suffix(b"\r\n").unwrap_or(part);
        if let Some(split) = find(part, b"\r\n\r\n", 0) {
            let head = String::from_utf8_lossy(&part[..split]);
            let name = head.lines().find(|l| l.to_ascii_lowercase().starts_with("content-disposition")).and_then(|l| {
                l.split(';')
                    .map(str::trim)
                    .find_map(|p| p.strip_prefix("name=").map(|v| v.trim_matches('"').to_owned()))
            });
            if let Some(name) = name {
                out.push((name, part[split + 4..].to_vec()));
            }
        }
        pos = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoded_fields() {
        let f = fields(
            "application/x-www-form-urlencoded; charset=UTF-8",
            b"XMLResponse=%3Ca%20b%3D%22c%22%2F%3E&ResponseType=HTTP-Security-Layer-RESPONSE&x=1+2",
        );
        assert_eq!(field(&f, "XMLResponse"), Some(&b"<a b=\"c\"/>"[..]));
        assert_eq!(field(&f, "x"), Some(&b"1 2"[..]));
        assert_eq!(percent_decode(b"%4"), b"%4");
        assert_eq!(percent_decode(b"100%"), b"100%");
    }

    #[test]
    fn multipart_fields() {
        let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"ResponseType\"\r\n\r\nHTTP-Security-Layer-RESPONSE\r\n--XyZ\r\nContent-Disposition: form-data; name=\"XMLResponse\"\r\nContent-Type: text/xml\r\n\r\n<r>\r\n</r>\r\n--XyZ--\r\n";
        let f = fields("multipart/form-data; boundary=XyZ", body);
        assert_eq!(field(&f, "ResponseType"), Some(&b"HTTP-Security-Layer-RESPONSE"[..]));
        assert_eq!(field(&f, "XMLResponse"), Some(&b"<r>\r\n</r>"[..]));
        assert!(fields("text/plain", b"x").is_empty());
        assert!(fields("multipart/form-data; boundary=\"XyZ\"", body).len() == 2);
    }
}

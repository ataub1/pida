//! Eigener, lenienter COS-Lexer und Objektparser (ISO 32000-1 §7.2–§7.3).
//!
//! lopdf exportiert seinen Parser nicht. Für die Analyse der Querverweiskette (Trailer, xref-Tabellen,
//! xref-Streams), für das Nachladen einzelner Objekte an bekannten Offsets und für die
//! Content-Stream-Interpretation braucht `pdf-cos` aber einen Parser, der an beliebigen Offsets
//! aufsetzen kann. Er erzeugt lopdf-Objekte, damit das Objektmodell einheitlich bleibt.
//!
//! Der Parser ist bewusst nachsichtig (wie PDFBox/pdf.js): fehlende Werte werden zu `null`,
//! unbekannte Tokens beenden Arrays/Dictionaries, statt den ganzen Ladevorgang abzubrechen.

use lopdf::{Dictionary, Object, ObjectId, Stream, StringFormat};

/// Maximale Verschachtelungstiefe von Arrays/Dictionaries (Schutz vor Stack-Überlauf).
pub(crate) const MAX_NESTING: usize = 128;

/// PDF-Leerraum (ISO 32000-1 Tabelle 1).
#[inline]
pub(crate) fn is_whitespace(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

/// PDF-Begrenzer (ISO 32000-1 Tabelle 2).
#[inline]
pub(crate) fn is_delimiter(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

#[inline]
fn is_regular(b: u8) -> bool {
    !is_whitespace(b) && !is_delimiter(b)
}

/// Token des Lexers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token<'a> {
    Int(i64),
    Real(f32),
    Name(Vec<u8>),
    LitString(Vec<u8>),
    HexString(Vec<u8>),
    DictOpen,
    DictClose,
    ArrayOpen,
    ArrayClose,
    /// Schlüsselwort bzw. Operator (`obj`, `R`, `true`, `cm`, …).
    Keyword(&'a [u8]),
    /// Einzelnes, sonst nicht zuordenbares Zeichen (`{`, `}`, `)` …).
    Other(u8),
}

/// Lexer über einem Byte-Puffer.
#[derive(Debug, Clone)]
pub(crate) struct Lexer<'a> {
    data: &'a [u8],
    /// Aktuelle Position.
    pub pos: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos: pos.min(data.len()) }
    }

    pub(crate) fn data(&self) -> &'a [u8] {
        self.data
    }

    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Überspringt Leerraum und Kommentare.
    pub(crate) fn skip_ws(&mut self) {
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if is_whitespace(b) {
                self.pos += 1;
            } else if b == b'%' {
                while self.pos < self.data.len() && !matches!(self.data[self.pos], b'\r' | b'\n') {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    /// Nächstes Token (oder `None` am Ende).
    pub(crate) fn next_token(&mut self) -> Option<Token<'a>> {
        self.skip_ws();
        let data = self.data;
        let b = *data.get(self.pos)?;
        match b {
            b'/' => {
                self.pos += 1;
                Some(Token::Name(self.read_name()))
            }
            b'(' => {
                self.pos += 1;
                Some(Token::LitString(self.read_literal_string()))
            }
            b'<' => {
                if data.get(self.pos + 1) == Some(&b'<') {
                    self.pos += 2;
                    Some(Token::DictOpen)
                } else {
                    self.pos += 1;
                    Some(Token::HexString(self.read_hex_string()))
                }
            }
            b'>' => {
                if data.get(self.pos + 1) == Some(&b'>') {
                    self.pos += 2;
                    Some(Token::DictClose)
                } else {
                    self.pos += 1;
                    Some(Token::Other(b'>'))
                }
            }
            b'[' => {
                self.pos += 1;
                Some(Token::ArrayOpen)
            }
            b']' => {
                self.pos += 1;
                Some(Token::ArrayClose)
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => Some(self.read_number()),
            _ if is_regular(b) => {
                let start = self.pos;
                while self.pos < data.len() && is_regular(data[self.pos]) {
                    self.pos += 1;
                }
                Some(Token::Keyword(&data[start..self.pos]))
            }
            _ => {
                self.pos += 1;
                Some(Token::Other(b))
            }
        }
    }

    /// Nächstes Token, ohne die Position zu verändern.
    pub(crate) fn peek_token(&mut self) -> Option<Token<'a>> {
        let save = self.pos;
        let t = self.next_token();
        self.pos = save;
        t
    }

    fn read_name(&mut self) -> Vec<u8> {
        let data = self.data;
        let mut out = Vec::new();
        while self.pos < data.len() && is_regular(data[self.pos]) {
            let b = data[self.pos];
            if b == b'#'
                && let (Some(h), Some(l)) =
                    (data.get(self.pos + 1).and_then(|&c| hex_val(c)), data.get(self.pos + 2).and_then(|&c| hex_val(c)))
            {
                out.push(h << 4 | l);
                self.pos += 3;
                continue;
            }
            out.push(b);
            self.pos += 1;
        }
        out
    }

    fn read_literal_string(&mut self) -> Vec<u8> {
        let data = self.data;
        let mut out = Vec::new();
        let mut depth = 1usize;
        while self.pos < data.len() {
            let b = data[self.pos];
            self.pos += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return out;
                    }
                    out.push(b);
                }
                b'\\' => {
                    let Some(&e) = data.get(self.pos) else { break };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'(' | b')' | b'\\' => out.push(e),
                        b'\r' => {
                            // Zeilenfortsetzung: \ + EOL wird ignoriert.
                            if data.get(self.pos) == Some(&b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match data.get(self.pos) {
                                    Some(&d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xFF) as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'\r' => {
                    // Unmaskiertes EOL im Literal-String gilt als LF (ISO 32000-1 §7.3.4.2).
                    if data.get(self.pos) == Some(&b'\n') {
                        self.pos += 1;
                    }
                    out.push(b'\n');
                }
                _ => out.push(b),
            }
        }
        out
    }

    fn read_hex_string(&mut self) -> Vec<u8> {
        let data = self.data;
        let mut out = Vec::new();
        let mut hi: Option<u8> = None;
        while self.pos < data.len() {
            let b = data[self.pos];
            self.pos += 1;
            if b == b'>' {
                break;
            }
            if let Some(v) = hex_val(b) {
                match hi.take() {
                    Some(h) => out.push(h << 4 | v),
                    None => hi = Some(v),
                }
            }
            // Leerraum und ungültige Zeichen werden ignoriert (lenient).
        }
        if let Some(h) = hi {
            out.push(h << 4);
        }
        out
    }

    fn read_number(&mut self) -> Token<'a> {
        let data = self.data;
        let start = self.pos;
        while self.pos < data.len() && matches!(data[self.pos], b'+' | b'-' | b'.' | b'0'..=b'9') {
            self.pos += 1;
        }
        // Ein Zahl-Präfix, an dem reguläre Zeichen kleben (z. B. `12abc`), ist ein Schlüsselwort.
        if self.pos < data.len() && is_regular(data[self.pos]) {
            while self.pos < data.len() && is_regular(data[self.pos]) {
                self.pos += 1;
            }
            return Token::Keyword(&data[start..self.pos]);
        }
        let text = &data[start..self.pos];
        parse_number(text)
    }

    /// Liest ein direktes Objekt. `allow_refs` erlaubt `n g R`.
    pub(crate) fn parse_object(&mut self, allow_refs: bool) -> Option<Object> {
        self.parse_object_depth(allow_refs, 0)
    }

    fn parse_object_depth(&mut self, allow_refs: bool, depth: usize) -> Option<Object> {
        if depth > MAX_NESTING {
            return None;
        }
        let tok = self.next_token()?;
        self.object_from_token(tok, allow_refs, depth)
    }

    /// Wandelt ein bereits gelesenes Token in ein Objekt um (liest bei Arrays/Dicts weiter).
    pub(crate) fn object_from_token(&mut self, tok: Token<'a>, allow_refs: bool, depth: usize) -> Option<Object> {
        if depth > MAX_NESTING {
            // Schutz vor Stack-Überlauf durch feindliche Verschachtelung (`[[[[…`).
            return None;
        }
        match tok {
            Token::Int(n) => {
                if allow_refs && n >= 0 && n <= i64::from(u32::MAX) {
                    let save = self.pos;
                    if let Some(Token::Int(g)) = self.next_token()
                        && (0..=i64::from(u16::MAX)).contains(&g)
                        && let Some(Token::Keyword(b"R")) = self.next_token()
                    {
                        return Some(Object::Reference((n as u32, g as u16)));
                    }
                    self.pos = save;
                }
                Some(Object::Integer(n))
            }
            Token::Real(r) => Some(Object::Real(r)),
            Token::Name(n) => Some(Object::Name(n)),
            Token::LitString(s) => Some(Object::String(s, StringFormat::Literal)),
            Token::HexString(s) => Some(Object::String(s, StringFormat::Hexadecimal)),
            Token::ArrayOpen => {
                let mut items = Vec::new();
                loop {
                    let save = self.pos;
                    match self.next_token() {
                        None => break,
                        Some(Token::ArrayClose) => break,
                        Some(Token::DictClose) | Some(Token::Keyword(b"endobj" | b"stream" | b"endstream")) => {
                            // Kaputtes Array: an der Stelle beenden, Token zurückgeben.
                            self.pos = save;
                            break;
                        }
                        Some(t) => match self.object_from_token(t, allow_refs, depth + 1) {
                            Some(o) => items.push(o),
                            None => {
                                if self.pos == save {
                                    break;
                                }
                            }
                        },
                    }
                }
                Some(Object::Array(items))
            }
            Token::DictOpen => {
                let mut dict = Dictionary::new();
                loop {
                    let save = self.pos;
                    match self.next_token() {
                        None => break,
                        Some(Token::DictClose) => break,
                        Some(Token::Name(key)) => {
                            let vsave = self.pos;
                            match self.next_token() {
                                None => {
                                    dict.set(key, Object::Null);
                                    break;
                                }
                                Some(Token::DictClose) => {
                                    // Schlüssel ohne Wert (lenient): null.
                                    dict.set(key, Object::Null);
                                    break;
                                }
                                Some(Token::Keyword(b"endobj" | b"stream" | b"endstream")) => {
                                    dict.set(key, Object::Null);
                                    self.pos = vsave;
                                    break;
                                }
                                Some(t) => {
                                    let v = self.object_from_token(t, allow_refs, depth + 1).unwrap_or(Object::Null);
                                    dict.set(key, v);
                                }
                            }
                        }
                        Some(Token::Keyword(b"endobj" | b"stream" | b"endstream")) => {
                            self.pos = save;
                            break;
                        }
                        Some(_) => {
                            // Unerwartetes Token an Schlüsselposition: überspringen.
                            if self.pos == save {
                                break;
                            }
                        }
                    }
                }
                Some(Object::Dictionary(dict))
            }
            Token::Keyword(b"true") => Some(Object::Boolean(true)),
            Token::Keyword(b"false") => Some(Object::Boolean(false)),
            Token::Keyword(b"null") => Some(Object::Null),
            _ => None,
        }
    }

    /// Erwartet ein bestimmtes Schlüsselwort.
    pub(crate) fn expect_keyword(&mut self, kw: &[u8]) -> bool {
        let save = self.pos;
        match self.next_token() {
            Some(Token::Keyword(k)) if k == kw => true,
            _ => {
                self.pos = save;
                false
            }
        }
    }
}

#[inline]
pub(crate) fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn parse_number(text: &[u8]) -> Token<'static> {
    // Leniente Zahlen wie bei pdf.js: mehrfache Vorzeichen werden toleriert, `-` allein ist 0.
    let mut neg = false;
    let mut i = 0;
    while i < text.len() && matches!(text[i], b'+' | b'-') {
        if text[i] == b'-' {
            neg = !neg;
        }
        i += 1;
    }
    let body = &text[i..];
    if body.contains(&b'.') {
        // Nur die erste Dezimalstelle zählt; weitere Punkte/Vorzeichen werden abgeschnitten.
        let mut end = 0;
        let mut seen_dot = false;
        while end < body.len() {
            match body[end] {
                b'0'..=b'9' => {}
                b'.' if !seen_dot => seen_dot = true,
                _ => break,
            }
            end += 1;
        }
        let s = std::str::from_utf8(&body[..end]).unwrap_or("0");
        let v: f64 = if s == "." || s.is_empty() { 0.0 } else { s.parse().unwrap_or(0.0) };
        let v = if neg { -v } else { v };
        Token::Real(v as f32)
    } else {
        let mut end = 0;
        while end < body.len() && body[end].is_ascii_digit() {
            end += 1;
        }
        let s = std::str::from_utf8(&body[..end]).unwrap_or("0");
        if s.is_empty() {
            return Token::Int(0);
        }
        match s.parse::<i64>() {
            Ok(v) => Token::Int(if neg { -v } else { v }),
            Err(_) => {
                let v: f64 = s.parse().unwrap_or(0.0);
                Token::Real((if neg { -v } else { v }) as f32)
            }
        }
    }
}

/// Ergebnis von [`parse_indirect_object`].
#[derive(Debug)]
pub(crate) struct IndirectObject {
    pub id: ObjectId,
    pub object: Object,
    /// Offset direkt hinter dem Objekt (nach `endobj`, falls vorhanden).
    pub end: usize,
}

/// Prüft, ob bei `pos` (nach optionalem Leerraum) ein Objektkopf `n g obj` steht; liefert die ID.
pub(crate) fn object_header_at(data: &[u8], pos: usize) -> Option<(ObjectId, usize)> {
    let mut lx = Lexer::new(data, pos);
    let Some(Token::Int(n)) = lx.next_token() else { return None };
    let Some(Token::Int(g)) = lx.next_token() else { return None };
    if !lx.expect_keyword(b"obj") {
        return None;
    }
    if !(0..=i64::from(u32::MAX)).contains(&n) || !(0..=i64::from(u16::MAX)).contains(&g) {
        return None;
    }
    Some(((n as u32, g as u16), lx.pos))
}

/// Liest ein indirektes Objekt an `offset`. `/Length` als Referenz wird über `resolve_len`
/// aufgelöst; passt die Länge nicht, wird `endstream` gesucht (lenient wie PDFBox).
pub(crate) fn parse_indirect_object(
    data: &[u8],
    offset: usize,
    resolve_len: &dyn Fn(ObjectId) -> Option<i64>,
) -> Option<IndirectObject> {
    let (id, after_header) = object_header_at(data, offset)?;
    let mut lx = Lexer::new(data, after_header);
    let object = match lx.parse_object(true) {
        Some(o) => o,
        None => {
            // `n g obj endobj` (leer) gilt als null.
            Object::Null
        }
    };
    let save = lx.pos;
    if let Object::Dictionary(dict) = &object
        && lx.expect_keyword(b"stream")
    {
        let mut p = lx.pos;
        // EOL nach `stream`: CRLF oder LF (lenient auch CR allein oder Leerzeichen davor).
        while p < data.len() && data[p] == b' ' {
            p += 1;
        }
        if data.get(p) == Some(&b'\r') {
            p += 1;
            if data.get(p) == Some(&b'\n') {
                p += 1;
            }
        } else if data.get(p) == Some(&b'\n') {
            p += 1;
        }
        let declared = match dict.get(b"Length") {
            Ok(Object::Integer(n)) => Some(*n),
            Ok(Object::Real(r)) if r.fract() == 0.0 => Some(*r as i64),
            Ok(Object::Reference(rid)) => resolve_len(*rid),
            _ => None,
        };
        let mut content_end = None;
        if let Some(n) = declared
            && n >= 0
            && let Some(end) = p.checked_add(n as usize)
            && end <= data.len()
        {
            let mut q = end;
            while q < data.len() && is_whitespace(data[q]) {
                q += 1;
            }
            if data[q.min(data.len())..].starts_with(b"endstream") {
                content_end = Some(end);
            }
        }
        let content_end = match content_end {
            Some(e) => e,
            None => {
                // `endstream` suchen und ein abschließendes EOL abschneiden.
                let found = find(data, b"endstream", p)?;
                let mut e = found;
                if e > p && data[e - 1] == b'\n' {
                    e -= 1;
                }
                if e > p && data[e - 1] == b'\r' {
                    e -= 1;
                }
                e
            }
        };
        let content = data[p..content_end].to_vec();
        let mut lx2 = Lexer::new(data, content_end);
        lx2.skip_ws();
        let mut end = lx2.pos;
        if lx2.expect_keyword(b"endstream") {
            end = lx2.pos;
            if lx2.expect_keyword(b"endobj") {
                end = lx2.pos;
            }
        }
        let mut stream = Stream::new(dict.clone(), content);
        stream.start_position = Some(p);
        return Some(IndirectObject { id, object: Object::Stream(stream), end });
    }
    lx.pos = save;
    let mut end = lx.pos;
    if lx.expect_keyword(b"endobj") {
        end = lx.pos;
    }
    Some(IndirectObject { id, object, end })
}

/// Sammelt die Originalschreibweise aller Reals des Objekts ab `pos` mit ihrem Pfad
/// (Dictionary-Schlüssel bzw. Array-Index; Struktur wie [`Lexer::parse_object`]).
pub(crate) fn collect_real_literals(
    data: &[u8],
    pos: usize,
    out: &mut std::collections::HashMap<Vec<crate::PathSegment>, (f32, String)>,
) {
    let mut lx = Lexer::new(data, pos);
    let mut path = Vec::new();
    walk_literals(&mut lx, &mut path, out, 0);
}

fn walk_literals(
    lx: &mut Lexer<'_>,
    path: &mut Vec<crate::PathSegment>,
    out: &mut std::collections::HashMap<Vec<crate::PathSegment>, (f32, String)>,
    depth: usize,
) {
    use crate::PathSegment;
    if depth > MAX_NESTING {
        return;
    }
    lx.skip_ws();
    let start = lx.pos;
    let Some(tok) = lx.next_token() else { return };
    match tok {
        Token::Real(v) => {
            let text = String::from_utf8_lossy(&lx.data[start..lx.pos]).into_owned();
            out.insert(path.clone(), (v, text));
        }
        Token::Int(n) => {
            // Referenz `n g R` wie im Parser als ein Wert behandeln.
            if n >= 0 {
                let save = lx.pos;
                if let Some(Token::Int(_)) = lx.next_token()
                    && let Some(Token::Keyword(b"R")) = lx.next_token()
                {
                    return;
                }
                lx.pos = save;
            }
        }
        Token::ArrayOpen => {
            let mut i = 0;
            loop {
                match lx.peek_token() {
                    None => break,
                    Some(Token::ArrayClose) => {
                        lx.next_token();
                        break;
                    }
                    Some(Token::DictClose) | Some(Token::Keyword(b"endobj" | b"stream" | b"endstream")) => break,
                    Some(_) => {
                        let before = lx.pos;
                        path.push(PathSegment::Index(i));
                        walk_literals(lx, path, out, depth + 1);
                        path.pop();
                        if lx.pos == before {
                            break;
                        }
                        i += 1;
                    }
                }
            }
        }
        Token::DictOpen => loop {
            let save = lx.pos;
            match lx.next_token() {
                None | Some(Token::DictClose) => break,
                Some(Token::Name(key)) => {
                    if matches!(lx.peek_token(), None | Some(Token::DictClose)) {
                        continue;
                    }
                    path.push(PathSegment::Key(key));
                    walk_literals(lx, path, out, depth + 1);
                    path.pop();
                }
                Some(Token::Keyword(b"endobj" | b"stream" | b"endstream")) => {
                    lx.pos = save;
                    break;
                }
                Some(_) => {
                    if lx.pos == save {
                        break;
                    }
                }
            }
        },
        _ => {}
    }
}

/// Erstes Vorkommen von `needle` ab `from`.
pub(crate) fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= hay.len() {
        return None;
    }
    let first = needle[0];
    let mut i = from;
    while i + needle.len() <= hay.len() {
        let p = hay[i..hay.len() - needle.len() + 1].iter().position(|&b| b == first)?;
        let at = i + p;
        if &hay[at..at + needle.len()] == needle {
            return Some(at);
        }
        i = at + 1;
    }
    None
}

/// Letztes Vorkommen von `needle` vor `before` (exklusiv).
pub(crate) fn rfind(hay: &[u8], needle: &[u8], before: usize) -> Option<usize> {
    let end = before.min(hay.len());
    if needle.is_empty() || end < needle.len() {
        return None;
    }
    hay[..end].windows(needle.len()).rposition(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(s: &str) -> Object {
        Lexer::new(s.as_bytes(), 0).parse_object(true).unwrap()
    }

    #[test]
    fn primitives() {
        assert_eq!(obj("42"), Object::Integer(42));
        assert_eq!(obj("-17"), Object::Integer(-17));
        assert_eq!(obj("+3"), Object::Integer(3));
        assert_eq!(obj("3.5"), Object::Real(3.5));
        assert_eq!(obj("-.5"), Object::Real(-0.5));
        assert_eq!(obj("--2"), Object::Integer(2));
        assert_eq!(obj("true"), Object::Boolean(true));
        assert_eq!(obj("null"), Object::Null);
        assert_eq!(obj("/A#20B"), Object::Name(b"A B".to_vec()));
        assert_eq!(obj("12 0 R"), Object::Reference((12, 0)));
        assert_eq!(obj("12 0"), Object::Integer(12));
    }

    #[test]
    fn strings() {
        assert_eq!(obj("(a\\(b\\)c)"), Object::String(b"a(b)c".to_vec(), StringFormat::Literal));
        assert_eq!(obj("(x(y)z)"), Object::String(b"x(y)z".to_vec(), StringFormat::Literal));
        assert_eq!(obj("(\\101\\7)"), Object::String(vec![b'A', 7], StringFormat::Literal));
        assert_eq!(obj("(a\\\r\nb)"), Object::String(b"ab".to_vec(), StringFormat::Literal));
        assert_eq!(obj("(a\r\nb)"), Object::String(b"a\nb".to_vec(), StringFormat::Literal));
        assert_eq!(obj("<48 65 6c6C 6>"), Object::String(b"Hell`".to_vec(), StringFormat::Hexadecimal));
    }

    #[test]
    fn containers() {
        let o = obj("<< /Type /Page /Kids [1 0 R 2 0 R] /N << /A 1 /B [ ] >> /Empty >>");
        let d = o.as_dict().unwrap();
        assert_eq!(d.get(b"Type").unwrap(), &Object::Name(b"Page".to_vec()));
        assert_eq!(d.get(b"Kids").unwrap().as_array().unwrap().len(), 2);
        assert!(d.get(b"N").unwrap().as_dict().unwrap().get(b"B").unwrap().as_array().unwrap().is_empty());
        assert_eq!(d.get(b"Empty").unwrap(), &Object::Null);
        // Kommentar und kaputte Struktur
        let o = obj("[1 % Kommentar\n 2 ]");
        assert_eq!(o.as_array().unwrap().len(), 2);
        let o = obj("<< /A [1 2 >>");
        assert_eq!(o.as_dict().unwrap().get(b"A").unwrap().as_array().unwrap().len(), 2);
    }

    #[test]
    fn nesting_limit() {
        // Darf weder den Stack sprengen noch paniken; Ergebnis beliebig.
        for deep in ["[".repeat(200_000), "<<".repeat(200_000), "<< /A [".repeat(100_000)] {
            let _ = Lexer::new(deep.as_bytes(), 0).parse_object(true);
            let mut lits = std::collections::HashMap::new();
            collect_real_literals(deep.as_bytes(), 0, &mut lits);
        }
        // Innerhalb der Grenze bleibt die Struktur erhalten.
        let ok = format!("{}1{}", "[".repeat(MAX_NESTING - 1), "]".repeat(MAX_NESTING - 1));
        let mut o = Lexer::new(ok.as_bytes(), 0).parse_object(true).unwrap();
        let mut depth = 0;
        while let Object::Array(mut a) = o {
            depth += 1;
            o = a.pop().unwrap();
        }
        assert_eq!((depth, o), (MAX_NESTING - 1, Object::Integer(1)));
    }

    #[test]
    fn indirect_with_stream() {
        let data = b"xx 7 0 obj\n<< /Length 5 >>\nstream\r\nHELLO\r\nendstream\nendobj\n";
        let io = parse_indirect_object(data, 3, &|_| None).unwrap();
        assert_eq!(io.id, (7, 0));
        let s = io.object.as_stream().unwrap();
        assert_eq!(s.content, b"HELLO");
        assert_eq!(&data[io.end - 6..io.end], b"endobj");
        // falsche Länge → endstream-Suche
        let data = b"7 0 obj << /Length 99 >> stream\nHELLO\nendstream endobj";
        let io = parse_indirect_object(data, 0, &|_| None).unwrap();
        assert_eq!(io.object.as_stream().unwrap().content, b"HELLO");
        // indirekte Länge
        let data = b"7 0 obj << /Length 8 0 R >> stream\nHELLO\nendstream endobj";
        let io = parse_indirect_object(data, 0, &|id| (id == (8, 0)).then_some(5)).unwrap();
        assert_eq!(io.object.as_stream().unwrap().content, b"HELLO");
    }

    #[test]
    fn find_helpers() {
        assert_eq!(find(b"abcabc", b"bc", 0), Some(1));
        assert_eq!(find(b"abcabc", b"bc", 2), Some(4));
        assert_eq!(find(b"abc", b"x", 0), None);
        assert_eq!(rfind(b"abcabc", b"bc", 6), Some(4));
        assert_eq!(rfind(b"abcabc", b"bc", 4), Some(1));
    }
}

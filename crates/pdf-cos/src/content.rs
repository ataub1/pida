//! Content-Stream-Auswertung für Bild-`Do`-Aufrufe mit CTM (docs/analyse/04 §5.1, ADR 0001 §8.2).
//!
//! Wie PDF-AS (`SignaturePlaceholderExtractor`): nur `q`, `Q`, `cm` und `Do`; Inline-Bilder
//! (`BI … ID … EI`) werden übersprungen, Form-XObjects nur mit `descend_forms` betreten.
//! Die CTM beginnt mit der Einheitsmatrix im Seitenraum (wie PDFBox/PDFium).

use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::lexer::{self, Lexer, Token};
use crate::pages::{number, resolve};
use crate::{ImageInvocation, Limits, Matrix, Result};

/// Einheitsmatrix.
pub const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Matrixprodukt `m1 × m2` in PDF-Konvention (Zeilenvektoren, ISO 32000-1 §8.3.4).
/// `cm` setzt `CTM' = M × CTM`.
pub fn multiply(m1: &Matrix, m2: &Matrix) -> Matrix {
    [
        m1[0] * m2[0] + m1[1] * m2[2],
        m1[0] * m2[1] + m1[1] * m2[3],
        m1[2] * m2[0] + m1[3] * m2[2],
        m1[2] * m2[1] + m1[3] * m2[3],
        m1[4] * m2[0] + m1[5] * m2[2] + m2[4],
        m1[4] * m2[1] + m1[5] * m2[3] + m2[5],
    ]
}

/// Obergrenze für den `q`-Stapel (weitere `q` werden gezählt, aber nicht gespeichert).
const MAX_GSTATE_STACK: usize = 4096;

pub(crate) struct Interpreter<'a> {
    pub doc: &'a Document,
    pub limits: &'a Limits,
    pub page_index: usize,
    pub descend_forms: bool,
    pub out: Vec<ImageInvocation>,
    pub form_stack: HashSet<ObjectId>,
    pub decode: &'a dyn Fn(&lopdf::Stream) -> Result<Vec<u8>>,
}

fn matrix_from(doc: &Document, obj: Option<&Object>) -> Option<Matrix> {
    let arr = resolve(doc, obj?, 8)?.as_array().ok()?;
    if arr.len() != 6 {
        return None;
    }
    let mut m = [0f32; 6];
    for (i, o) in arr.iter().enumerate() {
        m[i] = number(doc, o)?;
    }
    Some(m)
}

fn dims(doc: &Document, d: &Dictionary, key: &[u8]) -> u32 {
    d.get(key).ok().and_then(|o| number(doc, o)).map(|v| v.max(0.0) as u32).unwrap_or(0)
}

impl Interpreter<'_> {
    /// Interpretiert einen (bereits dekodierten) Content-Stream.
    pub(crate) fn run(
        &mut self,
        content: &[u8],
        resources: Option<&Dictionary>,
        ctm0: Matrix,
        inside_form: bool,
        depth: u32,
    ) -> Result<()> {
        let mut lx = Lexer::new(content, 0);
        let mut ctm = ctm0;
        let mut stack: Vec<Matrix> = Vec::new();
        let mut overflow = 0usize;
        let mut operands: Vec<Object> = Vec::new();
        while let Some(tok) = lx.next_token() {
            match tok {
                Token::Keyword(op) => {
                    match op {
                        b"q" => {
                            if stack.len() < MAX_GSTATE_STACK {
                                stack.push(ctm);
                            } else {
                                overflow += 1;
                            }
                        }
                        b"Q" => {
                            if overflow > 0 {
                                overflow -= 1;
                            } else if let Some(m) = stack.pop() {
                                ctm = m;
                            }
                        }
                        b"cm" => {
                            if operands.len() >= 6 {
                                let nums: Option<Vec<f32>> = operands[operands.len() - 6..]
                                    .iter()
                                    .map(|o| match o {
                                        Object::Integer(i) => Some(*i as f32),
                                        Object::Real(r) => Some(*r),
                                        _ => None,
                                    })
                                    .collect();
                                if let Some(n) = nums {
                                    let m = [n[0], n[1], n[2], n[3], n[4], n[5]];
                                    ctm = multiply(&m, &ctm);
                                }
                            }
                        }
                        b"Do" => {
                            if let Some(Object::Name(name)) = operands.last() {
                                let name = name.clone();
                                self.invoke(&name, resources, ctm, inside_form, depth)?;
                            }
                        }
                        b"BI" => skip_inline_image(&mut lx),
                        _ => {}
                    }
                    operands.clear();
                }
                Token::ArrayClose | Token::DictClose | Token::Other(_) => operands.clear(),
                t => {
                    if let Some(o) = lx.object_from_token(t, false, 0) {
                        if operands.len() < 64 {
                            operands.push(o);
                        }
                    } else {
                        operands.clear();
                    }
                }
            }
        }
        Ok(())
    }

    fn invoke(
        &mut self,
        name: &[u8],
        resources: Option<&Dictionary>,
        ctm: Matrix,
        inside_form: bool,
        depth: u32,
    ) -> Result<()> {
        let doc = self.doc;
        let Some(res) = resources else { return Ok(()) };
        let Some(Object::Dictionary(xobjects)) = res.get(b"XObject").ok().and_then(|o| resolve(doc, o, 8)) else {
            return Ok(());
        };
        let Ok(entry) = xobjects.get(name) else { return Ok(()) };
        let xobject_id = match entry {
            Object::Reference(id) => Some(*id),
            _ => None,
        };
        let Some(Object::Stream(stream)) = resolve(doc, entry, 8) else { return Ok(()) };
        let subtype = stream.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok());
        match subtype {
            Some(b"Image") => {
                let order = self.out.len();
                self.out.push(ImageInvocation {
                    page_index: self.page_index,
                    order,
                    resource_name: String::from_utf8_lossy(name).into_owned(),
                    xobject_id,
                    ctm,
                    pixel_width: dims(doc, &stream.dict, b"Width"),
                    pixel_height: dims(doc, &stream.dict, b"Height"),
                    inside_form_xobject: inside_form,
                });
            }
            Some(b"Form") if self.descend_forms => {
                if depth >= self.limits.max_depth {
                    return Ok(()); // tiefe Verschachtelung: nicht weiter absteigen
                }
                if let Some(id) = xobject_id
                    && !self.form_stack.insert(id)
                {
                    return Ok(()); // Zyklus
                }
                let fm = matrix_from(doc, stream.dict.get(b"Matrix").ok()).unwrap_or(IDENTITY);
                let form_ctm = multiply(&fm, &ctm);
                let form_res = match stream.dict.get(b"Resources").ok().and_then(|o| resolve(doc, o, 8)) {
                    Some(Object::Dictionary(d)) => Some(d),
                    _ => resources,
                };
                let content = (self.decode)(stream);
                let result = match content {
                    Ok(c) => self.run(&c, form_res, form_ctm, true, depth + 1),
                    Err(_) => Ok(()), // nicht dekodierbare Form überspringen
                };
                if let Some(id) = xobject_id {
                    self.form_stack.remove(&id);
                }
                result?;
            }
            _ => {}
        }
        Ok(())
    }
}

/// Überspringt ein Inline-Bild bis hinter `EI`.
fn skip_inline_image(lx: &mut Lexer<'_>) {
    // Bild-Dictionary bis `ID`
    loop {
        match lx.next_token() {
            None => return,
            Some(Token::Keyword(b"ID")) => break,
            Some(_) => {}
        }
    }
    let data = lx.data();
    // Genau ein Leerraumzeichen nach ID, dann Binärdaten bis `<ws>EI<ws|EOF>`.
    let mut p = lx.pos + 1;
    while p + 2 <= data.len() {
        match lexer::find(data, b"EI", p) {
            None => {
                lx.pos = data.len();
                return;
            }
            Some(at) => {
                let before_ok = at > 0 && lexer::is_whitespace(data[at - 1]);
                let after_ok =
                    at + 2 >= data.len() || lexer::is_whitespace(data[at + 2]) || lexer::is_delimiter(data[at + 2]);
                if before_ok && after_ok {
                    lx.pos = at + 2;
                    return;
                }
                p = at + 1;
            }
        }
    }
    lx.pos = data.len();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_product() {
        let t = [1.0, 0.0, 0.0, 1.0, 10.0, 20.0];
        let s = [2.0, 0.0, 0.0, 3.0, 0.0, 0.0];
        // erst skalieren (innen), dann verschieben (außen): cm s nach cm t → s × t
        assert_eq!(multiply(&s, &t), [2.0, 0.0, 0.0, 3.0, 10.0, 20.0]);
        assert_eq!(multiply(&t, &s), [2.0, 0.0, 0.0, 3.0, 20.0, 60.0]);
        assert_eq!(multiply(&IDENTITY, &t), t);
    }

    #[test]
    fn inline_image_skip() {
        let data = b"BI /W 2 /H 2 /BPC 8 /CS /G ID \x01EIx\x02 EI q Q";
        let mut lx = Lexer::new(data, 0);
        assert_eq!(lx.next_token(), Some(Token::Keyword(b"BI")));
        skip_inline_image(&mut lx);
        assert_eq!(lx.next_token(), Some(Token::Keyword(b"q")));
    }
}

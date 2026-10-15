//! Synthetische Randfälle für `pida-pdf-cos` (Plan §6.1 K1/K2/K5, §7.7; analyse/03 §4.5,
//! analyse/04 §4.6/§5.1). Die PDFs werden im Test byteweise gebaut.

use std::collections::BTreeMap;
use std::io::Write as _;

use pida_pdf_cos::{CosError, Limits, Object, PathSegment, Rect, Rotation, SourcePdf, TailKind, XrefHealth, XrefKind};

/// Minimaler PDF-Bauer: Objekte, klassische xref-Abschnitte, xref-Streams, Increments.
struct Pdf {
    buf: Vec<u8>,
    /// Objekte des aktuellen Abschnitts
    pending: BTreeMap<u32, usize>,
    last_xref: Option<usize>,
}

impl Pdf {
    fn new(version: &str) -> Self {
        let mut buf = format!("%PDF-{version}\n").into_bytes();
        buf.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");
        Self { buf, pending: BTreeMap::new(), last_xref: None }
    }

    fn with_prefix(prefix: &[u8], version: &str) -> Self {
        let mut p = Self::new(version);
        let mut buf = prefix.to_vec();
        buf.extend_from_slice(&p.buf);
        p.buf = buf;
        p
    }

    fn obj(&mut self, num: u32, body: &str) -> &mut Self {
        self.obj_bytes(num, body.as_bytes())
    }

    fn obj_bytes(&mut self, num: u32, body: &[u8]) -> &mut Self {
        self.pending.insert(num, self.buf.len());
        writeln!(self.buf, "{num} 0 obj").unwrap();
        self.buf.extend_from_slice(body);
        self.buf.extend_from_slice(b"\nendobj\n");
        self
    }

    fn stream(&mut self, num: u32, extra: &str, data: &[u8]) -> &mut Self {
        let mut body = format!("<< /Length {} {extra} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.obj_bytes(num, &body)
    }

    /// Klassische xref-Tabelle der seit dem letzten Abschnitt geschriebenen Objekte.
    fn table(&mut self, trailer: &str, offset_shift: i64) -> &mut Self {
        let at = self.buf.len();
        let mut entries: Vec<(u32, usize)> = std::mem::take(&mut self.pending).into_iter().collect();
        if self.last_xref.is_none() {
            entries.insert(0, (0, usize::MAX));
        }
        self.buf.extend_from_slice(b"xref\n");
        let mut i = 0;
        while i < entries.len() {
            let start = entries[i].0;
            let mut j = i;
            while j + 1 < entries.len() && entries[j + 1].0 == entries[j].0 + 1 {
                j += 1;
            }
            writeln!(self.buf, "{} {}", start, j - i + 1).unwrap();
            for (_, off) in &entries[i..=j] {
                if *off == usize::MAX {
                    self.buf.extend_from_slice(b"0000000000 65535 f\r\n");
                } else {
                    write!(self.buf, "{:010} 00000 n\r\n", (*off as i64 + offset_shift).max(0)).unwrap();
                }
            }
            i = j + 1;
        }
        let prev = self.last_xref.map(|p| format!(" /Prev {p}")).unwrap_or_default();
        write!(self.buf, "trailer\n<< {trailer}{prev} >>\nstartxref\n{at}\n%%EOF\n").unwrap();
        self.last_xref = Some(at);
        self
    }

    fn bytes(&self) -> Vec<u8> {
        self.buf.clone()
    }
}

fn rfind(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).rposition(|w| w == needle).unwrap()
}

fn open(bytes: Vec<u8>) -> SourcePdf {
    SourcePdf::open(bytes, &Limits::default()).unwrap_or_else(|e| panic!("öffnen: {e}"))
}

/// Einseitiges Grunddokument (Katalog 1, Pages 2, Seite 3, Inhalt 4).
fn simple() -> Pdf {
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << >> >>")
        .stream(4, "", b"BT /F1 12 Tf 72 720 Td (NUR TEST) Tj ET");
    p
}

#[test]
fn inherited_page_attributes_like_pdfbox() {
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 4 /MediaBox [0 0 500 700] /CropBox [10 10 600 800] /Rotate 90 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] >>")
        .obj(5, "<< /Type /Pages /Parent 2 0 R /Kids [6 0 R 7 0 R 8 0 R] /Count 3 /Rotate -90 >>")
        .obj(6, "<< /Type /Page /Parent 5 0 R /Rotate 45 /UserUnit 2.5 >>")
        .obj(7, "<< /Type /Page /Parent 5 0 R /Annots 9 0 R >>")
        .obj(8, "<< /Type /Page /Parent 5 0 R /CropBox [300 400 100 200] /Rotate 180.0 >>")
        .obj(9, "[]")
        .table("/Size 10 /Root 1 0 R", 0);
    let s = open(p.bytes());
    let pg = s.pages();
    assert_eq!(pg.len(), 4);
    assert_eq!(pg.iter().map(|p| p.object_id.0).collect::<Vec<_>>(), [3, 6, 7, 8]);
    // Seite 3: eigene MediaBox, geerbte CropBox auf MediaBox beschnitten, geerbte Rotation
    assert_eq!(pg[0].media_box, Rect::new(0.0, 0.0, 200.0, 300.0));
    assert_eq!(pg[0].crop_box, Rect::new(10.0, 10.0, 200.0, 300.0));
    assert_eq!(pg[0].rotation, Rotation::R90);
    // Seite 6: Rotate 45 → 0 (wie PDFBox), UserUnit
    assert_eq!(pg[1].rotation, Rotation::R0);
    assert_eq!(pg[1].user_unit, 2.5);
    assert_eq!(pg[1].media_box, Rect::new(0.0, 0.0, 500.0, 700.0));
    // Seite 7: -90 aus dem Zwischenknoten → 270; /Annots indirekt
    assert_eq!(pg[2].rotation, Rotation::R270);
    assert_eq!(pg[2].annots_indirect, Some((9, 0)));
    assert_eq!(pg[2].visible_size(), (690.0, 490.0));
    // Seite 8: CropBox normalisiert, Rotate als Real
    assert_eq!(pg[3].crop_box, Rect::new(100.0, 200.0, 300.0, 400.0));
    assert_eq!(pg[3].rotation, Rotation::R180);

    // Ohne MediaBox: US-Letter (PDFBox `PDRectangle.LETTER`)
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R >>")
        .table("/Size 4 /Root 1 0 R", 0);
    let s = open(p.bytes());
    assert_eq!(s.pages()[0].media_box, Rect::new(0.0, 0.0, 612.0, 792.0));
    assert_eq!(s.pages()[0].crop_box, s.pages()[0].media_box);
}

#[test]
fn page_tree_cycles_depth_and_empty_documents() {
    // Zyklus: Kind verweist auf die Wurzel
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R >>")
        .obj(4, "<< /Type /Pages /Parent 2 0 R /Kids [2 0 R 3 0 R] /Count 1 >>")
        .table("/Size 5 /Root 1 0 R", 0);
    let s = open(p.bytes());
    assert_eq!(s.page_count(), 1, "doppelte Seite und Zyklus werden ignoriert");

    // Tiefe: Kette von 10 Pages-Knoten bei max_depth 5
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 10 0 R >>");
    for n in 10..20 {
        p.obj(n, &format!("<< /Type /Pages /Kids [{} 0 R] /Count 1 >>", n + 1));
    }
    p.obj(20, "<< /Type /Page >>").table("/Size 21 /Root 1 0 R", 0);
    let l = Limits { max_depth: 5, ..Limits::default() };
    assert!(matches!(SourcePdf::open(p.bytes(), &l), Err(CosError::LimitExceeded { .. })));
    assert_eq!(open(p.bytes()).page_count(), 1);

    // 0 Seiten → NoPages (PDFBox „Cannot sign an empty document“)
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [] /Count 0 >>")
        .table("/Size 3 /Root 1 0 R", 0);
    assert!(matches!(SourcePdf::open(p.bytes(), &Limits::default()), Err(CosError::NoPages)));
    // Kein /Root
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>").table("/Size 2", 0);
    assert!(SourcePdf::open(p.bytes(), &Limits::default()).is_err());
}

#[test]
fn tail_after_eof() {
    let base = {
        let mut p = simple();
        p.table("/Size 5 /Root 1 0 R", 0);
        p.bytes()
    };
    let s = open(base.clone());
    assert_eq!(s.tail().kind, TailKind::Clean);

    let mut ws = base.clone();
    ws.extend_from_slice(b"\r\n\n  \0\0\t\x0c");
    let s = open(ws.clone());
    assert_eq!(s.tail().kind, TailKind::Whitespace);
    assert_eq!(s.tail().eof_offset, base.len() as u64);
    assert_eq!(s.tail().len, 9);

    let mut garbage = base.clone();
    garbage.extend_from_slice(b"Fremdinhalt nach dem Dateiende");
    let s = open(garbage);
    assert_eq!(s.tail().kind, TailKind::Garbage);
    assert_eq!(s.xref_health(), XrefHealth::Intact);

    // Mehr als 512 Byte Leerraum: lopdf allein fände startxref nicht mehr.
    let mut long = base.clone();
    long.extend(std::iter::repeat_n(b' ', 4000));
    let s = open(long);
    assert_eq!(s.tail().kind, TailKind::Whitespace);
    assert_eq!(s.xref_health(), XrefHealth::Intact);
    assert_eq!(s.page_count(), 1);

    // Fehlendes EOL nach %%EOF
    let mut no_eol = base.clone();
    no_eol.pop();
    let s = open(no_eol.clone());
    assert_eq!(s.tail().kind, TailKind::Clean);
    assert_eq!(s.last_revision().end_offset, no_eol.len() as u64);
}

#[test]
fn damaged_cross_references_are_repaired_or_reconstructed() {
    // startxref um wenige Bytes daneben → Fensterkorrektur
    let mut p = simple();
    p.table("/Size 5 /Root 1 0 R", 0);
    let good = p.bytes();
    let sx_pos = rfind(&good, b"startxref\n") + 10;
    let sx_end = sx_pos + good[sx_pos..].iter().position(|&b| b == b'\n').unwrap();
    let sx: usize = std::str::from_utf8(&good[sx_pos..sx_end]).unwrap().parse().unwrap();
    let mut shifted = good[..sx_pos].to_vec();
    shifted.extend_from_slice(format!("{}", sx + 3).as_bytes());
    shifted.extend_from_slice(&good[sx_end..]);
    let s = open(shifted);
    assert_eq!(s.xref_health(), XrefHealth::Repaired);
    assert_eq!(s.page_count(), 1);
    assert_eq!(s.last_revision().startxref, sx as u64 + 3, "deklarierter Wert");
    assert_eq!(s.prev_offset(), sx as u64, "korrigierter Offset für /Prev");

    // Falsche Objekt-Offsets in der Tabelle → Objekte über Kopfsuche gefunden
    let mut p = simple();
    p.table("/Size 5 /Root 1 0 R", 7);
    let s = open(p.bytes());
    assert_eq!(s.xref_health(), XrefHealth::Repaired);
    assert_eq!(s.page_count(), 1);
    assert!(s.get_object((4, 0)).unwrap().as_stream().is_ok());

    // Kein startxref/%%EOF → rekonstruiert (K2), synthetische Revision
    let mut p = simple();
    p.table("/Size 5 /Root 1 0 R", 0);
    let mut b = p.bytes();
    let cut = rfind(&b, b"startxref");
    b.truncate(cut);
    let s = open(b);
    assert_eq!(s.xref_health(), XrefHealth::Reconstructed);
    assert_eq!(s.revisions().len(), 1);
    assert_eq!(s.tail().kind, TailKind::Garbage);
    assert_eq!(s.page_count(), 1);
}

#[test]
fn leading_junk_before_header() {
    // Offsets relativ zum Header (wie es lopdf/pdf.js erwarten)
    let prefix = b"MIME-Muell vor dem Header\r\n";
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R >>")
        .table("/Size 4 /Root 1 0 R", 0);
    let mut b = prefix.to_vec();
    b.extend_from_slice(&p.bytes());
    let s = open(b);
    assert_eq!(s.xref_offset_base(), prefix.len() as u64);
    assert_eq!(s.page_count(), 1);
    assert_eq!(s.pdf_version(), "1.4");
    // Absolute Offsets trotz Vorspann
    let mut p = Pdf::with_prefix(prefix, "1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R >>")
        .table("/Size 4 /Root 1 0 R", 0);
    let s = open(p.bytes());
    assert_eq!(s.xref_offset_base(), 0);
    assert_eq!(s.page_count(), 1);
}

#[test]
fn incremental_revisions_and_prev_chain() {
    let mut p = simple();
    p.table("/Size 5 /Root 1 0 R /ID [<00112233445566778899AABBCCDDEEFF> <00112233445566778899AABBCCDDEEFF>]", 0);
    let first_end = p.buf.len();
    p.obj(3, "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Rotate 90 >>").table("/Size 5 /Root 1 0 R", 0);
    p.obj(5, "<< /Producer (NUR TEST) >>").table("/Size 6 /Root 1 0 R /Info 5 0 R", 0);
    let s = open(p.bytes());
    let r = s.revisions();
    assert_eq!(r.len(), 3);
    assert_eq!(r[0].end_offset, first_end as u64);
    assert_eq!(r[1].prev, Some(r[0].startxref));
    assert_eq!(r[2].prev, Some(r[1].startxref));
    assert_eq!(r[2].size, 6);
    assert!(r.iter().all(|x| x.xref_kind == XrefKind::Table));
    assert_eq!(s.pages()[0].rotation, Rotation::R90, "neueste Version der Seite");
    assert_eq!(s.info_id(), Some((5, 0)));
    // /ID aus älterem Trailer ergänzt
    assert_eq!(s.trailer_id().unwrap()[0].len(), 16);
    assert_eq!(s.next_object_number(), 6);
}

/// lopdf 0.45 wertet `/XRefStm` älterer Revisionen nicht aus; pdf-cos lädt die Objekte nach.
#[test]
fn hybrid_objects_in_older_revision_are_found() {
    let mut p = simple();
    // Object Stream 6 mit Font 5 – beide nur im xref-Stream 7
    let font = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>";
    let header = b"5 0 ";
    let mut objstm = header.to_vec();
    objstm.extend_from_slice(font);
    let objstm_off = p.buf.len();
    p.stream(6, &format!("/Type /ObjStm /N 1 /First {}", header.len()), &objstm);
    p.pending.remove(&6);
    // xref-Stream (unkomprimiert, W [1 2 1]): Obj 5 → Typ 2 in 6, Obj 6 → Typ 1
    let mut rows = vec![2u8, 0, 6, 0];
    rows.extend_from_slice(&[1, (objstm_off >> 8) as u8, (objstm_off & 0xFF) as u8, 0]);
    let xs_off = p.buf.len();
    p.stream(7, "/Type /XRef /Size 8 /W [1 2 1] /Index [5 2]", &rows);
    let xs_entry = p.pending.remove(&7).unwrap();
    p.pending.insert(7, xs_entry);
    p.table(&format!("/Size 8 /Root 1 0 R /XRefStm {xs_off}"), 0);
    let s = open(p.bytes());
    assert_eq!(s.xref_kind(), XrefKind::Hybrid);
    assert!(s.get_object((5, 0)).unwrap().as_dict().unwrap().has_type(b"Font"));
    // Increment (Tabelle) darüber: Objekt 5 bleibt erreichbar
    p.obj(3, "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>")
        .table("/Size 8 /Root 1 0 R", 0);
    let s = open(p.bytes());
    assert_eq!(s.revisions().len(), 2);
    assert_eq!(s.xref_kind(), XrefKind::Table);
    assert!(
        s.get_object((5, 0)).unwrap().as_dict().unwrap().has_type(b"Font"),
        "Font aus dem /XRefStm der Vorrevision"
    );
    assert!(s.get_object((6, 0)).unwrap().as_stream().is_ok());
    assert_eq!(s.xref_health(), XrefHealth::Intact);
}

#[test]
fn unsupported_security_handlers_are_rejected() {
    for (filter, expected) in [("/Adobe.PubSec", "Adobe.PubSec"), ("/FooBar", "FooBar")] {
        let mut p = simple();
        p.obj(9, &format!("<< /Filter {filter} /V 4 /R 4 /Length 128 >>"));
        p.table("/Size 10 /Root 1 0 R /Encrypt 9 0 R /ID [<00> <00>]", 0);
        match SourcePdf::open(p.bytes(), &Limits::default()) {
            Err(CosError::UnsupportedSecurityHandler(h)) => assert_eq!(h, expected),
            other => panic!("{filter}: {other:?}"),
        }
    }
    // /Encrypt verweist ins Leere
    let mut p = simple();
    p.table("/Size 5 /Root 1 0 R /Encrypt 99 0 R", 0);
    assert!(matches!(SourcePdf::open(p.bytes(), &Limits::default()), Err(CosError::Decryption(_))));
}

#[test]
fn signature_field_tree() {
    let mut p = Pdf::new("1.7");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [10 0 R 15 0 R 16 0 R] /SigFlags 1 >> >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R /Annots [11 0 R 13 0 R] >>")
        .obj(4, "<< /Type /Page /Parent 2 0 R /Annots [14 0 R 16 0 R] >>")
        .obj(10, "<< /T (form) /FT /Sig /Ff 4 /Kids [11 0 R 12 0 R] >>")
        .obj(11, "<< /T (sig1) /Parent 10 0 R /Type /Annot /Subtype /Widget /Rect [10 20 110 70] /P 3 0 R /F 132 /Lock << /Action /All >> >>")
        .obj(12, "<< /T (sig2) /Parent 10 0 R /Kids [13 0 R 14 0 R] /V 20 0 R /SV << /Filter /Adobe.PPKLite >> >>")
        .obj(13, "<< /Parent 12 0 R /Type /Annot /Subtype /Widget /Rect [0 0 0 0] /P 3 0 R >>")
        .obj(14, "<< /Parent 12 0 R /Subtype /Widget /Rect [5 5 50 50] /F 4 >>")
        .obj(15, "<< /T (text) /FT /Tx /Subtype /Widget /Rect [0 0 1 1] >>")
        .obj(16, "<< /T <FEFF00FC0062006500720020> /FT /Sig /Subtype /Widget /Rect [1 2 3 4] >>")
        .obj(20, "<< /Type /Sig /Filter /Adobe.PPKLite /Contents <3000> /ByteRange [0 1 2 3] >>")
        .table("/Size 21 /Root 1 0 R", 0);
    let s = open(p.bytes());
    let f = s.signature_fields().unwrap();
    assert_eq!(f.iter().map(|x| x.qualified_name.as_str()).collect::<Vec<_>>(), ["form.sig1", "form.sig2", "über "]);
    let sig1 = &f[0];
    assert_eq!((sig1.depth, sig1.partial_name.as_deref(), sig1.field_flags), (1, Some("sig1"), 4));
    assert!(sig1.has_lock && !sig1.has_seed_value && !sig1.signed);
    assert_eq!(sig1.widgets.len(), 1);
    assert_eq!(sig1.widgets[0].id, (11, 0));
    assert_eq!(sig1.widgets[0].page_index, Some(0));
    assert_eq!(sig1.widgets[0].annot_flags, 132);
    assert_eq!(sig1.widgets[0].rect, Rect::new(10.0, 20.0, 110.0, 70.0));
    let sig2 = &f[1];
    assert!(sig2.signed && sig2.has_seed_value && !sig2.has_lock);
    assert_eq!(sig2.value_id, Some((20, 0)));
    assert_eq!(
        sig2.widgets.iter().map(|w| (w.id, w.page_index)).collect::<Vec<_>>(),
        [((13, 0), Some(0)), ((14, 0), Some(1))]
    );
    let uml = &f[2];
    assert_eq!(uml.depth, 0);
    assert_eq!(uml.widgets[0].page_index, Some(1), "Seite über /Annots gefunden");
    let info = s.acroform_info().unwrap().unwrap();
    assert_eq!((info.id, info.field_count, info.sig_flags), (None, 3, 1));
    // Debug ohne Feldnamen (Regel 9)
    let dbg = format!("{f:?}");
    assert!(!dbg.contains("sig1") && !dbg.contains("form") && !dbg.contains("über"), "{dbg}");
    assert!(dbg.contains("qualified_name_len: 9"), "{dbg}");
}

#[test]
fn image_invocations_with_forms_inline_images_and_cycles() {
    let content = b"q 2 0 0 2 10 10 cm /Im1 Do Q\nBI /W 1 /H 1 /CS /G /BPC 8 ID \x80 EI\n/Fm1 Do /Im1 Do\n/Missing Do";
    let mut p = Pdf::new("1.4");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 /Resources << /XObject << /Im1 5 0 R /Fm1 6 0 R >> >> >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>")
        .stream(4, "", content)
        .stream(5, "/Type /XObject /Subtype /Image /Width 4 /Height 3 /ColorSpace /DeviceGray /BitsPerComponent 8", &[0u8; 12])
        .stream(
            6,
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Matrix [1 0 0 1 100 0] /Resources << /XObject << /Im2 7 0 R /Fm1 6 0 R >> >>",
            b"q 50 0 0 50 0 0 cm /Im2 Do Q /Fm1 Do",
        )
        .stream(7, "/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8", &[255u8; 12])
        .table("/Size 8 /Root 1 0 R", 0);
    let s = open(p.bytes());
    let flat = s.image_invocations(0, false).unwrap();
    assert_eq!(flat.len(), 2);
    assert_eq!(
        (flat[0].resource_name.as_str(), flat[0].ctm, flat[0].order),
        ("Im1", [2.0, 0.0, 0.0, 2.0, 10.0, 10.0], 0)
    );
    assert_eq!((flat[1].ctm, flat[1].order), (pida_pdf_cos::IDENTITY, 1));
    assert_eq!((flat[0].pixel_width, flat[0].pixel_height, flat[0].xobject_id), (4, 3, Some((5, 0))));
    let deep = s.image_invocations(0, true).unwrap();
    assert_eq!(deep.len(), 3);
    assert_eq!(deep[1].resource_name, "Im2");
    assert!(deep[1].inside_form_xobject);
    assert_eq!(deep[1].ctm, [50.0, 0.0, 0.0, 50.0, 100.0, 0.0]);
    assert_eq!(deep.iter().map(|i| i.order).collect::<Vec<_>>(), [0, 1, 2]);
    assert!(!deep[2].inside_form_xobject);
    let img = s.decode_image((7, 0)).unwrap();
    assert_eq!((img.width, img.height), (2, 2));
}

#[test]
fn docmdp_usage_rights_xmp_and_version() {
    let xmp = br#"<?xpacket begin=""?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"><pdfaid:part>2</pdfaid:part><pdfaid:conformance>U</pdfaid:conformance></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;
    let mut p = Pdf::new("1.4");
    p.obj(
        1,
        "<< /Type /Catalog /Pages 2 0 R /Version /1.7 /Metadata 5 0 R /Perms << /DocMDP 6 0 R /UR3 7 0 R >> /NeedsRendering true /StructTreeRoot << /K [8 0 R] >> /AcroForm << /Fields [] /NeedAppearances true /XFA [] >> >>",
    )
    .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    .obj(3, "<< /Type /Page /Parent 2 0 R >>")
    .stream(5, "/Type /Metadata /Subtype /XML", xmp)
    .obj(6, "<< /Type /Sig /Reference [<< /Type /SigRef /TransformMethod /DocMDP /TransformParams << /P 1 /V /1.2 >> >>] /Contents <00> /ByteRange [0 0 0 0] >>")
    .obj(7, "<< /Type /Sig >>")
    .obj(8, "<< /S /Document >>")
    .table("/Size 9 /Root 1 0 R", 0);
    let s = open(p.bytes());
    assert_eq!(s.pdf_version(), "1.7");
    let d = s.doc_mdp().unwrap().unwrap();
    assert_eq!((d.permission, d.signature_id), (1, Some((6, 0))));
    assert!(s.has_usage_rights() && s.needs_rendering() && s.is_tagged());
    let a = s.pdfa_identification().unwrap().unwrap();
    assert_eq!((a.part, a.conformance.as_deref()), (2, Some("U")));
    let info = s.acroform_info().unwrap().unwrap();
    assert!(info.need_appearances && info.has_xfa && info.field_count == 0);
}

#[test]
fn decompression_bombs_are_bounded() {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(&vec![b' '; 8 * 1024 * 1024]).unwrap();
    let bomb = z.finish().unwrap();
    let mut p = Pdf::new("1.5");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>")
        .stream(4, "/Filter /FlateDecode", &bomb)
        .table("/Size 5 /Root 1 0 R", 0);
    let l = Limits { max_decoded_stream: 1024 * 1024, ..Limits::default() };
    let s = SourcePdf::open(p.bytes(), &l).unwrap();
    assert!(matches!(s.decoded_stream((4, 0)), Err(CosError::LimitExceeded { .. })));
    assert!(matches!(s.image_invocations(0, false), Err(CosError::LimitExceeded { .. })));
    // Mit Standardgrenzen unkritisch (8 MiB < Untergrenze des Verhältnisses)
    let s = open(p.bytes());
    assert_eq!(s.decoded_stream((4, 0)).unwrap().len(), 8 * 1024 * 1024);
}

#[test]
fn real_literals_keep_original_precision() {
    let mut p = Pdf::new("1.5");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
        .obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.303937007874 841.889763779528] /X << /Y [1 2 .5] >> >>",
        )
        .table("/Size 4 /Root 1 0 R", 0);
    let s = open(p.bytes());
    let l = s.real_literals((3, 0));
    let mb = s.get_object((3, 0)).unwrap().as_dict().unwrap().get(b"MediaBox").unwrap().as_array().unwrap();
    let Object::Real(w) = mb[2] else { panic!() };
    let path = [PathSegment::Key(b"MediaBox".to_vec()), PathSegment::Index(2)];
    assert_eq!(l.get(&path, w), Some("595.303937007874"));
    assert_eq!(l.get(&path, w + 1.0), None, "geänderter Wert → kein Originaltext");
    let path = [PathSegment::Key(b"X".to_vec()), PathSegment::Key(b"Y".to_vec()), PathSegment::Index(2)];
    assert_eq!(l.get(&path, 0.5), Some(".5"));
    assert_eq!(l.len(), 3);
    assert!(s.real_literals((99, 0)).is_empty());
    assert!(format!("{l:?}").contains("len: 3"));
}

/// Mutationstest (`rotate_090.pdf`): `/MediaBox [0 0.595.32 841.92]` liest lopdf als vier Werte
/// (`0.595`, `.32`), der eigene Lexer als ein Token. Der Originaltext `0.595.32` darf nicht als
/// Schreibweise für `0.595` gelten, sonst würde das fortgeschriebene Objekt fünf Werte enthalten.
#[test]
fn malformed_real_literals_are_not_reused() {
    let mut p = Pdf::new("1.5");
    p.obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
        .obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
        .obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0.595.32 841.92] /A [1.-5 +.25 -0.0 4.] >>")
        .table("/Size 4 /Root 1 0 R", 0);
    let s = open(p.bytes());
    let l = s.real_literals((3, 0));
    let mb = |i| [PathSegment::Key(b"MediaBox".to_vec()), PathSegment::Index(i)];
    assert_eq!(l.get(&mb(1), 0.595), None);
    let a = |i| [PathSegment::Key(b"A".to_vec()), PathSegment::Index(i)];
    assert_eq!(l.get(&a(0), 1.0), None, "1.-5 ist keine Zahl");
    assert_eq!(l.get(&a(1), 0.25), Some("+.25"));
    assert_eq!(l.get(&a(2), -0.0), Some("-0.0"));
    assert_eq!(l.get(&a(3), 4.0), Some("4."));
}

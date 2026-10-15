//! SL-1.2-Requests gegen die JAXB-Golden-Dateien von PDF-AS, Antworten gegen Fixtures,
//! Wohlgeformtheit zusätzlich gegen `xmllint` (übersprungen, wenn nicht installiert).

use std::io::Write as _;
use std::process::{Command, Stdio};

use base64::Engine as _;
use pdfover_sl12::*;

fn testdata(rel: &str) -> Vec<u8> {
    let path = format!("{}/../../testdata/sl12/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn golden(name: &str) -> String {
    String::from_utf8(testdata(&format!("jaxb-golden/{name}"))).unwrap()
}

fn cms(content: DocumentContent, from: u64, to: u64, pades: bool) -> SlRequest {
    let mut r = CreateCmsRequest::new(content, ExcludedByteRange { from, to });
    r.pades_compatibility = pades;
    SlRequest::CreateCmsSignature(r)
}

#[test]
fn infobox_read_request_equals_jaxb_output() {
    assert_eq!(SlRequest::InfoboxReadCertificates.to_xml(), golden("infobox-read-request.xml"));
}

#[test]
fn create_cms_upload_equals_jaxb_output() {
    assert_eq!(cms(DocumentContent::Upload, 1234, 5678, false).to_xml(), golden("create-cms-upload.xml"));
}

#[test]
fn create_cms_base64_equals_jaxb_output() {
    let data = b"%PDF-1.7\n\0\0\0 ende\n".to_vec();
    assert_eq!(cms(DocumentContent::Base64(data), 10, 12, false).to_xml(), golden("create-cms-base64.xml"));
}

#[test]
fn create_cms_pades_flag_and_large_offsets_equal_jaxb_output() {
    let xml = cms(DocumentContent::Upload, 4_294_967_296, 4_294_975_487, true).to_xml();
    assert_eq!(xml, golden("create-cms-upload-pades.xml"));
}

#[test]
fn null_operation_is_the_pdf_over_literal() {
    let xml = SlRequest::NullOperation.to_xml();
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><sl:NullOperationRequest xmlns:sl=\"http://www.buergerkarte.at/namespaces/securitylayer/1.2#\"/>"
    );
    // die JAXB-Variante wäre gleichwertig (nur andere Präfixe), PDF-Over sendet aber das Literal
    assert_ne!(xml, golden("null-operation-request-jaxb.xml"));
}

#[test]
fn large_base64_is_one_line_and_roundtrips() {
    let data: Vec<u8> = (0..1_048_576u32).map(|i| (i % 251) as u8).collect();
    let xml = cms(DocumentContent::Base64(data.clone()), 100, 4195, false).to_xml();
    let start = xml.find("<Base64Content>").unwrap() + "<Base64Content>".len();
    let end = xml.find("</Base64Content>").unwrap();
    let b64 = &xml[start..end];
    assert!(!b64.contains('\n'));
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(b64).unwrap(), data);
}

#[test]
fn http_body_variants() {
    let infobox = SlRequest::InfoboxReadCertificates;
    match http_body(&infobox, None, PartOrder::FileFirst) {
        HttpBody::UrlEncoded(fields) => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "XMLRequest");
            assert_eq!(fields[0].1, infobox.to_xml());
        }
        other => panic!("{other:?}"),
    }

    let upload = cms(DocumentContent::Upload, 1, 2, false);
    let pdf = b"%PDF-1.7 blackout".to_vec();
    match http_body(&upload, Some(&pdf), PartOrder::FileFirst) {
        HttpBody::Multipart(parts) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(
                (parts[0].name.as_str(), parts[0].filename.as_deref(), parts[0].content_type.as_deref()),
                ("fileupload", Some("sign.pdf"), Some("application/pdf"))
            );
            assert_eq!(parts[0].data, pdf);
            assert_eq!((parts[1].name.as_str(), parts[1].filename.as_deref()), ("XMLRequest", None));
            assert_eq!(parts[1].content_type.as_deref(), Some("text/plain; charset=UTF-8"));
            assert_eq!(parts[1].data, upload.to_xml().into_bytes());
        }
        other => panic!("{other:?}"),
    }
    match http_body(&upload, Some(&pdf), PartOrder::XmlFirst) {
        HttpBody::Multipart(parts) => {
            assert_eq!(parts[0].name, "XMLRequest");
            assert_eq!(parts[1].name, "fileupload");
            assert_eq!(parts[1].filename.as_deref(), Some("sign.pdf"));
            assert_eq!(parts[1].content_type.as_deref(), Some("application/octet-stream"));
        }
        other => panic!("{other:?}"),
    }
    // Base64-Modus der lokalen BKU: urlencoded mit eingebettetem Dokument
    let b64 = cms(DocumentContent::Base64(pdf.clone()), 1, 2, false);
    match http_body(&b64, None, PartOrder::XmlFirst) {
        HttpBody::UrlEncoded(fields) => {
            assert!(fields[0].1.contains("<Base64Content>JVBERi0xLjcgYmxhY2tvdXQ=</Base64Content>"))
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn parse_infobox_read_responses() {
    let cert =
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/keystores/test-rsa2048.crt.der")).unwrap();
    for name in ["infobox-read-response-sl-prefix.xml", "infobox-read-response-default-ns.xml"] {
        match parse_response(&testdata(&format!("responses/{name}"))).unwrap() {
            SlResponse::InfoboxRead(r) => {
                assert_eq!(r.signature_certificate(), Some(cert.as_slice()), "{name}");
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    match parse_response(&testdata("responses/infobox-read-response-sl-prefix.xml")).unwrap() {
        SlResponse::InfoboxRead(r) => {
            assert_eq!(r.pairs.len(), 2);
            assert_eq!(r.pairs[0], ("AndererSchluessel".to_owned(), vec![0, 1, 2]));
        }
        other => panic!("{other:?}"),
    }
    match parse_response(&testdata("responses/infobox-read-response-without-pair.xml")).unwrap() {
        SlResponse::InfoboxRead(r) => assert_eq!(r.signature_certificate(), None),
        other => panic!("{other:?}"),
    }
}

#[test]
fn parse_cms_null_and_error_responses() {
    match parse_response(&testdata("responses/create-cms-response.xml")).unwrap() {
        SlResponse::CreateCmsSignature(r) => assert_eq!(&r.cms[..4], &[0x30, 0x80, 0x06, 0x09]),
        other => panic!("{other:?}"),
    }
    assert_eq!(parse_response(&testdata("responses/null-operation-response.xml")).unwrap(), SlResponse::NullOperation);
    match parse_response(&testdata("responses/error-response-6001.xml")).unwrap() {
        SlResponse::Error(e) => {
            assert_eq!(e.code, 6001);
            assert!(e.is_user_cancel());
            assert_eq!(e.info, "Vorgang durch den Benutzer abgebrochen.");
        }
        other => panic!("{other:?}"),
    }
    match parse_response(&testdata("responses/error-response-1501.xml")).unwrap() {
        SlResponse::Error(e) => {
            assert_eq!(e.code, 1501);
            assert!(!e.is_user_cancel());
            assert_eq!(e.info, "Zertifikat ungültig & gesperrt");
        }
        other => panic!("{other:?}"),
    }
    assert!(ErrorResponse { code: 6000, info: String::new() }.is_user_cancel());
}

#[test]
fn parse_rejects_unexpected_documents() {
    assert_eq!(
        parse_response(&testdata("responses/unexpected-root.xml")),
        Err(SlError::UnexpectedRoot("InfoboxReadRequest".into()))
    );
    assert!(
        matches!(parse_response(&testdata("responses/foreign-namespace.xml")), Err(SlError::UnexpectedRoot(r)) if r.contains("urn:anderer-namespace"))
    );
    let ns = SL_NAMESPACE;
    assert_eq!(
        parse_response(format!("<sl:CreateCMSSignatureResponse xmlns:sl=\"{ns}\"/>").as_bytes()),
        Err(SlError::MissingElement("CMSSignature"))
    );
    assert!(matches!(
        parse_response(format!("<sl:CreateCMSSignatureResponse xmlns:sl=\"{ns}\"><sl:CMSSignature>@@@</sl:CMSSignature></sl:CreateCMSSignatureResponse>").as_bytes()),
        Err(SlError::Base64(_))
    ));
    assert_eq!(
        parse_response(format!("<ErrorResponse xmlns=\"{ns}\"><Info>x</Info></ErrorResponse>").as_bytes()),
        Err(SlError::MissingElement("ErrorCode"))
    );
    assert!(matches!(
        parse_response(format!("<ErrorResponse xmlns=\"{ns}\"><ErrorCode>abc</ErrorCode></ErrorResponse>").as_bytes()),
        Err(SlError::Xml(_))
    ));
    assert!(matches!(parse_response(b"<html><body>Fehler</body></html>"), Err(SlError::UnexpectedRoot(_))));
    assert!(matches!(parse_response(b"kein xml"), Err(SlError::Xml(_))));
    // Base64 ohne Padding wird wie von JAXB akzeptiert
    match parse_response(format!("<sl:CreateCMSSignatureResponse xmlns:sl=\"{ns}\"><sl:CMSSignature>AAEC\nAw</sl:CMSSignature></sl:CreateCMSSignatureResponse>").as_bytes()) {
        Ok(SlResponse::CreateCmsSignature(r)) => assert_eq!(r.cms, vec![0, 1, 2, 3]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn sl_response_detection() {
    for name in [
        "infobox-read-response-sl-prefix.xml",
        "infobox-read-response-default-ns.xml",
        "create-cms-response.xml",
        "error-response-6001.xml",
        "error-response-1501.xml",
        "null-operation-response.xml",
    ] {
        assert!(is_sl_response(&testdata(&format!("responses/{name}"))), "{name}");
    }
    assert!(!is_sl_response(&testdata("responses/unexpected-root.xml")));
    assert!(!is_sl_response(&testdata("responses/foreign-namespace.xml")));
    assert!(!is_sl_response(b"<html/>"));
    assert!(!is_sl_response(b""));
    assert!(!is_sl_response(SlRequest::InfoboxReadCertificates.to_xml().as_bytes()));
}

/// Orakel: `xmllint --noout` (libxml2) bestätigt Wohlgeformtheit und Namespaces der Requests.
#[test]
fn xmllint_accepts_all_requests() {
    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!("xmllint fehlt – Test übersprungen");
        return;
    }
    let requests = [
        SlRequest::InfoboxReadCertificates,
        SlRequest::NullOperation,
        cms(DocumentContent::Upload, 1, 2, true),
        cms(DocumentContent::Base64(vec![0u8; 5000]), 1, 2, false),
    ];
    for request in requests {
        let mut child = Command::new("xmllint")
            .args(["--noout", "--nsclean", "-"])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(request.to_xml().as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{request:?}: {}", String::from_utf8_lossy(&out.stderr));
        // Antworten des eigenen Parsers: die Requests selbst sind keine Responses
        assert!(!is_sl_response(request.to_xml().as_bytes()));
    }
}

/// Robustheit: zufällige und verstümmelte Antworten führen nie zu einem Panic.
#[test]
fn parser_survives_random_and_mutated_input() {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    const PARTS: &[&str] = &[
        "<",
        ">",
        "/",
        "=",
        "\"",
        " ",
        "sl:",
        "xmlns",
        ":sl",
        "xmlns:sl=\"http://www.buergerkarte.at/namespaces/securitylayer/1.2#\"",
        "ErrorResponse",
        "ErrorCode",
        "Info",
        "CreateCMSSignatureResponse",
        "CMSSignature",
        "InfoboxReadResponse",
        "AssocArrayData",
        "Pair",
        "Key",
        "Base64Content",
        "&amp;",
        "&#x0;",
        "&#xD800;",
        "&#99999999;",
        "&x;",
        "<![CDATA[",
        "]]>",
        "<!DOCTYPE",
        "<?xml",
        "?>",
        "<!--",
        "-->",
        "AAEC",
        "ä",
        "\u{0}",
        "\u{feff}",
    ];
    for _ in 0..20_000 {
        let n = next() % 24;
        let doc: String = (0..n).map(|_| PARTS[(next() % PARTS.len() as u64) as usize]).collect();
        let _ = parse_response(doc.as_bytes());
        let _ = is_sl_response(doc.as_bytes());
    }
    for name in ["infobox-read-response-sl-prefix.xml", "create-cms-response.xml", "error-response-6001.xml"] {
        let data = testdata(&format!("responses/{name}"));
        for _ in 0..500 {
            let mut d = data.clone();
            let i = (next() as usize) % d.len();
            d[i] = next() as u8;
            let _ = parse_response(&d[..(next() as usize) % (d.len() + 1)]);
            let _ = parse_response(&d);
        }
    }
}

#[test]
fn large_cms_response_and_many_pairs() {
    let ns = SL_NAMESPACE;
    let cms = vec![0x5au8; 3 * 1024 * 1024];
    let b64 = base64::engine::general_purpose::STANDARD.encode(&cms);
    // mit Zeilenumbrüchen alle 76 Zeichen (MIME-Stil)
    let wrapped: String =
        b64.as_bytes().chunks(76).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("\r\n");
    let xml = format!(
        "<CreateCMSSignatureResponse xmlns=\"{ns}\"><CMSSignature>{wrapped}</CMSSignature></CreateCMSSignatureResponse>"
    );
    match parse_response(xml.as_bytes()).unwrap() {
        SlResponse::CreateCmsSignature(r) => assert_eq!(r.cms, cms),
        other => panic!("{other:?}"),
    }
    let pairs: String =
        (0..5_000).map(|i| format!("<Pair Key=\"k{i}\"><Base64Content>AQ==</Base64Content></Pair>")).collect();
    let xml = format!(
        "<InfoboxReadResponse xmlns=\"{ns}\"><AssocArrayData>{pairs}<Pair Key=\"SecureSignatureKeypair\"><Base64Content>AgM=</Base64Content></Pair></AssocArrayData></InfoboxReadResponse>"
    );
    match parse_response(xml.as_bytes()).unwrap() {
        SlResponse::InfoboxRead(r) => assert_eq!(r.signature_certificate(), Some(&[2u8, 3][..])),
        other => panic!("{other:?}"),
    }
}

#[test]
fn unreadable_foreign_pair_does_not_break_the_certificate() {
    let ns = SL_NAMESPACE;
    let xml = |other: &str, cert: &str| {
        format!(
            "<sl:InfoboxReadResponse xmlns:sl=\"{ns}\"><sl:AssocArrayData><sl:Pair Key=\"Anderes\"><sl:Base64Content>{other}</sl:Base64Content></sl:Pair><sl:Pair Key=\"SecureSignatureKeypair\"><sl:Base64Content>{cert}</sl:Base64Content></sl:Pair></sl:AssocArrayData></sl:InfoboxReadResponse>"
        )
    };
    match parse_response(xml("@@kein base64@@", "AgM=").as_bytes()).unwrap() {
        SlResponse::InfoboxRead(r) => {
            assert_eq!(r.signature_certificate(), Some(&[2u8, 3][..]));
            assert_eq!(r.pairs.len(), 1);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(parse_response(xml("AQ==", "@@").as_bytes()), Err(SlError::Base64(_))));
}

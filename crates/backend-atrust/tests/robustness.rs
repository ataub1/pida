//! Robustheit gegen bösartige bzw. kaputte Eingaben: kein Panic, keine Endlosschleife, begrenzter
//! Aufwand (Seitenklassifikation, Poll-Skript/-Antwort, FIDO2-Optionen, Validierung).

mod support;

use pida_backend_atrust::fido2::{AssertionOptions, result_json};
use pida_backend_atrust::page::{PageBlock, classify, diagnostic, link_targets};
use pida_backend_atrust::poll::{parse_poll_response, parse_poll_script};
use pida_backend_atrust::selectors::SelectorSet;
use pida_backend_atrust::validate::{normalize_phone_number, validate_password, validate_tan};
use pida_backend_atrust::{Fido2Assertion, WEBAUTHN_ORIGIN};
use pida_net::Url;
use support::fixture;

/// Deterministischer Pseudozufall (xorshift64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Zeichenkette aus einem Alphabet mit HTML-/Skript-/Mehrbyte-Bausteinen.
    fn text(&mut self, max_parts: u64) -> String {
        const PARTS: &[&str] = &[
            "<",
            ">",
            "/",
            "\"",
            "'",
            "=",
            " ",
            "\n",
            "&",
            ";",
            "#",
            "form",
            "input",
            "name",
            "value",
            "type",
            "submit",
            "id",
            "handynummer",
            "signaturpasswort",
            "input_tan",
            "vergleichswert",
            "qrimage",
            "src",
            "jsLongPoll",
            "script",
            "qrpoll(\"",
            "\", \"",
            "\");",
            "fidoBlock",
            "credentialOptions",
            "href",
            "base",
            "meta",
            "http-equiv",
            "refresh",
            "ä",
            "€",
            "😀",
            "\u{2028}",
            "\u{0}",
            "%",
            "%2F",
            "../",
            "javascript:",
            "https://",
            "error",
            ".aspx",
            "<!--",
            "-->",
            "<![CDATA[",
            "]]>",
        ];
        let n = self.next() % max_parts;
        (0..n).map(|_| PARTS[(self.next() % PARTS.len() as u64) as usize]).collect()
    }
}

fn base() -> Url {
    Url::parse("https://service.a-trust.at/mobile/https-security-layer-request/error.aspx?sid=1").unwrap()
}

#[test]
fn classify_never_panics_on_random_markup() {
    let s = SelectorSet::builtin();
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for _ in 0..3_000 {
        let html = rng.text(80);
        let _ = classify(&base(), &html, &s);
        let _ = diagnostic(&base(), &html);
        assert!(link_targets(&base(), &html).iter().all(|t| !t.contains('?') && !t.contains('#')), "{html}");
    }
}

#[test]
fn classify_mutated_fixtures_never_panics() {
    let s = SelectorSet::builtin();
    let pages = [
        fixture("login.html", &[("SID", "1")]),
        fixture("sms.html", &[("SID", "1"), ("REF", "AB12")]),
        fixture("qr.html", &[("SID", "1"), ("REF", "AB12"), ("POLLID", "1"), ("TOKEN", "t")]),
        fixture("app.html", &[("SID", "1"), ("REF", "AB12"), ("POLLID", "1"), ("TOKEN", "t")]),
        fixture("fido2.html", &[("SID", "1"), ("OPTIONS", "{}")]),
    ];
    let mut rng = Rng(42);
    for page in &pages {
        let bytes = page.as_bytes();
        for _ in 0..400 {
            // Ausschnitt und Einfügung an zufälligen Zeichengrenzen
            let cut = (rng.next() as usize) % bytes.len();
            let cut = (0..=cut).rev().find(|i| page.is_char_boundary(*i)).unwrap_or(0);
            let mutated = format!("{}{}{}", &page[..cut], rng.text(6), &page[cut..]);
            let _ = classify(&base(), &mutated, &s);
            let _ = link_targets(&base(), &mutated);
            let truncated = &page[..cut];
            let _ = classify(&base(), truncated, &s);
        }
    }
}

#[test]
fn large_and_deep_documents_are_handled() {
    let s = SelectorSet::builtin();
    // tiefe Schachtelung (html5ever arbeitet iterativ, ego_tree gibt iterativ frei; der Aufwand des
    // Tree-Builders wächst quadratisch mit der Tiefe – 3 000 Ebenen bleiben unter einer Sekunde)
    let deep = format!("{}<form><input id=input_tan name=t></form>{}", "<div>".repeat(3_000), "</div>".repeat(3_000));
    // (auf `error.aspx` ohne Block seit Nachtrag 19 eine beendete Sitzung – jedenfalls kein SMS-Block)
    let deep_page = classify(&base(), &deep, &s);
    assert!(
        !matches!(deep_page, Ok(ref p) if matches!(p.block, PageBlock::SmsTan { .. })),
        "ohne Vergleichswert kein SMS-Block"
    );
    let _ = link_targets(&base(), &deep);
    // viele Felder: 2 000 Inputs mit 500 verschiedenen Namen (gleiche Namen überschreiben)
    let many: String = (0..2_000).map(|i| format!("<input name=f{} value=v>", i % 500)).collect();
    let html = format!("<form action=n>{many}<input type=submit id=NextBtn name=n></form>");
    let url = Url::parse("https://service.a-trust.at/x/UsernameInfo.aspx").unwrap();
    let p = classify(&url, &html, &s).unwrap();
    assert_eq!(p.form_fields.len(), 500);
    assert!(matches!(p.block, PageBlock::AutoSkip { .. }));
}

#[test]
fn poll_script_and_response_never_panic() {
    let mut rng = Rng(7);
    for _ in 0..5_000 {
        let script = rng.text(12);
        if let Some(info) = parse_poll_script(&script, &base()) {
            assert!(format!("{info:?}").contains("token: \"***\""));
        }
        let _ = parse_poll_response(script.as_bytes());
    }
    // tief geschachteltes JSON (serde_json begrenzt die Rekursion)
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    let _ = parse_poll_response(deep.as_bytes());
    let _ = parse_poll_response(&[0xff, 0xfe, 0x00]);
}

#[test]
fn fido2_options_never_panic() {
    let mut rng = Rng(99);
    let pieces = [
        "{",
        "}",
        "[",
        "]",
        ":",
        ",",
        "\"challenge\"",
        "\"rpId\"",
        "\"publicKey\"",
        "\"allowCredentials\"",
        "\"id\"",
        "\"type\"",
        "\"Buffer\"",
        "\"data\"",
        "\"AQID\"",
        "\"a-trust.at\"",
        "255",
        "256",
        "-1",
        "1e400",
        "null",
        "true",
        "\"timeout\"",
        "18446744073709551616",
    ];
    for _ in 0..10_000 {
        let n = rng.next() % 16;
        let json: String = (0..n).map(|_| pieces[(rng.next() % pieces.len() as u64) as usize]).collect();
        let _ = AssertionOptions::parse(&json, WEBAUTHN_ORIGIN);
    }
    // verschachtelte Buffer-Hüllen enden an der Rekursionsgrenze von serde_json
    let nested = format!("{{\"challenge\":{}\"AQ\"{}}}", "{\"type\":\"Buffer\",\"data\":".repeat(200), "}".repeat(200));
    assert!(AssertionOptions::parse(&nested, WEBAUTHN_ORIGIN).is_err());
    // große Werte im Ergebnis-JSON
    let a = Fido2Assertion {
        id: "\"\\\u{0}😀".into(),
        raw_id: vec![0; 1024],
        kind: "public-key".into(),
        authenticator_data: vec![1; 4096],
        client_data_json: vec![2; 4096],
        signature: vec![3; 512],
        user_handle: Some(vec![4; 64]),
    };
    let v: serde_json::Value = serde_json::from_str(&result_json(&a)).unwrap();
    assert_eq!(v["id"], "\"\\\u{0}😀");
}

#[test]
fn validation_never_panics() {
    let mut rng = Rng(5);
    for _ in 0..5_000 {
        let s = rng.text(10);
        let _ = normalize_phone_number(&s);
        let _ = validate_password(&s);
        let _ = validate_tan(&s, "AB12");
        let _ = validate_tan("x1", &s);
    }
    assert_eq!(normalize_phone_number("+43😀1234"), "+43😀1234");
    assert_eq!(normalize_phone_number("00😀"), "00😀");
}

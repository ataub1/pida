//! Seitenklassifikation gegen die synthetischen Fixtures (docs/analyse/02 §3.5).

mod support;

use pida_backend_atrust::page::{PageBlock, PageError, SubmitButton, classify, diagnostic};
use pida_backend_atrust::selectors::SelectorSet;
use pida_net::Url;
use support::{error_buttons, fixture};

fn url(path: &str) -> Url {
    Url::parse(&format!("https://service.a-trust.at/mobile/https-security-layer-request/{path}")).unwrap()
}

fn btn(name: &str, value: &str) -> Option<SubmitButton> {
    Some(SubmitButton { name: name.into(), value: value.into() })
}

#[test]
fn login_page_fields_links_and_buttons() {
    let s = SelectorSet::builtin();
    let p = classify(&url("Login.aspx?sid=S1"), &fixture("login.html", &[("SID", "S1")]), &s).unwrap();
    assert_eq!(
        p.form_action.as_str(),
        "https://service.a-trust.at/mobile/https-security-layer-request/Login.aspx?sid=S1"
    );
    // alle benannten Inputs außer submit; Checkbox ohne `checked` dabei; select/textarea nicht
    let names: Vec<&str> = p.form_fields.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        names,
        [
            "__VIEWSTATE",
            "__EVENTVALIDATION",
            "ctl00$Main$handynummer",
            "ctl00$Main$signaturpasswort",
            "ctl00$Main$merken"
        ]
    );
    assert_eq!(p.form_fields[0].1, "dGVzdC12aWV3c3RhdGU=");
    assert_eq!(p.form_fields[4].1, "on");
    assert_eq!(
        p.signature_data_link.as_ref().map(Url::as_str),
        Some("https://service.a-trust.at/mobile/https-security-layer-request/ShowSigobj.aspx?sid=S1")
    );
    assert!(p.sms_link.is_none() && p.fido2_link.is_none());
    assert_eq!(
        p.block,
        PageBlock::UsernamePassword {
            username_field: "ctl00$Main$handynummer".into(),
            password_field: "ctl00$Main$signaturpasswort".into(),
            submit: btn("ctl00$Main$Button_Identification", "Identifizieren"),
            cancel: btn("ctl00$Main$Button_Cancel", "Abbrechen"),
            error_message: None,
        }
    );
    // Debug zeigt keine Werte
    assert!(!format!("{p:?}").contains("dGVzdC12aWV3c3RhdGU"));
}

#[test]
fn login_error_selector_is_optional() {
    let mut s = SelectorSet::builtin();
    let html =
        fixture("login.html", &[("SID", "S1")]).replace("<form", "<div class=\"err\"> Passwort  falsch </div><form");
    let p = classify(&url("Login.aspx"), &html, &s).unwrap();
    assert!(
        matches!(p.block, PageBlock::UsernamePassword { error_message: None, .. }),
        "Standard wie PDF-Over: nicht ausgewertet"
    );
    s.login_error = ".err".into();
    let p = classify(&url("Login.aspx"), &html, &s).unwrap();
    assert!(
        matches!(p.block, PageBlock::UsernamePassword { error_message: Some(ref m), .. } if m == "Passwort falsch")
    );
}

#[test]
fn autoskip_pages() {
    let s = SelectorSet::builtin();
    for page in ["UsernameInfo.aspx?sid=1", "tanAppInfo.aspx", "x/tanAppInfo.aspx?y"] {
        let p = classify(&url(page), &fixture("usernameinfo.html", &[("ACTION", "next.aspx")]), &s).unwrap();
        assert_eq!(p.block, PageBlock::AutoSkip { submit: btn("ctl00$Main$NextBtn", "Weiter") }, "{page}");
    }
    // Groß-/Kleinschreibung der URL zählt (Java `contains`)
    let e = classify(&url("usernameinfo.aspx"), &fixture("usernameinfo.html", &[("ACTION", "n")]), &s);
    assert_eq!(e.unwrap_err(), PageError::BlockCount(0));
}

#[test]
fn interstitial_page_own_text() {
    let s = SelectorSet::builtin();
    let p = classify(&url("ExpiresInfo.aspx?sid=1"), &fixture("expiresinfo.html", &[("SID", "1")]), &s).unwrap();
    assert_eq!(
        p.block,
        PageBlock::Interstitial {
            message: "Ihr Zertifikat läuft in Tagen ab. Bitte rechtzeitig erneuern.".into(),
            next: btn("ctl00$Main$Button_Next", "Weiter"),
        }
    );
}

#[test]
fn error_pages() {
    let s = SelectorSet::builtin();
    let html =
        |back, cancel| fixture("error.html", &[("ACTION", "error.aspx"), ("BUTTONS", &error_buttons(back, cancel))]);
    let p = classify(&url("error.aspx?code=1"), &html(true, true), &s).unwrap();
    assert_eq!(
        p.block,
        PageBlock::Error {
            message: "Die Anmeldung ist fehlgeschlagen.\nBitte prüfen Sie Ihre Eingaben.".into(),
            back: btn("ctl00$Main$Button_Back", "Zurück"),
            cancel: btn("ctl00$Main$Button_Cancel", "Abbrechen"),
            recoverable: true,
            requires_response: true,
        }
    );
    for path in ["errorPage.aspx", "error_42.aspx", "SessionClosed.aspx", "sub/error.aspx", "error%5Fx.aspx"] {
        let p = classify(&url(path), &html(false, false), &s).unwrap();
        assert!(matches!(p.block, PageBlock::Error { recoverable: false, requires_response: false, .. }), "{path}");
    }
    // kein Fehler: falsche Endung, Großschreibung, Präfix nicht im letzten Segment
    for path in ["error.html", "Error.aspx", "error/x.aspx", "SessionClosed.aspx.bak", "sessionclosed.aspx"] {
        assert_eq!(classify(&url(path), &html(true, true), &s).unwrap_err(), PageError::BlockCount(0), "{path}");
    }
    // ohne #Label1 kein Fehlerblock
    let no_label = html(true, true).replace("id=\"Label1\"", "id=\"Label9\"");
    assert_eq!(classify(&url("error.aspx"), &no_label, &s).unwrap_err(), PageError::BlockCount(0));
    // ohne #LabelDetail nur Label1
    let no_detail = html(false, true).replace("id=\"LabelDetail\"", "id=\"x\"");
    let p = classify(&url("error.aspx"), &no_detail, &s).unwrap();
    assert!(matches!(p.block, PageBlock::Error { ref message, .. } if message == "Die Anmeldung ist fehlgeschlagen."));
}

#[test]
fn sms_page() {
    let s = SelectorSet::builtin();
    let p = classify(&url("sms.aspx?sid=7"), &fixture("sms.html", &[("SID", "7"), ("REF", " AB12 ")]), &s).unwrap();
    assert_eq!(
        p.block,
        PageBlock::SmsTan {
            tan_field: "ctl00$Main$input_tan".into(),
            reference_value: "AB12".into(),
            submit: btn("ctl00$Main$SignButton", "Signieren"),
        }
    );
    assert_eq!(
        p.fido2_link.as_ref().map(Url::as_str),
        Some("https://service.a-trust.at/mobile/https-security-layer-request/fido.aspx?sid=7")
    );
    // nur der Link mit ShowSigobj.aspx in #LinkList
    assert_eq!(p.signature_data_link.as_ref().unwrap().path(), "/mobile/https-security-layer-request/ShowSigobj.aspx");
    assert_eq!(p.signature_data_link.as_ref().unwrap().query(), Some("sid=7&doc=1"));
}

#[test]
fn qr_page_with_poll() {
    let s = SelectorSet::builtin();
    let html = fixture("qr.html", &[("SID", "Q"), ("REF", "XY99"), ("POLLID", "42"), ("TOKEN", "tok-1")]);
    let p = classify(&url("qr.aspx?sid=Q"), &html, &s).unwrap();
    let PageBlock::QrCode { reference_value, image_url, poll } = &p.block else { panic!("{:?}", p.block) };
    assert_eq!(reference_value, "XY99");
    assert_eq!(image_url.as_str(), "https://service.a-trust.at/mobile/https-security-layer-request/qrimage.ashx?sid=Q");
    let poll = poll.as_ref().unwrap();
    assert_eq!(poll.url.as_str(), "https://service.a-trust.at/mobile/poll/LongPoll.ashx?id=42");
    assert_eq!(poll.token, "tok-1");
    assert!(p.sms_link.is_some() && p.fido2_link.is_some());
    // ohne src kein QR-Block
    let no_src = html.replace("src=\"qrimage.ashx?sid=Q\"", "");
    assert!(classify(&url("qr.aspx"), &no_src, &s).is_err());
    // ohne gültiges Poll-Skript bzw. ohne Vergleichswert kein QR-Block (wie PDF-Over)
    for broken in [
        html.replace("qrpoll(", "andere("),
        html.replace("\", \"tok-1", "\",\"tok-1"),
        html.replace("id=\"jsLongPoll\"", "id=\"anderes\""),
        html.replace("id=\"vergleichswert\"", "id=\"x\""),
    ] {
        assert_eq!(classify(&url("qr.aspx"), &broken, &s).unwrap_err(), PageError::BlockCount(0));
    }
    let no_poll = html.replace("qrpoll(", "andere(");
    assert_eq!(classify(&url("qr.aspx"), &no_poll, &s).unwrap_err(), PageError::BlockCount(0));
}

#[test]
fn incomplete_app_and_qr_markers_do_not_create_extra_blocks() {
    // Eine SMS-Seite mit (verstecktem) App-Element ohne Poll-Skript bleibt eindeutig eine SMS-Seite
    // (PDF-Over: App-Block scheitert an `getLongPollInfo`). Eine tolerante Erkennung fände zwei Blöcke.
    let s = SelectorSet::builtin();
    let sms = fixture("sms.html", &[("SID", "7"), ("REF", "AB12")]);
    for extra in [
        "<div id=\"smartphoneAnimation\" style=\"display:none\"></div>",
        "<img id=\"biometricimage\" src=\"b.png\">",
        "<div class=\"ida-signtype\"><span id=\"SignType\">App</span></div>",
        "<img id=\"qrimage\" src=\"qr.png\">",
    ] {
        let html = sms.replace("</form>", &format!("{extra}</form>"));
        let p = classify(&url("sms.aspx?sid=7"), &html, &s).unwrap();
        assert!(matches!(p.block, PageBlock::SmsTan { .. }), "{extra}");
    }
    // mit Poll-Skript sind es dagegen wirklich zwei Blöcke (auch PDF-Over bricht dann ab)
    let html = sms.replace(
        "</form>",
        "<div id=\"smartphoneAnimation\"></div></form><div id=\"jsLongPoll\"><script>qrpoll(\"p\", \"t\");</script></div>",
    );
    assert_eq!(classify(&url("sms.aspx"), &html, &s).unwrap_err(), PageError::BlockCount(2));
}

#[test]
fn links_must_be_http() {
    let s = SelectorSet::builtin();
    let html = fixture("sms.html", &[("SID", "7"), ("REF", "AB12")])
        .replace("href=\"fido.aspx?sid=7\"", "href=\"javascript:__doPostBack('fido','')\"");
    let p = classify(&url("sms.aspx?sid=7"), &html, &s).unwrap();
    assert!(p.fido2_link.is_none(), "javascript:-Link gilt als fehlend");
    assert!(p.signature_data_link.is_some());
}

#[test]
fn app_pages() {
    let s = SelectorSet::builtin();
    let p = classify(
        &url("app.aspx?sid=A"),
        &fixture("app.html", &[("SID", "A"), ("REF", "R1"), ("POLLID", "9"), ("TOKEN", "T")]),
        &s,
    )
    .unwrap();
    let PageBlock::WaitingForApp2Fa { reference_value, poll } = &p.block else { panic!() };
    assert_eq!(reference_value.as_deref(), Some("R1"));
    assert_eq!(poll.as_ref().unwrap().url.as_str(), "https://service.a-trust.at/mobile/poll/LongPoll.ashx?id=9");

    let p = classify(&url("bio.aspx"), &fixture("app-biometric.html", &[]), &s).unwrap();
    let PageBlock::WaitingForApp2Fa { reference_value, poll } = &p.block else { panic!("{:?}", p.block) };
    assert_eq!(reference_value.as_deref(), Some("B1o"));
    assert_eq!(poll.as_ref().unwrap().token, "T-bio");
    // ohne Vergleichswert oder ohne Poll-Skript: kein App-Block (wie PDF-Over)
    for broken in [
        fixture("app-biometric.html", &[]).replace("id=\"vergleichswert\"", "id=\"x\""),
        fixture("app-biometric.html", &[]).replace("qrpoll(", "poll("),
    ] {
        assert_eq!(classify(&url("bio.aspx"), &broken, &s).unwrap_err(), PageError::BlockCount(0));
    }

    let p = classify(&url("signtype.aspx"), &fixture("app-signtype.html", &[]), &s).unwrap();
    let PageBlock::WaitingForApp2Fa { reference_value, poll } = &p.block else { panic!() };
    assert_eq!(reference_value.as_deref(), Some("K9 x2"));
    assert_eq!(poll.as_ref().unwrap().token, "T-1");
    // #SignType ohne .ida-signtype-Vorfahr zählt nicht
    let bare = fixture("app-signtype.html", &[]).replace("class=\"ida-signtype\"", "class=\"andere\"");
    assert_eq!(classify(&url("signtype.aspx"), &bare, &s).unwrap_err(), PageError::BlockCount(0));
}

#[test]
fn fido2_page() {
    let s = SelectorSet::builtin();
    let json = r#"{"challenge":"AAEC","rpId":"a-trust.at","allowCredentials":[],"userVerification":"preferred"}"#;
    let escaped = json.replace('"', "&quot;");
    let p =
        classify(&url("fido.aspx?sid=F"), &fixture("fido2.html", &[("SID", "F"), ("OPTIONS", &escaped)]), &s).unwrap();
    assert_eq!(
        p.block,
        PageBlock::Fido2 {
            options_json: json.into(),
            result_field: "ctl00$Main$credentialResult".into(),
            submit: btn("ctl00$Main$FidoContinue", "Weiter"),
        }
    );
    assert!(p.sms_link.is_some());
    // fehlt credentialOptions@value → kein Block
    let html = fixture("fido2.html", &[("SID", "F"), ("OPTIONS", "")]).replace(
        " value=\"\" />\n      <input type=\"hidden\" id=\"credentialResult\"",
        " />\n      <input type=\"hidden\" id=\"credentialResult\"",
    );
    assert!(classify(&url("fido.aspx"), &html, &s).is_err());
}

#[test]
fn form_and_block_counts() {
    let s = SelectorSet::builtin();
    assert_eq!(classify(&url("x.aspx"), &fixture("two-forms.html", &[]), &s).unwrap_err(), PageError::FormCount(2));
    assert_eq!(classify(&url("x.aspx"), "<html><body>keine</body></html>", &s).unwrap_err(), PageError::FormCount(0));
    assert_eq!(classify(&url("x.aspx"), &fixture("two-blocks.html", &[]), &s).unwrap_err(), PageError::BlockCount(2));
    assert_eq!(classify(&url("x.aspx"), &fixture("unknown.html", &[]), &s).unwrap_err(), PageError::BlockCount(0));
}

#[test]
fn submit_button_semantics() {
    let s = SelectorSet::builtin();
    let html =
        |button: &str| format!("<form action=\"n.aspx\"><input type=\"hidden\" name=\"a\" value=\"1\">{button}</form>");
    let page = |button: &str| classify(&url("UsernameInfo.aspx"), &html(button), &s).unwrap().block;
    assert_eq!(page("<input type=submit id=NextBtn name=n value=v>"), PageBlock::AutoSkip { submit: btn("n", "v") });
    // kein <input>, kein type=submit, leerer Name → ohne Button senden
    assert_eq!(page("<button id=NextBtn name=n>x</button>"), PageBlock::AutoSkip { submit: None });
    assert_eq!(page("<input type=button id=NextBtn name=n value=v>"), PageBlock::AutoSkip { submit: None });
    assert_eq!(page("<input type=submit id=NextBtn value=v>"), PageBlock::AutoSkip { submit: None });
    assert_eq!(page("<input type=Submit id=NextBtn name=n>"), PageBlock::AutoSkip { submit: btn("n", "") });
    assert_eq!(page(""), PageBlock::AutoSkip { submit: None });
}

#[test]
fn base_href_and_missing_action() {
    let s = SelectorSet::builtin();
    let html = "<html><head><base href=\"https://service.a-trust.at/mobile/other/\"></head><body><form><input type=submit id=NextBtn name=n value=v></form></body></html>";
    // AutoSkip über die <base>-URL? nein: Basis ist jetzt /mobile/other/ → kein UsernameInfo
    assert!(classify(&url("UsernameInfo.aspx"), html, &s).is_err());
    let html = html.replace("/mobile/other/", "/mobile/other/UsernameInfo.aspx");
    let p = classify(&url("irgendwo.aspx"), &html, &s).unwrap();
    assert_eq!(p.base_url.path(), "/mobile/other/UsernameInfo.aspx");
    assert_eq!(p.form_action, p.base_url, "fehlendes action → Dokument-URL");
}

#[test]
fn duplicate_field_names_keep_first_position() {
    let s = SelectorSet::builtin();
    let html = "<form action=a><input name=x value=1><input name=y value=2><input name=x value=3><input type=submit id=NextBtn name=n></form>";
    let p = classify(&url("UsernameInfo.aspx"), html, &s).unwrap();
    assert_eq!(p.form_fields, [("x".to_owned(), "3".to_owned()), ("y".to_owned(), "2".to_owned())]);
}

#[test]
fn diagnostic_has_no_personal_data() {
    let d = diagnostic(&url("neu.aspx?sid=GEHEIM"), &fixture("unknown.html", &[]));
    assert!(d.contains("path=/mobile/https-security-layer-request/neu.aspx"), "{d}");
    assert!(d.contains("forms=1"));
    assert!(d.contains("neuesFeld") && d.contains("weiterKnopf"));
    assert!(!d.contains("GEHEIM") && !d.contains("max.mustermann"), "{d}");
}

#[test]
fn updated_selector_file_without_code_change() {
    // A-Trust benennt die TAN-ID um: nur die Selektor-Datei ändert sich
    let html = fixture("sms.html", &[("SID", "1"), ("REF", "AB12")]).replace("id=\"input_tan\"", "id=\"tanInput\"");
    let old = SelectorSet::builtin();
    assert!(classify(&url("sms.aspx"), &html, &old).is_err());
    let mut json: serde_json::Value = serde_json::from_str(pida_backend_atrust::selectors::BUILTIN_JSON).unwrap();
    json["version"] = 2.into();
    json["ids"]["input_tan"] = "#tanInput".into();
    let new = SelectorSet::from_json_validated(&json.to_string()).unwrap();
    assert!(matches!(classify(&url("sms.aspx"), &html, &new).unwrap().block, PageBlock::SmsTan { .. }));
}

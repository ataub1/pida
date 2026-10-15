//! Kompletter A-Trust-Dialog (Szenario A) gegen einen lokalen Mock-Server mit synthetischen
//! Fixtures (nur 127.0.0.1, keine echten Zugangsdaten, keine Requests an A-Trust).

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use pdfover_backend_api::testing::{BrokerCall, ScriptedBroker, ScriptedReply};
use pdfover_backend_api::{
    BackendError, BackendEvent, BackendSession, FormAnswer, FormProblem, NativeForm, NoticeAnswer, NoticeButtons,
    NoticeCode, NoticeKind, Progress, SignResult, SignatureBackend,
};
use pdfover_backend_atrust::{Fido2Error, fido2};
use pdfover_net::testing::{MockResponse, MockServer, RecordedRequest};
use support::*;
use zeroize::Zeroizing;

const USER: &str = "testuser";
const PASSWORD: &str = "geheim-123";

fn login_reply(password: &str, remember: bool) -> ScriptedReply {
    ScriptedReply::Form(FormAnswer::Login {
        username: USER.into(),
        password: Zeroizing::new(password.into()),
        remember,
    })
}

fn tan(value: &str) -> ScriptedReply {
    ScriptedReply::Form(FormAnswer::Tan(Zeroizing::new(value.into())))
}

fn p(path: &str) -> String {
    format!("{BASE}{path}")
}

fn is(req: &RecordedRequest, method: &str, path: &str) -> bool {
    req.method == method && req.path() == p(path)
}

fn sid(req: &RecordedRequest) -> String {
    req.query().and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("sid="))).unwrap_or("").to_owned()
}

/// Initial-Request: 302 auf die Login-Seite, neue Sitzung `S<n>` mit Cookie.
fn start_session(n: usize) -> MockResponse {
    MockResponse::redirect(302, &format!("Login.aspx?sid=S{n}"))
        .with_header("Set-Cookie", &format!("ASP.NET_SessionId=sess{n}; path=/; HttpOnly"))
}

fn credentials_ok(req: &RecordedRequest) -> bool {
    req.part("ctl00$Main$handynummer").is_some_and(|p| p.text() == USER)
        && req.part("ctl00$Main$signaturpasswort").is_some_and(|p| p.text() == PASSWORD)
        && req.part("ctl00$Main$Button_Identification").is_some()
}

fn forms(broker: &ScriptedBroker) -> Vec<NativeForm> {
    broker.calls().into_iter().filter_map(|c| if let BrokerCall::Form(f) = c { Some(f) } else { None }).collect()
}

fn events(broker: &ScriptedBroker) -> Vec<BackendEvent> {
    broker.calls().into_iter().filter_map(|c| if let BrokerCall::Event(e) = c { Some(e) } else { None }).collect()
}

fn notices(broker: &ScriptedBroker) -> Vec<pdfover_backend_api::Notice> {
    broker.calls().into_iter().filter_map(|c| if let BrokerCall::Notice(n) = c { Some(n) } else { None }).collect()
}

async fn open(
    backend: &pdfover_backend_atrust::AtrustNativeBackend,
    broker: &Arc<ScriptedBroker>,
) -> Box<dyn BackendSession> {
    backend.open(ctx(), broker.clone()).await.unwrap()
}

#[tokio::test]
async fn login_then_sms_tan_and_second_request_logs_in_automatically() {
    let sl = Arc::new(Counter::default());
    let server = MockServer::start({
        let sl = sl.clone();
        move |req| {
            if is(req, "POST", "default.aspx") {
                return start_session(sl.next());
            }
            if is(req, "GET", "Login.aspx") {
                return page("login.html", &[("SID", &sid(req))]);
            }
            if is(req, "POST", "Login.aspx") {
                assert!(credentials_ok(req));
                return if sid(req) == "S0" {
                    MockResponse::redirect(302, "UsernameInfo.aspx?sid=S0")
                } else {
                    MockResponse::redirect(302, &format!("sms.aspx?sid={}", sid(req)))
                };
            }
            if is(req, "GET", "UsernameInfo.aspx") {
                return page("usernameinfo.html", &[("ACTION", "UsernameInfo.aspx?sid=S0")]);
            }
            if is(req, "POST", "UsernameInfo.aspx") {
                assert!(req.part("ctl00$Main$NextBtn").is_some());
                return infobox_xml();
            }
            if is(req, "GET", "sms.aspx") {
                return page("sms.html", &[("SID", &sid(req)), ("REF", "AB12")]);
            }
            if is(req, "POST", "sms.aspx") {
                assert_eq!(req.part("ctl00$Main$input_tan").unwrap().text(), "x7k2q1");
                return cms_xml();
            }
            MockResponse::status(404)
        }
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), tan(" x7k2q1 ")]));
    let mut session = open(&backend, &broker).await;

    let chain = session.certificate().await.unwrap();
    assert_eq!(chain.signer.as_der(), cert_der().as_slice());
    let (pdf, job) = document();
    let results = session.sign(vec![job]).await.unwrap();
    assert_eq!(results, vec![SignResult::Cms(FAKE_CMS.to_vec())]);

    // genau eine Anmeldemaske, eine TAN-Maske; im zweiten SL-Request automatisch angemeldet
    let f = forms(&broker);
    assert_eq!(f.len(), 2, "{f:?}");
    assert!(matches!(&f[0], NativeForm::Login { prefill_username: None, remember: false, .. }));
    match &f[1] {
        NativeForm::SmsTan { reference_value, ctx } => {
            assert_eq!(reference_value, "AB12");
            assert!(ctx.can_switch_to_fido2, "FidoButton vorhanden");
            assert!(!ctx.fido2_visible, "kein Provider");
            assert!(!ctx.can_switch_to_sms);
            // „Signaturdaten anzeigen“ nur über https (ADR 0001 §8.6) – der Mock-Endpunkt ist http
            assert_eq!(ctx.signature_data_url, None);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        events(&broker).iter().filter(|e| matches!(e, BackendEvent::LoginSucceeded { .. })).collect::<Vec<_>>(),
        [&BackendEvent::LoginSucceeded { username: USER.into(), remember: false }]
    );
    assert!(events(&broker).iter().any(|e| matches!(e, BackendEvent::ServerTime(_))));
    assert!(broker.calls().contains(&BrokerCall::Progress(Progress::ReadingCertificate)));
    assert!(broker.calls().contains(&BrokerCall::Progress(Progress::Done)));
    assert_eq!(broker.remaining(), 0);

    let reqs = server.requests();
    // R1a: urlencoded InfoboxRead
    assert_eq!(reqs[0].header("content-type"), Some("application/x-www-form-urlencoded; charset=UTF-8"));
    assert!(reqs[0].form_field("XMLRequest").unwrap().contains("<InfoboxReadRequest"));
    // R1b: multipart, fileupload zuerst
    let cms_req = reqs.iter().filter(|r| is(r, "POST", "default.aspx")).nth(1).unwrap();
    let parts = cms_req.multipart();
    assert_eq!(parts.len(), 2);
    assert_eq!((parts[0].name.as_str(), parts[0].filename.as_deref()), ("fileupload", Some("sign.pdf")));
    assert_eq!(parts[0].content_type.as_deref(), Some("application/pdf"));
    assert_eq!(parts[0].data, pdf);
    assert_eq!(parts[1].name, "XMLRequest");
    assert_eq!(parts[1].content_type.as_deref(), Some("text/plain; charset=UTF-8"));
    let xml = parts[1].text();
    assert!(xml.contains("<Content Reference=\"formdata:fileupload\"/>"), "{xml}");
    assert!(xml.contains("<From>37</From>") && xml.contains("<To>70</To>"), "{xml}");
    // Formular-Submit: alle Felder, nur der geklickte Button
    let login_post = reqs.iter().find(|r| is(r, "POST", "Login.aspx")).unwrap();
    let names: Vec<String> = login_post.multipart().into_iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        [
            "__VIEWSTATE",
            "__EVENTVALIDATION",
            "ctl00$Main$handynummer",
            "ctl00$Main$signaturpasswort",
            "ctl00$Main$merken",
            "ctl00$Main$Button_Identification"
        ]
    );
    assert!(login_post.multipart().iter().all(|p| p.content_type.as_deref() == Some("text/plain; charset=UTF-8")));
    // je SL-Request ein eigener Cookie-Jar
    for r in &reqs[1..] {
        let cookie = r.header("cookie");
        match sid(r).as_str() {
            "S0" => assert_eq!(cookie, Some("ASP.NET_SessionId=sess0"), "{r:?}"),
            "S1" => assert_eq!(cookie, Some("ASP.NET_SessionId=sess1"), "{r:?}"),
            _ => {}
        }
    }
    assert_eq!(cms_req.header("cookie"), None, "zweiter SL-Request startet mit leerem Jar");
    assert!(reqs.iter().all(|r| r.header("user-agent").is_some_and(|ua| ua.starts_with("PDF-Over NG v"))));
}

#[tokio::test]
async fn app_confirmation_with_long_poll() {
    let polls = Arc::new(Mutex::new(Vec::<RecordedRequest>::new()));
    let finished = Arc::new(Counter::default());
    let server = MockServer::start({
        let polls = polls.clone();
        let finished = finished.clone();
        move |req| {
            if is(req, "POST", "default.aspx") {
                return start_session(0);
            }
            if is(req, "GET", "Login.aspx") {
                return page("login.html", &[("SID", "S0")]);
            }
            if is(req, "POST", "Login.aspx") {
                return MockResponse::redirect(302, "app.aspx?sid=S0");
            }
            if is(req, "GET", "app.aspx") {
                if finished.get() > 0 {
                    return infobox_xml();
                }
                return page("app.html", &[("SID", "S0"), ("REF", "Q7"), ("POLLID", "77"), ("TOKEN", "tok-abc")]);
            }
            if req.path() == "/mobile/poll/LongPoll.ashx" {
                let mut log = polls.lock().unwrap();
                log.push(req.clone());
                return if log.len() < 3 {
                    MockResponse::json(r#"{"Fin":false,"Wait":true,"Error":false}"#)
                } else {
                    finished.next();
                    MockResponse::json(r#"{"Fin":true,"Wait":false,"Error":false}"#)
                };
            }
            MockResponse::status(404)
        }
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), ScriptedReply::Pending]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();

    let polls = polls.lock().unwrap().clone();
    assert_eq!(polls.len(), 3);
    for poll in &polls {
        assert_eq!(poll.header("authorization"), Some("Bearer tok-abc"));
        assert_eq!(poll.header("cookie"), None, "Long-Poll ohne Cookies");
        assert_eq!(poll.query(), Some("id=77"));
    }
    let f = forms(&broker);
    match &f[1] {
        NativeForm::WaitForApp { reference_value, timeout_secs, ctx } => {
            assert_eq!(reference_value.as_deref(), Some("Q7"));
            assert_eq!(*timeout_secs, Some(5));
            assert!(ctx.can_switch_to_sms && ctx.can_switch_to_fido2);
        }
        other => panic!("{other:?}"),
    }
    let calls = broker.calls();
    let wait = calls.iter().position(|c| matches!(c, BrokerCall::Form(NativeForm::WaitForApp { .. }))).unwrap();
    assert_eq!(calls[wait + 1], BrokerCall::FormClosed, "Maske wird bei „fertig“ geschlossen");
    // R7: Neuladen der App-Seite mit Session-Cookie
    let reloads: Vec<_> = server.requests().into_iter().filter(|r| is(r, "GET", "app.aspx")).collect();
    assert_eq!(reloads.len(), 2);
    assert_eq!(reloads[1].header("cookie"), Some("ASP.NET_SessionId=sess0"));
}

#[tokio::test]
async fn qr_code_image_without_cookies_and_poll_error_reloads() {
    let server = MockServer::start({
        let state = Arc::new(Counter::default());
        move |req| {
            if is(req, "POST", "default.aspx") {
                return start_session(0);
            }
            if is(req, "GET", "Login.aspx") {
                return page("login.html", &[("SID", "S0")]);
            }
            if is(req, "POST", "Login.aspx") {
                return MockResponse::redirect(302, "qr.aspx?sid=S0");
            }
            if is(req, "GET", "qr.aspx") {
                return if state.get() > 0 {
                    infobox_xml()
                } else {
                    page("qr.html", &[("SID", "S0"), ("REF", "QR12"), ("POLLID", "5"), ("TOKEN", "qt")])
                };
            }
            if is(req, "GET", "qrimage.ashx") {
                assert_eq!(req.header("cookie"), None, "QR-Bild ohne Cookies");
                return MockResponse::body("image/png", b"\x89PNG\r\n\x1a\nFAKE".to_vec());
            }
            if req.path() == "/mobile/poll/LongPoll.ashx" {
                state.next();
                return MockResponse::json(r#"{"Fin":false,"Wait":false,"Error":true}"#);
            }
            MockResponse::status(404)
        }
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), ScriptedReply::Pending]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    match &forms(&broker)[1] {
        NativeForm::QrCode { reference_value, image, timeout_secs, ctx } => {
            assert_eq!(reference_value, "QR12");
            let image = image.as_ref().unwrap();
            assert_eq!(image.bytes, b"\x89PNG\r\n\x1a\nFAKE");
            assert_eq!(image.mime.as_deref(), Some("image/png"));
            assert_eq!(*timeout_secs, Some(5));
            assert_eq!(ctx.problem, None);
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn qr_image_failure_and_manual_continue_and_switch_to_sms() {
    let server = MockServer::start({
        let continued = Arc::new(Counter::default());
        move |req| {
            if is(req, "POST", "default.aspx") {
                return start_session(0);
            }
            if is(req, "GET", "Login.aspx") {
                return page("login.html", &[("SID", "S0")]);
            }
            if is(req, "POST", "Login.aspx") {
                return MockResponse::redirect(302, "qr.aspx?sid=S0");
            }
            if is(req, "GET", "qr.aspx") {
                continued.next();
                return page("qr.html", &[("SID", "S0"), ("REF", "QR12"), ("POLLID", "5"), ("TOKEN", "qt")]);
            }
            if is(req, "GET", "qrimage.ashx") {
                return MockResponse::status(404);
            }
            if req.path() == "/mobile/poll/LongPoll.ashx" {
                return MockResponse::json(r#"{"Fin":false,"Wait":true,"Error":false}"#);
            }
            if is(req, "GET", "sms.aspx") {
                return page("sms.html", &[("SID", "S0"), ("REF", "SM34")]);
            }
            if is(req, "POST", "sms.aspx") {
                return infobox_xml();
            }
            MockResponse::status(404)
        }
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([
        login_reply(PASSWORD, false),
        ScriptedReply::Form(FormAnswer::Continue),
        ScriptedReply::Form(FormAnswer::SwitchToSms),
        tan("123456"),
    ]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let f = forms(&broker);
    assert!(
        matches!(&f[1], NativeForm::QrCode { image: None, ctx, .. } if ctx.problem == Some(FormProblem::QrImageFailed))
    );
    assert!(matches!(&f[2], NativeForm::QrCode { .. }), "„fertig“ → Seite neu geladen");
    assert!(matches!(&f[3], NativeForm::SmsTan { reference_value, .. } if reference_value == "SM34"));
    assert_eq!(server.requests().iter().filter(|r| is(r, "GET", "qr.aspx")).count(), 2);
}

fn fido_options() -> String {
    r#"{"challenge":"AAECAw","rpId":"a-trust.at","allowCredentials":[{"type":"public-key","id":"BAUG"}],"userVerification":"preferred","timeout":60000}"#.into()
}

fn fido_server(sms_first: bool) -> impl Fn(&RecordedRequest) -> MockResponse + Send + Sync + 'static {
    move |req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            return MockResponse::redirect(302, if sms_first { "sms.aspx?sid=S0" } else { "fido.aspx?sid=S0" });
        }
        if is(req, "GET", "sms.aspx") {
            return page("sms.html", &[("SID", "S0"), ("REF", "AB12")]);
        }
        if is(req, "POST", "sms.aspx") {
            return infobox_xml();
        }
        if is(req, "GET", "fido.aspx") {
            return page("fido2.html", &[("SID", "S0"), ("OPTIONS", &fido_options().replace('"', "&quot;"))]);
        }
        if is(req, "POST", "fido.aspx") {
            assert!(req.part("ctl00$Main$FidoContinue").is_some());
            return infobox_xml();
        }
        MockResponse::status(404)
    }
}

#[tokio::test]
async fn fido2_default_with_mocked_provider() {
    let server = MockServer::start(fido_server(true)).await;
    let provider = FakeFido2::new(vec![Ok(FakeFido2::assertion())]);
    let backend = backend(&server, |c| {
        c.fido2 = Some(provider.clone());
        c.fido2_default = true;
    });
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), ScriptedReply::Pending]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();

    // FIDO2-Default: SMS-Seite mit FidoButton → sofort GET auf den FIDO-Link, keine TAN-Maske
    let f = forms(&broker);
    assert_eq!(f.len(), 2, "{f:?}");
    assert!(
        matches!(&f[1], NativeForm::Fido2 { in_progress: true, ctx } if ctx.fido2_visible && ctx.can_switch_to_sms)
    );
    let calls = broker.calls();
    let fido = calls.iter().position(|c| matches!(c, BrokerCall::Form(NativeForm::Fido2 { .. }))).unwrap();
    assert_eq!(calls[fido + 1], BrokerCall::FormClosed);
    let requests = provider.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].options_json, fido_options());
    assert_eq!(requests[0].origin, "https://service.a-trust.at");
    let post = server.requests().into_iter().find(|r| is(r, "POST", "fido.aspx")).unwrap();
    let result = post.part("ctl00$Main$credentialResult").unwrap().text();
    assert_eq!(result, fido2::result_json(&FakeFido2::assertion()));
    let v: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(v["response"]["userHandle"].is_null());
    assert_eq!(v["response"]["signature"], "MET/");
    assert!(!server.requests().iter().any(|r| is(r, "POST", "sms.aspx")));
}

#[tokio::test]
async fn fido2_error_then_switch_to_sms() {
    let server = MockServer::start(fido_server(false)).await;
    let provider = FakeFido2::new(vec![Err(Fido2Error::Device("Token entfernt".into()))]);
    let backend = backend(&server, |c| c.fido2 = Some(provider.clone()));
    let broker = Arc::new(ScriptedBroker::new([
        login_reply(PASSWORD, false),
        ScriptedReply::Pending,
        ScriptedReply::Form(FormAnswer::SwitchToSms),
        tan("777777"),
    ]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let f = forms(&broker);
    assert!(matches!(&f[1], NativeForm::Fido2 { in_progress: true, .. }));
    match &f[2] {
        NativeForm::Fido2 { in_progress: false, ctx } => {
            assert!(matches!(&ctx.problem, Some(FormProblem::Fido2Failed(m)) if m.contains("Token entfernt")));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(&f[3], NativeForm::SmsTan { .. }));
}

#[tokio::test]
async fn fido2_cancel_at_token_retry_succeeds() {
    let server = MockServer::start(fido_server(false)).await;
    let provider = FakeFido2::new(vec![Err(Fido2Error::Cancelled), Ok(FakeFido2::assertion())]);
    let backend = backend(&server, |c| c.fido2 = Some(provider.clone()));
    let broker = Arc::new(ScriptedBroker::new([
        login_reply(PASSWORD, false),
        ScriptedReply::Pending,
        ScriptedReply::Form(FormAnswer::StartFido2),
        ScriptedReply::Pending,
    ]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let f = forms(&broker);
    assert!(
        matches!(&f[2], NativeForm::Fido2 { in_progress: false, ctx } if ctx.problem.is_none()),
        "Abbruch am Token ohne Fehlermeldung"
    );
    assert!(matches!(&f[3], NativeForm::Fido2 { in_progress: true, .. }));
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn fido2_unavailable_offers_sms_only() {
    let server = MockServer::start(fido_server(true)).await;
    let backend = backend(&server, |c| c.fido2_default = true); // ohne Provider wirkt der Default nicht
    let broker = Arc::new(ScriptedBroker::new([
        login_reply(PASSWORD, false),
        ScriptedReply::Form(FormAnswer::SwitchToFido2),
        ScriptedReply::Form(FormAnswer::SwitchToSms),
        tan("424242"),
    ]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let f = forms(&broker);
    assert!(matches!(&f[1], NativeForm::SmsTan { .. }), "kein FIDO2-Default ohne Provider");
    match &f[2] {
        NativeForm::Fido2 { in_progress: false, ctx } => {
            assert!(!ctx.fido2_visible);
            assert!(matches!(ctx.problem, Some(FormProblem::Fido2Failed(_))));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(&f[3], NativeForm::SmsTan { .. }));
}

#[tokio::test]
async fn fido2_user_cancel_while_running_cancels_provider() {
    let server = MockServer::start(fido_server(false)).await;
    let provider = Arc::new(FakeFido2 {
        delay: Duration::from_millis(400),
        ..Arc::try_unwrap(FakeFido2::new(vec![Ok(FakeFido2::assertion())])).unwrap()
    });
    let backend = backend(&server, |c| c.fido2 = Some(provider.clone()));
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert!(provider.cancels.load(std::sync::atomic::Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn interstitial_and_tan_app_info_autoskip() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            return MockResponse::redirect(302, "ExpiresInfo.aspx?sid=S0");
        }
        if is(req, "GET", "ExpiresInfo.aspx") {
            return page("expiresinfo.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "ExpiresInfo.aspx") {
            assert!(req.part("ctl00$Main$Button_Next").is_some());
            return MockResponse::redirect(301, "tanAppInfo.aspx?sid=S0");
        }
        if is(req, "GET", "tanAppInfo.aspx") {
            return page("usernameinfo.html", &[("ACTION", "tanAppInfo.aspx?sid=S0")]);
        }
        if is(req, "POST", "tanAppInfo.aspx") {
            assert!(req.part("ctl00$Main$NextBtn").is_some());
            assert!(req.part("ctl00$Main$Button_Cancel").is_none(), "nur der geklickte Button");
            return infobox_xml();
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), ScriptedReply::Notice(NoticeAnswer::Ok)]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let n = notices(&broker);
    assert_eq!(n.len(), 1);
    assert_eq!(
        (n[0].kind, n[0].code.clone(), n[0].buttons),
        (NoticeKind::Info, NoticeCode::AtrustInterstitial, NoticeButtons::Ok)
    );
    assert_eq!(n[0].text.as_deref(), Some("Ihr Zertifikat läuft in Tagen ab. Bitte rechtzeitig erneuern."));

    // Abbruch im Hinweis = Nutzerabbruch
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
}

/// Mock: falsches Passwort → Fehlerseite; `Button_Back` → Login; `Button_Cancel` → SL 6001.
fn error_flow(
    buttons: (bool, bool),
    path: &'static str,
) -> impl Fn(&RecordedRequest) -> MockResponse + Send + Sync + 'static {
    move |req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            return if credentials_ok(req) {
                MockResponse::redirect(302, "UsernameInfo.aspx?sid=S0")
            } else {
                MockResponse::redirect(302, &format!("{path}?sid=S0"))
            };
        }
        if is(req, "GET", path) {
            return page(
                "error.html",
                &[("ACTION", &format!("{path}?sid=S0")), ("BUTTONS", &error_buttons(buttons.0, buttons.1))],
            );
        }
        if is(req, "POST", path) {
            if req.part("ctl00$Main$Button_Back").is_some() {
                return MockResponse::redirect(302, "Login.aspx?sid=S0");
            }
            assert!(req.part("ctl00$Main$Button_Cancel").is_some());
            return sl_error_xml(6001);
        }
        if is(req, "GET", "UsernameInfo.aspx") {
            return page("usernameinfo.html", &[("ACTION", "UsernameInfo.aspx?sid=S0")]);
        }
        if is(req, "POST", "UsernameInfo.aspx") {
            return infobox_xml();
        }
        MockResponse::status(404)
    }
}

#[tokio::test]
async fn error_page_retry_asks_again_without_resending() {
    let server = MockServer::start(error_flow((true, true), "error.aspx")).await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([
        login_reply("falsch-1", false),
        ScriptedReply::Notice(NoticeAnswer::Retry),
        login_reply(PASSWORD, false),
    ]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let n = notices(&broker);
    assert_eq!(
        (n[0].kind, n[0].code.clone(), n[0].buttons),
        (NoticeKind::Error, NoticeCode::AtrustError, NoticeButtons::RetryCancel)
    );
    assert_eq!(n[0].text.as_deref(), Some("Die Anmeldung ist fehlgeschlagen.\nBitte prüfen Sie Ihre Eingaben."));
    let f = forms(&broker);
    match &f[1] {
        NativeForm::Login { prefill_username, ctx, .. } => {
            assert_eq!(prefill_username.as_deref(), Some(USER), "Benutzername bleibt");
            assert_eq!(ctx.problem, Some(FormProblem::LoginRejected));
        }
        other => panic!("{other:?}"),
    }
    let posts = server.requests().into_iter().filter(|r| is(r, "POST", "Login.aspx")).count();
    assert_eq!(posts, 2);
}

#[tokio::test]
async fn error_page_cancel_reports_to_atrust_and_ends_with_sl_cancel() {
    let server = MockServer::start(error_flow((true, true), "errorLogin.aspx")).await;
    let backend = backend(&server, |_| {});
    let broker =
        Arc::new(ScriptedBroker::new([login_reply("falsch-1", false), ScriptedReply::Notice(NoticeAnswer::Cancel)]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert!(
        server
            .requests()
            .iter()
            .any(|r| is(r, "POST", "errorLogin.aspx") && r.part("ctl00$Main$Button_Cancel").is_some())
    );
}

#[tokio::test]
async fn unrecoverable_error_without_response_is_a_silent_cancel() {
    let server = MockServer::start(error_flow((false, false), "SessionClosed.aspx")).await;
    let backend = backend(&server, |_| {});
    let broker =
        Arc::new(ScriptedBroker::new([login_reply("falsch-1", false), ScriptedReply::Notice(NoticeAnswer::Ok)]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    let n = notices(&broker);
    assert_eq!(n[0].buttons, NoticeButtons::Ok);
    assert!(!server.requests().iter().any(|r| is(r, "POST", "SessionClosed.aspx")));
    // unrecoverable mit Button_Cancel: Abbruch wird an A-Trust gemeldet
    let server = MockServer::start(error_flow((false, true), "error.aspx")).await;
    let backend = support::backend(&server, |_| {});
    let broker =
        Arc::new(ScriptedBroker::new([login_reply("falsch-1", false), ScriptedReply::Notice(NoticeAnswer::Ok)]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert!(server.requests().iter().any(|r| is(r, "POST", "error.aspx")));
}

#[tokio::test]
async fn repeated_login_page_is_never_resent_automatically() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            if req.part("ctl00$Main$Button_Cancel").is_some() {
                return sl_error_xml(6001);
            }
            // A-Trust liefert wieder die Login-Seite (statt einer Fehlerseite)
            return page("login.html", &[("SID", "S0")]);
        }
        MockResponse::status(404)
    })
    .await;
    // Passwort aus `-p` vorbelegt: der erste Login läuft ohne Maske
    let backend = backend(&server, |c| {
        c.prefill_username = Some(USER.into());
        c.prefill_password = Some(Zeroizing::new("falsch-cli".into()));
    });
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    let f = forms(&broker);
    assert_eq!(f.len(), 1);
    assert!(matches!(&f[0], NativeForm::Login { ctx, .. } if ctx.problem == Some(FormProblem::LoginRejected)));
    let posts: Vec<_> = server.requests().into_iter().filter(|r| is(r, "POST", "Login.aspx")).collect();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].part("ctl00$Main$signaturpasswort").unwrap().text(), "falsch-cli");
    assert!(posts[1].part("ctl00$Main$Button_Cancel").is_some(), "zweiter POST ist der Abbruch, kein erneuter Login");
    assert!(posts[1].part("ctl00$Main$Button_Identification").is_none());
}

#[tokio::test]
async fn invalid_password_length_is_reasked() {
    let server = MockServer::start(error_flow((true, true), "error.aspx")).await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply("1234", false), login_reply(PASSWORD, false)]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let f = forms(&broker);
    assert!(matches!(&f[1], NativeForm::Login { ctx, .. } if ctx.problem == Some(FormProblem::PasswordTooShort)));
    assert_eq!(server.requests().iter().filter(|r| is(r, "POST", "Login.aspx")).count(), 1);
}

#[tokio::test]
async fn user_cancel_in_login_sms_and_app() {
    // Login: Abbruch → Button_Cancel an A-Trust → SL 6001 → Cancelled
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            if req.part("ctl00$Main$Button_Cancel").is_some() {
                return sl_error_xml(6001);
            }
            return MockResponse::redirect(
                302,
                if req.part("ctl00$Main$handynummer").unwrap().text() == "app" {
                    "app.aspx?sid=S0"
                } else {
                    "sms.aspx?sid=S0"
                },
            );
        }
        if is(req, "GET", "sms.aspx") {
            return page("sms.html", &[("SID", "S0"), ("REF", "AB12")]);
        }
        if is(req, "GET", "app.aspx") {
            return page("app.html", &[("SID", "S0"), ("REF", "R"), ("POLLID", "1"), ("TOKEN", "t")]);
        }
        if req.path() == "/mobile/poll/LongPoll.ashx" {
            return MockResponse::json(r#"{"Fin":false,"Wait":true,"Error":false}"#);
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |c| c.remember = true);
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);

    // SMS: Abbruch ohne Meldung an A-Trust
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, true), ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    let before = server.requests().len();
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert!(!server.requests()[before..].iter().any(|r| is(r, "POST", "sms.aspx")));

    // App: Abbruch löscht das gemerkte Passwort, der Benutzername bleibt
    let broker = Arc::new(ScriptedBroker::new([
        ScriptedReply::Form(FormAnswer::Login {
            username: "app".into(),
            password: Zeroizing::new(PASSWORD.into()),
            remember: true,
        }),
        ScriptedReply::Cancel,
    ]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    let _ = session.certificate().await;
    match &forms(&broker)[0] {
        NativeForm::Login { prefill_username, remember, .. } => {
            assert_eq!(prefill_username.as_deref(), Some("app"));
            assert!(*remember);
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn cancel_token_aborts_waiting_for_app() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            return MockResponse::redirect(302, "app.aspx?sid=S0");
        }
        if is(req, "GET", "app.aspx") {
            return page("app.html", &[("SID", "S0"), ("REF", "R"), ("POLLID", "1"), ("TOKEN", "t")]);
        }
        if req.path() == "/mobile/poll/LongPoll.ashx" {
            return MockResponse::json(r#"{"Fin":false,"Wait":true,"Error":false}"#)
                .with_delay(Duration::from_millis(200));
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false), ScriptedReply::Pending]));
    let context = ctx();
    let cancel = context.cancel.clone();
    let mut session = backend.open(context, broker.clone()).await.unwrap();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        cancel.cancel();
    });
    let started = std::time::Instant::now();
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(4));
    canceller.await.unwrap();
    assert_eq!(broker.calls().last(), Some(&BrokerCall::FormClosed));
    session.cancel().await; // idempotent
}

#[tokio::test]
async fn http_errors_show_notice_then_cancel() {
    for (status, code) in [
        (413u16, NoticeCode::DocumentTooLarge),
        (407, NoticeCode::ProxyAuthRequired),
        (500, NoticeCode::Http { status: 500 }),
        (303, NoticeCode::Http { status: 303 }),
        (307, NoticeCode::Http { status: 307 }),
    ] {
        let server = MockServer::start(move |_| MockResponse::status(status)).await;
        let backend = backend(&server, |_| {});
        let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Notice(NoticeAnswer::Ok)]));
        let mut session = open(&backend, &broker).await;
        let (_, job) = document();
        assert_eq!(session.sign(vec![job]).await.unwrap_err(), BackendError::Cancelled, "{status}");
        let n = notices(&broker);
        assert_eq!(n.len(), 1);
        assert_eq!((n[0].kind, n[0].code.clone(), n[0].buttons), (NoticeKind::Error, code, NoticeButtons::Ok));
        if status == 500 {
            // `atrusterror.http_generic` „HTTP %d %s“: Status im Code, Text = Reason-Phrase
            assert_eq!(n[0].text.as_deref(), Some("Internal Server Error"));
        }
        assert_eq!(server.requests().len(), 1, "keine Redirect-Verfolgung bei {status}");
    }
}

#[tokio::test]
async fn http_errors_without_notice_support_return_specific_errors() {
    for (status, expected) in [
        (413u16, BackendError::DocumentTooLarge),
        (407, BackendError::ProxyAuthRequired),
        (502, BackendError::Http { status: 502, reason: "Gateway Kaputt".into() }),
    ] {
        let server = MockServer::start(move |_| MockResponse::status(status).with_reason("Gateway Kaputt")).await;
        let backend = backend(&server, |_| {});
        let broker = Arc::new(ScriptedBroker::new([])); // notice → Unsupported
        let mut session = open(&backend, &broker).await;
        let err = session.certificate().await.unwrap_err();
        let expected = match expected {
            BackendError::Http { status, .. } => BackendError::Http { status, reason: "Gateway Kaputt".into() },
            e => e,
        };
        assert_eq!(err, expected);
    }
}

#[tokio::test]
async fn loop_guard_stops_redirect_loops() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, "loop.aspx");
        }
        MockResponse::redirect(302, "loop.aspx")
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::LoopDetected);
    assert_eq!(server.requests().len(), 51);
}

#[tokio::test]
async fn refresh_header_and_meta_refresh_are_followed_as_get() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::html("<p>bitte warten</p>").with_header("Refresh", "0; URL=Weiter.aspx?sid=S0");
        }
        if is(req, "GET", "Weiter.aspx") {
            return page("meta-refresh.html", &[("TARGET", "Login.aspx?sid=S0")]);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            return MockResponse::redirect(302, "/mobile/https-security-layer-request/UsernameInfo.aspx?sid=S0");
        }
        if is(req, "GET", "UsernameInfo.aspx") {
            return page("usernameinfo.html", &[("ACTION", "UsernameInfo.aspx?sid=S0")]);
        }
        if is(req, "POST", "UsernameInfo.aspx") {
            return infobox_xml();
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false)]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let methods: Vec<(String, String)> =
        server.requests().into_iter().map(|r| (r.method.clone(), r.path().to_owned())).collect();
    assert_eq!(methods[1], ("GET".to_owned(), p("Weiter.aspx")));
    assert_eq!(methods[2], ("GET".to_owned(), p("Login.aspx")));
}

#[tokio::test]
async fn protocol_errors() {
    // 302 ohne Location
    let server = MockServer::start(|_| MockResponse::status(302)).await;
    let backend = backend(&server, |_| {});
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    assert!(matches!(session.certificate().await.unwrap_err(), BackendError::Protocol(m) if m.contains("Location")));
    // ungültiger Refresh-Header
    let server = MockServer::start(|_| MockResponse::html("x").with_header("Refresh", "jetzt")).await;
    let backend = support::backend(&server, |_| {});
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    assert!(matches!(session.certificate().await.unwrap_err(), BackendError::Protocol(_)));
    // unbekannter Content-Type
    let server = MockServer::start(|_| MockResponse::json("{}")).await;
    let backend = support::backend(&server, |_| {});
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    assert!(
        matches!(session.certificate().await.unwrap_err(), BackendError::UnknownPage { diagnostic } if diagnostic.contains("application/json"))
    );
    // XML, aber keine SL-Antwort
    let server = MockServer::start(|_| MockResponse::xml("<html/>")).await;
    let backend = support::backend(&server, |_| {});
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    assert!(matches!(session.certificate().await.unwrap_err(), BackendError::Protocol(_)));
    // Verbindung abgelehnt → Network (Wiederholen möglich)
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let mut cfg = support::config(&server);
    cfg.endpoint = pdfover_net::Url::parse(&format!("http://{addr}/default.aspx")).unwrap();
    let backend = pdfover_backend_atrust::AtrustNativeBackend::new(cfg);
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    let err = session.certificate().await.unwrap_err();
    assert!(matches!(err, BackendError::Network(_)), "{err:?}");
    assert!(err.is_retryable());
}

#[tokio::test]
async fn unknown_page_aborts_with_diagnostic_without_personal_data() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, "neu.aspx?sid=GEHEIM");
        }
        page("unknown.html", &[])
    })
    .await;
    let backend = backend(&server, |_| {});
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    match session.certificate().await.unwrap_err() {
        BackendError::UnknownPage { diagnostic } => {
            assert!(diagnostic.contains("0 Blöcke"), "{diagnostic}");
            assert!(diagnostic.contains("/neu.aspx"));
            assert!(!diagnostic.contains("GEHEIM") && !diagnostic.contains("max.mustermann"), "{diagnostic}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn security_layer_error_is_reported() {
    let server = MockServer::start(|_| sl_error_xml(1501)).await;
    let backend = backend(&server, |_| {});
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    assert_eq!(
        session.certificate().await.unwrap_err(),
        BackendError::SecurityLayer { code: 1501, info: "Fehler 1501".into() }
    );
}

#[tokio::test]
async fn remembered_credentials_are_reused_by_the_next_session() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            assert!(credentials_ok(req));
            return MockResponse::redirect(302, "UsernameInfo.aspx?sid=S0");
        }
        if is(req, "GET", "UsernameInfo.aspx") {
            return page("usernameinfo.html", &[("ACTION", "UsernameInfo.aspx?sid=S0")]);
        }
        if is(req, "POST", "UsernameInfo.aspx") {
            return infobox_xml();
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, true)]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    assert!(events(&broker).contains(&BackendEvent::LoginSucceeded { username: USER.into(), remember: true }));

    // nächste Sitzung: ohne Maske
    let broker2 = Arc::new(ScriptedBroker::new([]));
    let mut session = open(&backend, &broker2).await;
    session.certificate().await.unwrap();
    assert!(forms(&broker2).is_empty());

    // Vergessen → wieder fragen
    backend.forget_credentials();
    let broker3 = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false)]));
    let mut session = open(&backend, &broker3).await;
    session.certificate().await.unwrap();
    assert_eq!(forms(&broker3).len(), 1);
}

#[tokio::test]
async fn unticking_remember_forgets_credentials() {
    let server = MockServer::start(error_flow((true, true), "error.aspx")).await;
    let backend = backend(&server, |c| c.remember = true);
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false)]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    let e = events(&broker);
    assert!(e.contains(&BackendEvent::ForgetCredentials));
    assert!(e.contains(&BackendEvent::LoginSucceeded { username: USER.into(), remember: false }));
}

#[tokio::test]
async fn batch_signs_each_document_with_one_login() {
    let sl = Arc::new(Counter::default());
    let server = MockServer::start({
        let sl = sl.clone();
        move |req| {
            if is(req, "POST", "default.aspx") {
                return start_session(sl.next());
            }
            if is(req, "GET", "Login.aspx") {
                return page("login.html", &[("SID", &sid(req))]);
            }
            if is(req, "POST", "Login.aspx") {
                assert!(credentials_ok(req));
                return MockResponse::redirect(302, &format!("app.aspx?sid={}", sid(req)));
            }
            if is(req, "GET", "app.aspx") {
                return cms_xml_with(sid(req).as_bytes());
            }
            MockResponse::status(404)
        }
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([login_reply(PASSWORD, false)]));
    let mut session = open(&backend, &broker).await;
    let (_, a) = document();
    let (_, b) = document();
    let (_, c) = document();
    let results = session.sign(vec![a, b, c]).await.unwrap();
    assert_eq!(
        results,
        vec![SignResult::Cms(b"S0".to_vec()), SignResult::Cms(b"S1".to_vec()), SignResult::Cms(b"S2".to_vec())]
    );
    assert_eq!(forms(&broker).len(), 1, "eine Anmeldung je Batch");
    let progress: Vec<_> =
        broker.calls().into_iter().filter(|c| matches!(c, BrokerCall::Progress(Progress::Document { .. }))).collect();
    assert_eq!(progress.len(), 3);
    assert_eq!(progress[2], BrokerCall::Progress(Progress::Document { index: 3, total: 3 }));
}

#[tokio::test]
async fn app_switch_to_fido2_and_poll_unknown_keeps_waiting() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            return MockResponse::redirect(302, "app.aspx?sid=S0");
        }
        if is(req, "GET", "app.aspx") {
            return page("app.html", &[("SID", "S0"), ("REF", "R"), ("POLLID", "1"), ("TOKEN", "t")]);
        }
        if req.path() == "/mobile/poll/LongPoll.ashx" {
            return MockResponse::json(r#"{"Fin":false,"Wait":false,"Error":false}"#);
        }
        if is(req, "GET", "fido.aspx") {
            return page("fido2.html", &[("SID", "S0"), ("OPTIONS", "{&quot;challenge&quot;:&quot;AQ&quot;}")]);
        }
        if is(req, "POST", "fido.aspx") {
            return infobox_xml();
        }
        MockResponse::status(404)
    })
    .await;
    let provider = FakeFido2::new(vec![Ok(FakeFido2::assertion())]);
    let backend = backend(&server, |c| c.fido2 = Some(provider.clone()));
    // Poll endet ohne Signal (Unknown) → Maske bleibt offen; dann wechselt die Nutzerin auf FIDO2
    let broker = Arc::new(SlowBroker {
        inner: ScriptedBroker::new([
            login_reply(PASSWORD, false),
            ScriptedReply::Form(FormAnswer::SwitchToFido2),
            ScriptedReply::Pending,
        ]),
        delay: Duration::from_millis(600),
    });
    let mut session = backend.open(ctx(), broker.clone()).await.unwrap();
    session.certificate().await.unwrap();
    assert_eq!(server.requests().iter().filter(|r| r.path() == "/mobile/poll/LongPoll.ashx").count(), 1);
    assert!(matches!(&forms(&broker.inner)[1], NativeForm::WaitForApp { .. }));
    // FIDO2-Seite erreicht, Assertion gesendet (die verzögerte Maske wurde vorher gedroppt)
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert!(server.requests().iter().any(|r| is(r, "POST", "fido.aspx")));
}

#[test]
fn signature_data_link_opens_only_atrust_urls() {
    let endpoint = pdfover_net::Url::parse(pdfover_backend_atrust::PROD_ENDPOINT).unwrap();
    let broker = ScriptedBroker::new([]);
    pdfover_backend_atrust::open_signature_data(
        &broker,
        "https://service.a-trust.at/mobile/https-security-layer-request/ShowSigobj.aspx?sid=1",
        &endpoint,
    )
    .unwrap();
    assert_eq!(
        broker.calls(),
        [BrokerCall::OpenExternal(
            "https://service.a-trust.at/mobile/https-security-layer-request/ShowSigobj.aspx?sid=1".into()
        )]
    );
    for bad in [
        "http://service.a-trust.at/ShowSigobj.aspx",
        "https://example.org/ShowSigobj.aspx",
        "javascript:alert(1)",
        "kein link",
    ] {
        assert!(
            matches!(
                pdfover_backend_atrust::open_signature_data(&broker, bad, &endpoint),
                Err(BackendError::Protocol(_))
            ),
            "{bad}"
        );
    }
    assert_eq!(broker.calls().len(), 1);
}

#[tokio::test]
async fn headless_is_unavailable() {
    let server = MockServer::start(|_| MockResponse::status(500)).await;
    let backend = backend(&server, |_| {});
    let mut context = ctx();
    context.headless = true;
    let Err(err) = backend.open(context, Arc::new(ScriptedBroker::new([]))).await else {
        panic!("headless darf nicht öffnen")
    };
    assert!(matches!(err, BackendError::Unavailable(_)));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn modell_a_jobs_are_rejected() {
    let server = MockServer::start(|_| MockResponse::status(500)).await;
    let backend = backend(&server, |_| {});
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([]))).await;
    let job = pdfover_backend_api::SignJob::Digest(pdfover_backend_api::DigestJob {
        display_name: "x".into(),
        alg: pdfover_backend_api::DigestAlg::Sha256,
        digest: vec![0; 32],
    });
    assert!(matches!(session.sign(vec![job]).await.unwrap_err(), BackendError::Internal(_)));
    assert!(server.requests().is_empty());
}

/// Zweiter Mock-Server (andere Origin = „fremder Host“), der Login-, SMS- und FIDO2-Seiten liefert.
async fn foreign_host() -> MockServer {
    MockServer::start(|req| {
        if req.path().ends_with("Login.aspx") {
            return page("login.html", &[("SID", "F")]);
        }
        if req.path().ends_with("sms.aspx") {
            return page("sms.html", &[("SID", "F"), ("REF", "AB12")]);
        }
        if req.path().ends_with("fido.aspx") {
            return page("fido2.html", &[("SID", "F"), ("OPTIONS", "{&quot;challenge&quot;:&quot;AQ&quot;}")]);
        }
        MockResponse::status(404)
    })
    .await
}

#[tokio::test]
async fn credentials_tan_and_fido2_never_go_to_foreign_hosts() {
    let foreign = foreign_host().await;
    for target in ["Login.aspx", "sms.aspx", "fido.aspx"] {
        let location = foreign.url(&p(&format!("{target}?sid=F"))).to_string();
        let server = MockServer::start(move |req| {
            if is(req, "POST", "default.aspx") {
                return MockResponse::redirect(302, &location);
            }
            MockResponse::status(404)
        })
        .await;
        let provider = FakeFido2::new(vec![Ok(FakeFido2::assertion())]);
        let backend = backend(&server, |c| {
            c.prefill_username = Some(USER.into());
            c.prefill_password = Some(Zeroizing::new(PASSWORD.into()));
            c.fido2 = Some(provider.clone());
        });
        let broker = Arc::new(ScriptedBroker::new([tan("123456"), ScriptedReply::Pending]));
        let mut session = open(&backend, &broker).await;
        let err = session.certificate().await.unwrap_err();
        assert!(
            matches!(&err, BackendError::Protocol(m) if m.contains("gehört nicht zu A-Trust")),
            "{target}: {err:?}"
        );
        assert!(forms(&broker).is_empty(), "{target}: keine Maske für eine fremde Seite");
        assert!(provider.requests.lock().unwrap().is_empty(), "{target}: keine FIDO2-Assertion");
    }
    // der fremde Host hat nur GETs gesehen – weder Passwort noch TAN noch Assertion
    assert!(foreign.requests().iter().all(|r| r.method == "GET" && r.body.is_empty()));
    assert_eq!(foreign.requests().len(), 3);
}

#[tokio::test]
async fn form_action_to_foreign_host_is_refused() {
    let foreign = foreign_host().await;
    let action = foreign.url(&p("Login.aspx?sid=F")).to_string();
    let server = MockServer::start(move |req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return MockResponse::html(fixture("login.html", &[("SID", "S0")]).replace("./Login.aspx?sid=S0", &action));
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |c| {
        c.prefill_username = Some(USER.into());
        c.prefill_password = Some(Zeroizing::new(PASSWORD.into()));
    });
    let broker = Arc::new(ScriptedBroker::new([]));
    let mut session = open(&backend, &broker).await;
    assert!(matches!(session.certificate().await.unwrap_err(), BackendError::Protocol(_)));
    assert!(foreign.requests().is_empty(), "Zugangsdaten nicht an das fremde Formularziel");
}

#[tokio::test]
async fn base_href_cannot_fake_the_page_origin() {
    // Die fremde Seite gibt sich per <base href> als Endpunkt-Origin aus; maßgeblich ist die URL,
    // von der sie geladen wurde.
    let server_url = Arc::new(Mutex::new(String::new()));
    let foreign = MockServer::start({
        let server_url = server_url.clone();
        move |_| {
            let base = format!("<head><base href=\"{}\"></head>", server_url.lock().unwrap());
            MockResponse::html(
                fixture("fido2.html", &[("SID", "F"), ("OPTIONS", "{&quot;challenge&quot;:&quot;AQ&quot;}")])
                    .replace("<head>", &base),
            )
        }
    })
    .await;
    let location = foreign.url(&p("fido.aspx?sid=F")).to_string();
    let server = MockServer::start(move |req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, &location);
        }
        MockResponse::status(404)
    })
    .await;
    *server_url.lock().unwrap() = server.url(&p("fido.aspx")).to_string();
    let provider = FakeFido2::new(vec![Ok(FakeFido2::assertion())]);
    let backend = backend(&server, |c| c.fido2 = Some(provider.clone()));
    let mut session = open(&backend, &Arc::new(ScriptedBroker::new([ScriptedReply::Pending]))).await;
    assert!(matches!(session.certificate().await.unwrap_err(), BackendError::Protocol(_)));
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn login_cancel_without_cancel_button_ends_immediately() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return MockResponse::html(
                fixture("login.html", &[("SID", "S0")]).replace("id=\"Button_Cancel\"", "id=\"Button_Abbruch\""),
            );
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert_eq!(forms(&broker).len(), 1, "keine Schleife aus Abbruch und erneuter Maske");
    assert!(!server.requests().iter().any(|r| is(r, "POST", "Login.aspx")));
}

#[tokio::test]
async fn error_page_cancel_without_submittable_cancel_button_ends_immediately() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, "error.aspx?sid=S0");
        }
        if is(req, "GET", "error.aspx") {
            // #Button_Cancel existiert (requiresResponse), ist aber kein <input type=submit>
            let buttons = "<button id=\"Button_Cancel\" name=\"c\">Abbrechen</button>";
            return page("error.html", &[("ACTION", "error.aspx?sid=S0"), ("BUTTONS", buttons)]);
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Notice(NoticeAnswer::Ok)]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    assert!(!server.requests().iter().any(|r| r.method == "POST" && r.path() == p("error.aspx")));
}

#[tokio::test]
async fn app_page_without_poll_script_is_unknown_like_pdf_over() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, "app.aspx?sid=S0");
        }
        page("app.html", &[("SID", "S0"), ("REF", "R"), ("POLLID", "1"), ("TOKEN", "t")])
            .with_body(fixture("app.html", &[("SID", "S0"), ("REF", "R")]).replace("qrpoll(", "x("))
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([]));
    let mut session = open(&backend, &broker).await;
    assert!(matches!(session.certificate().await.unwrap_err(), BackendError::UnknownPage { .. }));
    assert!(forms(&broker).is_empty());
}

#[tokio::test]
async fn rejected_cli_password_is_not_resent_in_the_next_session() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return start_session(0);
        }
        if is(req, "GET", "Login.aspx") {
            return page("login.html", &[("SID", "S0")]);
        }
        if is(req, "POST", "Login.aspx") {
            if req.part("ctl00$Main$Button_Cancel").is_some() {
                return sl_error_xml(6001);
            }
            return page("login.html", &[("SID", "S0")]); // Passwort abgelehnt
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |c| {
        c.prefill_username = Some(USER.into());
        c.prefill_password = Some(Zeroizing::new("falsch-cli".into()));
    });
    // 1. Sitzung: -p wird einmal gesendet, abgelehnt, die Nutzerin bricht ab
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    // 2. Sitzung: das abgelehnte Passwort wird nicht erneut automatisch gesendet
    let broker = Arc::new(ScriptedBroker::new([ScriptedReply::Cancel]));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::Cancelled);
    match &forms(&broker)[..] {
        [NativeForm::Login { prefill_username, ctx, .. }] => {
            assert_eq!(prefill_username.as_deref(), Some(USER), "Benutzername bleibt");
            assert_eq!(ctx.problem, None);
        }
        other => panic!("{other:?}"),
    }
    let logins: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|r| is(r, "POST", "Login.aspx") && r.part("ctl00$Main$Button_Identification").is_some())
        .collect();
    assert_eq!(logins.len(), 1, "das -p-Passwort wurde genau einmal gesendet");
}

#[tokio::test]
async fn signature_data_link_is_offered_only_for_https_atrust_urls() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, "sms.aspx?sid=S0");
        }
        if is(req, "GET", "sms.aspx") {
            return MockResponse::html(fixture("sms.html", &[("SID", "S0"), ("REF", "AB12")]).replace(
                "href=\"ShowSigobj.aspx?sid=S0&amp;doc=1\"",
                "href=\"https://service.a-trust.at/mobile/ShowSigobj.aspx?sid=S0&amp;doc=1\"",
            ));
        }
        if is(req, "POST", "sms.aspx") {
            return infobox_xml();
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |_| {});
    let broker = Arc::new(ScriptedBroker::new([tan("123456")]));
    let mut session = open(&backend, &broker).await;
    session.certificate().await.unwrap();
    match &forms(&broker)[0] {
        NativeForm::SmsTan { ctx, .. } => assert_eq!(
            ctx.signature_data_url.as_deref(),
            Some("https://service.a-trust.at/mobile/ShowSigobj.aspx?sid=S0&doc=1")
        ),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn redirect_and_autoskip_loops_end_even_when_the_server_is_slow() {
    // Ein langsamer Server (> 40 ms je Antwort) löst den PDF-Over-Loop-Guard (50 in 2 s) nie aus;
    // hier wird er abgeschaltet, damit der Test schnell bleibt – es greifen die zusätzlichen Grenzen.
    let relaxed = |c: &mut pdfover_backend_atrust::AtrustConfig| {
        c.loop_guard =
            pdfover_backend_atrust::LoopGuardConfig { window: Duration::from_secs(2), max_requests: 100_000 };
    };
    let server = MockServer::start(|_| MockResponse::redirect(302, "loop.aspx")).await;
    let first = backend(&server, relaxed);
    let mut session = open(&first, &Arc::new(ScriptedBroker::new([]))).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::LoopDetected);
    assert_eq!(server.requests().len(), 101, "Initial-Request + 100 Weiterleitungen");

    // AutoSkip-Seite, die sich nach dem Absenden selbst wieder liefert
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, "UsernameInfo.aspx?sid=S0");
        }
        page("usernameinfo.html", &[("ACTION", "UsernameInfo.aspx?sid=S0")])
    })
    .await;
    let second = backend(&server, relaxed);
    let mut session = open(&second, &Arc::new(ScriptedBroker::new([]))).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::LoopDetected);
    let posts = server.requests().iter().filter(|r| is(r, "POST", "UsernameInfo.aspx")).count();
    assert_eq!(posts, 20);
}

#[tokio::test]
async fn poll_reporting_finished_forever_does_not_loop_endlessly() {
    let server = MockServer::start(|req| {
        if is(req, "POST", "default.aspx") {
            return MockResponse::redirect(302, "app.aspx?sid=S0");
        }
        if is(req, "GET", "app.aspx") {
            return page("app.html", &[("SID", "S0"), ("REF", "R"), ("POLLID", "1"), ("TOKEN", "t")]);
        }
        if req.path() == "/mobile/poll/LongPoll.ashx" {
            return MockResponse::json(r#"{"Fin":true,"Wait":false,"Error":false}"#);
        }
        MockResponse::status(404)
    })
    .await;
    let backend = backend(&server, |c| {
        c.loop_guard =
            pdfover_backend_atrust::LoopGuardConfig { window: Duration::from_secs(2), max_requests: 100_000 };
    });
    let replies: Vec<ScriptedReply> = (0..40).map(|_| ScriptedReply::Pending).collect();
    let broker = Arc::new(ScriptedBroker::new(replies));
    let mut session = open(&backend, &broker).await;
    assert_eq!(session.certificate().await.unwrap_err(), BackendError::LoopDetected);
    let reloads = server.requests().iter().filter(|r| is(r, "GET", "app.aspx")).count();
    assert_eq!(reloads, 31, "erste Anzeige + 30 Neuladungen");
}

/// Provider, dessen Schlüssel erst später „eingesteckt“ wird.
#[derive(Debug)]
struct LatePlugged {
    plugged: std::sync::atomic::AtomicBool,
    inner: Arc<FakeFido2>,
}

impl pdfover_backend_atrust::Fido2Provider for LatePlugged {
    fn is_available(&self) -> bool {
        self.plugged.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn get_assertion(
        &self,
        request: &pdfover_backend_atrust::Fido2Request,
        parent: Option<pdfover_backend_api::WindowHandle>,
    ) -> Result<pdfover_backend_atrust::Fido2Assertion, Fido2Error> {
        self.inner.get_assertion(request, parent)
    }
    fn cancel(&self) {
        self.inner.cancel()
    }
}

/// Broker, der beim ersten FIDO2-Hinweis den Schlüssel „einsteckt“ und „erneut versuchen“ wählt.
struct PlugInBroker {
    inner: ScriptedBroker,
    provider: Arc<LatePlugged>,
}

#[async_trait::async_trait]
impl pdfover_backend_api::InteractionBroker for PlugInBroker {
    async fn native_form(&self, form: NativeForm) -> Result<FormAnswer, pdfover_backend_api::InteractionError> {
        if matches!(&form, NativeForm::Fido2 { in_progress: false, .. }) {
            self.provider.plugged.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        self.inner.native_form(form).await
    }
    async fn secret(
        &self,
        prompt: pdfover_backend_api::SecretPrompt,
    ) -> Result<Zeroizing<String>, pdfover_backend_api::InteractionError> {
        self.inner.secret(prompt).await
    }
    async fn notice(
        &self,
        notice: pdfover_backend_api::Notice,
    ) -> Result<NoticeAnswer, pdfover_backend_api::InteractionError> {
        self.inner.notice(notice).await
    }
    fn progress(&self, progress: Progress) {
        self.inner.progress(progress)
    }
    fn event(&self, event: BackendEvent) {
        self.inner.event(event)
    }
    fn open_external(&self, url: &str) -> Result<(), pdfover_backend_api::InteractionError> {
        self.inner.open_external(url)
    }
}

#[tokio::test]
async fn security_key_plugged_in_later_can_be_used_after_retry() {
    let server = MockServer::start(fido_server(false)).await;
    let provider = Arc::new(LatePlugged {
        plugged: std::sync::atomic::AtomicBool::new(false),
        inner: FakeFido2::new(vec![Ok(FakeFido2::assertion())]),
    });
    let backend = backend(&server, |c| c.fido2 = Some(provider.clone()));
    let broker = Arc::new(PlugInBroker {
        inner: ScriptedBroker::new([
            login_reply(PASSWORD, false),
            ScriptedReply::Form(FormAnswer::StartFido2),
            ScriptedReply::Pending,
        ]),
        provider: provider.clone(),
    });
    let mut session = backend.open(ctx(), broker.clone()).await.unwrap();
    session.certificate().await.unwrap();
    let f = forms(&broker.inner);
    match &f[1] {
        NativeForm::Fido2 { in_progress: false, ctx } => {
            assert!(ctx.fido2_visible, "Plattform kann FIDO2, der Knopf bleibt sichtbar");
            assert!(matches!(ctx.problem, Some(FormProblem::Fido2Failed(_))));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(&f[2], NativeForm::Fido2 { in_progress: true, .. }));
    assert_eq!(provider.inner.requests.lock().unwrap().len(), 1);
    assert!(server.requests().iter().any(|r| is(r, "POST", "fido.aspx")));
}

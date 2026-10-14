//! Long-Poll-Schleife gegen den Mock-Server (docs/analyse/02 §3.6).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use pdfover_backend_api::CancelToken;
use pdfover_backend_atrust::poll::{MIN_POLL_INTERVAL, PollConfig, PollInfo, PollOutcome, run};
use pdfover_net::testing::{MockResponse, MockServer};
use pdfover_net::{HttpClient, NetConfig, ProxyConfig, Purpose};

fn client(timeout: Option<Duration>) -> HttpClient {
    let cfg = NetConfig { proxy: ProxyConfig::None, request_timeout: timeout, ..NetConfig::default() };
    HttpClient::new(&cfg, Purpose::AtrustPoll).unwrap()
}

fn cfg(total_ms: u64, backoff_ms: u64) -> PollConfig {
    PollConfig {
        total_timeout: Duration::from_millis(total_ms),
        error_backoff: Duration::from_millis(backoff_ms),
        after_finish: Duration::from_millis(1),
    }
}

async fn server_with(responses: Vec<MockResponse>) -> (MockServer, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let server = MockServer::start(move |_| {
        let i = c.fetch_add(1, Ordering::SeqCst);
        responses.get(i).cloned().unwrap_or_else(|| responses.last().cloned().unwrap())
    })
    .await;
    (server, count)
}

fn info(server: &MockServer) -> PollInfo {
    PollInfo { url: server.url("/poll"), token: "tok".into() }
}

#[tokio::test]
async fn wait_wait_fin() {
    let (server, count) = server_with(vec![
        MockResponse::json(r#"{"Fin":false,"Wait":true,"Error":false}"#),
        MockResponse::json(r#"{"Fin":false,"Wait":true,"Error":false}"#),
        MockResponse::json(r#"{"Fin":true}"#),
    ])
    .await;
    let started = Instant::now();
    let outcome = run(&client(None), &info(&server), cfg(10_000, 50), &CancelToken::new()).await;
    assert_eq!(outcome, PollOutcome::Reload);
    assert_eq!(count.load(Ordering::SeqCst), 3);
    assert!(started.elapsed() >= MIN_POLL_INTERVAL * 2, "Mindestabstand zwischen Polls");
    for r in server.requests() {
        assert_eq!(r.header("authorization"), Some("Bearer tok"));
        assert_eq!(r.method, "GET");
    }
}

#[tokio::test]
async fn error_flag_reloads_and_unknown_stops() {
    let (server, _) = server_with(vec![MockResponse::json(r#"{"Fin":false,"Wait":false,"Error":true}"#)]).await;
    assert_eq!(run(&client(None), &info(&server), cfg(10_000, 50), &CancelToken::new()).await, PollOutcome::Reload);
    let (server, count) = server_with(vec![MockResponse::json(r#"{"Fin":false,"Wait":false,"Error":false}"#)]).await;
    assert_eq!(run(&client(None), &info(&server), cfg(10_000, 50), &CancelToken::new()).await, PollOutcome::Stopped);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn malformed_response_backs_off_then_reloads() {
    let (server, count) = server_with(vec![MockResponse::html("<html>Fehler</html>").with_header("X", "y")]).await;
    let started = Instant::now();
    assert_eq!(run(&client(None), &info(&server), cfg(10_000, 300), &CancelToken::new()).await, PollOutcome::Reload);
    assert!(started.elapsed() >= Duration::from_millis(300));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    // HTTP-Status wird wie in PDF-Over nicht geprüft: 500 mit JSON {"Fin":true} zählt als fertig
    let (server, _) = server_with(vec![MockResponse::status(500).with_body(r#"{"Fin":true}"#)]).await;
    assert_eq!(run(&client(None), &info(&server), cfg(10_000, 300), &CancelToken::new()).await, PollOutcome::Reload);
}

#[tokio::test]
async fn no_response_keeps_polling_until_deadline() {
    let (server, count) = server_with(vec![MockResponse::no_response()]).await;
    let started = Instant::now();
    assert_eq!(run(&client(None), &info(&server), cfg(900, 5_000), &CancelToken::new()).await, PollOutcome::Reload);
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(900) && elapsed < Duration::from_secs(4), "{elapsed:?}");
    assert!(count.load(Ordering::SeqCst) >= 3, "weiter gepollt");
}

#[tokio::test]
async fn request_timeout_counts_like_no_response() {
    let (server, count) = server_with(vec![
        MockResponse::json(r#"{"Fin":true}"#).with_delay(Duration::from_secs(2)),
        MockResponse::json(r#"{"Fin":true}"#),
    ])
    .await;
    // Timeout vor Ablauf der Frist → erneut pollen, zweite Antwort ist „fertig“
    let outcome =
        run(&client(Some(Duration::from_millis(200))), &info(&server), cfg(10_000, 5_000), &CancelToken::new()).await;
    assert_eq!(outcome, PollOutcome::Reload);
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn network_error_backs_off_then_reloads() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let info = PollInfo { url: pdfover_net::Url::parse(&format!("http://{addr}/poll")).unwrap(), token: "t".into() };
    let started = Instant::now();
    assert_eq!(run(&client(None), &info, cfg(10_000, 200), &CancelToken::new()).await, PollOutcome::Reload);
    assert!(started.elapsed() >= Duration::from_millis(200));
}

#[tokio::test]
async fn cancel_stops_immediately() {
    let (server, _) =
        server_with(vec![MockResponse::json(r#"{"Wait":true,"Fin":false}"#).with_delay(Duration::from_secs(5))]).await;
    let cancel = CancelToken::new();
    let c = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        c.cancel();
    });
    let started = Instant::now();
    assert_eq!(run(&client(None), &info(&server), cfg(10_000, 50), &cancel).await, PollOutcome::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
    // bereits abgebrochen: kein Request
    let (server, count) = server_with(vec![MockResponse::json("{}")]).await;
    assert_eq!(run(&client(None), &info(&server), cfg(10_000, 50), &cancel).await, PollOutcome::Cancelled);
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

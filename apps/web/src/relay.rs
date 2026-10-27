//! SL-Transport zur Seite ([`pida_backend_atrust_web::SlRelay`]): Der Kern übergibt jeden Befehl an den
//! JavaScript-Rückruf `sl(art, id, xml)`; die Seite zeigt das A-Trust-Overlay bzw. spricht mit dem Rückkanal und
//! meldet die Antwort über [`sl_resolve`]. `art` = `exchange` (Befehl `xml`, Antwort zu `id` erwartet) oder
//! `close` (`xml` = `done`/`aborted`).

use std::cell::RefCell;
use std::collections::HashMap;

use async_trait::async_trait;
use pida_backend_atrust_web::{CloseOutcome, RelayError, SlRelay};
use tokio::sync::oneshot;
use wasm_bindgen::prelude::*;

type Reply = Result<Vec<u8>, RelayError>;

thread_local! {
    static SL: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    static PENDING: RefCell<HashMap<u32, oneshot::Sender<Reply>>> = RefCell::new(HashMap::new());
    static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
}

pub(crate) fn install(sl: js_sys::Function) {
    SL.with(|s| *s.borrow_mut() = Some(sl));
}

fn call(kind: &str, id: u32, payload: &str) -> Result<(), RelayError> {
    SL.with(|s| match s.borrow().as_ref() {
        Some(f) => f
            .call3(&JsValue::NULL, &JsValue::from_str(kind), &JsValue::from(id), &JsValue::from_str(payload))
            .map(|_| ())
            .map_err(|_| RelayError::Failed("SL-Transport der Seite fehlgeschlagen".into())),
        None => Err(RelayError::Failed("kein SL-Transport".into())),
    })
}

/// Antwort der Seite auf `sl("exchange", id, …)`: `ok` mit der SL-Antwort (`XMLResponse`) bzw. Fehler `error` =
/// `cancelled` (Overlay geschlossen), `timeout` oder ein Text.
#[wasm_bindgen]
pub fn sl_resolve(id: u32, ok: bool, response: &[u8], error: &str) {
    let Some(tx) = PENDING.with(|p| p.borrow_mut().remove(&id)) else { return };
    let reply = if ok {
        Ok(response.to_vec())
    } else {
        Err(match error {
            "cancelled" => RelayError::Cancelled,
            "timeout" => RelayError::Timeout,
            other => RelayError::Failed(other.to_owned()),
        })
    };
    let _ = tx.send(reply);
}

/// Transport über die Seite (ein Kern je Seite; Zustand in `thread_local`, daher `Send`/`Sync` ohne Felder).
#[derive(Debug, Clone, Copy)]
pub(crate) struct WebSlRelay;

#[async_trait]
impl SlRelay for WebSlRelay {
    async fn exchange(&self, request: String) -> Result<Vec<u8>, RelayError> {
        let id = NEXT.with(|n| {
            let id = n.get();
            n.set(id.wrapping_add(1).max(1));
            id
        });
        let (tx, rx) = oneshot::channel();
        PENDING.with(|p| p.borrow_mut().insert(id, tx));
        if let Err(e) = call("exchange", id, &request) {
            PENDING.with(|p| p.borrow_mut().remove(&id));
            return Err(e);
        }
        // Drop (Abbruch im Kern): Eintrag entfernen, eine späte Antwort der Seite läuft ins Leere
        struct Forget(u32);
        impl Drop for Forget {
            fn drop(&mut self) {
                PENDING.with(|p| p.borrow_mut().remove(&self.0));
            }
        }
        let _forget = Forget(id);
        rx.await.unwrap_or(Err(RelayError::Cancelled))
    }

    fn close(&self, outcome: CloseOutcome) {
        let kind = match outcome {
            CloseOutcome::Done => "done",
            CloseOutcome::Aborted => "aborted",
        };
        let _ = call("close", 0, kind);
    }
}

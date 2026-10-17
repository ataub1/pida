//! [`InteractionBroker`] des Kerns: Masken, Passwortabfragen und Meldungen der Backends werden zu
//! [`InteractionView`]s im `ViewState`; die Antworten kommen als `UiEvent` zurück
//! (Plan §3.4, ADR 0001 §5.5/§8.6).
//!
//! - Jede Anfrage bekommt eine ID; die UI antwortet mit `SubmitForm`/`SubmitSecret`/`AnswerNotice`.
//! - Droppt ein Backend das Future (Long-Poll meldet „fertig“, R7), verschwindet die Maske aus dem
//!   `ViewState` (Guard beim Drop).
//! - Ohne Oberfläche ([`CoreBroker::new`] mit `interactive = false`) liefert jede Abfrage
//!   [`InteractionError::Unsupported`].
//! - Passwörter und TANs liegen nur als `Zeroizing` im RAM und erscheinen nie in Logs oder `Debug`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use pida_backend_api::{
    BackendEvent, FormAnswer, FormContext, InteractionBroker, InteractionError, NativeForm, Notice, NoticeAnswer,
    Progress, SecretPrompt, WindowHandle,
};
use tokio::sync::oneshot;
use zeroize::Zeroizing;

use crate::view::{FormAnswerView, InteractionView};

/// Rückkanal zum Kern-Task.
pub(crate) trait BrokerSink: Send + Sync {
    /// Der Interaktionszustand hat sich geändert (ViewState neu veröffentlichen).
    fn changed(&self);
    /// Backend-Ereignis (Persistenz liegt beim Kern).
    fn backend_event(&self, event: BackendEvent);
}

/// Shell-Funktionen, die der Kern nicht selbst hat (System-Opener, Fensterhandle, Mobile: Teilen und
/// Exportieren). Fehlertexte enthalten keine Pfade oder URLs (Regel 9).
pub trait ShellHooks: Send + Sync {
    /// Öffnet eine Datei mit der Standardanwendung (F-87; Mobile: QuickLook bzw. `ACTION_VIEW`).
    fn open_path(&self, path: &std::path::Path) -> Result<(), String>;
    /// Öffnet eine `https://`-URL im Systembrowser (F-68).
    fn open_url(&self, url: &str) -> Result<(), String>;
    /// Elternfenster für modale Systemdialoge (FIDO2 unter Windows).
    fn parent_window(&self) -> Option<WindowHandle> {
        None
    }
    /// Teilt eine Datei (Mobile: Share-Sheet `UIActivityViewController` bzw. `ACTION_SEND` mit
    /// `FileProvider`-URI). Blockiert höchstens bis das System-UI angezeigt ist. Standard: nicht
    /// unterstützt (Desktop).
    fn share_path(&self, path: &std::path::Path) -> Result<(), String> {
        let _ = path;
        Err("unsupported".into())
    }
    /// Exportiert eine Datei („Sichern unter“: `UIDocumentPickerViewController(forExporting:)` bzw. SAF
    /// `ACTION_CREATE_DOCUMENT` mit `file_name` als Vorschlag). Blockiert, bis die Nutzerin gewählt hat;
    /// `Ok(Some(name))` = gesichert unter dem tatsächlichen Dateinamen (von der Nutzerin geändert bzw. vom System
    /// vergeben, z. B. „Vertrag_signed (3).pdf“; kennt die Shell ihn nicht, der Vorschlag), `Ok(None)` =
    /// abgebrochen. Standard: nicht unterstützt (Desktop: Speichern-Dialog des Kerns).
    fn export_path(&self, path: &std::path::Path, file_name: &str) -> Result<Option<String>, String> {
        let _ = (path, file_name);
        Err("unsupported".into())
    }
    /// Zeigt eine Datei im Dateimanager (Finder bzw. Explorer; „Zuletzt signiert“ › „Im Ordner zeigen“, ADR 0004
    /// Nachtrag 15). Standard: nicht unterstützt (Mobile).
    fn reveal_path(&self, path: &std::path::Path) -> Result<(), String> {
        let _ = path;
        Err("unsupported".into())
    }
}

enum Reply {
    Form(oneshot::Sender<Result<FormAnswer, InteractionError>>),
    Secret(oneshot::Sender<Result<Zeroizing<String>, InteractionError>>),
    Notice(oneshot::Sender<Result<NoticeAnswer, InteractionError>>),
}

struct Pending {
    view: InteractionView,
    reply: Reply,
}

#[derive(Default)]
struct Inner {
    pending: BTreeMap<u64, Pending>,
    progress: Option<Progress>,
}

/// Broker des Kerns.
pub(crate) struct CoreBroker {
    inner: Mutex<Inner>,
    next_id: AtomicU64,
    interactive: bool,
    sink: Arc<dyn BrokerSink>,
    shell: Mutex<Option<Arc<dyn ShellHooks>>>,
}

impl std::fmt::Debug for CoreBroker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pending = self.inner.lock().map(|i| i.pending.len()).unwrap_or(0);
        f.debug_struct("CoreBroker").field("interactive", &self.interactive).field("pending", &pending).finish()
    }
}

/// Entfernt die Anfrage beim Drop (Long-Poll fertig bzw. beantwortet).
struct RemoveOnDrop<'a> {
    broker: &'a CoreBroker,
    id: u64,
}

impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) {
        let removed = self.broker.lock().pending.remove(&self.id).is_some();
        if removed {
            self.broker.sink.changed();
        }
    }
}

impl CoreBroker {
    pub(crate) fn new(interactive: bool, sink: Arc<dyn BrokerSink>) -> CoreBroker {
        CoreBroker {
            inner: Mutex::new(Inner::default()),
            next_id: AtomicU64::new(1),
            interactive,
            sink,
            shell: Mutex::new(None),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn set_shell(&self, shell: Option<Arc<dyn ShellHooks>>) {
        *self.shell.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = shell;
    }

    fn shell(&self) -> Option<Arc<dyn ShellHooks>> {
        self.shell.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    pub(crate) fn interactive(&self) -> bool {
        self.interactive
    }

    /// Aktuelle Interaktion (jüngste offene Anfrage, sonst Fortschritt).
    pub(crate) fn current(&self) -> InteractionView {
        let inner = self.lock();
        if let Some((_, p)) = inner.pending.iter().next_back() {
            return p.view.clone();
        }
        InteractionView::Waiting { progress: inner.progress.clone().unwrap_or(Progress::Connecting) }
    }

    /// Gibt es eine offene Anfrage? (Tests)
    #[cfg(test)]
    pub(crate) fn has_pending(&self) -> bool {
        !self.lock().pending.is_empty()
    }

    /// Fortschritt zurücksetzen (neuer Signiervorgang).
    pub(crate) fn reset(&self) {
        let mut inner = self.lock();
        inner.progress = None;
        // offene Anfragen eines abgebrochenen Vorgangs schließen
        for (_, p) in std::mem::take(&mut inner.pending) {
            reply_cancelled(p.reply);
        }
    }

    /// Kontext der offenen Maske `request` (für „Signaturdaten anzeigen“).
    pub(crate) fn form_context(&self, request: u64) -> Option<FormContext> {
        let inner = self.lock();
        match &inner.pending.get(&request)?.view {
            InteractionView::Form { form, .. } => Some(form_ctx(form).clone()),
            _ => None,
        }
    }

    /// Maskenantwort; `false`, wenn keine passende Anfrage offen ist.
    pub(crate) fn answer_form(&self, request: u64, answer: FormAnswerView) -> bool {
        let p = self.take_if(request, |p| matches!(p.reply, Reply::Form(_)));
        match p.map(|p| p.reply) {
            Some(Reply::Form(tx)) => {
                let _ = tx.send(Ok(form_answer(answer)));
                self.sink.changed();
                true
            }
            _ => false,
        }
    }

    /// Passwortantwort.
    pub(crate) fn answer_secret(&self, request: u64, secret: Zeroizing<String>) -> bool {
        let p = self.take_if(request, |p| matches!(p.reply, Reply::Secret(_)));
        match p.map(|p| p.reply) {
            Some(Reply::Secret(tx)) => {
                let _ = tx.send(Ok(secret));
                self.sink.changed();
                true
            }
            _ => false,
        }
    }

    /// Antwort auf eine Meldung.
    pub(crate) fn answer_notice(&self, request: u64, answer: NoticeAnswer) -> bool {
        let p = self.take_if(request, |p| matches!(p.reply, Reply::Notice(_)));
        match p.map(|p| p.reply) {
            Some(Reply::Notice(tx)) => {
                let _ = tx.send(Ok(answer));
                self.sink.changed();
                true
            }
            _ => false,
        }
    }

    /// Mobile-Lebenszyklus: Wartet die jüngste Maske auf die App „ID Austria“ bzw. einen QR-Scan
    /// (Long-Poll im Hintergrund), wird sie mit „weiter“ beantwortet – das Backend lädt die Seite neu
    /// (R7) und startet damit einen frischen Long-Poll. Nach der Rückkehr aus der App ist die alte
    /// Verbindung oft vom System getrennt; ohne Neustart würde erst nach Fehler-Backoff bzw. 300-s-Frist
    /// weitergemacht. Liefert, ob neu gestartet wurde.
    pub(crate) fn restart_long_poll(&self) -> bool {
        let p = {
            let mut inner = self.lock();
            let waiting = inner.pending.iter().next_back().and_then(|(id, p)| match &p.view {
                InteractionView::Form { form: NativeForm::WaitForApp { .. } | NativeForm::QrCode { .. }, .. } => {
                    Some(*id)
                }
                _ => None,
            });
            let Some(id) = waiting else { return false };
            inner.pending.remove(&id)
        };
        match p.map(|p| p.reply) {
            Some(Reply::Form(tx)) => {
                let _ = tx.send(Ok(FormAnswer::Continue));
                self.sink.changed();
                true
            }
            _ => false,
        }
    }

    /// Bricht die jüngste offene Anfrage ab (`Cancel` der UI); `false`, wenn keine offen ist.
    pub(crate) fn cancel_current(&self) -> bool {
        let p = {
            let mut inner = self.lock();
            let Some(id) = inner.pending.keys().next_back().copied() else { return false };
            inner.pending.remove(&id)
        };
        if let Some(p) = p {
            reply_cancelled(p.reply);
            self.sink.changed();
        }
        true
    }

    fn take_if(&self, request: u64, pred: impl Fn(&Pending) -> bool) -> Option<Pending> {
        let mut inner = self.lock();
        if inner.pending.get(&request).is_some_and(pred) { inner.pending.remove(&request) } else { None }
    }

    fn register(&self, view: impl FnOnce(u64) -> InteractionView, reply: Reply) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.lock().pending.insert(id, Pending { view: view(id), reply });
        self.sink.changed();
        id
    }
}

fn reply_cancelled(reply: Reply) {
    match reply {
        Reply::Form(tx) => {
            let _ = tx.send(Err(InteractionError::Cancelled));
        }
        Reply::Secret(tx) => {
            let _ = tx.send(Err(InteractionError::Cancelled));
        }
        Reply::Notice(tx) => {
            let _ = tx.send(Err(InteractionError::Cancelled));
        }
    }
}

fn form_ctx(form: &NativeForm) -> &FormContext {
    match form {
        NativeForm::Login { ctx, .. }
        | NativeForm::SmsTan { ctx, .. }
        | NativeForm::QrCode { ctx, .. }
        | NativeForm::WaitForApp { ctx, .. }
        | NativeForm::Fido2 { ctx, .. } => ctx,
    }
}

/// serde-Antwort → `FormAnswer` (Passwort/TAN sofort in `Zeroizing`).
pub(crate) fn form_answer(answer: FormAnswerView) -> FormAnswer {
    match answer {
        FormAnswerView::Login { username, password, remember } => {
            FormAnswer::Login { username, password: Zeroizing::new(password), remember }
        }
        FormAnswerView::Tan { tan } => FormAnswer::Tan(Zeroizing::new(tan)),
        FormAnswerView::SwitchToSms => FormAnswer::SwitchToSms,
        FormAnswerView::SwitchToFido2 => FormAnswer::SwitchToFido2,
        FormAnswerView::StartFido2 => FormAnswer::StartFido2,
        FormAnswerView::Continue => FormAnswer::Continue,
    }
}

#[async_trait]
impl InteractionBroker for CoreBroker {
    async fn native_form(&self, form: NativeForm) -> Result<FormAnswer, InteractionError> {
        if !self.interactive {
            return Err(InteractionError::Unsupported);
        }
        let (tx, rx) = oneshot::channel();
        let id = self.register(|request| InteractionView::Form { request, form }, Reply::Form(tx));
        let _guard = RemoveOnDrop { broker: self, id };
        rx.await.unwrap_or(Err(InteractionError::Cancelled))
    }

    async fn secret(&self, prompt: SecretPrompt) -> Result<Zeroizing<String>, InteractionError> {
        if !self.interactive {
            return Err(InteractionError::Unsupported);
        }
        let (tx, rx) = oneshot::channel();
        // „Passwort merken?“ ergänzt die Projektion (`CoreTask::interaction_view`)
        let id = self.register(
            |request| InteractionView::Secret { request, prompt, ask_storage: false, keychain_available: false },
            Reply::Secret(tx),
        );
        let _guard = RemoveOnDrop { broker: self, id };
        rx.await.unwrap_or(Err(InteractionError::Cancelled))
    }

    async fn notice(&self, notice: Notice) -> Result<NoticeAnswer, InteractionError> {
        if !self.interactive {
            return Err(InteractionError::Unsupported);
        }
        let (tx, rx) = oneshot::channel();
        let id = self.register(|request| InteractionView::Notice { request, notice }, Reply::Notice(tx));
        let _guard = RemoveOnDrop { broker: self, id };
        rx.await.unwrap_or(Err(InteractionError::Cancelled))
    }

    fn progress(&self, progress: Progress) {
        self.lock().progress = Some(progress);
        self.sink.changed();
    }

    fn event(&self, event: BackendEvent) {
        self.sink.backend_event(event);
    }

    fn open_external(&self, url: &str) -> Result<(), InteractionError> {
        if !url.starts_with("https://") {
            return Err(InteractionError::Unsupported);
        }
        match self.shell() {
            Some(shell) => shell.open_url(url).map_err(|e| {
                tracing::warn!(error = %e, "URL konnte nicht geöffnet werden");
                InteractionError::Unsupported
            }),
            None => Err(InteractionError::Unsupported),
        }
    }

    fn parent_window(&self) -> Option<WindowHandle> {
        self.shell().and_then(|s| s.parent_window())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_backend_api::{NoticeButtons, NoticeCode, NoticeKind, SecretKind};
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct Sink {
        changed: AtomicUsize,
        events: Mutex<Vec<BackendEvent>>,
    }

    impl BrokerSink for Sink {
        fn changed(&self) {
            self.changed.fetch_add(1, Ordering::SeqCst);
        }
        fn backend_event(&self, event: BackendEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn broker() -> (Arc<CoreBroker>, Arc<Sink>) {
        let sink = Arc::new(Sink::default());
        (Arc::new(CoreBroker::new(true, sink.clone())), sink)
    }

    fn login_form() -> NativeForm {
        NativeForm::Login { prefill_username: None, remember: false, ctx: FormContext::default() }
    }

    #[tokio::test]
    async fn form_roundtrip_and_view() {
        let (b, sink) = broker();
        let b2 = b.clone();
        let task = tokio::spawn(async move { b2.native_form(login_form()).await });
        tokio::task::yield_now().await;
        while !b.has_pending() {
            tokio::task::yield_now().await;
        }
        let InteractionView::Form { request, .. } = b.current() else { panic!() };
        assert!(!b.answer_form(request + 100, FormAnswerView::Continue));
        let answer = FormAnswerView::Login { username: "u".into(), password: "geheim".into(), remember: true };
        assert!(b.answer_form(request, answer));
        let got = task.await.unwrap().unwrap();
        assert!(matches!(got, FormAnswer::Login { remember: true, .. }));
        assert!(!b.has_pending());
        assert!(sink.changed.load(Ordering::SeqCst) >= 2);
        assert!(matches!(b.current(), InteractionView::Waiting { .. }));
    }

    #[tokio::test]
    async fn dropped_form_disappears_long_poll() {
        let (b, _) = broker();
        let fut = b.native_form(login_form());
        let r = tokio::time::timeout(std::time::Duration::from_millis(10), fut).await;
        assert!(r.is_err());
        assert!(!b.has_pending(), "Maske geschlossen, sobald das Backend das Future droppt");
    }

    #[tokio::test]
    async fn cancel_current_cancels_secret() {
        let (b, _) = broker();
        let b2 = b.clone();
        let prompt = SecretPrompt { kind: SecretKind::KeystorePassword, subject: "k.p12".into(), retry: false };
        let task = tokio::spawn(async move { b2.secret(prompt).await });
        while !b.has_pending() {
            tokio::task::yield_now().await;
        }
        assert!(b.cancel_current());
        assert_eq!(task.await.unwrap(), Err(InteractionError::Cancelled));
        assert!(!b.cancel_current());
    }

    #[tokio::test]
    async fn notices_and_progress() {
        let (b, _) = broker();
        b.progress(Progress::Signing);
        assert_eq!(b.current(), InteractionView::Waiting { progress: Progress::Signing });
        let b2 = b.clone();
        let n = Notice {
            kind: NoticeKind::Error,
            code: NoticeCode::LocalBkuNotReachable,
            text: None,
            buttons: NoticeButtons::RetryCancel,
        };
        let task = tokio::spawn(async move { b2.notice(n).await });
        while !b.has_pending() {
            tokio::task::yield_now().await;
        }
        let InteractionView::Notice { request, .. } = b.current() else { panic!() };
        assert!(b.answer_notice(request, NoticeAnswer::Retry));
        assert_eq!(task.await.unwrap(), Ok(NoticeAnswer::Retry));
    }

    #[tokio::test]
    async fn non_interactive_broker_refuses() {
        let sink = Arc::new(Sink::default());
        let b = CoreBroker::new(false, sink);
        assert_eq!(b.native_form(login_form()).await, Err(InteractionError::Unsupported));
        assert_eq!(b.open_external("http://x"), Err(InteractionError::Unsupported));
        assert_eq!(b.open_external("https://x"), Err(InteractionError::Unsupported));
    }

    /// Rückkehr aus der App „ID Austria“: nur eine wartende App-/QR-Maske wird mit „weiter“ beantwortet
    /// (Backend lädt neu und pollt frisch); Anmelde- und TAN-Masken bleiben unberührt.
    #[tokio::test]
    async fn restart_long_poll_answers_only_waiting_masks() {
        let (b, _) = broker();
        assert!(!b.restart_long_poll(), "nichts offen");
        let b2 = b.clone();
        let login = tokio::spawn(async move { b2.native_form(login_form()).await });
        while !b.has_pending() {
            tokio::task::yield_now().await;
        }
        assert!(!b.restart_long_poll(), "Anmeldung bleibt offen");
        assert!(b.cancel_current());
        let _ = login.await;
        let b3 = b.clone();
        let wait = tokio::spawn(async move {
            b3.native_form(NativeForm::WaitForApp {
                reference_value: Some("AB12".into()),
                timeout_secs: Some(300),
                ctx: FormContext::default(),
            })
            .await
        });
        while !b.has_pending() {
            tokio::task::yield_now().await;
        }
        assert!(b.restart_long_poll());
        assert_eq!(wait.await.unwrap(), Ok(FormAnswer::Continue));
        assert!(!b.has_pending());
    }

    #[test]
    fn shell_hooks_share_and_export_default_to_unsupported() {
        struct Desktop;
        impl ShellHooks for Desktop {
            fn open_path(&self, _: &std::path::Path) -> Result<(), String> {
                Ok(())
            }
            fn open_url(&self, _: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let p = std::path::Path::new("/x/a_signed.pdf");
        assert!(Desktop.share_path(p).is_err());
        assert!(Desktop.export_path(p, "a_signed.pdf").is_err());
    }

    #[test]
    fn form_answer_conversion_redacts() {
        let a = form_answer(FormAnswerView::Tan { tan: "123456".into() });
        assert_eq!(format!("{a:?}"), "Tan(***)");
    }
}

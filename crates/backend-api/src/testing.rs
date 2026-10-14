//! Test-Hilfen für Backends (Feature `testing`): skriptgesteuerter [`InteractionBroker`] und ein
//! Dokument im Speicher als [`DocumentSource`]. Keine Netz- oder UI-Abhängigkeit.

use std::collections::VecDeque;
use std::io::Read;
use std::sync::Mutex;

use async_trait::async_trait;
use zeroize::Zeroizing;

use crate::{
    BackendEvent, DocumentSource, ExcludedByteRange, FormAnswer, InteractionBroker, InteractionError, NativeForm,
    Notice, NoticeAnswer, Progress, SecretPrompt,
};

/// Vorgegebene Antwort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedReply {
    /// Antwort auf `native_form`
    Form(FormAnswer),
    /// Antwort auf `secret`
    Secret(String),
    /// Antwort auf `notice`
    Notice(NoticeAnswer),
    /// Abbruch durch die Nutzerin
    Cancel,
    /// Maske bleibt offen, bis das Backend das Future droppt (Long-Poll meldet „fertig“, R7);
    /// beim Drop wird [`BrokerCall::FormClosed`] protokolliert. Nur für `native_form`.
    Pending,
}

/// Protokollierter Aufruf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerCall {
    /// `native_form`
    Form(NativeForm),
    /// `secret`
    Secret(SecretPrompt),
    /// `notice`
    Notice(Notice),
    /// `progress`
    Progress(Progress),
    /// `event`
    Event(BackendEvent),
    /// `open_external`
    OpenExternal(String),
    /// Eine mit [`ScriptedReply::Pending`] offene Maske wurde vom Backend gedroppt (Dialog geschlossen).
    FormClosed,
}

/// Protokolliert beim Drop, dass eine offene Maske geschlossen wurde.
struct CloseGuard<'a> {
    calls: &'a Mutex<Vec<BrokerCall>>,
}

impl Drop for CloseGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut c) = self.calls.lock() {
            c.push(BrokerCall::FormClosed);
        }
    }
}

/// Broker, der Antworten aus einer Warteschlange liefert und alle Aufrufe protokolliert.
/// Passt eine Antwort nicht zur Anfrage oder ist die Warteschlange leer, liefert er
/// [`InteractionError::Unsupported`] (der Test sieht das im Protokoll).
#[derive(Debug, Default)]
pub struct ScriptedBroker {
    replies: Mutex<VecDeque<ScriptedReply>>,
    calls: Mutex<Vec<BrokerCall>>,
}

impl ScriptedBroker {
    /// Neuer Broker mit Antworten in Reihenfolge.
    pub fn new(replies: impl IntoIterator<Item = ScriptedReply>) -> Self {
        Self { replies: Mutex::new(replies.into_iter().collect()), calls: Mutex::new(Vec::new()) }
    }

    /// Bisherige Aufrufe.
    pub fn calls(&self) -> Vec<BrokerCall> {
        self.calls.lock().expect("poisoned").clone()
    }

    /// Noch nicht verbrauchte Antworten.
    pub fn remaining(&self) -> usize {
        self.replies.lock().expect("poisoned").len()
    }

    fn record(&self, call: BrokerCall) {
        self.calls.lock().expect("poisoned").push(call);
    }

    fn next(&self) -> Option<ScriptedReply> {
        self.replies.lock().expect("poisoned").pop_front()
    }
}

#[async_trait]
impl InteractionBroker for ScriptedBroker {
    async fn native_form(&self, form: NativeForm) -> Result<FormAnswer, InteractionError> {
        self.record(BrokerCall::Form(form));
        match self.next() {
            Some(ScriptedReply::Form(a)) => Ok(a),
            Some(ScriptedReply::Cancel) => Err(InteractionError::Cancelled),
            Some(ScriptedReply::Pending) => {
                let _guard = CloseGuard { calls: &self.calls };
                std::future::pending::<()>().await;
                unreachable!("pending() endet nie")
            }
            _ => Err(InteractionError::Unsupported),
        }
    }

    async fn secret(&self, prompt: SecretPrompt) -> Result<Zeroizing<String>, InteractionError> {
        self.record(BrokerCall::Secret(prompt));
        match self.next() {
            Some(ScriptedReply::Secret(s)) => Ok(Zeroizing::new(s)),
            Some(ScriptedReply::Cancel) => Err(InteractionError::Cancelled),
            _ => Err(InteractionError::Unsupported),
        }
    }

    async fn notice(&self, notice: Notice) -> Result<NoticeAnswer, InteractionError> {
        self.record(BrokerCall::Notice(notice));
        match self.next() {
            Some(ScriptedReply::Notice(a)) => Ok(a),
            Some(ScriptedReply::Cancel) => Err(InteractionError::Cancelled),
            _ => Err(InteractionError::Unsupported),
        }
    }

    fn progress(&self, progress: Progress) {
        self.record(BrokerCall::Progress(progress));
    }

    fn event(&self, event: BackendEvent) {
        self.record(BrokerCall::Event(event));
    }

    fn open_external(&self, url: &str) -> Result<(), InteractionError> {
        self.record(BrokerCall::OpenExternal(url.to_owned()));
        Ok(())
    }
}

/// Vorbereitetes Dokument im Speicher (Bytes sind bereits geschwärzt). `Debug` ohne Inhalt.
#[derive(Clone, PartialEq, Eq)]
pub struct MemoryDocument {
    /// Anzeigename
    pub name: String,
    /// Blackout-PDF
    pub blacked_out: Vec<u8>,
    /// ausgeschlossener Bereich
    pub excluded: ExcludedByteRange,
}

impl std::fmt::Debug for MemoryDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryDocument")
            .field("name", &self.name)
            .field("len", &self.blacked_out.len())
            .field("excluded", &self.excluded)
            .finish()
    }
}

impl DocumentSource for MemoryDocument {
    fn display_name(&self) -> &str {
        &self.name
    }

    fn len(&self) -> u64 {
        self.blacked_out.len() as u64
    }

    fn excluded_range(&self) -> ExcludedByteRange {
        self.excluded
    }

    fn open_blacked_out(&self) -> std::io::Result<Box<dyn Read + Send + '_>> {
        Ok(Box::new(std::io::Cursor::new(self.blacked_out.as_slice())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FormContext;

    #[tokio::test]
    async fn scripted_replies_in_order() {
        let b = ScriptedBroker::new([
            ScriptedReply::Form(FormAnswer::Continue),
            ScriptedReply::Secret("test".into()),
            ScriptedReply::Cancel,
        ]);
        let form = NativeForm::WaitForApp {
            reference_value: Some("AB12".into()),
            timeout_secs: Some(300),
            ctx: FormContext::default(),
        };
        assert_eq!(b.native_form(form.clone()).await, Ok(FormAnswer::Continue));
        let prompt = SecretPrompt { kind: crate::SecretKind::KeystorePassword, subject: "k.p12".into(), retry: false };
        assert_eq!(b.secret(prompt).await.unwrap().as_str(), "test");
        assert_eq!(b.native_form(form.clone()).await, Err(InteractionError::Cancelled));
        assert_eq!(b.remaining(), 0);
        assert_eq!(b.calls().len(), 3);
    }

    #[tokio::test]
    async fn pending_form_records_close_on_drop() {
        let b = ScriptedBroker::new([ScriptedReply::Pending]);
        let form = NativeForm::QrCode {
            reference_value: "AB12".into(),
            image: None,
            timeout_secs: None,
            ctx: FormContext::default(),
        };
        // Long-Poll „fertig“: das Backend wartet nicht länger auf die Maske und droppt das Future.
        let res = tokio::time::timeout(std::time::Duration::from_millis(20), b.native_form(form)).await;
        assert!(res.is_err(), "Pending darf nicht von selbst enden");
        assert_eq!(b.calls().last(), Some(&BrokerCall::FormClosed));
    }

    #[test]
    fn memory_document_reads_back() {
        let d = MemoryDocument {
            name: "x.pdf".into(),
            blacked_out: b"%PDF".to_vec(),
            excluded: ExcludedByteRange { from: 1, to: 2 },
        };
        assert_eq!(d.blacked_out_bytes().unwrap(), b"%PDF");
        assert_eq!(d.len(), 4);
    }
}

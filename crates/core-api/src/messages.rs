//! Meldungs-IDs (i18n) für Backend-Meldungen und Maskenhinweise – gemeinsam für Tauri-Shell und CLI
//! (docs/analyse/01 §4.10 Dialog-Katalog).

use std::collections::BTreeMap;

use pida_backend_api::{FormProblem, Notice, NoticeCode, Progress};

fn one(k: &str, v: impl Into<String>) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert(k.to_owned(), v.into());
    m
}

/// Meldung eines Backends → (Meldungs-ID, Parameter). Rohtexte von A-Trust stehen in `arg1`.
pub fn notice_message(n: &Notice) -> (String, BTreeMap<String, String>) {
    let text = n.text.clone().unwrap_or_default();
    let (id, params) = match &n.code {
        NoticeCode::AtrustInterstitial => ("common-info", one("arg1", text)),
        NoticeCode::AtrustError => ("atrusterror-message", one("arg1", text)),
        NoticeCode::ProxyAuthRequired => ("error-ProxyAuthRequired", BTreeMap::new()),
        NoticeCode::DocumentTooLarge => ("atrusterror-http_413", BTreeMap::new()),
        NoticeCode::Http { status } => {
            let mut m = one("arg1", status.to_string());
            m.insert("arg2".into(), text);
            ("atrusterror-http_generic", m)
        }
        NoticeCode::ConnectionFailed => ("error-FailedToConnect", one("arg1", text)),
        NoticeCode::LocalBkuNotReachable => ("error-LocalBKU", BTreeMap::new()),
        NoticeCode::KeystoreFileMissing => ("error-KeyStoreFileNotExist", one("arg1", text)),
        NoticeCode::KeystoreAliasMissing => ("error-KeyStoreAliasExist", one("arg1", text)),
        NoticeCode::KeystoreError => ("error-KeyStore", BTreeMap::new()),
        NoticeCode::KeystoreConversionNeeded => ("ng-migrate-keystore-jks", one("value", text)),
    };
    (id.to_owned(), params)
}

/// Hinweis in einer Maske → (Meldungs-ID, Parameter).
pub fn form_problem_message(p: &FormProblem) -> (String, BTreeMap<String, String>) {
    let (id, params) = match p {
        FormProblem::PasswordTooShort => ("exception-PasswordTooShort", BTreeMap::new()),
        FormProblem::PasswordTooLong => ("exception-PasswordTooLong", BTreeMap::new()),
        FormProblem::NoTan => ("error-NoTan", BTreeMap::new()),
        FormProblem::TanTooLong => ("error-TanTooLong", BTreeMap::new()),
        FormProblem::EnteredReferenceValue => ("error-EnteredReferenceValue", BTreeMap::new()),
        FormProblem::QrImageFailed => ("error-FailedToLoadQRCode", BTreeMap::new()),
        FormProblem::Fido2Failed(detail) => ("backend-fido2", one("detail", detail.clone())),
        FormProblem::Fido2Error { code, detail } => (code.as_str(), one("detail", detail.clone())),
        FormProblem::LoginRejected => ("ng-atrust-login-rejected", BTreeMap::new()),
        FormProblem::Atrust(text) => ("atrusterror-message", one("arg1", text.clone())),
    };
    (id.to_owned(), params)
}

/// Fortschritt → Meldungs-ID (Warte-Screen, Live-Region).
pub fn progress_message(p: &Progress) -> (String, BTreeMap<String, String>) {
    match p {
        Progress::Document { index, total } => {
            let mut m = one("index", index.to_string());
            m.insert("total".into(), total.to_string());
            ("ng-progress-document".into(), m)
        }
        Progress::Connecting => ("ng-progress-connecting".into(), BTreeMap::new()),
        Progress::ReadingCertificate => ("ng-progress-certificate".into(), BTreeMap::new()),
        Progress::WaitingForServer => ("ng-progress-waiting".into(), BTreeMap::new()),
        Progress::Signing => ("waiting-message".into(), BTreeMap::new()),
        Progress::Done => ("ng-progress-done".into(), BTreeMap::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pida_backend_api::{NoticeButtons, NoticeKind};

    #[test]
    fn notices_map_to_pdf_over_dialogs() {
        let n = Notice {
            kind: NoticeKind::Error,
            code: NoticeCode::Http { status: 500 },
            text: Some("Internal".into()),
            buttons: NoticeButtons::Ok,
        };
        let (id, p) = notice_message(&n);
        assert_eq!(id, "atrusterror-http_generic");
        assert_eq!(p["arg1"], "500");
        assert_eq!(p["arg2"], "Internal");
        let n = Notice { code: NoticeCode::LocalBkuNotReachable, text: None, ..n };
        assert_eq!(notice_message(&n).0, "error-LocalBKU");
        assert_eq!(form_problem_message(&FormProblem::NoTan).0, "error-NoTan");
        assert_eq!(progress_message(&Progress::Signing).0, "waiting-message");
    }
}

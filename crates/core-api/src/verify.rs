//! Prüfmodus im Kern (ADR 0004 §3.9, Paket K4).
//!
//! - `OpenVerify { path }` (bzw. `VerifyRecent`) lädt die Datei im Blocking-Pool (Grenzen wie beim Öffnen),
//!   prüft alle Signaturen mit `pades::verify_document` und legt das Dokument mit **eigener** `DocId` in den
//!   `DocStore` (Seitenkacheln über `pida-render://page/<doc>/<page>`), nicht in die Warteschlange. Der
//!   Workflow-Zustand bleibt unverändert; der Prüfmodus liegt darüber (`ViewState::verify`).
//! - Erlaubt in `Idle`, `Output`, `BatchOverview`, `Positioning` ([`verify_allowed`], `actions.verify`);
//!   während des Signierens Hinweis `ng-verify-busy`.
//! - `CloseVerify` gibt das Dokument frei; ein neuer `OpenVerify` ersetzt das vorige (verspätete Ergebnisse
//!   einer älteren Prüfung werden über eine Generation verworfen).
//! - `SignVerified` („Dieses Dokument signieren“) schließt den Prüfmodus und öffnet die Datei wie
//!   `open_files` – nur mit `actions.open_files`; ist ein Dokument offen, kommt sie in die Warteschlange
//!   (Hinweis `ng-batch-added`).
//! - Wortlaut (ADR 0004 §8 F3): PIDA behauptet nie „gültig“ bzw. „rechtsgültig“; geprüft sind Integrität und
//!   Zertifikatsangaben, **nicht** Vertrauenskette und Sperrstatus (`TrustCheck::NotChecked`), QES nur „laut
//!   Zertifikat“ (qcStatements).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pida_cades::{CmsWarning, KeyType, SignatureAlg};
use pida_pades::{Integrity, SignatureVerification, VerifyReport};
use pida_pdf_cos::{Limits, PageInfo, SourcePdf};
use pida_workflow::{DocId, Event, State};

use crate::docs::DocData;
use crate::runtime::{CoreTask, Msg};
use crate::view::{
    AppearanceView, DocInfo, QualificationClaim, SignatureCoverage, SignatureFormat, SignatureIntegrity, TrustCheck,
    VerifiedSignatureView, VerifyStatus, VerifySummaryView, VerifyView,
};

/// Meldungs-ID: während des Signierens kein Prüfmodus.
pub(crate) const MSG_VERIFY_BUSY: &str = "ng-verify-busy";
/// Meldungs-ID bzw. `VerifyStatus::Failed::code`: Datei nicht als PDF lesbar.
pub(crate) const MSG_VERIFY_OPEN_FAILED: &str = "ng-verify-open-failed";
/// Hinweise zu intakten Signaturen (i18n-IDs, `VerifiedSignatureView::warnings`).
pub(crate) const MSG_WARNING_SHA1: &str = "ng-verify-warning-sha1";
pub(crate) const MSG_WARNING_SIGNING_CERTIFICATE_V1: &str = "ng-verify-warning-signing-certificate-v1";
pub(crate) const MSG_WARNING_WEAK_KEY: &str = "ng-verify-warning-weak-key";

/// Prüfmodus erlaubt (ADR 0004 §3.9): Start, Arbeitsbereich, Fertig, Stapelübersicht – der Zustand bleibt
/// erhalten.
pub(crate) fn verify_allowed(state: &State) -> bool {
    matches!(state, State::Idle { .. } | State::Output { .. } | State::BatchOverview { .. } | State::Positioning { .. })
}

/// Zustände, in denen `OpenVerify` mit `ng-verify-busy` beantwortet wird (Signieren läuft).
fn signing_busy(state: &State) -> bool {
    matches!(
        state,
        State::Signing { .. } | State::BatchSigning { .. } | State::Resuming { .. } | State::ChoosingBackend { .. }
    )
}

/// Offene Prüfung. `Debug` ohne Pfad (Regel 9).
struct OpenVerify {
    generation: u64,
    path: PathBuf,
    doc: DocId,
    view: VerifyView,
}

impl std::fmt::Debug for OpenVerify {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenVerify")
            .field("generation", &self.generation)
            .field("doc", &self.doc)
            .field("view", &self.view)
            .finish_non_exhaustive()
    }
}

/// Zustand des Prüfmodus im Kern-Task.
#[derive(Debug, Default)]
pub(crate) struct VerifyState {
    current: Option<OpenVerify>,
    generation: u64,
}

/// Ergebnis des Ladens (Blocking-Pool → Kern-Task). `Debug` ohne Inhalt (Regel 9).
pub(crate) struct Verified {
    pub generation: u64,
    pub result: Result<(Arc<DocData>, VerifyView), String>,
}

impl std::fmt::Debug for Verified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Verified")
            .field("generation", &self.generation)
            .field("ok", &self.result.is_ok())
            .finish_non_exhaustive()
    }
}

/// Lädt und prüft eine Datei (läuft im Blocking-Pool).
fn load(id: DocId, path: &Path) -> Result<(Arc<DocData>, VerifyView), String> {
    let src = SourcePdf::open_file(path, &Limits::default()).map_err(|e| {
        // nur der stabile Code (keine Inhalte oder Pfade im Log, Regel 9)
        tracing::info!(code = pida_pades::reject_open_error(&e).code, "Prüfmodus: Datei nicht lesbar");
        MSG_VERIFY_OPEN_FAILED.to_owned()
    })?;
    let report = pida_pades::verify_document(&src).map_err(|e| {
        tracing::info!(code = e.code(), "Prüfmodus: Signaturen nicht lesbar");
        MSG_VERIFY_OPEN_FAILED.to_owned()
    })?;
    let data = Arc::new(DocData::for_verify(id, path, Arc::new(src)));
    let doc = DocInfo {
        id,
        name: data.name.clone(),
        page_count: data.pages.len(),
        page_sizes: data.page_sizes(),
        signed: !report.signatures.is_empty(),
    };
    let view = build_view(data.name.clone(), doc, &report, &data.pages);
    Ok((data, view))
}

/// `VerifyView` aus dem Bericht (Seiten für die Umrechnung der Widgets nach `PAGE_ABS`).
pub(crate) fn build_view(file_name: String, doc: DocInfo, report: &VerifyReport, pages: &[PageInfo]) -> VerifyView {
    let signatures: Vec<VerifiedSignatureView> = report
        .signatures
        .iter()
        .enumerate()
        .map(|(i, s)| signature_view(u32::try_from(i + 1).unwrap_or(u32::MAX), s, pages))
        .collect();
    VerifyView {
        file_name,
        status: VerifyStatus::Ready,
        doc: Some(doc),
        summary: summary(report, &signatures),
        signatures,
        revisions: u32::try_from(report.revisions).unwrap_or(u32::MAX),
    }
}

/// Gesamtergebnis: verändert vor nicht prüfbar vor „nach der letzten Signatur ergänzt“.
fn summary(report: &VerifyReport, sigs: &[VerifiedSignatureView]) -> VerifySummaryView {
    if sigs.is_empty() {
        VerifySummaryView::NoSignatures
    } else if sigs.iter().any(|s| matches!(s.integrity, SignatureIntegrity::Broken { .. })) {
        VerifySummaryView::SomeBroken
    } else if sigs.iter().any(|s| matches!(s.integrity, SignatureIntegrity::NotVerifiable { .. })) {
        VerifySummaryView::SomeNotVerifiable
    } else if report.modified_after_last_signature() {
        VerifySummaryView::ModifiedAfterLastSignature
    } else {
        VerifySummaryView::AllIntact
    }
}

fn signature_view(index: u32, s: &SignatureVerification, pages: &[PageInfo]) -> VerifiedSignatureView {
    let cert = s.signer.as_ref();
    let rfc = crate::workspace::rfc3339;
    VerifiedSignatureView {
        index,
        field_name: s.field_name.clone(),
        signer: cert.and_then(pida_cades::Certificate::subject_cn),
        organization: cert.and_then(|c| c.subject_attr(OID_O)),
        issuer: cert.and_then(|c| c.issuer_attr(OID_CN).or_else(|| c.issuer_attr(OID_O))),
        serial: cert.map(pida_cades::Certificate::serial_decimal),
        cert_valid_from: cert.map(|c| rfc(c.not_before())),
        cert_valid_to: cert.map(|c| rfc(c.not_after())),
        signing_time: s.signing_time.map(local_rfc3339),
        cert_valid_at_signing: cert.zip(s.signing_time).map(|(c, t)| c.is_valid_at(t)),
        integrity: match &s.integrity {
            Integrity::Intact => SignatureIntegrity::Intact,
            Integrity::Broken { code } => SignatureIntegrity::Broken { code: code.clone() },
            Integrity::NotVerifiable { code } => SignatureIntegrity::NotVerifiable { code: code.clone() },
        },
        coverage: if s.covers_whole_file {
            SignatureCoverage::WholeDocument
        } else {
            SignatureCoverage::Revision {
                later_revisions: u32::try_from(s.later_revisions).unwrap_or(u32::MAX),
                later_signatures: u32::try_from(s.later_signatures).unwrap_or(u32::MAX),
            }
        },
        qualification: match cert {
            Some(c) => {
                let qc = pida_cades::qc_statements(c);
                if qc.compliance {
                    QualificationClaim::Qualified { qscd: qc.sscd }
                } else {
                    QualificationClaim::NotQualified
                }
            }
            None => QualificationClaim::Unknown,
        },
        format: format(s),
        digest: s.digest_alg.map(|d| d.name().to_owned()),
        algorithm: s.signature_alg.map(|a| algorithm(a, cert.and_then(|c| c.key_type().ok()))),
        appearance: s.widget.and_then(|(page, rect)| {
            let info = pages.get(page)?;
            Some(AppearanceView {
                page,
                rect: crate::workspace::rect_pt(pida_placement::user_rect_to_visible(info, rect)),
            })
        }),
        warnings: if s.integrity == Integrity::Intact { warning_ids(&s.warnings) } else { Vec::new() },
        trust: TrustCheck::NotChecked,
    }
}

const OID_CN: &str = "2.5.4.3";
const OID_O: &str = "2.5.4.10";

/// `/SubFilter` → Format (ADR 0004 §3.9); Dokument-Zeitstempel = sonstiges.
fn format(s: &SignatureVerification) -> SignatureFormat {
    if s.doc_timestamp {
        return SignatureFormat::Other;
    }
    match s.sub_filter.as_deref() {
        Some("ETSI.CAdES.detached") => SignatureFormat::Pades,
        Some("adbe.pkcs7.detached") => SignatureFormat::Pkcs7Detached,
        Some("adbe.pkcs7.sha1") => SignatureFormat::Pkcs7Sha1,
        _ => SignatureFormat::Other,
    }
}

/// Signaturverfahren als Text (z. B. „ECDSA P-256“, „RSA 2048“, „RSA-PSS 3072“).
fn algorithm(alg: SignatureAlg, key: Option<KeyType>) -> String {
    let rsa_bits = |key: &Option<KeyType>| match key {
        Some(KeyType::Rsa { bits }) => format!(" {bits}"),
        _ => String::new(),
    };
    match alg {
        SignatureAlg::RsaPkcs1v15(_) => format!("RSA{}", rsa_bits(&key)),
        SignatureAlg::RsaPss { .. } => format!("RSA-PSS{}", rsa_bits(&key)),
        SignatureAlg::Ecdsa(_) => match key {
            Some(KeyType::Ec { curve }) => format!("ECDSA {}", curve_name(&curve)),
            _ => "ECDSA".into(),
        },
    }
}

fn curve_name(c: &pida_cades::EcCurve) -> String {
    use pida_cades::EcCurve::*;
    match c {
        P256 => "P-256".into(),
        P384 => "P-384".into(),
        P521 => "P-521".into(),
        BrainpoolP256r1 => "brainpoolP256r1".into(),
        BrainpoolP384r1 => "brainpoolP384r1".into(),
        BrainpoolP512r1 => "brainpoolP512r1".into(),
        Other(oid) => oid.clone(),
    }
}

/// `CmsWarning` → i18n-IDs (ohne Doppelte; nicht DER-sortierte Attribute sind kein Hinweis für Nutzerinnen).
fn warning_ids(warnings: &[CmsWarning]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in warnings {
        let id = match w {
            CmsWarning::Sha1Digest => MSG_WARNING_SHA1,
            CmsWarning::SigningCertificateV1 => MSG_WARNING_SIGNING_CERTIFICATE_V1,
            CmsWarning::WeakRsaKey { .. } => MSG_WARNING_WEAK_KEY,
            CmsWarning::NonDerSignedAttributes => continue,
        };
        if !out.iter().any(|o| o == id) {
            out.push(id.to_owned());
        }
    }
    out
}

/// RFC 3339 in der lokalen Zeitzone des Rechners zum jeweiligen Zeitpunkt (Sommerzeit über `jiff`).
fn local_rfc3339(t: time::OffsetDateTime) -> String {
    let offset = jiff::Timestamp::from_second(t.unix_timestamp())
        .ok()
        .map(|ts| ts.to_zoned(jiff::tz::TimeZone::system()).offset().seconds())
        .and_then(|s| time::UtcOffset::from_whole_seconds(s).ok())
        .unwrap_or(time::UtcOffset::UTC);
    crate::workspace::rfc3339(t.to_offset(offset))
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl CoreTask {
    /// `OpenVerify`: Datei prüfen (ersetzt eine offene Prüfung).
    pub(crate) fn open_verify(&mut self, path: PathBuf) {
        if !self.ws.enabled {
            tracing::debug!("Prüfmodus nur in der neuen Oberfläche");
            return;
        }
        if !verify_allowed(&self.state) {
            if signing_busy(&self.state) {
                self.notify_code(MSG_VERIFY_BUSY);
            } else {
                tracing::debug!(state = self.state.name(), "open_verify in diesem Zustand nicht möglich");
            }
            return;
        }
        self.release_verify();
        let doc = self.allocate_doc_id();
        self.verify.generation += 1;
        let generation = self.verify.generation;
        let view = VerifyView {
            file_name: file_name(&path),
            status: VerifyStatus::Loading,
            doc: None,
            signatures: Vec::new(),
            summary: VerifySummaryView::NoSignatures,
            revisions: 0,
        };
        self.verify.current = Some(OpenVerify { generation, path: path.clone(), doc, view });
        let shared = self.shared.clone();
        tokio::task::spawn_blocking(move || {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| load(doc, &path))).unwrap_or_else(|_| {
                    tracing::error!("Prüfmodus abgebrochen (Programmfehler)");
                    Err(MSG_VERIFY_OPEN_FAILED.to_owned())
                });
            shared.send(Msg::Verified(Box::new(Verified { generation, result })));
        });
    }

    /// Ergebnis des Ladens: nur für die aktuelle Prüfung (sonst verworfen, das Dokument bleibt nicht im
    /// Speicher).
    pub(crate) fn on_verified(&mut self, verified: Verified) {
        let Some(open) = self.verify.current.as_mut().filter(|o| o.generation == verified.generation) else {
            tracing::debug!("Ergebnis einer verworfenen Prüfung verworfen");
            return;
        };
        match verified.result {
            Ok((data, view)) => {
                self.shared.docs.insert(data);
                open.view = view;
            }
            Err(code) => open.view.status = VerifyStatus::Failed { code },
        }
    }

    /// `CloseVerify`.
    pub(crate) fn close_verify(&mut self) {
        self.release_verify();
    }

    /// Prüfdokument freigeben (Schließen, neue Prüfung, Programmfehler).
    pub(crate) fn release_verify(&mut self) {
        if let Some(open) = self.verify.current.take() {
            self.shared.docs.remove(open.doc);
        }
    }

    /// `VerifyRecent`: Eintrag aus „Zuletzt signiert“ im Prüfmodus öffnen (fehlende Datei → Hinweis in
    /// `recent.rs`).
    pub(crate) fn verify_recent_path(&mut self, path: PathBuf) {
        self.open_verify(path);
    }

    /// `SignVerified` („Dieses Dokument signieren“): Prüfmodus schließen, Datei wie `open_files` öffnen.
    pub(crate) fn sign_verified(&mut self) {
        let Some(path) = self.verify.current.as_ref().map(|o| o.path.clone()) else {
            tracing::debug!("sign_verified ohne offene Prüfung");
            return;
        };
        let actions = crate::actions::actions_for(&self.state, self.state.settings_enabled());
        if !actions.open_files {
            tracing::debug!(state = self.state.name(), "sign_verified: Öffnen gerade nicht möglich");
            return;
        }
        self.release_verify();
        let docs = self.input_documents(std::slice::from_ref(&path));
        let Some(id) = docs.first().map(|d| d.id) else {
            // inzwischen gelöscht bzw. verschoben
            self.notify_code(MSG_VERIFY_OPEN_FAILED);
            return;
        };
        self.apply(Event::FilesAdded(docs));
        // ohne Stapelübersicht meldet der Reducer das Einreihen nicht (mit Übersicht: `ng-batch-added`)
        let queued = self.state.queue().is_some_and(|q| q.pending.iter().any(|d| d.id == id));
        if queued && !self.ctx.batch() {
            let mut params = std::collections::BTreeMap::new();
            params.insert("count".to_owned(), "1".to_owned());
            self.notify(pida_workflow::MSG_BATCH_ADDED, params);
        }
    }

    /// Prüfmodus offen (auch während des Ladens)?
    pub(crate) fn verify_open(&self) -> bool {
        self.verify.current.is_some()
    }

    /// `ViewState::verify`.
    pub(crate) fn verify_view(&self) -> Option<VerifyView> {
        self.verify.current.as_ref().map(|o| o.view.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_names() {
        use pida_cades::{DigestAlg, EcCurve};
        let ec = |c| Some(KeyType::Ec { curve: c });
        assert_eq!(algorithm(SignatureAlg::Ecdsa(DigestAlg::Sha256), ec(EcCurve::P256)), "ECDSA P-256");
        assert_eq!(algorithm(SignatureAlg::Ecdsa(DigestAlg::Sha384), ec(EcCurve::P384)), "ECDSA P-384");
        assert_eq!(algorithm(SignatureAlg::Ecdsa(DigestAlg::Sha256), None), "ECDSA");
        let rsa = Some(KeyType::Rsa { bits: 2048 });
        assert_eq!(algorithm(SignatureAlg::RsaPkcs1v15(DigestAlg::Sha256), rsa.clone()), "RSA 2048");
        assert_eq!(algorithm(SignatureAlg::RsaPss { digest: DigestAlg::Sha256, salt_len: 32 }, rsa), "RSA-PSS 2048");
    }

    #[test]
    fn warnings_map_to_message_ids() {
        let ids = warning_ids(&[
            CmsWarning::Sha1Digest,
            CmsWarning::SigningCertificateV1,
            CmsWarning::NonDerSignedAttributes,
            CmsWarning::WeakRsaKey { bits: 1024 },
            CmsWarning::Sha1Digest,
        ]);
        assert_eq!(ids, vec![MSG_WARNING_SHA1, MSG_WARNING_SIGNING_CERTIFICATE_V1, MSG_WARNING_WEAK_KEY]);
    }

    #[test]
    fn allowed_states() {
        let q = pida_workflow::Queue::default;
        assert!(verify_allowed(&State::Idle { queue: q() }));
        assert!(!verify_allowed(&State::Analyzing { queue: q() }));
        assert!(!verify_allowed(&State::Starting));
        assert!(signing_busy(&State::Resuming {
            queue: q(),
            plan: pida_workflow::ResumePlan {
                position: pida_workflow::PositionChoice::Auto,
                backend: pida_backend_api::BackendId::Keystore,
            },
            doc: None,
        }));
    }
}

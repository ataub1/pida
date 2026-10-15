//! Kern-Task: hält den Workflow-Zustand, wendet den Reducer an, führt Effekte aus und projiziert den
//! `ViewState` (Plan §3.4/§3.5). Alles, was den Zustand ändert, läuft seriell über einen Kanal; lange
//! Effekte (Laden, Backend-Aufrufe, `prepare`, `finish`, Schreiben) laufen als Tokio-Tasks und melden
//! sich als Ereignis zurück. Ergebnisse eines abgebrochenen Signiervorgangs werden über eine
//! Epoche verworfen.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use pdfover_backend_api::{
    BackendError, BackendEvent, BackendId, BackendSession, CancelToken, Capabilities, CertChain, DigestJob,
    DocumentJob, DocumentSource, ExcludedByteRange, Platform, SessionContext, SignInput, SignJob, SignResult,
};
use pdfover_cades::{Expect, ExpectedSigner, SignedAttrs, SignedAttrsBuilder, SigningTime};
use pdfover_config::{CertCacheKey, CertificateCache, Effective, Overlay, Policy, Settings};
use pdfover_i18n::Lang;
use pdfover_pades::{CheckContext, FinishError, Invariant, InvariantViolation, OutputTarget, PadesError, Reserve};
use pdfover_placement::{PageTransform, Placement, Point, ScreenGeometry};
use pdfover_render::RenderBackend;
use pdfover_sigblock::{FontProvider, ProfileStore};
use pdfover_workflow::{
    BackendOption, Context, DocId, Effect, Event, ExitCode, FsProbe, InputDocument, OutputDecision, PositionChoice,
    ResumePlan, State, Step, WorkflowError, chosen_path, decide_output, reduce, resolve_without_dialog,
};
use tokio::sync::{mpsc, watch};

use crate::broker::{BrokerSink, CoreBroker, ShellHooks};
use crate::config::{ConfigState, PRIVACY_TEXT_VERSION, StartupProblem, validate};
use crate::docs::{DocData, DocStore, Loaded};
use crate::engine::{PadesEngine, PreparedDoc, SignedDoc};
use crate::plan::{self, BlockSpec, PlanInput, SpecInput};
use crate::resume;
use crate::view::{
    BackendOptionView, BlockView, DocInfo, DoneItemView, FieldInfo, KeystoreImportView, MainBarView,
    MigrationOfferView, NoticeView, OutputView, PlatformView, PrivacyNoticeView, QrInfo, QueueItemView,
    ResumeOfferView, SaveDialogView, SettingsView, StepView, UiEvent, ViewState,
};

/// Nachricht an den Kern-Task.
pub(crate) enum Msg {
    Ui(UiEvent),
    /// Ergebnis eines Effekts; `epoch` gesetzt = Signiervorgang (verworfen, wenn veraltet)
    Event {
        epoch: Option<u64>,
        event: Event,
    },
    Loaded(Box<Loaded>, Event),
    /// Ergebnis von „Automatisch platzieren“ für ein Dokument (verworfen, wenn inzwischen ein anderes
    /// Dokument positioniert wird)
    AutoPlaced(DocId, Placement),
    BlockBuilt(BlockInfo),
    Notices(Vec<(String, BTreeMap<String, String>)>),
    Refresh,
    BackendEvent(BackendEvent),
    Shutdown,
}

/// Blockvorschau (aus dem echten Appearance-Objekt, Plan §6.6).
#[derive(Clone, Default)]
pub(crate) struct BlockInfo {
    pub doc: Option<DocId>,
    pub width: f32,
    pub height: f32,
    pub revision: u64,
    pub pdf: Option<Arc<[u8]>>,
}

impl std::fmt::Debug for BlockInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockInfo")
            .field("doc", &self.doc)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("revision", &self.revision)
            .field("pdf_len", &self.pdf.as_ref().map(|p| p.len()))
            .finish()
    }
}

/// Schnappschuss der Konfiguration für Hintergrund-Tasks. `Debug` ohne Werte (Regel 9).
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub settings: Settings,
    pub overlay: Overlay,
    pub policy: Option<Policy>,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot").field("overlay", &self.overlay).finish_non_exhaustive()
    }
}

/// Art eines UI-Ereignisses für Logs (ohne Pfade, Einstellungen oder Antworten, Regel 9).
fn ui_event_kind(e: &UiEvent) -> &'static str {
    match e {
        UiEvent::OpenFiles { .. } => "open_files",
        UiEvent::AnswerWarning { .. } => "answer_warning",
        UiEvent::PlaceholderAnswer { .. } => "placeholder_answer",
        UiEvent::MoveBlock { .. } => "move_block",
        UiEvent::NudgeBlock { .. } => "nudge_block",
        UiEvent::ChangePage { .. } => "change_page",
        UiEvent::ToggleNewPage => "toggle_new_page",
        UiEvent::AutoPlace => "auto_place",
        UiEvent::ConfirmPosition => "confirm_position",
        UiEvent::ChooseInvisible => "choose_invisible",
        UiEvent::ChooseBackend { .. } => "choose_backend",
        UiEvent::AcknowledgePrivacy { .. } => "acknowledge_privacy",
        UiEvent::SubmitForm { .. } => "submit_form",
        UiEvent::SubmitSecret { .. } => "submit_secret",
        UiEvent::AnswerNotice { .. } => "answer_notice",
        UiEvent::OpenSignatureData { .. } => "open_signature_data",
        UiEvent::Cancel => "cancel",
        UiEvent::Retry => "retry",
        UiEvent::Save { .. } => "save",
        UiEvent::ConfirmOverwrite { .. } => "confirm_overwrite",
        UiEvent::OpenResult => "open_result",
        UiEvent::ShareResult => "share_result",
        UiEvent::ExportResult => "export_result",
        UiEvent::Navigate { .. } => "navigate",
        UiEvent::OpenSettings => "open_settings",
        UiEvent::SaveSettings { .. } => "save_settings",
        UiEvent::CloseSettings => "close_settings",
        UiEvent::ResetSettings => "reset_settings",
        UiEvent::Backgrounded => "backgrounded",
        UiEvent::Foregrounded => "foregrounded",
        UiEvent::ResumeSignature => "resume_signature",
        UiEvent::DiscardResume => "discard_resume",
        UiEvent::ImportKeystore { .. } => "import_keystore",
        UiEvent::DismissKeystoreImport => "dismiss_keystore_import",
        UiEvent::Quit => "quit",
        UiEvent::DismissNotice { .. } => "dismiss_notice",
        UiEvent::AcceptMigration { .. } => "accept_migration",
        UiEvent::DeclineMigration => "decline_migration",
    }
}

impl Snapshot {
    fn effective(&self) -> Effective<'_> {
        Effective::new(&self.settings, &self.overlay, self.policy.as_ref())
    }
}

/// Zustand eines Signiervorgangs (zwischen den Effekten).
#[derive(Default)]
pub(crate) struct SignSlot {
    epoch: u64,
    cancel: CancelToken,
    caps: Option<Capabilities>,
    session: Option<Box<dyn BackendSession>>,
    chain: Option<CertChain>,
    prepared: Option<Box<dyn PreparedDoc>>,
    /// CMS-Profil des Plans (Modell A: die SignedAttributes müssen mit genau diesem Profil gebaut werden,
    /// auch wenn sich die Einstellungen zwischen `prepare` und `sign` ändern)
    cms_profile: Option<pdfover_cades::CmsProfile>,
    attrs: Option<SignedAttrs>,
    result: Option<SignResult>,
    signed: Option<Arc<dyn SignedDoc>>,
    /// Zertifikats-Cache (Opt-in) dieses Vorgangs: Cache, Schlüssel und ob `chain` aus dem Cache stammt
    cert_cache: Option<(CertificateCache, CertCacheKey, bool)>,
}

/// Meldungs-ID: das Zertifikat aus dem Zertifikats-Cache passt nicht mehr (erneuert, anderes Konto);
/// der Eintrag ist gelöscht, „Wiederholen“ liest das Zertifikat neu und baut den Block neu.
pub(crate) const MSG_CERT_CACHE_MISMATCH: &str = "ng-certcache-mismatch";

/// Backends, für die der Zertifikats-Cache gilt: nur dort kostet das Lesen des Zertifikats eine eigene
/// Bestätigung (ID Austria, Plan §4.2 „Zwei SL-Requests“). Lokale BKU und Keystore lesen es ohne
/// Rückfrage.
fn uses_certificate_cache(id: BackendId) -> bool {
    matches!(id, BackendId::AtrustNative)
}

/// Fehlercodes der CMS-Prüfung, die ein anderes Signerzertifikat als das erwartete bedeuten.
fn is_signer_mismatch(code: &str) -> bool {
    matches!(code, "cms-unexpected-signer" | "cms-signing-certificate-mismatch")
}

/// Gemeinsame Daten (Kern-Task, Effekt-Tasks, Shell-Threads).
pub(crate) struct Shared {
    pub docs: DocStore,
    pub render: Option<Arc<dyn RenderBackend>>,
    pub engine: Arc<dyn PadesEngine>,
    pub profiles: Option<Arc<ProfileStore>>,
    pub fonts: Arc<FontProvider>,
    pub block: Mutex<BlockInfo>,
    pub broker: Arc<CoreBroker>,
    pub sign: Mutex<SignSlot>,
    pub tx: mpsc::UnboundedSender<Msg>,
    pub shell: Mutex<Option<Arc<dyn ShellHooks>>>,
    pub tile_scale: Mutex<f32>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("docs", &self.docs)
            .field("render", &self.render.is_some())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Shared {
    fn send(&self, msg: Msg) {
        let _ = self.tx.send(msg);
    }

    fn shell(&self) -> Option<Arc<dyn ShellHooks>> {
        lock(&self.shell).clone()
    }

    /// Blockvorschau bauen (Profil, Bildmarke, Hinweis, Breite der angezeigten Seite).
    pub(crate) fn build_block(&self, snap: &Snapshot, doc: Option<&DocData>, page: usize) -> Result<BlockInfo, String> {
        let store = self.profiles.as_ref().ok_or("Profile nicht verfügbar")?;
        let emblem = snap.effective().emblem().and_then(plan::load_emblem);
        let spec = plan::block_spec(
            store,
            SpecInput {
                effective: snap.effective(),
                system_language: Lang::system(),
                pdfa: doc.and_then(|d| d.report.pdfa.as_ref()),
                cert: None,
                signing_time: plan::local_now(),
                emblem: emblem.as_ref(),
            },
        )
        .map_err(|e| e.code())?;
        let a4 = [plan::a4_page()];
        let pages: &[pdfover_pdf_cos::PageInfo] =
            doc.map(|d| d.pages.as_slice()).filter(|p| !p.is_empty()).unwrap_or(&a4);
        let geom = plan::block_geometry(&spec, pages, page, store, &self.fonts).map_err(|e| e.code())?;
        let pdf = if spec.visible() {
            let app = spec.build(geom.width, 0, store, &self.fonts).map_err(|e| e.code())?;
            app.to_standalone_pdf().ok().map(Arc::from)
        } else {
            None
        };
        Ok(BlockInfo { doc: doc.map(|d| d.id), width: geom.width, height: geom.height, revision: 0, pdf })
    }
}

/// Rückkanal des Brokers.
struct Sink(mpsc::UnboundedSender<Msg>);

impl BrokerSink for Sink {
    fn changed(&self) {
        let _ = self.0.send(Msg::Refresh);
    }
    fn backend_event(&self, event: BackendEvent) {
        let _ = self.0.send(Msg::BackendEvent(event));
    }
}

pub(crate) fn broker_sink(tx: mpsc::UnboundedSender<Msg>) -> Arc<dyn BrokerSink> {
    Arc::new(Sink(tx))
}

/// Plattform und Mobile-Optionen des Kerns (`CoreOptions::persist_prepared`, `default_output_dir`).
#[derive(Debug, Clone)]
pub(crate) struct MobileSetup {
    pub platform: Platform,
    /// vorbereiteten Zustand für „Signatur fortsetzen“ sichern (iOS/Android)
    pub persist_prepared: bool,
    /// Ausgabeordner ohne Einstellung (App-Dokumente auf Mobile)
    pub default_output_dir: Option<PathBuf>,
}

/// Meldungs-ID: Hinweis zu ID Austria auf Mobile – FIDO2 ist auf diesem Gerät nicht verfügbar (Plan
/// §4.7, §5.2); die Oberfläche zeigt FIDO2 ausgegraut mit diesem Text.
pub(crate) const MSG_FIDO2_UNAVAILABLE_MOBILE: &str = "ng-fido2-unavailable-mobile";
/// Meldungs-ID: PKCS#12 importiert.
pub(crate) const MSG_KEYSTORE_IMPORTED: &str = "ng-keystore-imported";
/// Meldungs-ID: Import gerade nicht möglich (Signiervorgang läuft).
pub(crate) const MSG_KEYSTORE_IMPORT_BUSY: &str = "ng-keystore-import-busy";
/// Meldungs-ID: Datei nicht lesbar bzw. zu groß.
pub(crate) const MSG_KEYSTORE_IMPORT_UNREADABLE: &str = "ng-keystore-import-unreadable";
/// Meldungs-ID: keine PKCS#12-Datei.
pub(crate) const MSG_KEYSTORE_IMPORT_FORMAT: &str = "ng-keystore-import-format";
/// Meldungs-ID: Kopie in die Sandbox fehlgeschlagen.
pub(crate) const MSG_KEYSTORE_IMPORT_FAILED: &str = "ng-keystore-import-failed";
/// Höchstgröße einer importierten PKCS#12-Datei (typisch wenige KiB).
const MAX_KEYSTORE_BYTES: u64 = 4 * 1024 * 1024;
/// Ordner der importierten Keystores unter `Paths::data_dir`.
pub(crate) const KEYSTORE_DIR: &str = "keystores";

/// Offener Dialog der Ausgabe (aus den Effekten `AskOverwrite`/`AskSavePath`).
#[derive(Debug, Clone, PartialEq)]
enum OutputDialog {
    Overwrite(PathBuf),
    Save(SaveDialogView),
}

/// Der Kern-Task.
pub(crate) struct CoreTask {
    pub shared: Arc<Shared>,
    pub state: State,
    pub ctx: Context,
    pub cfg: ConfigState,
    pub registry: crate::backends::Registry,
    pub headless: bool,
    pub view_tx: watch::Sender<ViewState>,
    pub finished_tx: watch::Sender<Option<ExitCode>>,
    pub startup: Option<StartupProblem>,
    pub fido2_available: bool,
    notices: Vec<NoticeView>,
    next_notice: u64,
    next_doc: u64,
    settings_view: Option<SettingsView>,
    options: Vec<BackendOption>,
    dialog: Option<OutputDialog>,
    queue: VecDeque<Event>,
    /// Import-Angebot mit „enthält Klartext-Passwörter“ (einmal je Altdatei ermittelt)
    migration_view: Option<MigrationOfferView>,
    /// Plattform und Mobile-Optionen
    mobile: MobileSetup,
    /// App im Hintergrund (Lebenszyklus; kein Abbruch)
    backgrounded: bool,
    /// gespeicherter, noch nicht angenommener Vorgang („Signatur fortsetzen“)
    resume_offer: Option<resume::Persisted>,
    /// angenommener Vorgang: Dokument und Zustand für die Wiederherstellung beim nächsten `Prepare`
    resume_active: Option<(DocId, resume::Persisted)>,
    /// der Ordner des persistierten Zustands gehört dem laufenden Vorgang (sonst hält er ein noch nicht
    /// angenommenes Angebot, das `DeletePersisted` eines anderen Vorgangs nicht löschen darf)
    persist_owned: bool,
    /// Ergebnis des PKCS#12-Imports
    keystore_import: Option<KeystoreImportView>,
}

impl CoreTask {
    pub(crate) fn new(
        shared: Arc<Shared>,
        cfg: ConfigState,
        registry: crate::backends::Registry,
        headless: bool,
        view_tx: watch::Sender<ViewState>,
        finished_tx: watch::Sender<Option<ExitCode>>,
        startup: Option<StartupProblem>,
        fido2_available: bool,
        mobile: MobileSetup,
    ) -> CoreTask {
        let ctx = cfg.context(headless);
        CoreTask {
            shared,
            state: State::Starting,
            ctx,
            cfg,
            registry,
            headless,
            view_tx,
            finished_tx,
            startup,
            fido2_available,
            notices: Vec::new(),
            next_notice: 1,
            next_doc: 1,
            settings_view: None,
            options: Vec::new(),
            dialog: None,
            queue: VecDeque::new(),
            migration_view: None,
            mobile,
            backgrounded: false,
            resume_offer: None,
            resume_active: None,
            persist_owned: false,
            keystore_import: None,
        }
    }

    /// Ordner des persistierten Signierzustands (App-Sandbox, nicht der Cache).
    fn resume_dir(&self) -> PathBuf {
        self.cfg.paths.temp_dir.join(resume::RESUME_DIR)
    }

    /// Beim Start: unterbrochenen Vorgang suchen und „Signatur fortsetzen“ anbieten (Mobile).
    fn load_resume_offer(&mut self) {
        if !self.mobile.persist_prepared {
            return;
        }
        let dir = self.resume_dir();
        match resume::load(&dir, SystemTime::now()) {
            resume::Loaded::Nothing => {}
            resume::Loaded::Offer(p) => {
                tracing::info!("unterbrochener Signiervorgang gefunden – „Signatur fortsetzen“ wird angeboten");
                self.resume_offer = Some(*p);
            }
            resume::Loaded::Expired => {
                tracing::info!("unterbrochener Signiervorgang zu alt – verworfen");
                resume::delete(&dir);
                self.notify(resume::MSG_EXPIRED, BTreeMap::new());
            }
            resume::Loaded::Invalid(reason) => {
                tracing::warn!(reason = %reason, "persistierter Signierzustand ungültig – verworfen");
                resume::delete(&dir);
            }
        }
    }

    /// `Effect::DeletePersisted`: nur den Zustand des laufenden Vorgangs löschen.
    fn delete_persisted(&mut self) {
        self.resume_active = None;
        if self.mobile.persist_prepared && self.persist_owned {
            resume::delete(&self.resume_dir());
            self.persist_owned = false;
        }
    }

    /// „Signatur fortsetzen“ angenommen: Dokument (Sandbox-Kopie) mit Position und Signiermittel an den
    /// Workflow; das vorbereitete Dokument wird beim nächsten `Prepare` wiederhergestellt.
    fn resume_signature(&mut self) {
        if !matches!(self.state, State::Idle { .. }) {
            tracing::debug!("„Signatur fortsetzen“ nur im Auswahl-Screen");
            return;
        }
        let Some(persisted) = self.resume_offer.take() else { return };
        let document = persisted.document();
        if !document.is_file() {
            resume::delete(&persisted.dir);
            self.notify(resume::MSG_RESTORE_FAILED, BTreeMap::new());
            return;
        }
        let id = DocId(self.next_doc);
        self.next_doc += 1;
        let input = InputDocument { id, path: document, display_name: persisted.record.name.clone() };
        let pending = self.input_documents(&persisted.existing_pending());
        let plan = ResumePlan { position: persisted.record.position.clone(), backend: persisted.record.backend };
        tracing::info!(backend = plan.backend.as_str(), pending = pending.len(), "Signatur wird fortgesetzt");
        self.persist_owned = true;
        self.resume_active = Some((id, persisted));
        self.apply(Event::ResumeRequested { document: input, plan, pending });
    }

    /// „Signatur fortsetzen“ verworfen.
    fn discard_resume(&mut self) {
        if let Some(p) = self.resume_offer.take() {
            tracing::info!("unterbrochener Signiervorgang verworfen");
            resume::delete(&p.dir);
        }
    }

    /// Lebenszyklus „Vordergrund“: Wartet ID Austria auf die Bestätigung in der App bzw. einen QR-Scan,
    /// wird der Long-Poll sofort neu gestartet (Seite neu laden, R7) – der Wechsel ist kein Abbruch.
    fn foregrounded(&mut self) {
        let was_background = std::mem::replace(&mut self.backgrounded, false);
        self.apply(Event::Foregrounded);
        if was_background && matches!(self.state, State::Signing { .. }) && self.shared.broker.restart_long_poll() {
            tracing::info!("nach der Rückkehr in die App: Long-Poll neu gestartet");
        }
    }

    /// PKCS#12-Import in die App-Sandbox (Mobile, Plan §5.2): Datei kopieren, Passwort prüfen,
    /// Einstellungen › Keystore setzen, Passwort optional im sicheren Speicher.
    fn import_keystore(&mut self, path: &Path, password: Option<zeroize::Zeroizing<String>>, remember: bool) {
        let file_name =
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "keystore.p12".into());
        let result = self.import_keystore_inner(path, &file_name, password, remember);
        let (ok, code, password_stored) = match result {
            Ok(stored) => (true, MSG_KEYSTORE_IMPORTED.to_owned(), stored),
            Err(code) => (false, code, false),
        };
        tracing::info!(ok, code = %code, "PKCS#12-Import");
        self.keystore_import = Some(KeystoreImportView { file_name, ok, code, password_stored });
    }

    fn import_keystore_inner(
        &mut self,
        path: &Path,
        file_name: &str,
        password: Option<zeroize::Zeroizing<String>>,
        remember: bool,
    ) -> Result<bool, String> {
        // nicht während eines Signiervorgangs (Einstellungen dürfen dort nicht wechseln); in der
        // Signiermittel-Auswahl ja – danach ist der Keystore dort sofort verfügbar
        let busy = matches!(
            self.state,
            State::Signing { .. } | State::Resuming { .. } | State::Starting | State::Finished { .. }
        );
        if busy {
            return Err(MSG_KEYSTORE_IMPORT_BUSY.into());
        }
        let len = std::fs::metadata(path).map(|m| m.len()).map_err(|_| MSG_KEYSTORE_IMPORT_UNREADABLE.to_owned())?;
        if len == 0 || len > MAX_KEYSTORE_BYTES {
            return Err(MSG_KEYSTORE_IMPORT_UNREADABLE.into());
        }
        let bytes = std::fs::read(path).map_err(|_| MSG_KEYSTORE_IMPORT_UNREADABLE.to_owned())?;
        match pdfover_backend_keystore::detect_format(&bytes) {
            pdfover_backend_keystore::KeystoreFormat::Pkcs12 => {}
            pdfover_backend_keystore::KeystoreFormat::Jks | pdfover_backend_keystore::KeystoreFormat::Jceks => {
                return Err("keystore-conversion-needed".into());
            }
            pdfover_backend_keystore::KeystoreFormat::Unknown => return Err(MSG_KEYSTORE_IMPORT_FORMAT.into()),
        }
        // Passwort prüfen (falls angegeben); bei genau einem Schlüssel wird dessen Alias übernommen
        let mut alias = None;
        if let Some(pw) = &password {
            let entries = pdfover_backend_keystore::list_aliases(&bytes, pw).map_err(|e| e.code().to_owned())?;
            let keys: Vec<_> = entries.into_iter().filter(|e| e.is_key_entry).collect();
            if keys.is_empty() {
                return Err("keystore-no-key-entry".into());
            }
            if keys.len() == 1 {
                alias = keys.into_iter().next().map(|k| k.alias);
            }
        }
        // in die Sandbox kopieren (Ordner 0700, Datei 0600, atomar)
        let dir = self.cfg.paths.data_dir.join(KEYSTORE_DIR);
        let target = dir.join(sanitize_file_name(file_name, "keystore.p12"));
        write_private_file(&dir, &target, &bytes).map_err(|_| MSG_KEYSTORE_IMPORT_FAILED.to_owned())?;
        // vorher importierte Datei (anderer Name) samt gespeicherter Passwörter entfernen
        if let Some(old) = self.cfg.settings.keystore.file.clone()
            && old != target
            && old.parent() == Some(dir.as_path())
        {
            let _ = std::fs::remove_file(&old);
            let _ = self.cfg.secrets.delete(&pdfover_config::SecretId::keystore_store(&old));
            let old_alias = self.cfg.settings.keystore.alias.clone().unwrap_or_default();
            let _ = self.cfg.secrets.delete(&pdfover_config::SecretId::keystore_key(&old, &old_alias));
        }
        let mut stored = false;
        if let (true, Some(pw)) = (remember, &password)
            && self.cfg.secrets.is_persistent()
        {
            let a = alias.clone().unwrap_or_default();
            let secrets = [
                (pdfover_config::SecretId::keystore_store(&target), pw.clone()),
                (pdfover_config::SecretId::keystore_key(&target, &a), pw.clone()),
            ];
            match pdfover_config::secrets::store_all(self.cfg.secrets.as_ref(), &secrets) {
                Ok(_) => stored = true,
                Err(e) => tracing::warn!(id = e.message_id(), "Keystore-Passwort nicht im sicheren Speicher"),
            }
        }
        let ks = &mut self.cfg.settings.keystore;
        ks.enabled = true;
        ks.file = Some(target);
        ks.declared_type = Some("PKCS12".into());
        ks.alias = alias;
        if stored {
            ks.password_storage = pdfover_config::PasswordStorage::Keychain;
        } else if ks.password_storage == pdfover_config::PasswordStorage::Keychain {
            ks.password_storage = pdfover_config::PasswordStorage::Memory;
        }
        if let Err(e) = self.cfg.save() {
            tracing::warn!(id = e.message_id(), "Einstellungen nach dem Import nicht gespeichert");
            self.notify("error-FailedToSaveSettings", BTreeMap::new());
        }
        if self.settings_view.is_some() {
            self.settings_view = Some(self.settings_view_for(self.cfg.settings.clone(), BTreeMap::new()));
        }
        self.ctx = self.cfg.context(self.headless);
        self.registry.configure(&self.cfg);
        self.apply(Event::SettingsChanged);
        Ok(stored)
    }

    /// Import-Angebot für die Ansicht (Klartext-Passwörter einmal je Altdatei ermitteln).
    fn refresh_migration_view(&mut self) {
        let Some(file) = self.cfg.migration_offer.clone() else {
            self.migration_view = None;
            return;
        };
        if self.migration_view.as_ref().is_some_and(|m| m.legacy_file == file) {
            return;
        }
        let legacy_dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        // eigene Panik-Grenze: Diese Prüfung läuft nach jedem Ereignis (`guarded`); ein Programmfehler
        // beim Lesen der Altdatei darf das gerade verarbeitete Ereignis nicht verwerfen und sich nicht
        // bei jedem weiteren Ereignis wiederholen (dann: ohne Rückfrage zu Passwörtern)
        let probe = std::panic::catch_unwind(|| {
            pdfover_config::LegacyConfig::load(&file)
                .map(|legacy| {
                    let ctx = pdfover_config::MigrationContext {
                        legacy_dir,
                        system_language: Lang::system(),
                        check_paths: false,
                    };
                    !pdfover_config::migrate(&legacy, &ctx).report.secrets_found.is_empty()
                })
                .unwrap_or(false)
        });
        let plaintext_secrets = probe.unwrap_or_else(|_| {
            tracing::error!("Altkonfiguration nicht auswertbar (Programmfehler) – Angebot ohne Passwortfrage");
            false
        });
        self.migration_view = Some(MigrationOfferView { legacy_file: file, plaintext_secrets });
    }

    /// Hauptschleife.
    pub(crate) async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Msg>) {
        self.guarded(|t| t.start());
        self.publish();
        // Backend-Auswahl aktuell halten (lokale BKU erscheint, sobald sie erreichbar ist, F-51)
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let msg = tokio::select! {
                m = rx.recv() => m,
                _ = tick.tick() => {
                    self.guarded(|t| t.refresh_backend_options());
                    self.publish();
                    continue;
                }
            };
            let Some(msg) = msg else { break };
            if matches!(msg, Msg::Shutdown) {
                break;
            }
            self.guarded(|t| t.handle(msg));
            self.publish();
        }
        self.cancel_session();
    }

    /// Führt `f` aus; eine Panik (Programmfehler, feindliche Eingabe in einer Bibliothek) beendet nicht
    /// den Kern-Task (die UI hinge sonst ohne Rückmeldung), sondern wird wie in PDF-Over zu
    /// `error.Unexpected` (F-106): mit Oberfläche Fehleranzeige und weiter mit der Dokumentauswahl,
    /// ohne Oberfläche Programmende mit Exit-Code 1.
    fn guarded(&mut self, f: impl FnOnce(&mut CoreTask)) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f(self);
            self.refresh_migration_view();
        }));
        if result.is_err() {
            self.recover_from_panic();
        }
    }

    fn recover_from_panic(&mut self) {
        // kein Paniktext im Log: er könnte Pfade oder Werte enthalten (Regel 9)
        tracing::error!(state = self.state.name(), "unerwarteter Fehler im Kern – Vorgang verworfen");
        self.queue.clear();
        self.dialog = None;
        self.settings_view = None;
        self.cancel_session();
        let error = WorkflowError::Unexpected("Programmfehler".into());
        let mut queue = self.state.queue().cloned().unwrap_or_default();
        for d in queue.current.iter().chain(queue.pending.iter()) {
            self.shared.docs.remove(d.id);
        }
        if queue.current.is_some() {
            queue.finish_current(pdfover_workflow::DocResult::Failed { error: error.clone() });
        }
        queue.clear();
        if self.headless {
            self.state = State::Finished { exit: ExitCode::General, results: queue.done };
            self.publish();
            let _ = self.finished_tx.send(Some(ExitCode::General));
        } else {
            self.state = State::Error { queue, error, retry: None, from: None };
        }
    }

    /// Eine Nachricht verarbeiten.
    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Shutdown => {}
            Msg::Ui(e) => self.on_ui(e),
            Msg::Event { epoch, event } => {
                if epoch.is_none_or(|e| e == lock(&self.shared.sign).epoch) {
                    self.apply(event);
                } else {
                    tracing::debug!("veraltetes Ergebnis eines abgebrochenen Vorgangs verworfen");
                }
            }
            Msg::Loaded(loaded, event) => self.on_loaded(*loaded, event),
            Msg::AutoPlaced(doc, placement) => {
                if matches!(&self.state, State::Positioning { doc: facts, .. } if facts.id == doc) {
                    self.apply(Event::PlacementChanged(placement));
                }
            }
            Msg::BlockBuilt(info) => self.on_block(info),
            Msg::Notices(list) => {
                for (code, params) in list {
                    self.notify(&code, params);
                }
            }
            Msg::Refresh => {}
            Msg::BackendEvent(e) => self.on_backend_event(e),
        }
    }

    /// Ergebnis der Analyse übernehmen – nur, wenn das Dokument noch analysiert wird. Wurde es inzwischen
    /// verworfen (Hauptleiste „Öffnen“, Abbrechen, Beenden), bleiben Dokument, Renderer und Hinweise
    /// nicht im Speicher zurück.
    fn on_loaded(&mut self, loaded: Loaded, event: Event) {
        let id = loaded.data.id;
        let current = matches!(
            &self.state,
            State::Analyzing { queue } | State::Resuming { queue, doc: None, .. } if queue.current_id() == Some(id)
        );
        if !current {
            tracing::debug!("Analyse eines verworfenen Dokuments verworfen");
            return;
        }
        // Warnungen der Analyse: mit Oberfläche als Rückfrage (State::AnalysisWarning), ohne als Hinweis
        if self.headless {
            for w in &loaded.warnings {
                self.notify(w, BTreeMap::new());
            }
        }
        self.shared.docs.insert(loaded.data.clone());
        self.apply(event);
    }

    /// In der Backend-Auswahl die Verfügbarkeit neu ermitteln (ohne automatische Wahl).
    fn refresh_backend_options(&mut self) {
        if !matches!(self.state, State::ChoosingBackend { auto_choice: false, .. }) {
            return;
        }
        let env = self.registry.environment(self.shared.broker.interactive(), &self.cfg);
        let options = self.registry.options(&env);
        if options != self.options {
            self.options = options.clone();
            self.apply(Event::BackendsProbed(options));
        }
    }

    /// Start: Einstellungen unlesbar → Rückfrage (F-05), sonst Dokumente aus dem Overlay.
    fn start(&mut self) {
        if let Some(StartupProblem::Unreadable(e)) = self.startup.take() {
            if self.headless {
                // ohne Oberfläche gibt es keine Rückfrage (Werksreset): Grund als Hinweis, dann Ende (Exit 1)
                let params = e
                    .message_args()
                    .into_iter()
                    .map(|(k, v)| {
                        let v = match v {
                            pdfover_i18n::ArgValue::Str(s) => s,
                            pdfover_i18n::ArgValue::Num(n) => n.to_string(),
                        };
                        (k.to_owned(), v)
                    })
                    .collect();
                self.notify(e.message_id(), params);
            }
            self.apply(Event::SettingsUnreadable(e.localized(self.cfg.language())));
            return;
        }
        self.registry.configure(&self.cfg);
        // Schlüsselbund gewünscht, aber nicht verfügbar (z. B. Linux ohne Secret Service): Hinweis
        if self.cfg.settings.keystore.password_storage == pdfover_config::PasswordStorage::Keychain
            && !self.cfg.secrets.is_persistent()
        {
            self.notify("ng-secrets-not-persistent", BTreeMap::new());
        }
        self.load_resume_offer();
        let docs = self.input_documents(&self.cfg.overlay.input_files.clone());
        self.apply(Event::Started { documents: docs });
    }

    fn input_documents(&mut self, paths: &[PathBuf]) -> Vec<InputDocument> {
        let mut out = Vec::new();
        for p in paths {
            // F-12: nur vorhandene Dateien
            if !p.is_file() {
                tracing::info!("Eingabe übersprungen (keine Datei)");
                continue;
            }
            let id = DocId(self.next_doc);
            self.next_doc += 1;
            out.push(InputDocument::new(id, p.clone()));
        }
        out
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            settings: self.cfg.settings.clone(),
            overlay: self.cfg.overlay.clone(),
            policy: self.cfg.policy.clone(),
        }
    }

    fn notify(&mut self, code: &str, params: BTreeMap<String, String>) {
        // gleiche Meldung nicht doppelt stapeln
        if self.notices.iter().any(|n| n.code == code && n.params == params) {
            return;
        }
        self.notices.push(NoticeView { id: self.next_notice, code: code.to_owned(), params });
        self.next_notice += 1;
    }

    /// Reducer anwenden und Effekte ausführen (synchron erzeugte Folgeereignisse in Reihenfolge).
    pub(crate) fn apply(&mut self, event: Event) {
        self.queue.push_back(event);
        while let Some(event) = self.queue.pop_front() {
            // Kopie statt `mem::replace`: bricht der Reducer ab, bleibt der bisherige Zustand erhalten
            let t = reduce(self.state.clone(), event, &self.ctx);
            self.state = t.state;
            tracing::debug!(state = self.state.name(), effects = t.effects.len(), "Übergang");
            if !matches!(self.state, State::Output { decision: Some(_), saved: None, .. }) {
                self.dialog = None;
            }
            for effect in t.effects {
                self.execute(effect);
            }
        }
    }

    fn current_doc(&self) -> Option<Arc<DocData>> {
        self.state.queue().and_then(|q| q.current_id()).and_then(|id| self.shared.docs.get(id))
    }

    fn spawn_event<F>(&self, epoch: Option<u64>, fut: F)
    where
        F: std::future::Future<Output = Event> + Send + 'static,
    {
        let tx = self.shared.tx.clone();
        tokio::spawn(async move {
            let event = fut.await;
            let _ = tx.send(Msg::Event { epoch, event });
        });
    }

    /// Blockierende Arbeit; eine Panik (z. B. ein feindliches PDF in einer Bibliothek) wird zum
    /// Ereignis `on_panic` statt den Kern zu beenden.
    fn spawn_blocking_event<F>(&self, epoch: Option<u64>, on_panic: fn(String) -> Event, f: F)
    where
        F: FnOnce() -> Event + Send + 'static,
    {
        self.spawn_event(epoch, async move {
            match tokio::task::spawn_blocking(f).await {
                Ok(ev) => ev,
                Err(e) => {
                    tracing::error!(panic = e.is_panic(), "Hintergrundaufgabe abgebrochen");
                    on_panic(panic_text(e))
                }
            }
        });
    }

    fn sign_epoch(&self) -> u64 {
        lock(&self.shared.sign).epoch
    }

    fn cancel_session(&mut self) {
        let session = {
            let mut slot = lock(&self.shared.sign);
            slot.cancel.cancel();
            let s = slot.session.take();
            let epoch = slot.epoch + 1;
            *slot = SignSlot { epoch, ..SignSlot::default() };
            s
        };
        self.shared.broker.reset();
        if let Some(mut session) = session {
            tokio::spawn(async move { session.cancel().await });
        }
    }

    // --------------------------------------------------------------------------------------------
    // Effekte
    // --------------------------------------------------------------------------------------------

    fn execute(&mut self, effect: Effect) {
        match effect {
            Effect::Analyze(id) => self.effect_analyze(id),
            Effect::RenderPage { doc, page } => self.effect_render(doc, page),
            Effect::BuildBlockPreview(doc) => self.effect_block(Some(doc)),
            Effect::ProbeBackends => {
                let env = self.registry.environment(self.shared.broker.interactive(), &self.cfg);
                self.options = self.registry.options(&env);
                self.queue.push_back(Event::BackendsProbed(self.options.clone()));
            }
            Effect::OpenSession(id) => self.effect_open(id),
            Effect::ReadCertificate => self.effect_certificate(),
            Effect::Prepare { doc, position, min_reserve } => self.effect_prepare(doc, position, min_reserve),
            Effect::Sign => self.effect_sign(),
            Effect::Finish => self.effect_finish(),
            Effect::CloseSession => {
                let session = lock(&self.shared.sign).session.take();
                drop(session);
            }
            Effect::CancelSession => self.cancel_session(),
            Effect::DecideOutput(doc) => self.effect_decide(doc),
            Effect::AskSavePath(d) => {
                if let OutputDecision::Ask { directory, file_name } = d {
                    self.dialog = Some(OutputDialog::Save(SaveDialogView { directory, file_name }));
                }
            }
            Effect::AskOverwrite(p) => self.dialog = Some(OutputDialog::Overwrite(p)),
            Effect::Write(p) => self.effect_write(p, false),
            Effect::Overwrite(p) => self.effect_write(p, true),
            Effect::OpenResult(p) => self.effect_open_result(&p),
            Effect::ShareResult(p) => self.effect_share(p),
            Effect::ExportResult(p) => self.effect_export(p),
            Effect::Notify(code) => self.notify(&code, BTreeMap::new()),
            Effect::AcknowledgePrivacy(id) => {
                self.cfg.acknowledge_privacy(id);
                self.ctx = self.cfg.context(self.headless);
            }
            Effect::ResetSettings => self.reset_settings(true),
            Effect::DeletePersisted => self.delete_persisted(),
            Effect::Discard(id) => self.shared.docs.remove(id),
            Effect::RememberDirectory(dir) => self.cfg.remember_directory(&dir),
            Effect::Exit(code) => {
                self.cancel_session();
                // erst den Endzustand veröffentlichen (Ergebnisse), dann das Ende melden
                self.publish();
                let _ = self.finished_tx.send(Some(code));
            }
        }
    }

    fn effect_analyze(&mut self, id: DocId) {
        let Some(input) = self.state.queue().and_then(|q| q.current.clone()).filter(|d| d.id == id) else { return };
        let shared = self.shared.clone();
        let snap = self.snapshot();
        let search = self.ctx.placeholder_search;
        let fields = self.ctx.placeholder_fields;
        let tx = shared.tx.clone();
        tokio::spawn(async move {
            let work = {
                let shared = shared.clone();
                move || -> Result<(Loaded, Option<Placement>), WorkflowError> {
                    let render = shared.render.as_deref();
                    let loaded = crate::docs::load(id, &input.path, shared.engine.as_ref(), search, fields, render)?;
                    let start = match (render, shared.profiles.as_deref()) {
                        (Some(_), Some(store)) => start_placement(&shared, &snap, &loaded.data, store),
                        _ => None,
                    };
                    Ok((loaded, start))
                }
            };
            let msg = match tokio::task::spawn_blocking(work).await {
                Ok(Ok((loaded, start))) => {
                    let report = &loaded.data.report;
                    let event = Event::Analyzed {
                        doc: id,
                        class: report.class,
                        placeholder: loaded.placeholder.clone(),
                        signed: report.is_signed(),
                        page_count: loaded.data.pages.len(),
                        start,
                        warnings: loaded.warnings.clone(),
                    };
                    Msg::Loaded(Box::new(loaded), event)
                }
                Ok(Err(error)) => Msg::Event { epoch: None, event: Event::AnalysisFailed { doc: id, error } },
                Err(e) => Msg::Event {
                    epoch: None,
                    event: Event::AnalysisFailed { doc: id, error: WorkflowError::Unexpected(panic_text(e)) },
                },
            };
            let _ = tx.send(msg);
        });
    }

    fn effect_render(&mut self, doc: DocId, page: usize) {
        let Some(render) = self.shared.render.clone() else { return };
        let shared = self.shared.clone();
        let scale = *lock(&shared.tile_scale);
        tokio::task::spawn_blocking(move || {
            if let Err(e) = shared.docs.tile(doc, page, scale, render.as_ref()) {
                tracing::debug!(code = e.code(), "Vorab-Rendern fehlgeschlagen");
            }
        });
    }

    fn effect_block(&mut self, doc: Option<DocId>) {
        let shared = self.shared.clone();
        let snap = self.snapshot();
        let data = doc.and_then(|d| shared.docs.get(d));
        let page = match &self.state {
            State::Positioning { page, doc: facts, .. } => (*page).min(facts.page_count.saturating_sub(1)),
            _ => data.as_ref().map(|d| d.pages.len().saturating_sub(1)).unwrap_or(0),
        };
        tokio::task::spawn_blocking(move || match shared.build_block(&snap, data.as_deref(), page) {
            Ok(info) => shared.send(Msg::BlockBuilt(info)),
            Err(code) => {
                tracing::warn!(code = %code, "Blockvorschau nicht erstellt");
                shared.send(Msg::Notices(vec![(code, BTreeMap::new())]));
            }
        });
    }

    fn on_block(&mut self, mut info: BlockInfo) {
        {
            let mut block = lock(&self.shared.block);
            info.revision = block.revision + 1;
            *block = info.clone();
        }
        // Blockgröße geändert (Profil, Hinweis, Logo): Position anpassen und klemmen (F-43)
        if let State::Positioning { placement: Some(p), doc, .. } = &self.state
            && info.doc == Some(doc.id)
            && ((p.width - info.width).abs() > 0.01 || (p.height - info.height).abs() > 0.01)
        {
            let moved = Placement { width: info.width, height: info.height, ..*p };
            if let Some(clamped) = self.clamp(&moved) {
                self.apply(Event::PlacementChanged(clamped));
            }
        }
    }

    /// Klemmt eine Position auf die (sichtbare) Seite (F-35, KR-F11).
    fn clamp(&self, p: &Placement) -> Option<Placement> {
        let doc = self.current_doc()?;
        let page = if p.page_index < doc.pages.len() {
            doc.pages[p.page_index].clone()
        } else {
            pdfover_placement::new_page_geometry(doc.pages.last()?)
        };
        let t = PageTransform::new(&page, ScreenGeometry::identity());
        let pt = t.clamp_block(Point { x: p.x, y: p.y }, p.width, p.height);
        Some(Placement { x: pt.x, y: pt.y, ..*p })
    }

    fn session_ctx(&self) -> SessionContext {
        SessionContext {
            language: self.cfg.language(),
            batch_size: self.state.queue().map(|q| q.len()).unwrap_or(1).max(1),
            cancel: lock(&self.shared.sign).cancel.clone(),
            headless: !self.shared.broker.interactive(),
        }
    }

    fn effect_open(&mut self, id: BackendId) {
        // neuer Vorgang: alte Reste verwerfen, neue Epoche
        self.cancel_session();
        let caps = self.options.iter().find(|o| o.id == id).and_then(|o| o.capabilities.clone());
        lock(&self.shared.sign).caps = caps;
        let epoch = self.sign_epoch();
        let Some(backend) = self.registry.get(id) else {
            self.queue.push_back(Event::BackendFailed(BackendError::Unavailable(id.as_str().into())));
            return;
        };
        let ctx = self.session_ctx();
        let broker: Arc<dyn pdfover_backend_api::InteractionBroker> = self.shared.broker.clone();
        let shared = self.shared.clone();
        self.spawn_event(Some(epoch), async move {
            let cancel = ctx.cancel.clone();
            let opened = tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(BackendError::Cancelled),
                r = backend.open(ctx, broker) => r,
            };
            match opened {
                Ok(session) => {
                    let mut slot = lock(&shared.sign);
                    if slot.epoch == epoch {
                        slot.session = Some(session);
                    }
                    Event::PhaseCompleted(pdfover_workflow::SigningPhase::OpeningSession)
                }
                Err(e) => Event::BackendFailed(e),
            }
        });
    }

    /// Cache und Schlüssel für das Backend des laufenden Vorgangs (nur mit Opt-in).
    fn certificate_cache(&self) -> Option<(CertificateCache, CertCacheKey)> {
        let settings = &self.cfg.settings.certificate_cache;
        let State::Signing { backend, .. } = &self.state else { return None };
        if !settings.enabled || !uses_certificate_cache(*backend) {
            return None;
        }
        let account = self.cfg.effective().username().map(pdfover_config::normalize_mobile_number).unwrap_or_default();
        Some((CertificateCache::new(&self.cfg.paths, settings), CertCacheKey::new(backend.as_str(), account)))
    }

    /// Gültiges Zertifikat aus dem Cache (sonst wird der Eintrag verworfen).
    fn cached_certificate(cache: &CertificateCache, key: &CertCacheKey) -> Option<CertChain> {
        let der = match cache.get(key) {
            Ok(Some(der)) => der,
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!(id = e.message_id(), "Zertifikats-Cache nicht lesbar");
                return None;
            }
        };
        match pdfover_cades::Certificate::from_der(&der) {
            Ok(cert) if cert.is_valid_at(time::OffsetDateTime::now_utc()) => Some(CertChain::new(cert)),
            _ => {
                tracing::info!("Zertifikat im Cache ungültig oder abgelaufen – wird neu gelesen");
                let _ = cache.remove(key);
                None
            }
        }
    }

    fn effect_certificate(&mut self) {
        let cache = self.certificate_cache();
        if let Some((cache, key)) = &cache
            && let Some(chain) = Self::cached_certificate(cache, key)
        {
            // Plan §4.2: ab der zweiten Signatur entfällt das Lesen (InfoboxRead); I3 erzwingt danach die
            // Übereinstimmung mit dem Signerzertifikat der Antwort.
            tracing::info!("Zertifikat aus dem Zertifikats-Cache");
            let mut slot = lock(&self.shared.sign);
            slot.chain = Some(chain);
            slot.cert_cache = Some((cache.clone(), key.clone(), true));
            drop(slot);
            self.queue.push_back(Event::PhaseCompleted(pdfover_workflow::SigningPhase::ReadingCertificate));
            return;
        }
        lock(&self.shared.sign).cert_cache = cache.map(|(c, k)| (c, k, false));
        let shared = self.shared.clone();
        let (epoch, session, cancel) = {
            let mut slot = lock(&shared.sign);
            (slot.epoch, slot.session.take(), slot.cancel.clone())
        };
        let Some(mut session) = session else {
            self.queue.push_back(Event::BackendFailed(BackendError::Internal("keine Sitzung".into())));
            return;
        };
        self.spawn_event(Some(epoch), async move {
            let r = tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(BackendError::Cancelled),
                r = session.certificate() => r,
            };
            if r.is_err() && cancel.is_cancelled() {
                session.cancel().await;
            }
            let mut slot = lock(&shared.sign);
            if slot.epoch != epoch {
                return Event::BackendFailed(BackendError::Cancelled);
            }
            slot.session = Some(session);
            match r {
                Ok(chain) => {
                    slot.chain = Some(chain);
                    Event::PhaseCompleted(pdfover_workflow::SigningPhase::ReadingCertificate)
                }
                Err(e) => Event::BackendFailed(e),
            }
        });
    }

    fn effect_prepare(&mut self, doc: DocId, position: PositionChoice, min_reserve: Option<usize>) {
        let Some(data) = self.shared.docs.get(doc) else {
            self.queue.push_back(Event::SigningFailed(WorkflowError::Unexpected("Dokument nicht geladen".into())));
            return;
        };
        let shared = self.shared.clone();
        let snap = self.snapshot();
        let (epoch, chain, caps) = {
            let slot = lock(&shared.sign);
            (slot.epoch, slot.chain.clone(), slot.caps.clone())
        };
        let backend = match &self.state {
            State::Signing { backend, .. } => Some(*backend),
            _ => None,
        };
        // „Signatur fortsetzen“: genau einmal wiederherstellen – nur für dieses Dokument, dasselbe
        // Signiermittel und ohne vergrößerte Reserve (sonst neu vorbereiten)
        let restore = self
            .resume_active
            .take()
            .filter(|(id, p)| *id == doc && Some(p.record.backend) == backend && min_reserve.is_none())
            .map(|(_, p)| p);
        // Mobile: nach `prepare` für „Signatur fortsetzen“ sichern (ersetzt ein nicht angenommenes Angebot)
        let persist = match (self.mobile.persist_prepared, backend) {
            (true, Some(backend)) if restore.is_none() => {
                self.resume_offer = None;
                self.persist_owned = true;
                let pending =
                    self.state.queue().map(|q| q.pending.iter().map(|d| d.path.clone()).collect()).unwrap_or_default();
                let name = self
                    .state
                    .queue()
                    .and_then(|q| q.current.as_ref())
                    .map(|d| d.display_name.clone())
                    .unwrap_or_else(|| data.name.clone());
                Some(PersistJob { dir: self.resume_dir(), name, backend, pending })
            }
            _ => None,
        };
        self.spawn_blocking_event(Some(epoch), signing_panic, move || {
            if let Some(persisted) = &restore {
                match persisted.restore(shared.engine.as_ref(), &data.src) {
                    Ok(prepared) => {
                        tracing::info!("vorbereitetes Dokument wiederhergestellt („Signatur fortsetzen“)");
                        let profile = prepared
                            .cms_profile()
                            .unwrap_or_else(|| plan::cms_profile(snap.settings.signature.cms_profile));
                        return store_prepared(&shared, epoch, prepared, profile);
                    }
                    Err(code) => {
                        // neu vorbereiten (mit dem eben gelesenen Zertifikat); der alte Zustand ist verbraucht
                        tracing::warn!(code = %code, "Wiederherstellen fehlgeschlagen – neu vorbereiten");
                        shared.send(Msg::Notices(vec![(resume::MSG_RESTORE_FAILED.to_owned(), BTreeMap::new())]));
                    }
                }
            }
            let model_a = caps.as_ref().is_some_and(|c| matches!(c.input, SignInput::Digest { .. }));
            let reserve = match (min_reserve, model_a) {
                (Some(n), _) => Reserve::Bytes(n),
                (None, true) => Reserve::ModelA,
                (None, false) => Reserve::ModelB,
            };
            let Some(store) = shared.profiles.as_deref() else {
                return Event::SigningFailed(prepare_error("sigblock-missing-resource", "Profile"));
            };
            let emblem = snap.effective().emblem().and_then(plan::load_emblem);
            // QR-Platzhalter mit `profile=…` ersetzt das Profil (PDF-AS `placeholder_profile_overwrite`)
            let qr_profile = match &position {
                PositionChoice::Placeholder(qr) => qr.payload.profile.as_deref(),
                _ => None,
            };
            let spec = match plan::placeholder_block_spec(
                store,
                SpecInput {
                    effective: snap.effective(),
                    system_language: Lang::system(),
                    pdfa: data.report.pdfa.as_ref(),
                    cert: chain.as_ref().map(|c| &c.signer),
                    signing_time: plan::local_now(),
                    emblem: emblem.as_ref(),
                },
                qr_profile,
            ) {
                Ok((s, notice)) => {
                    if let Some((code, profile)) = notice {
                        let params = BTreeMap::from([("profile".to_owned(), profile)]);
                        shared.send(Msg::Notices(vec![(code.to_owned(), params)]));
                    }
                    s
                }
                Err(e) => return Event::SigningFailed(prepare_error(&e.code(), &e.to_string())),
            };
            let cms_profile = plan::cms_profile(snap.settings.signature.cms_profile);
            let built = data.with_renderer_opt(shared.render.as_deref(), |renderer| {
                plan::build_plan(PlanInput {
                    report: &data.report,
                    pages: &data.pages,
                    position: &position,
                    spec: &spec,
                    chain: chain.as_ref(),
                    reserve,
                    cms_profile,
                    renderer,
                    store,
                    fonts: &shared.fonts,
                })
            });
            let built = match built {
                Ok(b) => b,
                Err(e) => return Event::SigningFailed(prepare_error(&e.code(), &e.to_string())),
            };
            if !built.notices.is_empty() {
                shared.send(Msg::Notices(built.notices.iter().map(|c| (c.clone(), BTreeMap::new())).collect()));
            }
            let prepared = match shared.engine.prepare(&data.src, &built.plan, chain.as_ref()) {
                Ok(p) => p,
                Err(e) => return Event::SigningFailed(pades_error(&e)),
            };
            if let Some(job) = &persist {
                job.save(&data, prepared.as_ref(), &position);
            }
            store_prepared(&shared, epoch, prepared, cms_profile)
        });
    }

    fn effect_sign(&mut self) {
        let shared = self.shared.clone();
        let name = self.current_doc().map(|d| d.name.clone()).unwrap_or_default();
        let (epoch, session, prepared, chain, caps, cancel, cms_profile) = {
            let mut slot = lock(&shared.sign);
            (
                slot.epoch,
                slot.session.take(),
                slot.prepared.take(),
                slot.chain.clone(),
                slot.caps.clone(),
                slot.cancel.clone(),
                slot.cms_profile.unwrap_or_else(|| plan::cms_profile(self.cfg.settings.signature.cms_profile)),
            )
        };
        let (Some(mut session), Some(prepared)) = (session, prepared) else {
            self.queue.push_back(Event::BackendFailed(BackendError::Internal("Sitzung oder Dokument fehlt".into())));
            return;
        };
        let model_a = caps.as_ref().is_some_and(|c| matches!(c.input, SignInput::Digest { .. }));
        self.spawn_event(Some(epoch), async move {
            // Auftrag bauen
            let (job, holder, attrs) = if model_a {
                let Some(chain) = chain.as_ref() else {
                    return Event::SigningFailed(prepare_error("backend-internal", "Zertifikat fehlt"));
                };
                match digest_job(prepared.as_ref(), chain, cms_profile, &name) {
                    Ok((job, attrs)) => (SignJob::Digest(job), PreparedHolder::Owned(prepared), Some(attrs)),
                    Err(e) => return Event::SigningFailed(prepare_error(e.code(), &e.to_string())),
                }
            } else {
                let source = Arc::new(PreparedSource { name: name.clone(), inner: prepared });
                let job = SignJob::Document(DocumentJob {
                    source: source.clone(),
                    expected_signer: chain.as_ref().map(|c| c.signer.clone()),
                });
                (job, PreparedHolder::Shared(source), None)
            };
            let r = tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(BackendError::Cancelled),
                r = session.sign(vec![job]) => r,
            };
            if r.is_err() && cancel.is_cancelled() {
                session.cancel().await;
            }
            let prepared = match holder.into_inner() {
                Some(p) => p,
                None => return Event::BackendFailed(BackendError::Internal("Dokument wird noch gehalten".into())),
            };
            let mut slot = lock(&shared.sign);
            if slot.epoch != epoch {
                return Event::BackendFailed(BackendError::Cancelled);
            }
            slot.session = Some(session);
            slot.prepared = Some(prepared);
            match r {
                Ok(mut results) if results.len() == 1 => {
                    slot.result = results.pop();
                    slot.attrs = attrs;
                    Event::PhaseCompleted(pdfover_workflow::SigningPhase::Signing)
                }
                Ok(results) => Event::BackendFailed(BackendError::Internal(format!(
                    "{} Ergebnisse für einen Auftrag",
                    results.len()
                ))),
                Err(e) => Event::BackendFailed(e),
            }
        });
    }

    fn effect_finish(&mut self) {
        let shared = self.shared.clone();
        let data = self.current_doc();
        let (epoch, prepared, result, attrs, chain, cert_cache) = {
            let mut slot = lock(&shared.sign);
            (
                slot.epoch,
                slot.prepared.take(),
                slot.result.take(),
                slot.attrs.take(),
                slot.chain.clone(),
                slot.cert_cache.clone(),
            )
        };
        self.spawn_blocking_event(Some(epoch), signing_panic, move || {
            let (Some(data), Some(prepared), Some(result)) = (data, prepared, result) else {
                return Event::SigningFailed(WorkflowError::Unexpected("Signaturergebnis fehlt".into()));
            };
            let finished = match (result, attrs) {
                (SignResult::Cms(cms), _) => {
                    let expect = Expect {
                        signer: chain
                            .as_ref()
                            .map_or(ExpectedSigner::Any, |c| ExpectedSigner::Certificate(c.signer.clone())),
                        ..Expect::default()
                    };
                    prepared.finish_cms(&cms, &expect)
                }
                (SignResult::Raw { signature, chain: result_chain }, Some(attrs)) => {
                    prepared.finish_raw(&attrs, &signature, &result_chain)
                }
                (SignResult::Raw { .. }, None) => {
                    return Event::SigningFailed(WorkflowError::Unexpected("Attribute fehlen".into()));
                }
            };
            let signed = match finished {
                Ok(s) => s,
                Err(FinishError::CmsTooLarge { needed, reserve }) => return Event::CmsTooLarge { needed, reserve },
                Err(FinishError::Cms(e)) => {
                    if let Some((cache, key, true)) = &cert_cache
                        && is_signer_mismatch(e.code())
                    {
                        // Zertifikat erneuert bzw. anderes Konto: Ergebnis verwerfen, Eintrag löschen;
                        // „Wiederholen“ liest das Zertifikat neu und baut den Block neu (Plan §4.2).
                        tracing::warn!(code = e.code(), "Zertifikat aus dem Cache passt nicht – Eintrag gelöscht");
                        let _ = cache.remove(key);
                        return Event::SigningFailed(prepare_error(MSG_CERT_CACHE_MISMATCH, e.code()));
                    }
                    return Event::SelfCheckFailed(InvariantViolation {
                        invariant: Invariant::I3Cms,
                        detail: e.code().into(),
                    });
                }
                Err(FinishError::Incremental(e)) => {
                    return Event::SigningFailed(prepare_error("error-PrepareDocument", &e.to_string()));
                }
            };
            let check = CheckContext {
                source: &data.src,
                expected_signer: chain.as_ref().map(|c| &c.signer),
                renderer: shared.render.as_deref(),
            };
            match signed.self_check(&check) {
                Ok(report) => {
                    for w in &report.warnings {
                        tracing::info!(warning = %w, "Selbstprüfung: Hinweis");
                    }
                }
                Err(v) => return Event::SelfCheckFailed(v),
            }
            // bestätigtes Signerzertifikat merken (nur mit Opt-in; nur das öffentliche Zertifikat)
            if let (Some((cache, key, _)), Some(chain)) = (&cert_cache, &chain)
                && let Err(e) = cache.put(key, chain.signer.as_der())
            {
                tracing::warn!(id = e.message_id(), "Zertifikats-Cache nicht beschreibbar");
            }
            let mut slot = lock(&shared.sign);
            if slot.epoch != epoch {
                return Event::BackendFailed(BackendError::Cancelled);
            }
            slot.signed = Some(Arc::from(signed));
            Event::PhaseCompleted(pdfover_workflow::SigningPhase::Finishing)
        });
    }

    fn effect_decide(&mut self, doc: DocId) {
        let Some(input) = self.state.queue().and_then(|q| q.current.clone()).filter(|d| d.id == doc) else { return };
        // Mobile: ohne Einstellung in die App-Dokumente (kein Speichern-Dialog; Teilen/Exportieren danach)
        let output = self.cfg.output().or_else(|| {
            let dir = self.mobile.default_output_dir.clone()?;
            match std::fs::create_dir_all(&dir) {
                Ok(()) => Some(dir),
                Err(e) => {
                    tracing::warn!(kind = ?e.kind(), "App-Dokumente nicht anlegbar – Speichern-Dialog");
                    None
                }
            }
        });
        let d = decide_output(&input.path, output.as_deref(), &self.cfg.settings.output.postfix, &FsProbe);
        let d = if self.headless { resolve_without_dialog(d, &FsProbe) } else { d };
        self.queue.push_back(Event::OutputDecided(d));
    }

    fn effect_write(&mut self, path: PathBuf, overwrite: bool) {
        let signed = lock(&self.shared.sign).signed.clone();
        let Some(signed) = signed else {
            self.queue.push_back(Event::SaveFailed("kein signiertes Dokument".into()));
            return;
        };
        self.spawn_blocking_event(None, Event::SaveFailed, move || {
            if !overwrite && path.exists() {
                return Event::SaveFailed(format!("{} existiert bereits", file_name(&path)));
            }
            match signed.write_atomically(&OutputTarget { path, overwrite }) {
                Ok(info) => Event::Saved(info),
                Err(e) => Event::SaveFailed(e.to_string()),
            }
        });
    }

    fn effect_open_result(&mut self, path: &Path) {
        let result = match self.shared.shell() {
            Some(shell) => shell.open_path(path),
            None => Err("keine Shell".into()),
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "Ergebnis konnte nicht geöffnet werden");
            let mut p = BTreeMap::new();
            p.insert("arg1".into(), file_name(path));
            self.notify("error-FailedToOpenDocument", p);
        }
    }

    /// Ergebnis teilen (Mobile, Share-Sheet); im Hintergrund, Fehler als Hinweis.
    fn effect_share(&mut self, path: PathBuf) {
        let Some(shell) = self.shared.shell() else {
            self.notify("ng-share-failed", BTreeMap::new());
            return;
        };
        let shared = self.shared.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = shell.share_path(&path) {
                tracing::warn!(error = %e, "Teilen fehlgeschlagen");
                shared.send(Msg::Notices(vec![("ng-share-failed".to_owned(), BTreeMap::new())]));
            }
        });
    }

    /// Ergebnis exportieren („Sichern unter“, Mobile); im Hintergrund, Ergebnis als Hinweis.
    fn effect_export(&mut self, path: PathBuf) {
        let Some(shell) = self.shared.shell() else {
            self.notify("ng-export-failed", BTreeMap::new());
            return;
        };
        let shared = self.shared.clone();
        let name = file_name(&path);
        tokio::task::spawn_blocking(move || {
            let notice = match shell.export_path(&path, &name) {
                Ok(true) => Some("ng-export-done"),
                Ok(false) => None,
                Err(e) => {
                    tracing::warn!(error = %e, "Exportieren fehlgeschlagen");
                    Some("ng-export-failed")
                }
            };
            if let Some(code) = notice {
                let params = BTreeMap::from([("name".to_owned(), name)]);
                shared.send(Msg::Notices(vec![(code.to_owned(), params)]));
            }
        });
    }

    /// „Einstellungen zurücksetzen“ (F-05, Plan §7.1): nach `.old` verschieben, neu laden.
    fn reset_settings(&mut self, restart: bool) {
        let paths = self.cfg.paths.clone();
        match paths.reset() {
            Ok(backup) => {
                let mut p = BTreeMap::new();
                p.insert("path".into(), backup.display().to_string());
                self.notify("ng-config-reset-done", p);
            }
            Err(e) => tracing::warn!(id = e.message_id(), "Zurücksetzen fehlgeschlagen"),
        }
        let mut overlay = self.cfg.overlay.clone();
        overlay.config_file = None;
        match ConfigState::load(paths, overlay, self.cfg.policy.clone(), self.cfg.secrets.clone(), None, false) {
            Ok((cfg, _)) => self.cfg = cfg,
            Err(e) => tracing::warn!(id = e.message_id(), "Einstellungen nach Zurücksetzen nicht geladen"),
        }
        self.ctx = self.cfg.context(self.headless);
        self.registry.configure(&self.cfg);
        if restart {
            let docs = self.input_documents(&self.cfg.overlay.input_files.clone());
            self.queue.push_back(Event::Started { documents: docs });
        }
    }

    // --------------------------------------------------------------------------------------------
    // UI-Ereignisse
    // --------------------------------------------------------------------------------------------

    pub(crate) fn on_ui(&mut self, event: UiEvent) {
        tracing::debug!(event = ui_event_kind(&event), "UI");
        match event {
            UiEvent::OpenFiles { paths } => {
                let docs = self.input_documents(&paths);
                if !docs.is_empty() {
                    self.apply(Event::FilesAdded(docs));
                }
            }
            UiEvent::AnswerWarning { proceed } => self.apply(Event::WarningAnswered(proceed)),
            UiEvent::PlaceholderAnswer { answer, field_index } => {
                self.apply(Event::PlaceholderAnswered { answer, field_index });
            }
            UiEvent::MoveBlock { page, x, y } => {
                if let State::Positioning { placement: Some(p), .. } = &self.state
                    && x.is_finite()
                    && y.is_finite()
                {
                    let moved = Placement { page_index: page, x, y, ..*p };
                    if let Some(c) = self.clamp(&moved) {
                        self.apply(Event::PlacementChanged(c));
                    }
                }
            }
            UiEvent::NudgeBlock { dx, dy, fine, coarse } => {
                if let State::Positioning { placement: Some(p), .. } = &self.state {
                    let step = if coarse {
                        pdfover_placement::KEYBOARD_COARSE_STEP_PT
                    } else if fine {
                        pdfover_placement::KEYBOARD_FINE_STEP_PT
                    } else {
                        pdfover_placement::KEYBOARD_STEP_PT
                    };
                    let moved = Placement { x: p.x + dx as f32 * step, y: p.y + dy as f32 * step, ..*p };
                    if let Some(c) = self.clamp(&moved) {
                        self.apply(Event::PlacementChanged(c));
                    }
                }
            }
            UiEvent::ChangePage { page } => self.apply(Event::PageChanged(page)),
            UiEvent::ToggleNewPage => self.apply(Event::NewPageToggled),
            UiEvent::AutoPlace => self.auto_place(),
            UiEvent::ConfirmPosition => self.apply(Event::PositionConfirmed),
            UiEvent::ChooseInvisible => self.apply(Event::InvisibleChosen),
            UiEvent::ChooseBackend { backend } => self.apply(Event::BackendChosen(backend)),
            UiEvent::AcknowledgePrivacy { accepted } => self.apply(Event::PrivacyAnswered(accepted)),
            UiEvent::SubmitForm { request, answer } => {
                if !self.shared.broker.answer_form(request, answer) {
                    tracing::debug!(request, "Maskenantwort ohne offene Anfrage");
                }
            }
            UiEvent::SubmitSecret { request, secret } => {
                let secret = zeroize::Zeroizing::new(secret);
                if !self.shared.broker.answer_secret(request, secret) {
                    tracing::debug!(request, "Passwortantwort ohne offene Anfrage");
                }
            }
            UiEvent::AnswerNotice { request, answer } => {
                if !self.shared.broker.answer_notice(request, answer) {
                    tracing::debug!(request, "Antwort ohne offene Meldung");
                }
            }
            UiEvent::OpenSignatureData { request } => self.open_signature_data(request),
            UiEvent::Cancel => {
                // Signieren mit offener Maske: Maske abbrechen (das Backend meldet den Abbruch, F-72)
                if matches!(self.state, State::Signing { .. }) && self.shared.broker.cancel_current() {
                    return;
                }
                self.apply(Event::Cancel);
            }
            UiEvent::Retry => {
                if matches!(self.state, State::Starting) {
                    return;
                }
                self.apply(Event::Retry);
            }
            UiEvent::Save { path: Some(path) } => {
                self.dialog = None;
                self.apply(Event::OutputDecided(chosen_path(path, &FsProbe)));
            }
            UiEvent::Save { path: None } => {
                let dialog_open =
                    matches!(self.state, State::Output { decision: Some(OutputDecision::Ask { .. }), .. });
                self.dialog = None;
                self.apply(if dialog_open { Event::OutputChosen(None) } else { Event::SaveRequested });
            }
            UiEvent::ConfirmOverwrite { overwrite } => {
                self.dialog = None;
                self.apply(Event::OverwriteAnswered(overwrite));
            }
            UiEvent::OpenResult => self.apply(Event::OpenResultRequested),
            UiEvent::ShareResult => self.apply(Event::ShareResultRequested),
            UiEvent::ExportResult => self.apply(Event::ExportResultRequested),
            UiEvent::Navigate { step } => {
                if step != Step::Finish {
                    self.apply(Event::Navigate(step));
                }
            }
            UiEvent::OpenSettings => {
                if self.state.settings_enabled() || matches!(self.state, State::Error { .. }) {
                    self.settings_view = Some(self.settings_view_for(self.cfg.settings.clone(), BTreeMap::new()));
                }
            }
            UiEvent::SaveSettings { settings } => {
                // nur aus der geöffneten Einstellungsansicht (Overlay); sonst könnten Einstellungen z. B.
                // mitten im Signieren wechseln
                if self.settings_view.is_some() {
                    self.save_settings(*settings);
                } else {
                    tracing::debug!("Einstellungen ohne geöffnete Ansicht verworfen");
                }
            }
            UiEvent::CloseSettings => self.settings_view = None,
            UiEvent::ResetSettings => {
                self.settings_view = None;
                if matches!(&self.state, State::Error { error: WorkflowError::Config(_), .. }) {
                    self.apply(Event::Retry);
                } else {
                    self.reset_settings(false);
                    self.apply(Event::SettingsChanged);
                }
            }
            UiEvent::Backgrounded => {
                // kein Abbruch: Sitzung, Masken und persistierter Zustand bleiben (Plan §3.5)
                self.backgrounded = true;
                self.apply(Event::Backgrounded);
            }
            UiEvent::Foregrounded => self.foregrounded(),
            UiEvent::ResumeSignature => self.resume_signature(),
            UiEvent::DiscardResume => self.discard_resume(),
            UiEvent::ImportKeystore { path, password, remember } => {
                let password = password.map(zeroize::Zeroizing::new);
                self.import_keystore(&path, password, remember);
            }
            UiEvent::DismissKeystoreImport => self.keystore_import = None,
            UiEvent::Quit => self.apply(Event::Quit),
            UiEvent::DismissNotice { id } => self.notices.retain(|n| n.id != id),
            UiEvent::AcceptMigration { store_secrets } => self.accept_migration(store_secrets),
            UiEvent::DeclineMigration => {
                self.cfg.migration_offer = None;
                if let Err(e) = self.cfg.save() {
                    tracing::warn!(id = e.message_id(), "Standardeinstellungen nicht gespeichert");
                }
            }
        }
    }

    fn settings_view_for(&self, settings: Settings, problems: BTreeMap<String, String>) -> SettingsView {
        SettingsView {
            settings,
            problems,
            keychain_available: self.cfg.secrets.is_persistent(),
            fido2_available: self.fido2_available,
            read_only: self.cfg.read_only(),
        }
    }

    fn save_settings(&mut self, mut settings: Settings) {
        settings.sanitize();
        let (problems, blocking) = validate(&settings);
        if blocking {
            self.settings_view = Some(self.settings_view_for(settings, problems));
            return;
        }
        self.cfg.settings = settings;
        if let Err(e) = self.cfg.save() {
            tracing::warn!(id = e.message_id(), "Einstellungen nicht gespeichert");
            self.notify("error-FailedToSaveSettings", BTreeMap::new());
            self.settings_view = Some(self.settings_view_for(self.cfg.settings.clone(), problems));
            return;
        }
        self.settings_view = None;
        for (field, code) in problems {
            let mut p = BTreeMap::new();
            p.insert("field".into(), field);
            self.notify(&code, p);
        }
        self.ctx = self.cfg.context(self.headless);
        self.registry.configure(&self.cfg);
        self.apply(Event::SettingsChanged);
    }

    fn accept_migration(&mut self, store_secrets: bool) {
        let Some(file) = self.cfg.migration_offer.take() else { return };
        let legacy_dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let result = pdfover_config::LegacyConfig::load(&file).map(|legacy| {
            let ctx =
                pdfover_config::MigrationContext { legacy_dir, system_language: Lang::system(), check_paths: true };
            pdfover_config::migrate(&legacy, &ctx)
        });
        let applied = result.and_then(|r| {
            let opts = pdfover_config::ApplyOptions { store_secrets, ..pdfover_config::ApplyOptions::default() };
            pdfover_config::apply_migration(&r, &self.cfg.paths, self.cfg.secrets.as_ref(), &opts).map(|a| (r, a))
        });
        match applied {
            Ok((r, _)) => {
                let mut p = BTreeMap::new();
                p.insert("imported".into(), r.report.imported.len().to_string());
                p.insert("skipped".into(), r.report.skipped.len().to_string());
                self.notify("ng-migrate-done", p);
                self.cfg.settings = r.settings;
                self.ctx = self.cfg.context(self.headless);
                self.registry.configure(&self.cfg);
                self.apply(Event::SettingsChanged);
            }
            Err(e) => {
                let mut p = BTreeMap::new();
                p.insert("detail".into(), e.to_string());
                self.notify(e.message_id(), p);
            }
        }
    }

    fn open_signature_data(&mut self, request: u64) {
        let Some(ctx) = self.shared.broker.form_context(request) else { return };
        let Some(url) = ctx.signature_data_url else { return };
        let endpoint = self
            .registry
            .atrust()
            .map(|a| a.config().endpoint.clone())
            .or_else(|| pdfover_net::Url::parse(pdfover_backend_atrust::PROD_ENDPOINT).ok());
        let Some(endpoint) = endpoint else { return };
        if let Err(e) = pdfover_backend_atrust::open_signature_data(self.shared.broker.as_ref(), &url, &endpoint) {
            tracing::warn!(code = e.code(), "Signaturdaten nicht geöffnet");
            self.notify("atrust-signature-data-link-rejected", BTreeMap::new());
        }
    }

    /// „Automatisch platzieren“: Position wie bei `p:auto` berechnen und anzeigen.
    fn auto_place(&mut self) {
        let State::Positioning { doc: facts, .. } = &self.state else { return };
        let doc_id = facts.id;
        let Some(data) = self.shared.docs.get(doc_id) else { return };
        let shared = self.shared.clone();
        let snap = self.snapshot();
        tokio::task::spawn_blocking(move || {
            let Some(store) = shared.profiles.as_deref() else { return };
            let emblem = snap.effective().emblem().and_then(plan::load_emblem);
            let Ok(spec) = plan::block_spec(
                store,
                SpecInput {
                    effective: snap.effective(),
                    system_language: Lang::system(),
                    pdfa: data.report.pdfa.as_ref(),
                    cert: None,
                    signing_time: plan::local_now(),
                    emblem: emblem.as_ref(),
                },
            ) else {
                return;
            };
            let built = data.with_renderer_opt(shared.render.as_deref(), |renderer| {
                plan::build_plan(PlanInput {
                    report: &data.report,
                    pages: &data.pages,
                    position: &PositionChoice::Auto,
                    spec: &spec,
                    chain: None,
                    reserve: Reserve::ModelB,
                    cms_profile: Default::default(),
                    renderer,
                    store,
                    fonts: &shared.fonts,
                })
            });
            match built.map(|b| b.resolved) {
                Ok(Some(r)) => {
                    let page_index = if r.new_page { data.pages.len() } else { r.page_index };
                    let p = Placement { page_index, x: r.x, y: r.y, width: r.width, height: r.height };
                    shared.send(Msg::AutoPlaced(doc_id, p));
                }
                Ok(None) => {}
                Err(e) => shared.send(Msg::Notices(vec![(e.code(), BTreeMap::new())])),
            }
        });
    }

    // --------------------------------------------------------------------------------------------
    // Backend-Ereignisse (Persistenz liegt beim Kern, ADR 0001 §5.5)
    // --------------------------------------------------------------------------------------------

    fn on_backend_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::LoginSucceeded { username, remember } => {
                if remember {
                    let normalized = pdfover_backend_atrust::validate::normalize_phone_number(&username);
                    let s = &mut self.cfg.settings.id_austria;
                    if s.username.as_deref() != Some(normalized.as_str()) || !s.remember {
                        s.username = Some(normalized);
                        s.remember = true;
                        if let Err(e) = self.cfg.save() {
                            tracing::warn!(id = e.message_id(), "Benutzername nicht gespeichert");
                        }
                        // das Backend kennt die Daten schon (Passwort im RAM) – nicht neu bauen
                        self.registry.adopt_atrust_settings(&self.cfg);
                    }
                }
            }
            BackendEvent::ForgetCredentials => {
                let s = &mut self.cfg.settings.id_austria;
                if s.username.is_some() || s.remember {
                    s.username = None;
                    s.remember = false;
                    if let Err(e) = self.cfg.save() {
                        tracing::warn!(id = e.message_id(), "Einstellungen nicht gespeichert");
                    }
                    self.registry.adopt_atrust_settings(&self.cfg);
                }
            }
            BackendEvent::ServerTime(server) => {
                let skew = (time::OffsetDateTime::now_utc() - server).whole_seconds().abs();
                if skew > 300 {
                    let mut p = BTreeMap::new();
                    p.insert("minutes".into(), (skew / 60).to_string());
                    self.notify("ng-clock-skew", p);
                }
            }
        }
    }

    // --------------------------------------------------------------------------------------------
    // ViewState
    // --------------------------------------------------------------------------------------------

    fn publish(&self) {
        let view = self.project();
        self.view_tx.send_if_modified(|v| {
            if *v == view {
                false
            } else {
                *v = view;
                true
            }
        });
    }

    fn queue_items(&self) -> Vec<QueueItemView> {
        let Some(q) = self.state.queue() else { return Vec::new() };
        q.current
            .iter()
            .map(|d| (d, true))
            .chain(q.pending.iter().map(|d| (d, false)))
            .map(|(d, current)| QueueItemView { id: d.id, name: d.display_name.clone(), current })
            .collect()
    }

    fn doc_info(&self, id: DocId) -> DocInfo {
        match self.shared.docs.get(id) {
            Some(d) => DocInfo {
                id,
                name: d.name.clone(),
                page_count: d.pages.len(),
                page_sizes: d.page_sizes(),
                signed: d.report.is_signed(),
            },
            None => {
                let name = self
                    .state
                    .queue()
                    .and_then(|q| q.current.iter().chain(q.pending.iter()).find(|d| d.id == id))
                    .map(|d| d.display_name.clone())
                    .unwrap_or_default();
                DocInfo { id, name, page_count: 0, page_sizes: Vec::new(), signed: false }
            }
        }
    }

    pub(crate) fn project(&self) -> ViewState {
        let step = self.step_view(&self.state);
        let settings_open = self.settings_view.is_some();
        let main_bar = MainBarView {
            steps: self.state.main_bar().to_vec(),
            settings_enabled: !settings_open && self.state.settings_enabled(),
        };
        let done = self
            .state
            .results()
            .iter()
            .map(|o| DoneItemView {
                id: o.doc.id,
                name: o.doc.display_name.clone(),
                input: o.doc.path.clone(),
                result: o.result.clone(),
            })
            .collect();
        ViewState {
            step,
            main_bar,
            settings: self.settings_view.clone(),
            language: self.cfg.language(),
            notices: self.notices.clone(),
            migration: self
                .migration_view
                .clone()
                .filter(|m| self.cfg.migration_offer.as_ref() == Some(&m.legacy_file)),
            done,
            platform: PlatformView::from(self.mobile.platform),
            resume: self.resume_offer.as_ref().filter(|_| matches!(self.state, State::Idle { .. })).map(|p| {
                ResumeOfferView {
                    name: p.record.name.clone(),
                    backend: p.record.backend,
                    age_minutes: p.age_minutes(SystemTime::now()),
                    pending: u32::try_from(p.record.pending.len()).unwrap_or(u32::MAX),
                }
            }),
            keystore_import: self.keystore_import.clone(),
        }
    }

    fn step_view(&self, state: &State) -> StepView {
        match state {
            State::Starting => StepView::Starting,
            State::Idle { .. } => StepView::Open { queue: self.queue_items() },
            State::Analyzing { .. } | State::Resuming { doc: None, .. } => {
                StepView::Loading { queue: self.queue_items() }
            }
            // Fortsetzen: Dokument geladen, Signiermittel wird geprüft
            State::Resuming { doc: Some(_), .. } => {
                StepView::Signing { interaction: self.shared.broker.current(), queue: self.queue_items() }
            }
            State::AnalysisWarning { warnings, doc, .. } => {
                StepView::Warning { doc: self.doc_info(doc.id), codes: warnings.clone() }
            }
            State::PlaceholderQuestion { found, doc, .. } => {
                let (fields, qr) = match found {
                    pdfover_workflow::PlaceholderFound::Fields(f) => (
                        f.iter().map(|c| FieldInfo { name: c.qualified_name.clone(), page: c.page_index }).collect(),
                        None,
                    ),
                    pdfover_workflow::PlaceholderFound::Qr(q) => {
                        (Vec::new(), Some(QrInfo { page: q.page_index, id: q.payload.id.clone() }))
                    }
                };
                StepView::Placeholder { doc: self.doc_info(doc.id), fields, qr }
            }
            State::Positioning { page, placement, new_page, doc, .. } => {
                let block = lock(&self.shared.block).clone();
                let placement = placement.or(doc.start).unwrap_or(Placement {
                    page_index: *page,
                    x: 0.0,
                    y: 0.0,
                    width: block.width,
                    height: block.height,
                });
                let mut announce = BTreeMap::new();
                announce.insert("page".into(), (placement.page_index + 1).to_string());
                announce.insert("pages".into(), (doc.page_count + usize::from(*new_page)).to_string());
                let mm = |pt: f32| format!("{:.0}", pt * 25.4 / 72.0);
                // Live-Region und Zahlenfelder nennen dieselben Kanten (PAGE_ABS): linke Kante und
                // Oberkante, jeweils von links bzw. von unten
                announce.insert("left".into(), mm(placement.x));
                announce.insert("top".into(), mm(placement.y));
                StepView::Position {
                    doc: self.doc_info(doc.id),
                    page: *page,
                    block: BlockView {
                        width: if block.doc == Some(doc.id) { block.width } else { placement.width },
                        height: if block.doc == Some(doc.id) { block.height } else { placement.height },
                        preview_revision: block.revision,
                    },
                    placement,
                    new_page: *new_page,
                    announce,
                }
            }
            State::ChoosingBackend { options, .. } => {
                StepView::Backend { options: option_views(options, self.mobile.platform), privacy_notice: None }
            }
            State::PrivacyNotice { backend, options, .. } => StepView::Backend {
                options: option_views(options, self.mobile.platform),
                privacy_notice: Some(PrivacyNoticeView {
                    backend: *backend,
                    version: PRIVACY_TEXT_VERSION,
                    custom_text: self.cfg.policy.as_ref().and_then(|p| p.privacy_notice_text.clone()),
                }),
            },
            State::Signing { .. } => {
                StepView::Signing { interaction: self.shared.broker.current(), queue: self.queue_items() }
            }
            State::Output { saved, decision, queue, .. } => {
                let (ask_overwrite, save_dialog) = match &self.dialog {
                    Some(OutputDialog::Overwrite(p)) => (Some(p.clone()), None),
                    Some(OutputDialog::Save(d)) => (None, Some(d.clone())),
                    None => (None, None),
                };
                StepView::Output {
                    result: OutputView {
                        path: saved.as_ref().map(|s| s.path.clone()),
                        save_pending: saved.is_none() && decision.is_none(),
                        ask_overwrite,
                        save_dialog,
                    },
                    next: queue.pending.front().map(|d| self.doc_info(d.id)),
                }
            }
            State::Error { error, retry, from, .. } => {
                let mut params = error.params();
                if let WorkflowError::Save(_) = error
                    && let Some(State::Output { decision: Some(OutputDecision::Save { path, .. }), .. }) =
                        from.as_deref()
                {
                    params.insert("arg1".into(), file_name(path));
                }
                // JKS/JCEKS: Konvertierungsassistent mit `keytool`-Aufruf (Plan §4.9)
                if let WorkflowError::Backend { code, detail } = error
                    && code == "backend-keystore-format"
                    && let Some(file) = self.cfg.effective().keystore_file()
                {
                    let format = match detail.as_str() {
                        "JKS" => pdfover_backend_keystore::KeystoreFormat::Jks,
                        "JCEKS" => pdfover_backend_keystore::KeystoreFormat::Jceks,
                        _ => pdfover_backend_keystore::KeystoreFormat::Unknown,
                    };
                    if let Some(hint) = pdfover_backend_keystore::conversion_hint(format, file) {
                        params.insert("hint".into(), hint);
                    }
                }
                StepView::Error { code: error.code(), params, retry: retry.is_some() }
            }
            State::Finished { exit, .. } => StepView::Finished { exit_code: *exit as u8 },
        }
    }
}

fn option_views(options: &[BackendOption], platform: Platform) -> Vec<BackendOptionView> {
    options
        .iter()
        .map(|o| {
            let mut notes = Vec::new();
            // Mobile: ID Austria ohne FIDO2 (Plan §4.7) – FIDO2 ausgegraut mit Hinweis
            if !platform.is_desktop()
                && o.id == BackendId::AtrustNative
                && o.capabilities.as_ref().is_none_or(|c| !c.factors.fido2)
            {
                notes.push(MSG_FIDO2_UNAVAILABLE_MOBILE.to_owned());
            }
            BackendOptionView { id: o.id, available: o.available(), capabilities: o.capabilities.clone(), notes }
        })
        .collect()
}

/// Sicherung des vorbereiteten Zustands nach `prepare` (Mobile, „Signatur fortsetzen“).
struct PersistJob {
    dir: PathBuf,
    name: String,
    backend: BackendId,
    pending: Vec<PathBuf>,
}

impl PersistJob {
    /// Sichert; Fehler sind nicht fatal (dann ohne „Fortsetzen“).
    fn save(&self, data: &DocData, prepared: &dyn PreparedDoc, position: &PositionChoice) {
        let src = data.src.bytes();
        let now = SystemTime::now();
        match resume::persist(&self.dir, &self.name, src, prepared, position, self.backend, self.pending.clone(), now) {
            Ok(()) => tracing::info!("vorbereiteter Zustand für „Signatur fortsetzen“ gesichert"),
            Err(e) => {
                tracing::warn!(error = %e, "vorbereiteter Zustand nicht gesichert – ohne „Signatur fortsetzen“");
                resume::delete(&self.dir);
            }
        }
    }
}

/// Vorbereitetes Dokument in den Signiervorgang übernehmen (verworfen, wenn der Vorgang inzwischen
/// abgebrochen wurde).
fn store_prepared(
    shared: &Shared,
    epoch: u64,
    prepared: Box<dyn PreparedDoc>,
    cms_profile: pdfover_cades::CmsProfile,
) -> Event {
    let mut slot = lock(&shared.sign);
    if slot.epoch != epoch {
        return Event::BackendFailed(BackendError::Cancelled);
    }
    slot.prepared = Some(prepared);
    slot.cms_profile = Some(cms_profile);
    Event::PhaseCompleted(pdfover_workflow::SigningPhase::Preparing)
}

/// Dateiname ohne Pfadanteile und Steuerzeichen; leer → `fallback`.
fn sanitize_file_name(name: &str, fallback: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '\0') { '_' } else { c }).collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() { fallback.to_owned() } else { trimmed.to_owned() }
}

/// Schreibt eine private Datei atomar (Ordner 0700, Datei 0600).
fn write_private_file(dir: &Path, target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = target.with_extension("part");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, target)
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

fn panic_text(e: tokio::task::JoinError) -> String {
    if !e.is_panic() {
        return "abgebrochen".into();
    }
    let p = e.into_panic();
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "Panik".into())
}

/// Panik beim Vorbereiten/Abschließen → unerwarteter Fehler (Wiederholen möglich).
fn signing_panic(text: String) -> Event {
    Event::SigningFailed(WorkflowError::Unexpected(text))
}

fn prepare_error(code: &str, detail: &str) -> WorkflowError {
    WorkflowError::Prepare { code: code.to_owned(), detail: detail.to_owned() }
}

/// Fehler von `prepare` (PDF-AS: `error.PrepareDocument` bzw. 11021).
fn pades_error(e: &PadesError) -> WorkflowError {
    match e {
        PadesError::Rejected(_) => WorkflowError::Rejected {
            class: pdfover_pades::AcceptanceClass::K5Rejected,
            code: "ng-document-rejected".into(),
        },
        PadesError::FieldAlreadySigned(_) => WorkflowError::Rejected {
            class: pdfover_pades::AcceptanceClass::K4Restricted,
            code: "ng-field-already-signed".into(),
        },
        PadesError::FieldLocked(_) => WorkflowError::Rejected {
            class: pdfover_pades::AcceptanceClass::K4Restricted,
            code: "ng-field-locked".into(),
        },
        PadesError::CertificateNotValid => prepare_error("ng-certificate-not-valid", ""),
        PadesError::SigBlock(s) => prepare_error(s.code(), &s.to_string()),
        PadesError::Cms(c) => prepare_error(c.code(), &c.to_string()),
        other => prepare_error("error-PrepareDocument", &other.to_string()),
    }
}

/// Modell A (ADR 0001 §8.3): Hashverfahren aus dem Schlüssel, Hash der ByteRange-Daten,
/// SignedAttributes, Hash der Attribute → `DigestJob`.
fn digest_job(
    prepared: &dyn PreparedDoc,
    chain: &CertChain,
    profile: pdfover_cades::CmsProfile,
    name: &str,
) -> Result<(DigestJob, SignedAttrs), pdfover_cades::CmsError> {
    let key = chain.signer.key_type()?;
    let alg = pdfover_cades::select_algorithm(&key)?.digest();
    let message_digest = prepared.digest(alg);
    let attrs = SignedAttrsBuilder::new(profile, &message_digest, &chain.signer, SigningTime(prepared.signing_time()))
        .build()?;
    let digest = attrs.digest(alg);
    Ok((DigestJob { display_name: name.to_owned(), alg, digest: digest.value }, attrs))
}

/// Startposition der Positionierung (letzte Seite, 50 %/25 %) mit der Blockgröße des Profils.
fn start_placement(shared: &Shared, snap: &Snapshot, data: &DocData, store: &ProfileStore) -> Option<Placement> {
    let spec: BlockSpec = plan::block_spec(
        store,
        SpecInput {
            effective: snap.effective(),
            system_language: Lang::system(),
            pdfa: data.report.pdfa.as_ref(),
            cert: None,
            signing_time: plan::local_now(),
            emblem: None,
        },
    )
    .ok()?;
    match plan::start_placement(&spec, &data.pages, store, &shared.fonts) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(code = %e.code(), "Startposition nicht berechenbar");
            None
        }
    }
}

/// `DocumentSource` um ein vorbereitetes Dokument (Modell B, ADR 0001 §5.3). Das Backend darf es
/// nicht über `sign` hinaus halten; danach übernimmt der Kern es wieder exklusiv.
pub(crate) struct PreparedSource {
    name: String,
    inner: Box<dyn PreparedDoc>,
}

impl std::fmt::Debug for PreparedSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSource").field("name", &self.name).field("len", &self.inner.total_len()).finish()
    }
}

impl DocumentSource for PreparedSource {
    fn display_name(&self) -> &str {
        &self.name
    }
    fn len(&self) -> u64 {
        self.inner.total_len()
    }
    fn excluded_range(&self) -> ExcludedByteRange {
        let r = self.inner.excluded_range();
        ExcludedByteRange { from: r.from, to: r.to }
    }
    fn open_blacked_out(&self) -> std::io::Result<Box<dyn std::io::Read + Send + '_>> {
        Ok(self.inner.blacked_out())
    }
}

enum PreparedHolder {
    Owned(Box<dyn PreparedDoc>),
    Shared(Arc<PreparedSource>),
}

impl PreparedHolder {
    fn into_inner(self) -> Option<Box<dyn PreparedDoc>> {
        match self {
            PreparedHolder::Owned(p) => Some(p),
            PreparedHolder::Shared(arc) => Arc::try_unwrap(arc).ok().map(|s| s.inner),
        }
    }
}

impl DocData {
    /// Wie `with_renderer`, aber ohne Renderer (bzw. bei Renderfehlern) mit `None`.
    pub(crate) fn with_renderer_opt<T>(
        &self,
        backend: Option<&dyn RenderBackend>,
        f: impl FnOnce(Option<&dyn pdfover_render::Renderer>) -> T,
    ) -> T {
        match backend {
            Some(b) => {
                let mut f = Some(f);
                match self.with_renderer(b, |r| (f.take().expect("einmal"))(Some(r))) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(code = e.code(), "Renderer nicht verfügbar");
                        (f.take().expect("nicht aufgerufen"))(None)
                    }
                }
            }
            None => f(None),
        }
    }
}

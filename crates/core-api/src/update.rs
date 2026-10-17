//! „Über PIDA“ und Update-Prüfung im Kern (ADR 0004 Nachtrag 21).
//!
//! - **Quelle:** die öffentlichen GitHub-Releases ([`RELEASES_API`], ohne Anmeldung) über `pida-net`
//!   (`Purpose::UpdateCheck`: Proxy der Einstellungen, Redirects, nur `https`), mit Zeitlimit und Größenlimit.
//!   Der Request trägt nur `Accept`, `X-GitHub-Api-Version` und den üblichen User-Agent (`PIDA v<Version>`) –
//!   **keine** Kennungen, Installations-IDs, Cookies oder Abfrageparameter. Heruntergeladen oder installiert wird
//!   nichts; bei einer neuen Version zeigt die Oberfläche einen Link auf die Release-Seite.
//! - **Regel:** Entwürfe zählen nie; Pre-Releases zählen, solange die laufende Version selbst `0.x` oder eine
//!   Pre-Release-Version ist (heute sind alle Releases Pre-Releases), sonst nur stabile. Verglichen wird nach
//!   SemVer 2.0 (mit Pre-Release-Kennungen, ohne Build-Metadaten); Tags heißen `v<semver>`, andere zählen nicht.
//! - **Wann:** beim Start höchstens einmal je [`STARTUP_INTERVAL`] im Hintergrund (nie blockierend; Ergebnis in
//!   [`CACHE_FILE`] im Cache-Ordner zwischengespeichert) und beim Öffnen von „Über PIDA“, wenn das letzte Ergebnis
//!   älter als [`ABOUT_RECHECK_AFTER`] ist – jeweils nur mit `general.update_check` und wenn die Richtlinie es nicht
//!   verbietet. Ausgeschaltet prüft „Jetzt prüfen“ einmalig auf ausdrücklichen Klick, mit Richtlinie „aus“ nie.
//!   Findet die Prüfung beim Start eine neue Version, erscheint der Hinweis [`MSG_UPDATE_AVAILABLE`] (kein Dialog).
//!   Auf iOS/Android (keine Verteilung über GitHub) und ohne Oberfläche (CLI) prüft der Kern nie.
//! - **Fehler** (offline, Zeitüberschreitung, Proxy, TLS, Rate-Limit 403/429, Serverfehler, ungültige Antwort) →
//!   Zustand `failed` mit Grund `ng-update-failed-*`; „auf dem neuesten Stand“ nur nach einer erfolgreichen Prüfung.
//! - **Logs** nennen nur Zustand, Version und Grund (keine Adressen, keine Personendaten).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use pida_net::{HttpClient, Purpose};
pub use pida_net::{NetConfig, NetError, ProxyConfig, Request, Response, Url};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::runtime::{CoreTask, Msg};
use crate::view::{AboutView, UpdateCheckState, UpdateCheckView};

/// Repository (Quellcode, Release-Seiten).
pub const REPOSITORY_URL: &str = "https://github.com/ataub1/pida";
/// Liste der Releases (GitHub REST API, öffentlich).
pub const RELEASES_API: &str = "https://api.github.com/repos/ataub1/pida/releases";
/// Versionsdatum des gebauten Stands (`JJJJ-MM-TT`, `build.rs`: `SOURCE_DATE_EPOCH`, sonst Commit-Datum, sonst
/// Datum des Bauens).
pub const RELEASE_DATE: &str = env!("PIDA_RELEASE_DATE");
/// Höchstens eine automatische Prüfung je Zeitraum beim Start.
pub const STARTUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// „Über PIDA“ prüft beim Öffnen erneut, wenn das letzte Ergebnis älter ist (sonst zeigt es das letzte).
pub const ABOUT_RECHECK_AFTER: Duration = Duration::from_secs(60 * 60);
/// Zwischenspeicher im Cache-Ordner (`Paths::cache_dir`): Zeitpunkte und gefundene Versionen, sonst nichts.
pub const CACHE_FILE: &str = "update-check.json";
/// Verbindungsaufbau.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Gesamtdauer eines Requests.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Höchstgröße der Antwort (die Liste der Releases ist wenige KiB bis einige 100 KiB groß).
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
/// Sicherheitsnetz, falls ein Transport nicht von selbst abbricht.
const OVERALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Zeitstempel bis so weit in der Zukunft gelten als „jetzt“ (Uhren mehrerer Geräte bzw. Zeitzonen).
const CLOCK_SLACK: Duration = Duration::from_secs(5 * 60);
/// Höchstzahl der zwischengespeicherten Versionen.
const MAX_CACHED_RELEASES: usize = 100;

/// Meldungs-ID: Prüfung beim Start fand eine neue Version (`$version`); Hinweis ohne Rückfrage.
pub const MSG_UPDATE_AVAILABLE: &str = "ng-update-available";
/// Grund: keine Verbindung (DNS, TCP, Verbindung ohne Antwort).
pub const MSG_FAILED_OFFLINE: &str = "ng-update-failed-offline";
/// Grund: Zeitüberschreitung.
pub const MSG_FAILED_TIMEOUT: &str = "ng-update-failed-timeout";
/// Grund: Proxy (auch eine Anmeldung, die der Proxy verlangt).
pub const MSG_FAILED_PROXY: &str = "ng-update-failed-proxy";
/// Grund: sichere Verbindung nicht möglich (TLS, Zertifikat).
pub const MSG_FAILED_TLS: &str = "ng-update-failed-tls";
/// Grund: GitHub begrenzt die Anfragen (HTTP 403/429).
pub const MSG_FAILED_RATE_LIMITED: &str = "ng-update-failed-rate-limited";
/// Grund: GitHub antwortet mit einem anderen Fehler (HTTP-Status ≠ 200).
pub const MSG_FAILED_SERVER: &str = "ng-update-failed-server";
/// Grund: Antwort nicht lesbar (kein JSON, zu groß, unerwartetes Format).
pub const MSG_FAILED_INVALID: &str = "ng-update-failed-invalid";

// ================================================================================================
// SemVer 2.0
// ================================================================================================

/// Kennung eines Pre-Release-Teils.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ident {
    Num(u64),
    Alpha(String),
}

/// Version nach SemVer 2.0.0 (`1.2.3-rc.1+build`); verglichen wird mit [`Version::cmp_precedence`] (ohne
/// Build-Metadaten).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<Ident>,
    build: String,
}

/// Zahl ohne führende Null (außer `0`).
fn numeric(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0')) {
        return None;
    }
    s.parse().ok()
}

/// Kennung aus `[0-9A-Za-z-]`, nicht leer.
fn ident_chars(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl Version {
    /// Liest `MAJOR.MINOR.PATCH[-PRE][+BUILD]` (ohne `v`); `None`, wenn es keine SemVer-Version ist.
    pub fn parse(s: &str) -> Option<Version> {
        let (rest, build) = match s.split_once('+') {
            Some((r, b)) => {
                if !b.split('.').all(ident_chars) {
                    return None;
                }
                (r, b.to_owned())
            }
            None => (s, String::new()),
        };
        let (core, pre) = match rest.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (rest, None),
        };
        let mut parts = core.split('.');
        let major = numeric(parts.next()?)?;
        let minor = numeric(parts.next()?)?;
        let patch = numeric(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(p) => {
                let mut out = Vec::new();
                for id in p.split('.') {
                    if !ident_chars(id) {
                        return None;
                    }
                    if id.bytes().all(|b| b.is_ascii_digit()) {
                        out.push(Ident::Num(numeric(id)?));
                    } else {
                        out.push(Ident::Alpha(id.to_owned()));
                    }
                }
                out
            }
        };
        Some(Version { major, minor, patch, pre, build })
    }

    /// Pre-Release-Version (`1.0.0-rc.1`)?
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    /// `0.x` (Entwicklungsphase nach SemVer)?
    pub fn is_initial_development(&self) -> bool {
        self.major == 0
    }

    /// Rangfolge nach SemVer 2.0.0 §11 (Build-Metadaten zählen nicht).
    pub fn cmp_precedence(&self, other: &Version) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    for (a, b) in self.pre.iter().zip(&other.pre) {
                        let o = match (a, b) {
                            (Ident::Num(x), Ident::Num(y)) => x.cmp(y),
                            (Ident::Num(_), Ident::Alpha(_)) => Ordering::Less,
                            (Ident::Alpha(_), Ident::Num(_)) => Ordering::Greater,
                            (Ident::Alpha(x), Ident::Alpha(y)) => x.as_bytes().cmp(y.as_bytes()),
                        };
                        if o != Ordering::Equal {
                            return o;
                        }
                    }
                    self.pre.len().cmp(&other.pre.len())
                }
            }
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (i, id) in self.pre.iter().enumerate() {
            f.write_str(if i == 0 { "-" } else { "." })?;
            match id {
                Ident::Num(n) => write!(f, "{n}")?,
                Ident::Alpha(s) => f.write_str(s)?,
            }
        }
        if !self.build.is_empty() {
            write!(f, "+{}", self.build)?;
        }
        Ok(())
    }
}

// ================================================================================================
// Releases
// ================================================================================================

/// Ein veröffentlichtes Release (kein Entwurf) mit Tag `v<semver>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Version aus dem Tag
    pub version: Version,
    /// als Pre-Release veröffentlicht (Kennzeichen auf GitHub)
    pub prerelease: bool,
}

impl Release {
    /// Zählt als Pre-Release: auf GitHub so gekennzeichnet **oder** eine SemVer-Pre-Release-Version.
    pub fn is_prerelease(&self) -> bool {
        self.prerelease || self.version.is_prerelease()
    }

    /// Tag auf GitHub.
    pub fn tag(&self) -> String {
        format!("v{}", self.version)
    }
}

/// Version aus einem Tag `v<semver>`.
pub fn version_from_tag(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix('v')?)
}

/// Antwort der GitHub-API (`GET /repos/{owner}/{repo}/releases`): Entwürfe und Tags ohne `v<semver>` fallen weg,
/// unbekannte Felder werden ignoriert. Keine Liste → [`MSG_FAILED_INVALID`].
pub fn parse_releases(body: &[u8]) -> Result<Vec<Release>, &'static str> {
    let list: Vec<serde_json::Value> = serde_json::from_slice(body).map_err(|_| MSG_FAILED_INVALID)?;
    Ok(list
        .iter()
        .filter(|r| !r.get("draft").and_then(serde_json::Value::as_bool).unwrap_or(false))
        .filter_map(|r| {
            let version = version_from_tag(r.get("tag_name")?.as_str()?)?;
            let prerelease = r.get("prerelease").and_then(serde_json::Value::as_bool).unwrap_or(false);
            Some(Release { version, prerelease })
        })
        .collect())
}

/// Ergebnis des Vergleichs mit der laufenden Version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluation {
    /// keine neuere Version; `latest` = neueste in Frage kommende (`None`: keine)
    UpToDate {
        /// neueste in Frage kommende Version
        latest: Option<Version>,
    },
    /// neuere Version verfügbar
    Available {
        /// die neue Version
        latest: Version,
    },
}

/// Kommen Pre-Releases in Frage? Ja, solange die laufende Version `0.x` oder selbst eine Pre-Release-Version ist.
pub fn includes_prereleases(current: &Version) -> bool {
    current.is_initial_development() || current.is_prerelease()
}

/// Neueste in Frage kommende Version gegen die laufende.
pub fn evaluate(current: &Version, releases: &[Release]) -> Evaluation {
    let pre = includes_prereleases(current);
    let latest = releases
        .iter()
        .filter(|r| pre || !r.is_prerelease())
        .map(|r| &r.version)
        .max_by(|a, b| a.cmp_precedence(b))
        .cloned();
    match latest {
        Some(v) if v.cmp_precedence(current) == Ordering::Greater => Evaluation::Available { latest: v },
        latest => Evaluation::UpToDate { latest },
    }
}

/// Release-Seite eines Tags (`…/releases/tag/v0.3.0`); nur Zeichen aus SemVer, sonst die Liste aller Releases.
pub fn release_page(tag: &str) -> String {
    if !tag.is_empty() && tag.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+')) {
        format!("{REPOSITORY_URL}/releases/tag/{tag}")
    } else {
        format!("{REPOSITORY_URL}/releases")
    }
}

// ================================================================================================
// Transport
// ================================================================================================

/// Transport der Update-Prüfung: der echte über `pida-net` ([`NetTransport`]) bzw. in Tests ein Ersatz
/// (`CoreOptions::update_transport`). Erhält den fertigen Request ([`releases_request`]) und die
/// Netz-Konfiguration ([`net_config`]).
#[async_trait::async_trait]
pub trait UpdateTransport: Send + Sync {
    /// Führt den Request aus (Antwortkörper vollständig gelesen).
    async fn fetch(&self, config: &NetConfig, request: Request) -> Result<Response, NetError>;
}

/// `pida-net` mit `Purpose::UpdateCheck` (ohne Cookies, Redirects, nur `https`, Proxy der Einstellungen).
#[derive(Debug, Default, Clone, Copy)]
pub struct NetTransport;

#[async_trait::async_trait]
impl UpdateTransport for NetTransport {
    async fn fetch(&self, config: &NetConfig, request: Request) -> Result<Response, NetError> {
        HttpClient::new(config, Purpose::UpdateCheck)?.execute(request).await
    }
}

/// Der Request: `GET` [`RELEASES_API`] ohne Abfrage, nur mit `Accept` und `X-GitHub-Api-Version` (der
/// User-Agent kommt aus [`NetConfig`]). Keine Kennungen.
pub fn releases_request() -> Result<Request, NetError> {
    let url = Url::parse(RELEASES_API).map_err(|e| NetError::InvalidUrl(e.to_string()))?;
    Ok(Request::get(url)
        .with_header("Accept", "application/vnd.github+json")
        .with_header("X-GitHub-Api-Version", "2022-11-28"))
}

/// Netz-Konfiguration: Proxy der Einstellungen, Standard-User-Agent (`PIDA v<Version>`), Zeit- und Größenlimit.
pub fn net_config(proxy: ProxyConfig) -> NetConfig {
    NetConfig {
        proxy,
        user_agent: None,
        connect_timeout: CONNECT_TIMEOUT,
        request_timeout: Some(REQUEST_TIMEOUT),
        max_body: MAX_RESPONSE_BYTES,
    }
}

/// Grund (i18n-ID) für einen Netzfehler.
pub fn reason_for_error(e: &NetError) -> &'static str {
    match e {
        NetError::Timeout => MSG_FAILED_TIMEOUT,
        NetError::Proxy(_) => MSG_FAILED_PROXY,
        NetError::Tls(_) => MSG_FAILED_TLS,
        NetError::Connect(_) | NetError::NoResponse => MSG_FAILED_OFFLINE,
        NetError::Body(_) | NetError::InvalidUrl(_) => MSG_FAILED_INVALID,
        // u. a. eine unbrauchbare manuelle Proxy-Angabe (Host, Port) oder eine Adresse ohne `https`
        NetError::Config(_) => MSG_FAILED_PROXY,
    }
}

/// Grund für einen HTTP-Status ≠ 200 (403/429: Rate-Limit der GitHub-API für Anfragen ohne Anmeldung).
pub fn reason_for_status(status: u16) -> &'static str {
    match status {
        403 | 429 => MSG_FAILED_RATE_LIMITED,
        _ => MSG_FAILED_SERVER,
    }
}

/// Fragt die Releases ab und liest sie (ohne Bewertung).
pub async fn fetch_releases(transport: &dyn UpdateTransport, proxy: ProxyConfig) -> Result<Vec<Release>, &'static str> {
    let request = releases_request().map_err(|e| reason_for_error(&e))?;
    let config = net_config(proxy);
    let response = match tokio::time::timeout(OVERALL_TIMEOUT, transport.fetch(&config, request)).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return Err(reason_for_error(&e)),
        Err(_) => return Err(MSG_FAILED_TIMEOUT),
    };
    if response.url.scheme() != "https" {
        return Err(MSG_FAILED_INVALID);
    }
    if response.status != 200 {
        tracing::info!(status = response.status, "Update-Prüfung: GitHub antwortet mit Fehler");
        return Err(reason_for_status(response.status));
    }
    parse_releases(&response.body)
}

// ================================================================================================
// Zwischenspeicher
// ================================================================================================

/// Eintrag des Zwischenspeichers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CachedRelease {
    tag: String,
    #[serde(default)]
    prerelease: bool,
}

/// Inhalt von [`CACHE_FILE`]: letzte erfolgreiche Prüfung samt Versionen und letzter Versuch – keine Kennungen.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CacheFile {
    /// letzte erfolgreiche Prüfung (RFC 3339)
    #[serde(default)]
    checked_at: Option<String>,
    /// letzter Versuch, auch erfolglos (RFC 3339)
    #[serde(default)]
    attempted_at: Option<String>,
    /// gefundene Releases der letzten erfolgreichen Prüfung
    #[serde(default)]
    releases: Vec<CachedRelease>,
}

fn load_cache(path: &Path) -> CacheFile {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_cache(path: &Path, cache: &CacheFile) {
    let result = (|| -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(cache).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        tracing::warn!(kind = ?e.kind(), "Update-Prüfung: Zwischenspeicher nicht geschrieben");
    }
}

fn parse_time(s: Option<&str>) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s?, &Rfc3339).ok()
}

/// Liegt `t` weniger als `max` zurück (leicht in der Zukunft zählt als „jetzt“, weiter in der Zukunft als alt)?
fn younger_than(t: OffsetDateTime, now: OffsetDateTime, max: Duration) -> bool {
    let age = now - t;
    age < time::Duration::try_from(max).unwrap_or(time::Duration::MAX)
        && age > -time::Duration::try_from(CLOCK_SLACK).unwrap_or(time::Duration::ZERO)
}

// ================================================================================================
// Zustand im Kern
// ================================================================================================

/// Anlass einer Prüfung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckKind {
    /// automatisch beim Start (höchstens einmal je [`STARTUP_INTERVAL`]; Hinweis bei neuer Version)
    Startup,
    /// automatisch beim Öffnen von „Über PIDA“
    About,
    /// „Jetzt prüfen“ bzw. „Erneut prüfen“
    Explicit,
}

/// Ergebnis der letzten Prüfung.
#[derive(Debug, Clone)]
enum Outcome {
    /// erfolgreich (auch aus dem Zwischenspeicher)
    Checked { releases: Vec<Release>, at: OffsetDateTime },
    /// nicht möglich (Grund)
    Failed { reason: &'static str },
}

/// Update-Prüfung und Angaben für „Über PIDA“.
pub(crate) struct UpdateState {
    /// Version der App (`CoreConfig::app_version`)
    version: String,
    current: Option<Version>,
    transport: Option<Arc<dyn UpdateTransport>>,
    cache_file: PathBuf,
    cache: CacheFile,
    /// laufende Prüfung
    running: Option<CheckKind>,
    generation: u64,
    outcome: Option<Outcome>,
    /// letzte erfolgreiche Prüfung (für `checked_at`, auch nach einem Fehler)
    last_success: Option<OffsetDateTime>,
    /// das Ergebnis stammt von einer ausdrücklichen Prüfung (zeigt es auch bei ausgeschalteter automatischer
    /// Prüfung, bis der Schalter wechselt)
    explicit_result: bool,
    /// zuletzt gesehener Wert des Schalters (Wechsel → `explicit_result` verfällt)
    enabled_seen: Option<bool>,
}

impl std::fmt::Debug for UpdateState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateState")
            .field("version", &self.version)
            .field("transport", &self.transport.is_some())
            .field("running", &self.running)
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

impl UpdateState {
    /// Zustand beim Start: Version, Transport (`None` = nie prüfen) und Zwischenspeicher aus `cache_dir`.
    pub(crate) fn new(version: &str, transport: Option<Arc<dyn UpdateTransport>>, cache_dir: &Path) -> UpdateState {
        let current = Version::parse(version.trim());
        if current.is_none() {
            tracing::warn!("Update-Prüfung: Version der App ist keine SemVer-Version – keine Prüfung");
        }
        let cache_file = cache_dir.join(CACHE_FILE);
        let cache = if transport.is_some() { load_cache(&cache_file) } else { CacheFile::default() };
        let checked = parse_time(cache.checked_at.as_deref());
        let outcome = checked.map(|at| Outcome::Checked {
            releases: cache
                .releases
                .iter()
                .filter_map(|r| Some(Release { version: version_from_tag(&r.tag)?, prerelease: r.prerelease }))
                .collect(),
            at,
        });
        UpdateState {
            version: version.to_owned(),
            current,
            transport,
            cache_file,
            cache,
            running: None,
            generation: 0,
            outcome,
            last_success: checked,
            explicit_result: false,
            enabled_seen: None,
        }
    }

    /// Ergebnis jünger als `max`?
    fn fresh(&self, max: Duration) -> bool {
        matches!(&self.outcome, Some(Outcome::Checked { at, .. }) if younger_than(*at, OffsetDateTime::now_utc(), max))
    }

    /// Versuch (auch erfolglos) beim Start jünger als [`STARTUP_INTERVAL`]?
    fn attempted_recently(&self) -> bool {
        let last = parse_time(self.cache.attempted_at.as_deref()).or(self.last_success);
        last.is_some_and(|t| younger_than(t, OffsetDateTime::now_utc(), STARTUP_INTERVAL))
    }
}

impl CoreTask {
    /// Prüft der Kern hier überhaupt? Nur am Desktop mit Oberfläche, mit Transport und SemVer-Version.
    pub(crate) fn updates_supported(&self) -> bool {
        self.mobile.platform.is_desktop()
            && !self.headless
            && self.updates.transport.is_some()
            && self.updates.current.is_some()
    }

    /// Richtlinie `update_check = false`: nie prüfen, auch nicht auf Klick.
    fn update_policy_off(&self) -> bool {
        self.cfg.policy.as_ref().and_then(|p| p.update_check) == Some(false)
    }

    /// Automatische Prüfung an (Richtlinie vor Einstellung)?
    fn update_auto_enabled(&self) -> bool {
        self.cfg.effective().update_check()
    }

    /// Beim Start: im Hintergrund prüfen, wenn erlaubt und der letzte Versuch mindestens [`STARTUP_INTERVAL`] zurückliegt.
    pub(crate) fn update_check_on_startup(&mut self) {
        self.updates.enabled_seen = Some(self.update_auto_enabled());
        if !self.updates_supported() || self.update_policy_off() || !self.update_auto_enabled() {
            return;
        }
        if self.updates.attempted_recently() {
            tracing::debug!("Update-Prüfung: letzter Versuch jünger als 24 h – beim Start keine Prüfung");
            return;
        }
        self.begin_update_check(CheckKind::Startup);
    }

    /// `UiEvent::CheckForUpdates`: beim Öffnen von „Über PIDA“ (`explicit = false`) bzw. auf Klick.
    pub(crate) fn check_for_updates(&mut self, explicit: bool) {
        if !self.updates_supported() {
            tracing::debug!("Update-Prüfung hier nicht vorgesehen – ignoriert");
            return;
        }
        if self.update_policy_off() {
            tracing::info!("Update-Prüfung per Richtlinie abgeschaltet – ignoriert");
            return;
        }
        if self.updates.running.is_some() {
            return;
        }
        if !explicit && (!self.update_auto_enabled() || self.updates.fresh(ABOUT_RECHECK_AFTER)) {
            return;
        }
        self.begin_update_check(if explicit { CheckKind::Explicit } else { CheckKind::About });
    }

    fn begin_update_check(&mut self, kind: CheckKind) {
        let Some(transport) = self.updates.transport.clone() else { return };
        self.updates.generation += 1;
        self.updates.running = Some(kind);
        let generation = self.updates.generation;
        let proxy = crate::backends::proxy_config(&self.cfg);
        let shared = self.shared.clone();
        tracing::info!(kind = ?kind, "Update-Prüfung beginnt");
        tokio::spawn(async move {
            let result = fetch_releases(transport.as_ref(), proxy).await;
            shared.send(Msg::UpdateChecked { generation, kind, result });
        });
    }

    /// Ergebnis einer Prüfung (`Msg::UpdateChecked`).
    pub(crate) fn on_update_checked(
        &mut self,
        generation: u64,
        kind: CheckKind,
        result: Result<Vec<Release>, &'static str>,
    ) {
        if generation != self.updates.generation || self.updates.running.is_none() {
            return;
        }
        self.updates.running = None;
        let now = OffsetDateTime::now_utc();
        let stamp = crate::workspace::rfc3339(crate::plan::local_now());
        self.updates.cache.attempted_at = Some(stamp.clone());
        self.updates.explicit_result = kind == CheckKind::Explicit;
        match result {
            Ok(mut releases) => {
                releases.sort_by(|a, b| b.version.cmp_precedence(&a.version));
                releases.truncate(MAX_CACHED_RELEASES);
                self.updates.cache.checked_at = Some(stamp);
                self.updates.cache.releases =
                    releases.iter().map(|r| CachedRelease { tag: r.tag(), prerelease: r.prerelease }).collect();
                self.updates.last_success = Some(now);
                let evaluation = self.updates.current.as_ref().map(|c| evaluate(c, &releases));
                match &evaluation {
                    Some(Evaluation::Available { latest }) => {
                        tracing::info!(latest = %latest, "Update-Prüfung: neue Version verfügbar");
                        if kind == CheckKind::Startup {
                            self.notify(
                                MSG_UPDATE_AVAILABLE,
                                BTreeMap::from([("version".to_owned(), latest.to_string())]),
                            );
                        }
                    }
                    Some(Evaluation::UpToDate { latest }) => tracing::info!(
                        latest = %latest.as_ref().map(ToString::to_string).unwrap_or_default(),
                        "Update-Prüfung: auf dem neuesten Stand"
                    ),
                    None => {}
                }
                self.updates.outcome = Some(Outcome::Checked { releases, at: now });
            }
            Err(reason) => {
                tracing::warn!(reason, "Update-Prüfung nicht möglich");
                self.updates.outcome = Some(Outcome::Failed { reason });
            }
        }
        let file = self.updates.cache_file.clone();
        save_cache(&file, &self.updates.cache);
    }

    /// Nach jeder Nachricht: Wechselt der Schalter, gilt ein Ergebnis aus „Jetzt prüfen“ nicht mehr als Ausnahme.
    pub(crate) fn sync_update_setting(&mut self) {
        let enabled = self.update_auto_enabled();
        if self.updates.enabled_seen.is_some_and(|seen| seen != enabled) {
            self.updates.explicit_result = false;
        }
        self.updates.enabled_seen = Some(enabled);
    }

    /// `ViewState::about`.
    pub(crate) fn about_view(&self) -> AboutView {
        let version = &self.updates.version;
        AboutView {
            version: version.clone(),
            release_date: RELEASE_DATE.to_owned(),
            release_notes_url: release_page(&format!("v{version}")),
            source_url: REPOSITORY_URL.to_owned(),
            update: self.update_view(),
        }
    }

    fn update_view(&self) -> Option<UpdateCheckView> {
        if !self.updates_supported() {
            return None;
        }
        let u = &self.updates;
        let checked_at =
            u.last_success.map(|t| crate::workspace::rfc3339(t.to_offset(crate::plan::local_now().offset())));
        let view = |state| UpdateCheckView {
            state,
            latest_version: None,
            release_url: None,
            checked_at: checked_at.clone(),
            reason: None,
        };
        if self.update_policy_off() {
            return Some(view(UpdateCheckState::PolicyOff));
        }
        if u.running.is_some() {
            return Some(view(UpdateCheckState::Checking));
        }
        if !self.update_auto_enabled() && !u.explicit_result {
            return Some(view(UpdateCheckState::Disabled));
        }
        let current = u.current.as_ref()?;
        Some(match &u.outcome {
            None => view(UpdateCheckState::Unchecked),
            Some(Outcome::Failed { reason }) => {
                UpdateCheckView { reason: Some((*reason).to_owned()), ..view(UpdateCheckState::Failed) }
            }
            Some(Outcome::Checked { releases, .. }) => match evaluate(current, releases) {
                Evaluation::Available { latest } => UpdateCheckView {
                    latest_version: Some(latest.to_string()),
                    release_url: Some(release_page(&format!("v{latest}"))),
                    ..view(UpdateCheckState::Available)
                },
                Evaluation::UpToDate { latest } => UpdateCheckView {
                    latest_version: latest.map(|v| v.to_string()),
                    ..view(UpdateCheckState::UpToDate)
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s}"))
    }

    fn rel(tag: &str, prerelease: bool) -> Release {
        Release { version: version_from_tag(tag).unwrap(), prerelease }
    }

    #[test]
    fn semver_parsing() {
        for ok in
            ["0.2.0", "1.0.0", "1.0.0-rc.1", "1.0.0-alpha.beta", "1.0.0+20261017", "1.0.0-0.3.7", "10.20.30-x-y.1+b.2"]
        {
            assert_eq!(v(ok).to_string(), ok);
        }
        for bad in [
            "",
            "1",
            "1.0",
            "1.0.0.0",
            "01.0.0",
            "1.02.0",
            "1.0.0-",
            "1.0.0-01",
            "1.0.0-a..b",
            "1.0.0+",
            "v1.0.0",
            "1.0.0-ä",
            " 1.0.0",
            "1.0.x",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad:?}");
        }
        assert!(v("1.0.0-rc.1").is_prerelease() && !v("1.0.0+b").is_prerelease());
        assert!(v("0.9.9").is_initial_development() && !v("1.0.0").is_initial_development());
        assert_eq!(version_from_tag("v0.2.0"), Some(v("0.2.0")));
        assert_eq!(version_from_tag("0.2.0"), None, "nur Tags `v<semver>`");
        assert_eq!(version_from_tag("release-1"), None);
    }

    /// SemVer 2.0.0 §11: 1.0.0-alpha < 1.0.0-alpha.1 < 1.0.0-alpha.beta < 1.0.0-beta < 1.0.0-beta.2 <
    /// 1.0.0-beta.11 < 1.0.0-rc.1 < 1.0.0; Build-Metadaten zählen nicht.
    #[test]
    fn semver_precedence() {
        let order = [
            "0.1.0",
            "0.2.0-rc.1",
            "0.2.0",
            "0.10.0",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "2.0.0",
        ];
        for w in order.windows(2) {
            assert_eq!(v(w[0]).cmp_precedence(&v(w[1])), Ordering::Less, "{} < {}", w[0], w[1]);
            assert_eq!(v(w[1]).cmp_precedence(&v(w[0])), Ordering::Greater);
        }
        assert_eq!(v("1.0.0+a").cmp_precedence(&v("1.0.0+b")), Ordering::Equal);
        assert_eq!(v("1.0.0-1").cmp_precedence(&v("1.0.0-a")), Ordering::Less, "numerisch vor alphanumerisch");
    }

    #[test]
    fn releases_from_the_github_api() {
        let body = br#"[
            {"tag_name":"v0.3.0","draft":true,"prerelease":false,"html_url":"https://github.com/ataub1/pida/releases/tag/v0.3.0"},
            {"tag_name":"v0.2.1-rc.1","draft":false,"prerelease":true,"name":"RC","assets":[]},
            {"tag_name":"v0.2.0","draft":false,"prerelease":true},
            {"tag_name":"nightly","draft":false,"prerelease":true},
            {"tag_name":"v0.1.0"},
            {"name":"ohne Tag"},
            42
        ]"#;
        let list = parse_releases(body).unwrap();
        assert_eq!(
            list,
            vec![rel("v0.2.1-rc.1", true), rel("v0.2.0", true), rel("v0.1.0", false)],
            "ohne Entwurf und fremde Tags"
        );
        assert_eq!(parse_releases(b"[]").unwrap(), Vec::new());
        for bad in [&b"{\"message\":\"Not Found\"}"[..], b"<html>", b"", b"null"] {
            assert_eq!(parse_releases(bad), Err(MSG_FAILED_INVALID), "{}", String::from_utf8_lossy(bad));
        }
    }

    /// Pre-Releases zählen nur für `0.x` bzw. Pre-Release-Versionen; Entwürfe nie (schon beim Lesen entfernt).
    #[test]
    fn prerelease_rule() {
        let today = [rel("v0.2.0", true), rel("v0.1.0", true)];
        assert_eq!(evaluate(&v("0.2.0"), &today), Evaluation::UpToDate { latest: Some(v("0.2.0")) });
        assert_eq!(evaluate(&v("0.1.0"), &today), Evaluation::Available { latest: v("0.2.0") });
        // eine Entwicklerversion vor dem nächsten Release ist auf dem neuesten Stand
        assert_eq!(evaluate(&v("0.3.0"), &today), Evaluation::UpToDate { latest: Some(v("0.2.0")) });
        let mixed = [rel("v1.1.0-rc.1", true), rel("v1.0.1", false), rel("v1.0.2", true), rel("v1.0.0", false)];
        // stabile Version: nur stabile Releases (weder Kennzeichen noch SemVer-Pre-Release)
        assert_eq!(evaluate(&v("1.0.0"), &mixed), Evaluation::Available { latest: v("1.0.1") });
        assert_eq!(evaluate(&v("1.0.1"), &mixed), Evaluation::UpToDate { latest: Some(v("1.0.1")) });
        // eine Pre-Release-Version sieht auch Pre-Releases
        assert_eq!(evaluate(&v("1.1.0-beta.2"), &mixed), Evaluation::Available { latest: v("1.1.0-rc.1") });
        assert_eq!(evaluate(&v("1.1.0-rc.1"), &mixed), Evaluation::UpToDate { latest: Some(v("1.1.0-rc.1")) });
        // ein SemVer-Pre-Release ohne Kennzeichen zählt trotzdem als Pre-Release
        assert_eq!(evaluate(&v("1.0.1"), &[rel("v1.1.0-rc.1", false)]), Evaluation::UpToDate { latest: None });
        assert_eq!(evaluate(&v("1.0.0"), &[]), Evaluation::UpToDate { latest: None });
        assert!(includes_prereleases(&v("0.2.0")) && includes_prereleases(&v("2.0.0-rc.1")));
        assert!(!includes_prereleases(&v("1.0.0")));
    }

    #[test]
    fn request_carries_no_identifiers() {
        let r = releases_request().unwrap();
        assert_eq!(r.method, pida_net::Method::Get);
        assert_eq!(r.url.as_str(), RELEASES_API);
        assert_eq!(r.url.scheme(), "https");
        assert!(r.url.query().is_none() && r.url.fragment().is_none(), "keine Abfrageparameter");
        assert!(r.url.username().is_empty() && r.url.password().is_none());
        let names: Vec<String> = r.headers.iter().map(|(k, _)| k.to_ascii_lowercase()).collect();
        assert_eq!(names, ["accept", "x-github-api-version"], "nur diese Header");
        assert_eq!(r.body, pida_net::Body::Empty);
        let c = net_config(ProxyConfig::None);
        assert_eq!(c.effective_user_agent(), pida_net::DEFAULT_USER_AGENT, "der übliche User-Agent");
        assert_eq!(c.max_body, MAX_RESPONSE_BYTES);
        assert_eq!(c.request_timeout, Some(REQUEST_TIMEOUT));
        assert!(!Purpose::UpdateCheck.uses_cookies() && Purpose::UpdateCheck.requires_https());
    }

    #[test]
    fn reasons_for_errors_and_status() {
        assert_eq!(reason_for_error(&NetError::Timeout), MSG_FAILED_TIMEOUT);
        assert_eq!(reason_for_error(&NetError::Connect("dns".into())), MSG_FAILED_OFFLINE);
        assert_eq!(reason_for_error(&NetError::NoResponse), MSG_FAILED_OFFLINE);
        assert_eq!(reason_for_error(&NetError::Proxy("407".into())), MSG_FAILED_PROXY);
        assert_eq!(reason_for_error(&NetError::Tls("cert".into())), MSG_FAILED_TLS);
        assert_eq!(reason_for_error(&NetError::Body("zu groß".into())), MSG_FAILED_INVALID);
        assert_eq!(reason_for_status(403), MSG_FAILED_RATE_LIMITED);
        assert_eq!(reason_for_status(429), MSG_FAILED_RATE_LIMITED);
        assert_eq!(reason_for_status(404), MSG_FAILED_SERVER);
        assert_eq!(reason_for_status(502), MSG_FAILED_SERVER);
    }

    #[test]
    fn release_pages() {
        assert_eq!(release_page("v0.2.0"), "https://github.com/ataub1/pida/releases/tag/v0.2.0");
        assert_eq!(release_page("v1.0.0-rc.1+b"), "https://github.com/ataub1/pida/releases/tag/v1.0.0-rc.1+b");
        assert_eq!(release_page("v0.2/../x"), "https://github.com/ataub1/pida/releases");
        assert_eq!(release_page(""), "https://github.com/ataub1/pida/releases");
    }

    #[test]
    fn release_date_is_an_iso_date() {
        let d = RELEASE_DATE.as_bytes();
        assert_eq!(d.len(), 10, "{RELEASE_DATE}");
        assert!(d.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() }));
        let year: u16 = RELEASE_DATE[..4].parse().unwrap();
        assert!(year >= 2026, "{RELEASE_DATE}");
    }

    #[test]
    fn age_with_clock_slack() {
        let now = OffsetDateTime::now_utc();
        let day = Duration::from_secs(86_400);
        assert!(younger_than(now - time::Duration::hours(23), now, day));
        assert!(!younger_than(now - time::Duration::hours(25), now, day));
        assert!(younger_than(now + time::Duration::minutes(2), now, day), "kleine Abweichung der Uhr");
        assert!(!younger_than(now + time::Duration::hours(3), now, day), "weit in der Zukunft = veraltet");
    }

    #[test]
    fn cache_file_round_trip_without_identifiers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache").join(CACHE_FILE);
        let cache = CacheFile {
            checked_at: Some("2026-10-17T10:00:00+02:00".into()),
            attempted_at: Some("2026-10-17T10:00:00+02:00".into()),
            releases: vec![CachedRelease { tag: "v0.2.0".into(), prerelease: true }],
        };
        save_cache(&path, &cache);
        assert_eq!(load_cache(&path), cache);
        let text = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["attempted_at", "checked_at", "releases"]);
        std::fs::write(&path, b"kaputt").unwrap();
        assert_eq!(load_cache(&path), CacheFile::default(), "unlesbar = leer");
    }
}

//! Single-Instance und früh eintreffende Dokumente („Öffnen mit“ beim Kaltstart).
//!
//! **Früh eintreffende Dokumente:** macOS ruft beim Start über ein Dokument (Doppelklick, „Öffnen mit“,
//! Ablage auf dem Dock-Symbol, App läuft noch nicht) `application:openURLs:` **vor**
//! `applicationDidFinishLaunching` auf; Tauri meldet `RunEvent::Opened` also, bevor `setup` die
//! [`Shell`](crate::shell::Shell) angelegt hat. Ebenso kann sich eine zweite Instanz melden, während die erste noch startet.
//! Solche Dokumente landen in [`EarlyDocuments`] (vor dem Bauen der App registriert) und gehen in
//! `setup` an die Shell, bevor der Kern startet – sonst wären sie verloren.
//!
//! **Single-Instance unter macOS:** `tauri-plugin-single-instance` (=2.5.4) legt seinen Unix-Socket im
//! **gemeinsamen** `/tmp` an (`/tmp/<identifier>_si.sock`) und verbindet sich beim Start, ohne den
//! Besitzer zu prüfen. Hat ein anderes lokales Konto dort einen eigenen lauschenden Socket angelegt,
//! schickte die App ihm Arbeitsordner und Startargumente – einschließlich `-p`, `-kspass` und
//! `-proxypass` – und beendete sich sofort. Die Shell registriert das Plugin unter macOS deshalb nur,
//! wenn der Socket fehlt oder als Socket dem eigenen Konto gehört ([`check_socket`]); sonst startet sie
//! ohne Single-Instance (Warnung im Log, ohne Pfad- oder Kontodaten). „Öffnen mit“ und Dock-Ablagen
//! übergibt LaunchServices ohnehin der laufenden Instanz (`RunEvent::Opened`); das Plugin betrifft unter
//! macOS nur Starts der Binary aus dem Terminal. Linux (D-Bus-Sitzungsbus) und Windows (benanntes
//! Mutex/Fensternachricht in der Sitzung) sind je Konto getrennt.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::shell::MAX_PENDING_DOCUMENTS;

/// Kennung der App (`identifier` in `tauri.conf.json`; Test `identifier_matches_tauri_conf`).
pub const IDENTIFIER: &str = "at.asit.pida";

/// Dokumente, die vor dem Anlegen der [`Shell`](crate::shell::Shell) eintreffen (Tauri-State, vor `build` registriert).
///
/// Solange die Vormerkung offen ist, nimmt [`EarlyDocuments::offer`] Dokumente an; `setup` legt erst
/// die Shell an und schließt dann die Vormerkung ([`EarlyDocuments::close`]). Beides geschieht unter
/// derselben Sperre – ein Dokument geht also entweder in die Vormerkung oder (danach) direkt an die
/// Shell, auch wenn die zweite Instanz sich aus einem anderen Thread meldet.
#[derive(Debug)]
pub struct EarlyDocuments(Mutex<Option<Vec<PathBuf>>>);

impl Default for EarlyDocuments {
    fn default() -> Self {
        EarlyDocuments(Mutex::new(Some(Vec::new())))
    }
}

impl EarlyDocuments {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Vec<PathBuf>>> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Merkt Dokumente vor (ohne Duplikate, höchstens [`MAX_PENDING_DOCUMENTS`]), solange die
    /// Vormerkung offen ist; danach kommen sie unverändert zurück (an die Shell geben).
    pub fn offer(&self, paths: Vec<PathBuf>) -> Option<Vec<PathBuf>> {
        let mut guard = self.lock();
        let Some(list) = guard.as_mut() else { return Some(paths) };
        for path in paths {
            if list.len() >= MAX_PENDING_DOCUMENTS {
                tracing::warn!("Zu viele Dokumente vor dem Start; weitere verworfen");
                break;
            }
            if !list.contains(&path) {
                list.push(path);
            }
        }
        tracing::info!(count = list.len(), "Dokumente vor dem Start der Shell vorgemerkt");
        None
    }

    /// Schließt die Vormerkung (nach dem Anlegen der Shell) und liefert die vorgemerkten Dokumente.
    pub fn close(&self) -> Vec<PathBuf> {
        self.lock().take().unwrap_or_default()
    }

    /// Anzahl vorgemerkter Dokumente.
    pub fn len(&self) -> usize {
        self.lock().as_ref().map_or(0, Vec::len)
    }

    /// Nichts vorgemerkt?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Übergibt Dokumente: in die offene Vormerkung, sonst über `to_shell` an die Shell (nur dann gefragt).
pub fn deliver(early: Option<&EarlyDocuments>, docs: Vec<PathBuf>, to_shell: impl FnOnce(Vec<PathBuf>) -> bool) {
    if docs.is_empty() {
        return;
    }
    let rest = match early {
        Some(early) => early.offer(docs),
        None => Some(docs),
    };
    if let Some(docs) = rest
        && !to_shell(docs)
    {
        tracing::warn!("Dokumente ohne Shell verworfen");
    }
}

/// Pfad des Sockets wie im Plugin (`platform_impl/macos.rs::socket_path`, ohne Feature `semver`).
pub fn macos_socket_path(identifier: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/{}_si.sock", identifier.replace(['.', '-'], "_")))
}

/// Befund zum Socket des Plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketCheck {
    /// kein Socket: diese Instanz wird die erste
    Absent,
    /// Socket des eigenen Kontos (laufende bzw. abgestürzte eigene Instanz)
    Own,
    /// fremder Besitzer, kein Socket (z. B. Datei, symbolischer Link) oder nicht prüfbar
    Foreign,
}

/// Prüft `path` (ohne symbolischen Links zu folgen) gegen das Konto `uid`.
#[cfg(unix)]
pub fn check_socket(path: &std::path::Path, uid: u32) -> SocketCheck {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SocketCheck::Absent,
        Err(_) => SocketCheck::Foreign,
        Ok(m) if m.file_type().is_socket() && m.uid() == uid => SocketCheck::Own,
        Ok(_) => SocketCheck::Foreign,
    }
}

/// Darf das Single-Instance-Plugin registriert werden? Unter macOS nur bei fehlendem bzw. eigenem
/// Socket (siehe Moduldoku), sonst immer. Läuft vor dem Einrichten des Logs; die Shell meldet ein
/// `false` später im Log.
pub fn single_instance_allowed() -> bool {
    #[cfg(target_os = "macos")]
    {
        let uid = rustix::process::geteuid().as_raw();
        matches!(check_socket(&macos_socket_path(IDENTIFIER), uid), SocketCheck::Absent | SocketCheck::Own)
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_matches_tauri_conf() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
        assert_eq!(conf["identifier"], IDENTIFIER);
        assert_eq!(macos_socket_path(IDENTIFIER), PathBuf::from("/tmp/at_asit_pida_si.sock"));
    }

    #[test]
    fn early_documents_are_kept_until_the_shell_exists() {
        let early = EarlyDocuments::default();
        let delivered = std::cell::RefCell::new(Vec::new());
        let to_shell = |docs: Vec<PathBuf>| {
            delivered.borrow_mut().extend(docs);
            true
        };
        assert!(early.is_empty());
        deliver(Some(&early), vec![PathBuf::from("/x/a.pdf"), PathBuf::from("/x/a.pdf")], to_shell);
        deliver(Some(&early), Vec::new(), to_shell);
        assert_eq!(early.len(), 1);
        assert!(delivered.borrow().is_empty(), "vor dem Schließen nichts an die Shell");
        let many: Vec<PathBuf> = (0..MAX_PENDING_DOCUMENTS + 5).map(|i| PathBuf::from(format!("/x/{i}.pdf"))).collect();
        assert_eq!(early.offer(many), None);
        assert_eq!(early.len(), MAX_PENDING_DOCUMENTS);
        // Shell angelegt: Vormerkung schließen, danach geht alles direkt an die Shell
        let kept = early.close();
        assert_eq!(kept.len(), MAX_PENDING_DOCUMENTS);
        assert_eq!(kept.first(), Some(&PathBuf::from("/x/a.pdf")));
        assert!(early.is_empty());
        assert!(early.close().is_empty());
        deliver(Some(&early), vec![PathBuf::from("/x/b.pdf")], to_shell);
        assert_eq!(*delivered.borrow(), vec![PathBuf::from("/x/b.pdf")]);
        // ohne Vormerkung und ohne Shell: keine Panik
        deliver(None, vec![PathBuf::from("/x/c.pdf")], |_| false);
    }

    #[cfg(unix)]
    #[test]
    fn only_own_sockets_are_trusted() {
        use std::os::unix::fs::MetadataExt;
        // kurzer Pfad (Unix-Sockets: höchstens ~100 Zeichen)
        let dir = PathBuf::from(format!("/tmp/pida-si-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("Ordner");
        let uid = std::fs::metadata(&dir).expect("Metadaten").uid();
        let socket = dir.join("s.sock");
        assert_eq!(check_socket(&socket, uid), SocketCheck::Absent);
        let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("Socket");
        assert_eq!(check_socket(&socket, uid), SocketCheck::Own);
        // anderer Besitzer
        assert_eq!(check_socket(&socket, uid.wrapping_add(1)), SocketCheck::Foreign);
        // keine Socket-Datei bzw. symbolischer Link auf den eigenen Socket
        let file = dir.join("f.sock");
        std::fs::write(&file, b"").expect("Datei");
        assert_eq!(check_socket(&file, uid), SocketCheck::Foreign);
        let link = dir.join("l.sock");
        std::os::unix::fs::symlink(&socket, &link).expect("Link");
        assert_eq!(check_socket(&link, uid), SocketCheck::Foreign);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

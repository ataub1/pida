//! Dateisystem im Speicher für den Browser: dieselben Namen und Signaturen wie der benutzte Teil von `std::fs`.
//! Ein Thread (WebAssembly im Browser), deshalb `thread_local` ohne Sperren. Pfade werden lexikalisch normalisiert
//! (`.` und `..` aufgelöst); Elternordner legt `write` bei Bedarf an.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone)]
enum Node {
    File { data: Vec<u8>, modified: SystemTime },
    Dir,
}

thread_local! {
    static FS: RefCell<BTreeMap<PathBuf, Node>> = RefCell::new(BTreeMap::from([(PathBuf::from("/"), Node::Dir)]));
}

fn now() -> SystemTime {
    crate::system_now()
}

fn norm(p: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for c in p.components() {
        match c {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

fn not_found(p: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{}: nicht vorhanden", p.display()))
}

fn ensure_parents(fs: &mut BTreeMap<PathBuf, Node>, p: &Path) {
    let mut cur = p.parent();
    let mut missing = Vec::new();
    while let Some(d) = cur {
        if fs.contains_key(d) {
            break;
        }
        missing.push(d.to_path_buf());
        cur = d.parent();
    }
    for d in missing {
        fs.insert(d, Node::Dir);
    }
}

/// Dateiinhalt lesen.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let p = norm(path.as_ref());
    FS.with(|fs| match fs.borrow().get(&p) {
        Some(Node::File { data, .. }) => Ok(data.clone()),
        Some(Node::Dir) => Err(io::Error::new(io::ErrorKind::IsADirectory, "Ordner")),
        None => Err(not_found(&p)),
    })
}

/// Dateiinhalt als UTF-8 lesen.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Datei schreiben (Elternordner werden angelegt).
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let p = norm(path.as_ref());
    FS.with(|fs| {
        let mut fs = fs.borrow_mut();
        if matches!(fs.get(&p), Some(Node::Dir)) {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, "Ordner"));
        }
        ensure_parents(&mut fs, &p);
        fs.insert(p, Node::File { data: contents.as_ref().to_vec(), modified: now() });
        Ok(())
    })
}

/// Ordner samt Eltern anlegen.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let p = norm(path.as_ref());
    FS.with(|fs| {
        let mut fs = fs.borrow_mut();
        if matches!(fs.get(&p), Some(Node::File { .. })) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Datei statt Ordner"));
        }
        ensure_parents(&mut fs, &p);
        fs.insert(p, Node::Dir);
        Ok(())
    })
}

/// Ordner anlegen (Eltern müssen existieren).
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let p = norm(path.as_ref());
    FS.with(|fs| {
        let mut fs = fs.borrow_mut();
        if fs.contains_key(&p) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        if p.parent().is_some_and(|d| !matches!(fs.get(d), Some(Node::Dir))) {
            return Err(not_found(&p));
        }
        fs.insert(p, Node::Dir);
        Ok(())
    })
}

/// Datei löschen.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let p = norm(path.as_ref());
    FS.with(|fs| match fs.borrow_mut().remove(&p) {
        Some(Node::File { .. }) => Ok(()),
        Some(n) => {
            fs.borrow_mut().insert(p, n);
            Err(io::Error::new(io::ErrorKind::IsADirectory, "Ordner"))
        }
        None => Err(not_found(&p)),
    })
}

/// Leeren Ordner löschen.
pub fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let p = norm(path.as_ref());
    FS.with(|fs| {
        let mut fs = fs.borrow_mut();
        if fs.keys().any(|k| k != &p && k.starts_with(&p)) {
            return Err(io::Error::new(io::ErrorKind::DirectoryNotEmpty, "Ordner nicht leer"));
        }
        fs.remove(&p).map(|_| ()).ok_or_else(|| not_found(&p))
    })
}

/// Ordner samt Inhalt löschen.
pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let p = norm(path.as_ref());
    FS.with(|fs| {
        let mut fs = fs.borrow_mut();
        let before = fs.len();
        fs.retain(|k, _| !k.starts_with(&p));
        if fs.len() == before { Err(not_found(&p)) } else { Ok(()) }
    })
}

/// Umbenennen (Datei oder Ordner samt Inhalt; ein vorhandenes Ziel wird ersetzt).
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let (from, to) = (norm(from.as_ref()), norm(to.as_ref()));
    FS.with(|fs| {
        let mut fs = fs.borrow_mut();
        if !fs.contains_key(&from) {
            return Err(not_found(&from));
        }
        let moved: Vec<(PathBuf, Node)> =
            fs.iter().filter(|(k, _)| k.starts_with(&from)).map(|(k, v)| (k.clone(), v.clone())).collect();
        fs.retain(|k, _| !k.starts_with(&from) && !k.starts_with(&to));
        ensure_parents(&mut fs, &to);
        for (k, v) in moved {
            let rel = k.strip_prefix(&from).unwrap_or(Path::new(""));
            fs.insert(if rel.as_os_str().is_empty() { to.clone() } else { to.join(rel) }, v);
        }
        Ok(())
    })
}

/// Datei kopieren; liefert die Zahl der Bytes.
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    let data = read(from)?;
    let n = data.len() as u64;
    write(to, data)?;
    Ok(n)
}

/// Im Browser ohne Wirkung (kein Linkkonzept) – Fehler „nicht unterstützt“.
pub fn hard_link(_from: impl AsRef<Path>, _to: impl AsRef<Path>) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

/// Normalisierter Pfad, falls vorhanden.
pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let p = norm(path.as_ref());
    FS.with(|fs| if fs.borrow().contains_key(&p) { Ok(p.clone()) } else { Err(not_found(&p)) })
}

/// Rechte gibt es im Speicher nicht – ohne Wirkung.
pub fn set_permissions(path: impl AsRef<Path>, _perm: Permissions) -> io::Result<()> {
    metadata(path).map(|_| ())
}

/// Rechte (nur für die Signatur von [`set_permissions`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permissions {
    readonly: bool,
}

impl Permissions {
    /// schreibgeschützt?
    pub fn readonly(&self) -> bool {
        self.readonly
    }
    /// schreibgeschützt setzen
    pub fn set_readonly(&mut self, readonly: bool) {
        self.readonly = readonly;
    }
}

/// Art eines Eintrags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileType {
    dir: bool,
}

impl FileType {
    /// Ordner?
    pub fn is_dir(&self) -> bool {
        self.dir
    }
    /// Datei?
    pub fn is_file(&self) -> bool {
        !self.dir
    }
    /// Verknüpfungen gibt es nicht.
    pub fn is_symlink(&self) -> bool {
        false
    }
}

/// Angaben zu einem Eintrag.
#[derive(Debug, Clone)]
pub struct Metadata {
    len: u64,
    dir: bool,
    modified: SystemTime,
}

impl Metadata {
    /// Größe in Bytes
    pub fn len(&self) -> u64 {
        self.len
    }
    /// leer?
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Datei?
    pub fn is_file(&self) -> bool {
        !self.dir
    }
    /// Ordner?
    pub fn is_dir(&self) -> bool {
        self.dir
    }
    /// Verknüpfungen gibt es nicht.
    pub fn is_symlink(&self) -> bool {
        false
    }
    /// Art
    pub fn file_type(&self) -> FileType {
        FileType { dir: self.dir }
    }
    /// letzte Änderung
    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(self.modified)
    }
    /// Rechte
    pub fn permissions(&self) -> Permissions {
        Permissions { readonly: false }
    }
}

/// Angaben zu einem Pfad.
pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    let p = norm(path.as_ref());
    FS.with(|fs| match fs.borrow().get(&p) {
        Some(Node::File { data, modified }) => Ok(Metadata { len: data.len() as u64, dir: false, modified: *modified }),
        Some(Node::Dir) => Ok(Metadata { len: 0, dir: true, modified: SystemTime::UNIX_EPOCH }),
        None => Err(not_found(&p)),
    })
}

/// wie [`metadata`] (Verknüpfungen gibt es nicht)
pub fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    metadata(path)
}

/// Eintrag eines Ordners.
#[derive(Debug, Clone)]
pub struct DirEntry {
    path: PathBuf,
    meta: Metadata,
}

impl DirEntry {
    /// voller Pfad
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
    /// Name
    pub fn file_name(&self) -> std::ffi::OsString {
        self.path.file_name().map(|n| n.to_os_string()).unwrap_or_default()
    }
    /// Angaben
    pub fn metadata(&self) -> io::Result<Metadata> {
        Ok(self.meta.clone())
    }
    /// Art
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.meta.file_type())
    }
}

/// Einträge eines Ordners (direkte Kinder, sortiert).
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<std::vec::IntoIter<io::Result<DirEntry>>> {
    let p = norm(path.as_ref());
    FS.with(|fs| {
        let fs = fs.borrow();
        if !matches!(fs.get(&p), Some(Node::Dir)) {
            return Err(not_found(&p));
        }
        let out: Vec<io::Result<DirEntry>> = fs
            .iter()
            .filter(|(k, _)| k.parent() == Some(p.as_path()) && **k != p)
            .map(|(k, v)| {
                let meta = match v {
                    Node::File { data, modified } => {
                        Metadata { len: data.len() as u64, dir: false, modified: *modified }
                    }
                    Node::Dir => Metadata { len: 0, dir: true, modified: SystemTime::UNIX_EPOCH },
                };
                Ok(DirEntry { path: k.clone(), meta })
            })
            .collect();
        Ok(out.into_iter())
    })
}

/// Offene Datei: Inhalt im Speicher, Schreibzugriffe gehen bei `flush` und beim Schließen zurück.
#[derive(Debug)]
pub struct File {
    path: PathBuf,
    cursor: io::Cursor<Vec<u8>>,
    writable: bool,
    dirty: bool,
}

impl File {
    /// Zum Lesen öffnen.
    pub fn open(path: impl AsRef<Path>) -> io::Result<File> {
        let p = norm(path.as_ref());
        let data = read(&p)?;
        Ok(File { path: p, cursor: io::Cursor::new(data), writable: false, dirty: false })
    }
    /// Zum Schreiben anlegen bzw. leeren.
    pub fn create(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().write(true).create(true).truncate(true).open(path)
    }
    /// Angaben
    pub fn metadata(&self) -> io::Result<Metadata> {
        Ok(Metadata { len: self.cursor.get_ref().len() as u64, dir: false, modified: now() })
    }
    /// Inhalt sichern (im Speicher ohne weitere Wirkung)
    pub fn sync_all(&self) -> io::Result<()> {
        Ok(())
    }
    /// wie [`File::sync_all`]
    pub fn sync_data(&self) -> io::Result<()> {
        Ok(())
    }
    /// Länge setzen
    pub fn set_len(&mut self, size: u64) -> io::Result<()> {
        self.cursor.get_mut().resize(size as usize, 0);
        self.dirty = true;
        Ok(())
    }
    /// Rechte (ohne Wirkung)
    pub fn set_permissions(&self, _perm: Permissions) -> io::Result<()> {
        Ok(())
    }
    fn store(&mut self) -> io::Result<()> {
        if self.writable && self.dirty {
            write(&self.path, self.cursor.get_ref())?;
            self.dirty = false;
        }
        Ok(())
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.writable {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "nur zum Lesen geöffnet"));
        }
        self.dirty = true;
        self.cursor.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.store()
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(pos)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = self.store();
    }
}

/// Optionen zum Öffnen.
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

impl OpenOptions {
    /// neue Optionen
    pub fn new() -> OpenOptions {
        OpenOptions::default()
    }
    /// lesen
    pub fn read(&mut self, v: bool) -> &mut Self {
        self.read = v;
        self
    }
    /// schreiben
    pub fn write(&mut self, v: bool) -> &mut Self {
        self.write = v;
        self
    }
    /// anhängen
    pub fn append(&mut self, v: bool) -> &mut Self {
        self.append = v;
        self.write |= v;
        self
    }
    /// leeren
    pub fn truncate(&mut self, v: bool) -> &mut Self {
        self.truncate = v;
        self
    }
    /// anlegen, falls nicht vorhanden
    pub fn create(&mut self, v: bool) -> &mut Self {
        self.create = v;
        self
    }
    /// nur neu anlegen
    pub fn create_new(&mut self, v: bool) -> &mut Self {
        self.create_new = v;
        self
    }
    /// Rechte (Unix; ohne Wirkung)
    pub fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }
    /// öffnen
    pub fn open(&self, path: impl AsRef<Path>) -> io::Result<File> {
        let p = norm(path.as_ref());
        let existing = read(&p);
        if self.create_new && existing.is_ok() {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        let mut data = match existing {
            Ok(d) => d,
            Err(_) if self.create || self.create_new => {
                write(&p, [])?;
                Vec::new()
            }
            Err(e) => return Err(e),
        };
        if self.truncate {
            data.clear();
        }
        let mut cursor = io::Cursor::new(data);
        if self.append {
            cursor.seek(SeekFrom::End(0))?;
        }
        Ok(File { path: p, cursor, writable: self.write, dirty: self.truncate })
    }
}

/// Web-Shell: alle Dateien unter `prefix` (Pfad, Inhalt) – zum Sichern im Browser.
pub fn snapshot(prefix: impl AsRef<Path>) -> Vec<(PathBuf, Vec<u8>)> {
    let p = norm(prefix.as_ref());
    FS.with(|fs| {
        fs.borrow()
            .iter()
            .filter(|(k, _)| k.starts_with(&p))
            .filter_map(|(k, v)| match v {
                Node::File { data, .. } => Some((k.clone(), data.clone())),
                Node::Dir => None,
            })
            .collect()
    })
}

/// Web-Shell: gesicherte Dateien zurückspielen (vor dem Start des Kerns).
pub fn restore(files: impl IntoIterator<Item = (PathBuf, Vec<u8>)>) -> io::Result<()> {
    for (p, data) in files {
        write(p, data)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_dirs_and_rename() {
        write("/pida/config/settings.toml", b"a = 1").unwrap();
        assert_eq!(read_to_string("/pida/config/./settings.toml").unwrap(), "a = 1");
        assert!(metadata("/pida/config").unwrap().is_dir(), "Elternordner angelegt");
        assert_eq!(metadata("/pida/config/settings.toml").unwrap().len(), 5);
        rename("/pida/config", "/pida/cfg2").unwrap();
        assert!(read("/pida/config/settings.toml").is_err());
        let names: Vec<_> = read_dir("/pida/cfg2").unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["settings.toml"]);
        remove_dir_all("/pida/cfg2").unwrap();
        assert!(metadata("/pida/cfg2").is_err());
    }

    #[test]
    fn files_open_options_and_snapshot() {
        {
            let mut f = OpenOptions::new().write(true).create_new(true).open("/out/a.pdf").unwrap();
            f.write_all(b"%PDF-1.7").unwrap();
        }
        assert!(OpenOptions::new().write(true).create_new(true).open("/out/a.pdf").is_err(), "create_new");
        let mut s = String::new();
        File::open("/out/a.pdf").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "%PDF-1.7");
        assert!(File::open("/out/a.pdf").unwrap().write(b"x").is_err(), "nur lesen");
        write("/keep/x", b"1").unwrap();
        let snap = snapshot("/keep");
        remove_dir_all("/keep").unwrap();
        restore(snap).unwrap();
        assert_eq!(read("/keep/x").unwrap(), b"1");
    }
}

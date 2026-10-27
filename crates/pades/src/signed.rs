//! Signiertes PDF, Selbstprüfung I1–I6 und atomares Schreiben (Plan §3.6, §6.2 „Ausgabe“).

use pida_fs::PathExt as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pida_cades::{AlgorithmPolicy, Certificate, DigestAlg, Expect, ExpectedSigner, verify_detached};
use pida_pdf_cos::{Limits, SourcePdf};
use pida_pdf_incremental::ByteRange;
use pida_render::RenderBackend;

use crate::PadesError;
use crate::prepared::PreparedMeta;
use crate::sigs;

/// Invariante (Plan §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Invariant {
    /// Original ist byte-identischer Präfix
    I1Prefix,
    /// ByteRange deckt alles außer `<…>` ab, `b + c` = Dateilänge
    I2ByteRange,
    /// genau ein SignerInfo, message-digest, Signatur, signing-certificate(-v2), erwarteter Signer
    I3Cms,
    /// Status jeder Vorsignatur unverändert, ByteRange endet unverändert im Präfix
    I4PriorSignatures,
    /// mit `pdf-cos` **und** PDFium parsebar, Seitenzahl gleich bzw. +1, neue Signatur gefunden
    I5Reparse,
    /// Increment enthält nur erlaubte Änderungen (Whitelist)
    I6AllowedChanges,
}

impl Invariant {
    /// Stabiler Fehlercode (`pades-invariant-i1` … `-i6`; CLI-Exit 6).
    pub fn code(self) -> &'static str {
        match self {
            Invariant::I1Prefix => "pades-invariant-i1",
            Invariant::I2ByteRange => "pades-invariant-i2",
            Invariant::I3Cms => "pades-invariant-i3",
            Invariant::I4PriorSignatures => "pades-invariant-i4",
            Invariant::I5Reparse => "pades-invariant-i5",
            Invariant::I6AllowedChanges => "pades-invariant-i6",
        }
    }
}

/// Verletzung (Fehlercode für UI/CLI-Exit 6; optional Diagnosepaket ohne Dokumentinhalt).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("Invariante {invariant:?} verletzt: {detail}")]
pub struct InvariantViolation {
    /// Invariante
    pub invariant: Invariant,
    /// Detail (ohne Dokumentinhalt)
    pub detail: String,
}

impl InvariantViolation {
    pub(crate) fn new(invariant: Invariant, detail: impl Into<String>) -> Self {
        Self { invariant, detail: detail.into() }
    }
}

/// Status einer Vorsignatur vorher/nachher (I4: gleich, nicht zwingend gültig).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PriorStatus {
    /// gültig
    Valid,
    /// ungültig
    Invalid,
    /// vom eigenen Verifier nicht prüfbar (z. B. `adbe.pkcs7.sha1`)
    NotVerifiable,
}

/// Kontext der Selbstprüfung.
#[derive(Clone, Copy)]
pub struct CheckContext<'a> {
    /// Original
    pub source: &'a SourcePdf,
    /// erwarteter Signer (InfoboxRead, Cache, Keystore)
    pub expected_signer: Option<&'a Certificate>,
    /// Renderer für I5 (PDFium); `None` nur in Tests ohne Bibliothek
    pub renderer: Option<&'a dyn RenderBackend>,
}

impl std::fmt::Debug for CheckContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckContext")
            .field("expected_signer", &self.expected_signer)
            .field("renderer", &self.renderer.is_some())
            .finish()
    }
}

/// Bericht der Selbstprüfung. `Debug` zeigt Feldnamen nur als Länge (Regel 9).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CheckReport {
    /// geprüfte Invarianten
    pub checked: Vec<Invariant>,
    /// Status der Vorsignaturen (Feldname, vorher, nachher)
    pub prior: Vec<(String, PriorStatus, PriorStatus)>,
    /// Warnungen (z. B. I5 ohne PDFium übersprungen)
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for CheckReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prior: Vec<(usize, &PriorStatus, &PriorStatus)> =
            self.prior.iter().map(|(n, a, b)| (n.chars().count(), a, b)).collect();
        f.debug_struct("CheckReport")
            .field("checked", &self.checked)
            .field("prior", &prior)
            .field("warnings", &self.warnings)
            .finish()
    }
}

/// Warnung: I5 ohne PDFium (nur `pdf-cos`).
pub const WARNING_I5_WITHOUT_PDFIUM: &str = "pades-i5-without-pdfium";

/// Meldung, wenn ohne bestandene Selbstprüfung geschrieben werden soll.
const NOT_SELF_CHECKED: &str = "Selbstprüfung (I1–I6) nicht bestanden – es wird nichts gespeichert";

/// Ziel der Ausgabe (Regeln F-81…F-85 löst der Workflow auf).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputTarget {
    /// Zieldatei
    pub path: PathBuf,
    /// Überschreiben erlaubt (nach Rückfrage)
    pub overwrite: bool,
}

/// Ergebnis des Schreibens.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputInfo {
    /// Pfad
    pub path: PathBuf,
    /// Länge
    pub len: u64,
    /// SHA-256 (hex)
    pub sha256: String,
}

/// Was `finish_*` über das Ergebnis weiß.
#[derive(Clone)]
pub(crate) struct SignedMeta {
    pub prepared: PreparedMeta,
    pub byte_range: ByteRange,
    pub contents_range: (u64, u64),
    /// Signer laut CMS (geprüft in `finish_*`)
    pub signer: Certificate,
}

/// Signiertes PDF (Original + Increment mit eingesetztem CMS). `Debug` ohne Inhalt (Regel 9).
///
/// Gespeichert wird nur nach bestandener Selbstprüfung: [`Self::write_atomically`] und
/// [`Self::write_to`] verweigern das Schreiben, solange [`Self::self_check`] auf **dieser**
/// Instanz nicht erfolgreich war (Plan §3.6 „Verletzt die Ausgabe eine Invariante, wird nichts
/// gespeichert“). [`Self::bytes`] bleibt für Vorschau und Diagnose ungeprüft.
pub struct SignedPdf {
    original: Arc<[u8]>,
    increment: Vec<u8>,
    meta: SignedMeta,
    /// `self_check` bestanden
    checked: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for SignedPdf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedPdf")
            .field("original_len", &self.original.len())
            .field("increment_len", &self.increment.len())
            .field("byte_range", &self.meta.byte_range)
            .field("checked", &self.is_checked())
            .finish()
    }
}

impl SignedPdf {
    pub(crate) fn new(original: Arc<[u8]>, increment: Vec<u8>, meta: SignedMeta) -> Self {
        Self { original, increment, meta, checked: std::sync::atomic::AtomicBool::new(false) }
    }

    /// Hat [`Self::self_check`] auf dieser Instanz bestanden?
    pub fn is_checked(&self) -> bool {
        self.checked.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Prüft I1–I6. Die erste Verletzung bricht ab; dann darf **nichts** gespeichert werden (das
    /// Schreiben bleibt gesperrt). Erst ein Erfolg gibt [`Self::write_atomically`] und
    /// [`Self::write_to`] frei.
    pub fn self_check(&self, ctx: &CheckContext<'_>) -> Result<CheckReport, InvariantViolation> {
        self.checked.store(false, std::sync::atomic::Ordering::Release);
        let report = self.run_checks(ctx)?;
        self.checked.store(true, std::sync::atomic::Ordering::Release);
        Ok(report)
    }

    fn run_checks(&self, ctx: &CheckContext<'_>) -> Result<CheckReport, InvariantViolation> {
        use Invariant::*;
        let mut checked = Vec::new();
        let mut warnings = Vec::new();
        let src = ctx.source;

        // I1: Präfix
        if self.original.len() as u64 != src.len() || self.original.as_ref() != src.bytes() {
            return Err(InvariantViolation::new(I1Prefix, "Original weicht vom Präfix der Ausgabe ab"));
        }
        if !self.increment.starts_with(b"\r\n") && !self.increment.starts_with(b"\n") {
            return Err(InvariantViolation::new(I1Prefix, "Increment beginnt nicht mit Zeilenende"));
        }
        checked.push(I1Prefix);

        let out: Arc<[u8]> = Arc::from(self.bytes());
        let total = out.len() as u64;

        // I5 (pdf-cos, zuerst – I2/I3/I6 arbeiten auf dem Re-Parse)
        let new = SourcePdf::open(Arc::clone(&out), &Limits::default())
            .map_err(|e| InvariantViolation::new(I5Reparse, format!("pdf-cos: {e}")))?;
        let expected_pages =
            self.meta.prepared.page_count_before + usize::from(self.meta.prepared.build.new_page_id.is_some());
        if new.page_count() != expected_pages {
            return Err(InvariantViolation::new(
                I5Reparse,
                format!("Seitenzahl {} statt {expected_pages}", new.page_count()),
            ));
        }
        let fields = new.signature_fields().map_err(|e| InvariantViolation::new(I5Reparse, format!("Felder: {e}")))?;
        let build = &self.meta.prepared.build;
        let field = fields
            .iter()
            .find(|f| f.field_id == build.field_id && f.signed && f.value_id == Some(build.sig_dict_id))
            .ok_or_else(|| InvariantViolation::new(I5Reparse, "neue Signatur im Re-Parse nicht gefunden"))?;
        if field.qualified_name != build.field_name {
            return Err(InvariantViolation::new(I5Reparse, "Feldname der neuen Signatur weicht ab"));
        }
        let info = sigs::read_sig_dict(&new, field)
            .ok_or_else(|| InvariantViolation::new(I5Reparse, "Signaturdictionary nicht lesbar"))?;

        // I2: ByteRange
        let br = info.byte_range().ok_or_else(|| InvariantViolation::new(I2ByteRange, "ByteRange fehlt/ungültig"))?;
        let [b0, b1, b2, b3] = br.0;
        let ok = br == self.meta.byte_range
            && br.is_consistent_with(total)
            && b0 == 0
            && b1 >= src.len()
            && out.get(b1 as usize) == Some(&b'<')
            && b2 >= 1
            && out.get(b2 as usize - 1) == Some(&b'>')
            && (b2 - b1) == 2 * info.contents.len() as u64 + 2
            && out[b1 as usize + 1..b2 as usize - 1].iter().all(u8::is_ascii_hexdigit)
            && (self.meta.contents_range == (b1, b2));
        if !ok {
            return Err(InvariantViolation::new(I2ByteRange, format!("ByteRange {:?} passt nicht zur Datei", br.0)));
        }
        let _ = b3;
        checked.push(I2ByteRange);

        // I3: CMS – erwartet wird das Zertifikat des Aufrufers bzw. aus `prepare`; beide müssen
        // übereinstimmen. Erst ohne beide gilt der Signer laut CMS (nur Strukturprüfung).
        let prepared_cert = self.meta.prepared.expected_cert();
        if let (Some(p), Some(x)) = (&prepared_cert, ctx.expected_signer)
            && p != x
        {
            return Err(InvariantViolation::new(I3Cms, "erwarteter Signer weicht vom Zertifikat aus prepare ab"));
        }
        let expected = ctx.expected_signer.cloned().or(prepared_cert).unwrap_or_else(|| self.meta.signer.clone());
        let expect = Expect {
            signer: ExpectedSigner::Certificate(expected),
            require_signing_certificate: true,
            max_len: None,
            policy: AlgorithmPolicy::NewSignature,
        };
        let mut reader = sigs::RangeReader::new(&out, &[b0 as i64, b1 as i64, b2 as i64, b3 as i64])
            .ok_or_else(|| InvariantViolation::new(I3Cms, "ByteRange außerhalb"))?;
        verify_detached(&info.contents, &mut reader, &expect)
            .map_err(|e| InvariantViolation::new(I3Cms, format!("CMS ({}): {e}", e.code())))?;
        checked.push(I3Cms);

        // I4: Vorsignaturen
        let mut prior = Vec::new();
        let old_fields =
            src.signature_fields().map_err(|e| InvariantViolation::new(I4PriorSignatures, e.to_string()))?;
        for of in old_fields.iter().filter(|f| f.signed) {
            let Some(before) = sigs::read_sig_dict(src, of) else { continue };
            let (st_before, _) = sigs::signature_status(src.bytes(), &before);
            if before.covered_end().is_some_and(|e| e > src.len()) {
                return Err(InvariantViolation::new(
                    I4PriorSignatures,
                    "ByteRange einer Vorsignatur endet nicht im Präfix",
                ));
            }
            let after_field = fields.iter().find(|f| f.field_id == of.field_id).ok_or_else(|| {
                InvariantViolation::new(I4PriorSignatures, "Vorsignatur nach dem Signieren nicht gefunden")
            })?;
            let after = sigs::read_sig_dict(&new, after_field).ok_or_else(|| {
                InvariantViolation::new(I4PriorSignatures, "Signaturdictionary einer Vorsignatur fehlt")
            })?;
            if after.byte_range_raw != before.byte_range_raw || after.contents != before.contents {
                return Err(InvariantViolation::new(
                    I4PriorSignatures,
                    "Signaturdictionary einer Vorsignatur geändert",
                ));
            }
            let (st_after, _) = sigs::signature_status(&out, &after);
            if st_after != st_before {
                return Err(InvariantViolation::new(
                    I4PriorSignatures,
                    format!("Status einer Vorsignatur {st_before:?} → {st_after:?}"),
                ));
            }
            prior.push((of.qualified_name.clone(), st_before, st_after));
        }
        checked.push(I4PriorSignatures);

        // I5 (PDFium)
        match ctx.renderer {
            Some(r) => {
                let doc = r
                    .open(Arc::clone(&out), None)
                    .map_err(|e| InvariantViolation::new(I5Reparse, format!("PDFium: {}", e.code())))?;
                if doc.page_count() != expected_pages {
                    return Err(InvariantViolation::new(
                        I5Reparse,
                        format!("PDFium: Seitenzahl {} statt {expected_pages}", doc.page_count()),
                    ));
                }
                if let Some(p) = build.page_index.filter(|p| *p < doc.page_count()) {
                    doc.render_page(p, 0.25)
                        .map_err(|e| InvariantViolation::new(I5Reparse, format!("PDFium-Render: {}", e.code())))?;
                }
            }
            None => warnings.push(WARNING_I5_WITHOUT_PDFIUM.to_string()),
        }
        checked.push(I5Reparse);

        // I6: Whitelist
        crate::check::allowed_changes(src, &new, &self.meta)?;
        checked.push(I6AllowedChanges);

        Ok(CheckReport { checked, prior, warnings })
    }

    /// Länge.
    pub fn len(&self) -> u64 {
        self.original.len() as u64 + self.increment.len() as u64
    }

    /// `true` bei Länge 0 (nie).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Vollständige Bytes (Kopie; für große Dateien [`Self::write_to`] bevorzugen).
    pub fn bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.len() as usize);
        v.extend_from_slice(&self.original);
        v.extend_from_slice(&self.increment);
        v
    }

    /// `/ByteRange` der neuen Signatur.
    pub fn byte_range(&self) -> ByteRange {
        self.meta.byte_range
    }

    /// Signerzertifikat laut CMS.
    pub fn signer(&self) -> &Certificate {
        &self.meta.signer
    }

    /// Schreibt in einen Writer (Mobile: SAF/`UIDocumentPicker`-Stream) – nur nach bestandener
    /// Selbstprüfung, sonst `ErrorKind::PermissionDenied` ohne ein geschriebenes Byte.
    pub fn write_to(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.is_checked() {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, NOT_SELF_CHECKED));
        }
        self.write_unchecked(w)
    }

    fn write_unchecked(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        w.write_all(&self.original)?;
        w.write_all(&self.increment)
    }

    /// Temp-Datei im Zielordner, `fsync`, atomares Umbenennen; das Original wird nie
    /// überschrieben, außer `target.overwrite`. Ohne `overwrite` scheitert das Schreiben mit
    /// `ErrorKind::AlreadyExists`, wenn das Ziel existiert (auch bei gleichzeitigem Anlegen:
    /// Hardlink statt Umbenennen).
    ///
    /// Ohne bestandene Selbstprüfung: [`PadesError::NotSelfChecked`], nichts wird angelegt.
    pub fn write_atomically(&self, target: &OutputTarget) -> Result<OutputInfo, PadesError> {
        if !self.is_checked() {
            return Err(PadesError::NotSelfChecked);
        }
        let path = &target.path;
        if !target.overwrite && path.fs_exists() {
            return Err(PadesError::Io(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Zieldatei existiert")));
        }
        let tmp = temp_path(path);
        let result = (|| -> std::io::Result<()> {
            let mut f = pida_fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            self.write_unchecked(&mut f)?;
            f.flush()?;
            f.sync_all()?;
            drop(f);
            if target.overwrite {
                pida_fs::rename(&tmp, path)?;
            } else {
                match pida_fs::hard_link(&tmp, path) {
                    Ok(()) => {
                        let _ = pida_fs::remove_file(&tmp);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(e),
                    // Dateisysteme ohne Hardlinks (FAT, Netzlaufwerke): Umbenennen nach Prüfung
                    Err(_) => {
                        if path.fs_exists() {
                            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Zieldatei existiert"));
                        }
                        pida_fs::rename(&tmp, path)?;
                    }
                }
            }
            sync_dir(path);
            Ok(())
        })();
        if let Err(e) = result {
            let _ = pida_fs::remove_file(&tmp);
            return Err(PadesError::Io(e));
        }
        let mut h = DigestAlg::Sha256.hasher();
        h.update(&self.original);
        h.update(&self.increment);
        Ok(OutputInfo { path: path.clone(), len: self.len(), sha256: h.finalize().to_hex() })
    }
}

/// Temp-Datei neben `path` (gleiches Verzeichnis, damit das Umbenennen atomar ist).
pub(crate) fn temp_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = pida_fs::system_now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "ausgabe".into());
    path.with_file_name(format!(".{name}.{}-{nanos}-{n}.pida-tmp", pida_fs::process_id()))
}

/// `fsync` des Verzeichnisses nach dem Umbenennen (Unix; sonst ohne Wirkung).
fn sync_dir(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
        if let Ok(d) = pida_fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

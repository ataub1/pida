//! Zweiphasenmodell: vorbereitetes PDF mit Platzhalter (Plan §3.4, §6.2).

use std::io::{Read, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pida_cades::{
    AlgorithmPolicy, AssembleOptions, CertChain, Certificate, CmsError, CmsProfile, Digest, DigestAlg, Expect,
    ExpectedSigner, RawSignature, SignedAttrs, assemble, estimate_cms_len, select_algorithm, verify_detached,
};
use pida_pdf_cos::{ObjectId, SourcePdf, TailKind, XrefHealth};
use pida_pdf_incremental::{
    ByteRange, ExcludedRange, Increment, RESERVE_MAX, RESERVE_MIN_MODEL_A, RESERVE_MODEL_B, WrittenKind,
};
use time::OffsetDateTime;

use crate::analyze::codes;
use crate::build::{self, BuildMeta};
use crate::plan::{Reserve, SignPlan};
use crate::signed::{SignedMeta, SignedPdf};
use crate::{FinishError, PadesError};

/// Zuschlag der Modell-A-Reserve über die exakte Höchstlänge (Plan §6.2).
const MODEL_A_MARGIN: usize = 1024;

/// Vorbereitetes, nach `prepare()` unveränderliches Dokument. `Debug` ohne Inhalt (Regel 9).
pub struct Prepared {
    original: Arc<[u8]>,
    increment: Increment,
    signing_time: OffsetDateTime,
    plan_profile_id: String,
    meta: PreparedMeta,
}

/// Was neben den Bytes zum vorbereiteten Zustand gehört (auch Inhalt der Persistenz-Datei).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PreparedMeta {
    pub build: BuildMeta,
    pub cms_profile: CmsProfile,
    pub include_intermediates: bool,
    /// DER des Zertifikats aus `prepare` (hex), Erwartung für I3
    pub expected_signer: Option<String>,
    pub page_count_before: usize,
}

impl PreparedMeta {
    /// Zertifikat aus `prepare` (maßgeblich für I3: Signaturblock und `/Name` stammen daraus).
    pub(crate) fn expected_cert(&self) -> Option<Certificate> {
        self.expected_signer.as_deref().and_then(|h| hex::decode(h).ok()).and_then(|d| Certificate::from_der(&d).ok())
    }
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("original_len", &self.original.len())
            .field("increment", &self.increment)
            .field("signing_time", &self.signing_time)
            .field("profile_id", &self.plan_profile_id)
            .field("new_field", &self.meta.build.new_field)
            .field("notices", &self.meta.build.notices)
            .finish()
    }
}

/// Persistierter Zustand (Mobile-Lebenszyklus: App-Wechsel ist kein Abbruch, Plan §3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedHandle {
    /// Datei in der App-Sandbox (Rechte 0600)
    pub path: PathBuf,
    /// SHA-256 über das fertige Platzhalter-PDF **und** die Begleitdatei (`<datei>.json`, in
    /// dieser Reihenfolge aneinandergehängt) – Integritätsprüfung beim Fortsetzen; die
    /// Begleitdatei trägt die Erwartungen der Selbstprüfung (Zertifikat, Feld, Seitenzahl).
    pub sha256: Vec<u8>,
}

/// SHA-256 über Platzhalter-PDF und Begleitdatei ([`PreparedHandle::sha256`]).
fn state_digest(pdf: &[u8], json: &[u8]) -> Vec<u8> {
    let mut h = DigestAlg::Sha256.hasher();
    h.update(pdf);
    h.update(json);
    h.finalize().value
}

/// Fehlertext für eine unlesbare Begleitdatei: nur die Position, nie Inhalte (Regel 9).
fn sidecar_error(e: &serde_json::Error) -> PadesError {
    PadesError::InvalidState(format!("Begleitdatei nicht lesbar (Zeile {}, Spalte {})", e.line(), e.column()))
}

/// Inhalt der Begleitdatei (`<datei>.json`) von [`Prepared::persist`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PersistedState {
    format: u32,
    original_len: u64,
    pdf_sha256: String,
    profile_id: String,
    signing_time_unix_nanos: i128,
    signing_time_offset_secs: i32,
    contents_range: (u64, u64),
    byte_range_field: (u64, u64),
    byte_range: [u64; 4],
    signature_dict_id: Option<ObjectId>,
    written: Vec<(ObjectId, WrittenKind)>,
    meta: PreparedMeta,
}

const PERSIST_FORMAT: u32 = 1;

/// Annahmeprüfung vor dem Aufbau: dieselben K5-Befunde wie [`crate::analyze`] in derselben
/// Reihenfolge (Plan §6.1; K2-Befunde mit Vorsignaturen sind K5).
fn preflight(src: &SourcePdf) -> Result<(), PadesError> {
    let reject = |code: &str| Err(PadesError::Rejected(code.into()));
    let signed = src.signature_fields()?.iter().any(|f| f.signed);
    match src.xref_health() {
        XrefHealth::Reconstructed => return reject(codes::XREF_RECONSTRUCTED),
        XrefHealth::Repaired if signed => return reject(codes::XREF_REPAIRED),
        _ => {}
    }
    if signed && src.tail().kind == TailKind::Garbage {
        return reject(codes::TAIL_GARBAGE);
    }
    if signed
        && src
            .signature_fields()?
            .iter()
            .filter(|f| f.signed)
            .filter_map(|f| crate::sigs::read_sig_dict(src, f))
            .any(|info| info.covered_end().is_some_and(|e| e > src.len()))
    {
        return reject(codes::PRIOR_SIGNATURE_BEYOND_EOF);
    }
    if !src.rights().may_sign() {
        return reject(codes::PERMISSIONS);
    }
    if src.doc_mdp()?.is_some_and(|d| d.permission == 1) {
        return reject(codes::DOCMDP_P1);
    }
    if src.needs_rendering() {
        return reject(codes::XFA_DYNAMIC);
    }
    Ok(())
}

/// `/Contents`-Reserve nach ADR 0001 §8.4.
pub(crate) fn compute_reserve(plan: &SignPlan, cert: Option<&CertChain>) -> Result<usize, PadesError> {
    let minimum = plan.signature_size.unwrap_or(0);
    let reserve = match plan.reserve {
        Reserve::ModelA => match cert {
            Some(chain) => {
                let alg = select_algorithm(&chain.signer.key_type()?)?;
                let est = estimate_cms_len(chain, plan.cms_profile, alg, plan.include_intermediates)?;
                (est + MODEL_A_MARGIN).max(RESERVE_MIN_MODEL_A).max(minimum)
            }
            // ohne Zertifikat (sollte bei Modell A nicht vorkommen) die sichere Modell-B-Reserve
            None => RESERVE_MODEL_B.max(minimum),
        },
        Reserve::ModelB | Reserve::ModelBAtLeast(_) => {
            let floor = match plan.reserve {
                Reserve::ModelBAtLeast(n) => n,
                _ => RESERVE_MODEL_B,
            };
            let mut r = floor.max(minimum);
            if let Some(chain) = cert
                && let Ok(key) = chain.signer.key_type()
                && let Ok(alg) = select_algorithm(&key)
                && let Ok(est) = estimate_cms_len(chain, CmsProfile::PdfasCompat, alg, true)
                && est.saturating_mul(2) > r
            {
                // Schätzung über 50 % der Reserve → vor `prepare` vergrößern (Plan §6.2)
                r = est.saturating_mul(2).div_ceil(1024) * 1024;
            }
            r
        }
        Reserve::Bytes(n) => n.max(minimum).max(1),
    };
    if reserve > RESERVE_MAX {
        return Err(PadesError::InvalidPlan(format!("Reserve {reserve} Bytes größer als {RESERVE_MAX}")));
    }
    Ok(reserve)
}

/// Baut das Increment (AcroForm, Feld/Widget, Signaturdictionary, Appearance, ggf. neue Seite,
/// OutputIntent, PDF/UA) und patcht die ByteRange. Prüft vorher die Zertifikatsgültigkeit.
pub fn prepare(src: &SourcePdf, plan: &SignPlan, cert: Option<&CertChain>) -> Result<Prepared, PadesError> {
    if let Some(chain) = cert
        && !chain.signer.is_valid_at(plan.signing_time)
    {
        return Err(PadesError::CertificateNotValid);
    }
    preflight(src)?;
    let reserve = compute_reserve(plan, cert)?;
    let out = build::build(src, plan, reserve)?;
    tracing::debug!(
        reserve,
        increment_len = out.increment.bytes.len(),
        new_field = out.meta.new_field,
        "Signatur vorbereitet"
    );
    Ok(Prepared {
        original: src.shared_bytes(),
        increment: out.increment,
        signing_time: plan.signing_time,
        plan_profile_id: plan.profile_id.clone(),
        meta: PreparedMeta {
            build: out.meta,
            cms_profile: plan.cms_profile,
            include_intermediates: plan.include_intermediates,
            expected_signer: cert.map(|c| hex::encode(c.signer.as_der())),
            page_count_before: src.page_count(),
        },
    })
}

/// Teil eines gestreamten Datenstroms.
enum Seg<'a> {
    Bytes(&'a [u8]),
    Zeros(u64),
}

/// Liest Segmente nacheinander (Blackout-PDF, ByteRange-Daten).
struct Segments<'a> {
    segs: Vec<Seg<'a>>,
    idx: usize,
    pos: u64,
}

impl Read for Segments<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while let Some(seg) = self.segs.get(self.idx) {
            let len = match seg {
                Seg::Bytes(b) => b.len() as u64,
                Seg::Zeros(n) => *n,
            };
            if self.pos >= len {
                self.idx += 1;
                self.pos = 0;
                continue;
            }
            let n = ((len - self.pos) as usize).min(buf.len());
            match seg {
                Seg::Bytes(b) => buf[..n].copy_from_slice(&b[self.pos as usize..self.pos as usize + n]),
                Seg::Zeros(_) => buf[..n].fill(0),
            }
            self.pos += n as u64;
            return Ok(n);
        }
        Ok(0)
    }
}

impl Prepared {
    /// `/ByteRange`.
    pub fn byte_range(&self) -> ByteRange {
        self.increment.byte_range
    }

    /// Für SL `ExcludedByteRange` (From/To inklusive).
    pub fn excluded_range(&self) -> ExcludedRange {
        self.increment.excluded_range()
    }

    /// Länge des fertigen PDFs.
    pub fn total_len(&self) -> u64 {
        self.increment.total_len()
    }

    /// CMS-Reserve in Bytes.
    pub fn reserve(&self) -> usize {
        self.increment.reserve()
    }

    /// Signierzeitpunkt (`/M`).
    pub fn signing_time(&self) -> OffsetDateTime {
        self.signing_time
    }

    /// Profil-ID des Plans.
    pub fn profile_id(&self) -> &str {
        &self.plan_profile_id
    }

    /// CMS-Profil des Plans (Modell A: dasselbe Profil für `SignedAttrsBuilder`).
    pub fn cms_profile(&self) -> CmsProfile {
        self.meta.cms_profile
    }

    /// Voll qualifizierter Name des neuen bzw. befüllten Signaturfelds (Dokumentinhalt –
    /// nicht loggen).
    pub fn field_name(&self) -> &str {
        &self.meta.build.field_name
    }

    /// Hinweise beim Aufbau (`pades-pdfua-skipped`, `pades-output-intent-skipped`).
    pub fn notices(&self) -> &[String] {
        &self.meta.build.notices
    }

    /// Wurde eine neue Seite angehängt?
    pub fn adds_page(&self) -> bool {
        self.meta.build.new_page_id.is_some()
    }

    fn contents_local(&self) -> std::ops::Range<usize> {
        self.increment.local(self.increment.contents_range.clone())
    }

    /// Modell B: komplettes PDF mit `0x00` in der Lücke `<…>`, gestreamt.
    pub fn blacked_out(&self) -> Box<dyn Read + Send + '_> {
        let r = self.contents_local();
        let inc = &self.increment.bytes;
        Box::new(Segments {
            segs: vec![
                Seg::Bytes(&self.original),
                Seg::Bytes(&inc[..r.start]),
                Seg::Zeros((r.end - r.start) as u64),
                Seg::Bytes(&inc[r.end..]),
            ],
            idx: 0,
            pos: 0,
        })
    }

    /// ByteRange-Daten (Original, Increment ohne `<…>`), gestreamt.
    fn signed_data(&self) -> Segments<'_> {
        let [a, b, c] = self.increment.byte_range_parts(&self.original);
        Segments { segs: vec![Seg::Bytes(a), Seg::Bytes(b), Seg::Bytes(c)], idx: 0, pos: 0 }
    }

    /// Modell A: Hash über die ByteRange-Daten (gestreamt).
    pub fn digest(&self, alg: DigestAlg) -> Digest {
        let mut h = alg.hasher();
        for part in self.increment.byte_range_parts(&self.original) {
            h.update(part);
        }
        h.finalize()
    }

    /// Fertiges Platzhalter-PDF (WYSIWYS-Ansicht rendert genau diese Bytes, Plan §4.5).
    pub fn placeholder_pdf(&self) -> Vec<u8> {
        self.increment.to_vec(&self.original)
    }

    /// Persistiert für den Mobile-Lebenszyklus: Platzhalter-PDF (`0600`) und Begleitdatei
    /// `<datei>.json` mit den Offsets; atomar (Temp-Datei, Umbenennen).
    pub fn persist(&self, dir: &Path) -> Result<PreparedHandle, PadesError> {
        let pdf = self.placeholder_pdf();
        let sha = DigestAlg::Sha256.digest(&pdf);
        let hex_sha = sha.to_hex();
        let path = dir.join(format!("pida-prepared-{}.pdf", &hex_sha[..16]));
        let state = PersistedState {
            format: PERSIST_FORMAT,
            original_len: self.original.len() as u64,
            pdf_sha256: hex_sha.clone(),
            profile_id: self.plan_profile_id.clone(),
            signing_time_unix_nanos: self.signing_time.unix_timestamp_nanos(),
            signing_time_offset_secs: self.signing_time.offset().whole_seconds(),
            contents_range: (self.increment.contents_range.start, self.increment.contents_range.end),
            byte_range_field: (self.increment.byte_range_field.start, self.increment.byte_range_field.end),
            byte_range: self.increment.byte_range.0,
            signature_dict_id: self.increment.signature_dict_id,
            written: self.increment.written.clone(),
            meta: self.meta.clone(),
        };
        let json = serde_json::to_vec(&state).map_err(|e| sidecar_error(&e))?;
        write_private(&path, &pdf)?;
        write_private(&path.with_extension("json"), &json)?;
        Ok(PreparedHandle { path, sha256: state_digest(&pdf, &json) })
    }

    /// Lädt einen persistierten Zustand (prüft SHA-256 über PDF und Begleitdatei, Präfix =
    /// Original und die Offsets gegen die Bytes).
    pub fn restore(handle: &PreparedHandle, src: &SourcePdf) -> Result<Prepared, PadesError> {
        let invalid = |m: &str| PadesError::InvalidState(m.to_string());
        let pdf = std::fs::read(&handle.path)?;
        let json = std::fs::read(handle.path.with_extension("json"))?;
        if state_digest(&pdf, &json) != handle.sha256 {
            return Err(invalid("Prüfsumme stimmt nicht"));
        }
        let sha = DigestAlg::Sha256.digest(&pdf);
        let st: PersistedState = serde_json::from_slice(&json).map_err(|e| sidecar_error(&e))?;
        if st.format != PERSIST_FORMAT || st.pdf_sha256 != sha.to_hex() {
            return Err(invalid("Begleitdatei passt nicht zum PDF"));
        }
        let base = src.len();
        if st.original_len != base || (pdf.len() as u64) <= base || &pdf[..base as usize] != src.bytes() {
            return Err(invalid("Original passt nicht"));
        }
        let increment = Increment {
            base_len: base,
            bytes: pdf[base as usize..].to_vec(),
            contents_range: st.contents_range.0..st.contents_range.1,
            byte_range_field: st.byte_range_field.0..st.byte_range_field.1,
            byte_range: ByteRange(st.byte_range),
            signature_dict_id: st.signature_dict_id,
            written: st.written,
        };
        // Offsets gegen die (geprüften) Bytes prüfen
        let total = increment.total_len();
        let (c0, c1) = st.contents_range;
        // Begleitdatei ist nicht durch die Prüfsumme geschützt: Arithmetik nur geprüft
        let ok_contents = c0 >= base
            && c1 <= total
            && c0.checked_add(2).is_some_and(|m| c1 >= m)
            && pdf[c0 as usize] == b'<'
            && pdf[c1 as usize - 1] == b'>'
            && pdf[c0 as usize + 1..c1 as usize - 1].iter().all(|b| *b == b'0');
        let br = increment.byte_range;
        let (f0, f1) = st.byte_range_field;
        let ok_br =
            ok_contents && br == ByteRange::for_contents(c0..c1, total) && f0 >= base && f1 <= total && f1 > f0 && {
                let field = &pdf[f0 as usize..f1 as usize];
                let text = br.to_pdf_string();
                field.starts_with(text.as_bytes()) && field[text.len()..].iter().all(|b| *b == b' ')
            };
        if !ok_contents || !ok_br {
            return Err(invalid("Platzhalter passt nicht zu den Offsets"));
        }
        let offset = time::UtcOffset::from_whole_seconds(st.signing_time_offset_secs)
            .map_err(|_| invalid("Zeitzone ungültig"))?;
        let signing_time = OffsetDateTime::from_unix_timestamp_nanos(st.signing_time_unix_nanos)
            .map_err(|_| invalid("Zeitpunkt ungültig"))?
            .to_offset(offset);
        // `/M` steht im Klartext, wenn das Dokument nicht verschlüsselt ist
        if src.encryption().is_none() {
            let m = format!("/M ({})", pida_pdf_incremental::format_pdf_date(signing_time));
            if !increment.bytes.windows(m.len()).any(|w| w == m.as_bytes()) {
                return Err(invalid("Signierzeitpunkt passt nicht zu /M"));
            }
        }
        Ok(Prepared {
            original: src.shared_bytes(),
            increment,
            signing_time,
            plan_profile_id: st.profile_id,
            meta: st.meta,
        })
    }

    /// Löscht einen persistierten Zustand (nach Erfolg oder Abbruch, Plan §3.5).
    pub fn discard_persisted(handle: &PreparedHandle) -> std::io::Result<()> {
        for p in [handle.path.clone(), handle.path.with_extension("json")] {
            match std::fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Modell B: Längenprüfung, `verify_detached` (I3), Einbetten.
    ///
    /// Die Prüfung verlangt immer wie PDF-AS genau einen SignerInfo, signing-certificate(-v2) und
    /// die Politik für neue Signaturen. Kam in `prepare` ein Zertifikat mit, ist **dieses** der
    /// erwartete Signer (Signaturblock und `/Name` stammen daraus; PDF-AS „Certificates
    /// missmatch!“); eine abweichende Erwartung in `expect` ist ein Fehler
    /// (`cms-unexpected-signer`). Ohne Zertifikat aus `prepare` gilt `expect.signer`. `cms` darf
    /// mit `0x00` aufgefüllt sein.
    pub fn finish_cms(self, cms: &[u8], expect: &Expect) -> Result<SignedPdf, FinishError> {
        let der = pida_cades::trim_contents_padding(cms)?;
        let reserve = self.reserve();
        if der.len() > reserve {
            return Err(FinishError::CmsTooLarge { needed: der.len(), reserve });
        }
        let mut e = expect.clone();
        e.policy = AlgorithmPolicy::NewSignature;
        e.require_signing_certificate = true;
        e.max_len = Some(reserve);
        if let Some(c) = self.meta.expected_cert() {
            if matches!(&e.signer, ExpectedSigner::Certificate(x) if *x != c) {
                return Err(FinishError::Cms(CmsError::UnexpectedSigner));
            }
            e.signer = ExpectedSigner::Certificate(c);
        }
        let report = verify_detached(der, &mut self.signed_data(), &e)?;
        let mut increment = self.increment;
        increment.embed_contents(der)?;
        let meta = SignedMeta {
            prepared: self.meta,
            byte_range: increment.byte_range,
            contents_range: (increment.contents_range.start, increment.contents_range.end),
            written: increment.written.clone(),
            signer: report.signer,
        };
        Ok(SignedPdf::new(self.original, increment.bytes, meta))
    }

    /// Modell A: `assemble()` aus Attributen, Rohsignatur und Kette (Profil und
    /// `include_intermediates` aus dem Plan), dann wie `finish_cms` mit dem Signer der Kette.
    pub fn finish_raw(
        self,
        attrs: &SignedAttrs,
        signature: &RawSignature,
        chain: &CertChain,
    ) -> Result<SignedPdf, FinishError> {
        if let Some(c) = self.meta.expected_cert()
            && c != chain.signer
        {
            return Err(FinishError::Cms(CmsError::UnexpectedSigner));
        }
        let opts =
            AssembleOptions { profile: self.meta.cms_profile, include_intermediates: self.meta.include_intermediates };
        let cms = assemble(attrs, signature, chain, &opts)?;
        let expect = Expect { signer: ExpectedSigner::Certificate(chain.signer.clone()), ..Expect::default() };
        self.finish_cms(&cms, &expect)
    }
}

/// Schreibt eine Datei nur für den Eigentümer lesbar (Unix `0600`), atomar über eine Temp-Datei.
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = crate::signed::temp_path(path);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let result = (|| {
        let mut f = opts.open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_read_in_order() {
        let mut s = Segments {
            segs: vec![Seg::Bytes(b"ab"), Seg::Zeros(3), Seg::Bytes(b""), Seg::Bytes(b"c")],
            idx: 0,
            pos: 0,
        };
        let mut out = Vec::new();
        let mut buf = [0u8; 2];
        loop {
            let n = s.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        assert_eq!(out, b"ab\0\0\0c");
    }

    #[test]
    fn reserve_rules() {
        let mut plan = SignPlan::invisible(OffsetDateTime::UNIX_EPOCH);
        plan.reserve = Reserve::ModelB;
        assert_eq!(compute_reserve(&plan, None).unwrap(), RESERVE_MODEL_B);
        plan.reserve = Reserve::Bytes(5000);
        assert_eq!(compute_reserve(&plan, None).unwrap(), 5000);
        plan.signature_size = Some(9000);
        assert_eq!(compute_reserve(&plan, None).unwrap(), 9000);
        plan.reserve = Reserve::Bytes(RESERVE_MAX + 1);
        assert!(compute_reserve(&plan, None).is_err());
        plan.signature_size = None;
        plan.reserve = Reserve::model_b(Some(32 * 1024));
        assert_eq!(compute_reserve(&plan, None).unwrap(), 32 * 1024);
        assert_eq!(Reserve::model_b(None), Reserve::ModelB);
    }

    #[test]
    fn model_b_reserve_grows_with_large_estimates() {
        // Kette mit RSA-3072-Signer und Zwischenzertifikaten: Schätzung × 2 > 16 KiB?
        let leaf = Certificate::from_der(
            &std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/cades/leaf-rsa3072.crt.der")).unwrap(),
        )
        .unwrap();
        let mut chain = CertChain::new(leaf.clone());
        for _ in 0..8 {
            chain.intermediates.push(leaf.clone());
        }
        let mut plan = SignPlan::invisible(OffsetDateTime::UNIX_EPOCH);
        plan.reserve = Reserve::ModelB;
        let est = estimate_cms_len(
            &chain,
            CmsProfile::PdfasCompat,
            select_algorithm(&leaf.key_type().unwrap()).unwrap(),
            true,
        )
        .unwrap();
        let r = compute_reserve(&plan, Some(&chain)).unwrap();
        assert!(r >= RESERVE_MODEL_B && r >= est * 2, "{r} / {est}");
        assert_eq!(r % 1024, 0);
        // Modell A: exakt + 1 KiB, mindestens 4 KiB
        plan.reserve = Reserve::ModelA;
        let one = CertChain::new(leaf.clone());
        let est_a = estimate_cms_len(
            &one,
            CmsProfile::PdfasCompat,
            select_algorithm(&leaf.key_type().unwrap()).unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(compute_reserve(&plan, Some(&one)).unwrap(), (est_a + MODEL_A_MARGIN).max(RESERVE_MIN_MODEL_A));
    }
}

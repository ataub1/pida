//! Eingangsanalyse und Annahmepolitik (Plan §6.1).
//!
//! | Klasse | Befund-Codes ([`codes`]) |
//! |---|---|
//! | K1 regulär (ggf. Warnung) | `pdf-xfa-hybrid`, `pdf-usage-rights`, `pdf-pdfa-need-appearances`, `pdf-prior-signature-invalid`, `pdf-prior-signature-unverifiable` |
//! | K2 reparierbar (nur ohne Vorsignaturen; Warnung = Zustimmung nötig) | `pdf-xref-repaired`, `pdf-tail-garbage` |
//! | K3 verschlüsselt | `pdf-encrypted` |
//! | K4 eingeschränkt | `pdf-docmdp-p2`, `pdf-docmdp-p3` (keine neue Seite, kein neues sichtbares Feld: [`DocReport::allows_new_visible_field`]), `pdf-field-locked`, `pdf-seed-value`, `pdf-seed-value-incompatible` (Warnung) |
//! | K5 abgelehnt | `pdf-docmdp-p1`, `pdf-permissions`, `pdf-xfa-dynamic`, `pdf-xref-reconstructed`, K2-Befunde bei Vorsignaturen; beim Öffnen ([`reject_open_error`]): `pdf-user-password`, `pdf-public-key`, `pdf-unsupported-security`, `pdf-no-pages`, `pdf-not-a-pdf`, `pdf-unreadable`, `pdf-limit-exceeded`, `pdf-decryption`, `pdf-io` |

use std::collections::BTreeSet;

use pdfover_pdf_cos::{CosError, Dictionary, Object, SigField, SourcePdf, TailKind, XrefHealth, XrefKind};
use pdfover_pdf_incremental::ByteRange;

use crate::PadesError;
use crate::signed::PriorStatus;
use crate::sigs;

/// Stabile Befund-Codes (Fluent-IDs, ADR 0001 Regel 11).
pub mod codes {
    /// DocMDP P=1 (keine Änderungen erlaubt) – K5
    pub const DOCMDP_P1: &str = "pdf-docmdp-p1";
    /// DocMDP P=2 (Formulare/Signaturen) – K4
    pub const DOCMDP_P2: &str = "pdf-docmdp-p2";
    /// DocMDP P=3 (zusätzlich Annotationen) – K4
    pub const DOCMDP_P3: &str = "pdf-docmdp-p3";
    /// Rechte erlauben das Signieren nicht (`error.pdf.sig.12`) – K5
    pub const PERMISSIONS: &str = "pdf-permissions";
    /// verschlüsselt (Standard-Handler ohne Benutzerpasswort) – K3
    pub const ENCRYPTED: &str = "pdf-encrypted";
    /// dynamisches XFA – K5
    pub const XFA_DYNAMIC: &str = "pdf-xfa-dynamic";
    /// AcroForm + XFA – Warnung
    pub const XFA_HYBRID: &str = "pdf-xfa-hybrid";
    /// `/Perms /UR3` – Warnung (Reader-Erweiterungen werden ungültig)
    pub const USAGE_RIGHTS: &str = "pdf-usage-rights";
    /// Querverweis korrigiert – K2 (mit Vorsignaturen K5)
    pub const XREF_REPAIRED: &str = "pdf-xref-repaired";
    /// Querverweis rekonstruiert (kein Increment möglich) – K5
    pub const XREF_RECONSTRUCTED: &str = "pdf-xref-reconstructed";
    /// Fremdinhalt nach `%%EOF` – K2 (mit Vorsignaturen K5)
    pub const TAIL_GARBAGE: &str = "pdf-tail-garbage";
    /// Felder durch `/Lock` einer Signatur gesperrt – K4
    pub const FIELD_LOCKED: &str = "pdf-field-locked";
    /// Seed Values kompatibel – K4
    pub const SEED_VALUE: &str = "pdf-seed-value";
    /// Seed Values nicht erfüllbar – K4, Warnung
    pub const SEED_VALUE_INCOMPATIBLE: &str = "pdf-seed-value-incompatible";
    /// PDF/A mit `/NeedAppearances true` (Eingabe nicht konform) – Warnung
    pub const PDFA_NEED_APPEARANCES: &str = "pdf-pdfa-need-appearances";
    /// Vorsignatur ungültig – Warnung (blockiert nicht, I4)
    pub const PRIOR_SIGNATURE_INVALID: &str = "pdf-prior-signature-invalid";
    /// Vorsignatur nicht prüfbar – Hinweis
    pub const PRIOR_SIGNATURE_UNVERIFIABLE: &str = "pdf-prior-signature-unverifiable";
    /// Benutzerpasswort – K5
    pub const USER_PASSWORD: &str = "pdf-user-password";
    /// Public-Key-Security – K5
    pub const PUBLIC_KEY: &str = "pdf-public-key";
    /// anderer Security-Handler – K5
    pub const UNSUPPORTED_SECURITY: &str = "pdf-unsupported-security";
    /// keine Seiten – K5
    pub const NO_PAGES: &str = "pdf-no-pages";
    /// keine PDF-Datei – K5
    pub const NOT_A_PDF: &str = "pdf-not-a-pdf";
    /// nicht lesbar – K5
    pub const UNREADABLE: &str = "pdf-unreadable";
    /// Grenze überschritten – K5
    pub const LIMIT_EXCEEDED: &str = "pdf-limit-exceeded";
    /// Entschlüsselung fehlgeschlagen – K5
    pub const DECRYPTION: &str = "pdf-decryption";
    /// E/A-Fehler – K5
    pub const IO: &str = "pdf-io";
}

/// Annahmeklasse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AcceptanceClass {
    /// K1 regulär (Tabelle, Stream, Hybrid, Object Streams, linearisiert, bereits signiert;
    /// Leerraum nach `%%EOF`)
    K1Regular,
    /// K2 reparierbar (nur ohne Signaturen, nur nach Zustimmung; erst nach Korpus-Freigabe aktiv)
    K2Repairable,
    /// K3 verschlüsselt (Standard-Handler ohne User-Passwort, Rechte wie PDF-AS)
    K3Encrypted,
    /// K4 eingeschränkt (DocMDP P=2/3, `/Lock`, Seed Values)
    K4Restricted,
    /// K5 abgelehnt (DocMDP P=1, User-Passwort, Public-Key, 0 Seiten, dynamisches XFA …)
    K5Rejected,
}

impl AcceptanceClass {
    /// Schärfe für „schärfster Befund“: K1 < K3 < K4 < K2 < K5 (K2 braucht eine Zustimmung).
    pub fn severity(self) -> u8 {
        match self {
            AcceptanceClass::K1Regular => 0,
            AcceptanceClass::K3Encrypted => 1,
            AcceptanceClass::K4Restricted => 2,
            AcceptanceClass::K2Repairable => 3,
            AcceptanceClass::K5Rejected => 4,
        }
    }

    /// Signieren grundsätzlich möglich (alles außer K5).
    pub fn is_signable(self) -> bool {
        self != AcceptanceClass::K5Rejected
    }
}

/// Einzelbefund mit stabilem Code (Fehlercodes für UI/CLI, Plan §8.9 G2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Finding {
    /// Code, z. B. `pdf-docmdp-p1`, `pdf-user-password`, `pdf-xfa-dynamic`, `pdf-usage-rights`
    pub code: String,
    /// Klasse, die der Befund bedingt
    pub class: AcceptanceClass,
    /// Nur Warnung (Abbruch möglich), z. B. Hybrid-XFA, `/Perms /UR3`, inkompatible Seed Values
    pub warning: bool,
}

impl Finding {
    fn new(code: &str, class: AcceptanceClass, warning: bool) -> Self {
        Finding { code: code.into(), class, warning }
    }
}

/// DocMDP-Zertifizierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DocMdp {
    /// Berechtigung `P` (1, 2, 3)
    pub permission: u8,
}

/// PDF/A-Kennung aus XMP.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PdfaId {
    /// `pdfaid:part`
    pub part: u8,
    /// `pdfaid:conformance`
    pub conformance: Option<String>,
}

/// XFA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum XfaKind {
    /// kein XFA
    None,
    /// AcroForm + `/XFA` (Hinweis)
    Hybrid,
    /// dynamisch (K5)
    Dynamic,
}

/// Vorhandene Signatur (für I4, Feldnamen, belegte QR-Platzhalter). `Debug` ohne Feldnamen und
/// ohne CMS (Zertifikats-Personendaten, ADR 0001 Regel 9).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExistingSignature {
    /// Feldname
    pub field_name: String,
    /// `/SubFilter`
    pub sub_filter: Option<String>,
    /// `/ByteRange`
    pub byte_range: Option<ByteRange>,
    /// `/Contents` (DER ohne Padding, falls trennbar). Nicht serialisiert: das CMS trägt
    /// Zertifikats-Personendaten und gehört nicht in IPC-/Diagnosedaten (Regel 9).
    #[serde(skip)]
    pub contents: Vec<u8>,
    /// `/Location` (Platzhaltername)
    pub location: Option<String>,
    /// ByteRange endet am Ende einer Revision
    pub covers_revision: Option<usize>,
    /// Prüfstatus (`cades::verify_detached`, `AlgorithmPolicy::PriorSignature`; additiv)
    pub status: PriorStatus,
    /// Fehlercode bei ungültiger bzw. nicht prüfbarer Signatur (`cms-…`, `pdf-signature-…`)
    pub status_code: Option<String>,
    /// Signatur deckt die ganze Datei ab (letzte Revision)
    pub covers_whole_file: bool,
}

impl std::fmt::Debug for ExistingSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExistingSignature")
            .field("field_name_len", &self.field_name.chars().count())
            .field("sub_filter", &self.sub_filter)
            .field("byte_range", &self.byte_range)
            .field("contents_len", &self.contents.len())
            .field("location", &self.location)
            .field("covers_revision", &self.covers_revision)
            .field("status", &self.status)
            .field("status_code", &self.status_code)
            .field("covers_whole_file", &self.covers_whole_file)
            .finish()
    }
}

/// Analysebericht.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocReport {
    /// Klasse (schärfster Befund)
    pub class: AcceptanceClass,
    /// Befunde
    pub findings: Vec<Finding>,
    /// Seitenzahl
    pub page_count: usize,
    /// xref-Art der letzten Revision
    pub xref_kind: XrefKind,
    /// Dateiende
    pub tail: TailKind,
    /// verschlüsselt
    pub encrypted: bool,
    /// Rechte erlauben das Signieren
    pub may_sign: bool,
    /// Signaturfelder (signiert und leer). Leere Felder, die eine vorhandene Signatur per `/Lock`
    /// sperrt, tragen das Feldflag `ReadOnly` (Bit 1), damit `placement::find_empty_sig_fields`
    /// sie nicht anbietet (Plan §6.1 K4).
    pub fields: Vec<SigField>,
    /// vorhandene Signaturen
    pub signatures: Vec<ExistingSignature>,
    /// DocMDP
    pub doc_mdp: Option<DocMdp>,
    /// PDF/A-Kennung
    pub pdfa: Option<PdfaId>,
    /// StructTreeRoot mit Kindern (PDF/UA-Ergänzung)
    pub tagged: bool,
    /// XFA
    pub xfa: XfaKind,
    /// `/Perms /UR3`
    pub usage_rights: bool,
    /// `/NeedAppearances true`
    pub need_appearances: bool,
    /// nächste freie Nummer für „PDF-AS Signatur`<n>`“ (nur oberste Ebene, Präfix-Zählung)
    pub next_field_number: u32,
}

impl DocReport {
    /// Bereits signiert (neue Seite nicht erlaubt).
    pub fn is_signed(&self) -> bool {
        !self.signatures.is_empty()
    }

    /// `/Location`-Werte vorhandener Signaturen (belegte QR-Platzhalter).
    pub fn used_placeholder_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.signatures {
            if let Some(l) = &s.location
                && !out.contains(l)
            {
                out.push(l.clone());
            }
        }
        out
    }

    /// Darf ein **neues sichtbares** Signaturfeld angelegt werden? Nicht bei zertifizierten
    /// Dokumenten (DocMDP): die Änderungsanalyse wertet es als Verletzung der Zertifizierung;
    /// dort bleiben vorhandene leere Felder und unsichtbare Signaturen (Plan §6.1 K4).
    pub fn allows_new_visible_field(&self) -> bool {
        self.doc_mdp.is_none()
    }

    /// Hat das Dokument einen Befund mit diesem Code?
    pub fn has_finding(&self, code: &str) -> bool {
        self.findings.iter().any(|f| f.code == code)
    }

    /// Warnungen, die vor dem Signieren eine Bestätigung brauchen (Abbruch möglich).
    pub fn warnings(&self) -> impl Iterator<Item = &Finding> + '_ {
        self.findings.iter().filter(|f| f.warning)
    }
}

/// Default-Präfix neuer Feldnamen (E-07).
pub(crate) const FIELD_PREFIX: &str = "PDF-AS Signatur";

/// [`ExistingSignature::status_code`]: `/V` ist kein lesbares Signaturdictionary.
const SIGNATURE_DICTIONARY_UNREADABLE: &str = "pdf-signature-dictionary";

/// Java `Integer.parseInt` (optionales Vorzeichen, ASCII-Ziffern, 32 Bit).
fn java_parse_int(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `PdfBoxUtils.countSignatures`: oberste Ebene von `/Fields`, `/FT /Sig`, `/T` beginnt mit dem
/// Präfix, Rest (alle Vorkommen des Präfixes entfernt wie Java `String.replace`) als Integer;
/// Maximum + 1.
pub(crate) fn next_field_number(fields: &[SigField], prefix: &str) -> u32 {
    let mut count: i64 = 0;
    for f in fields.iter().filter(|f| f.depth == 0) {
        let Some(name) = f.partial_name.as_deref() else { continue };
        if !name.starts_with(prefix) {
            continue;
        }
        if let Some(n) = java_parse_int(&name.replace(prefix, "")) {
            count = count.max(i64::from(n));
        }
    }
    u32::try_from(count + 1).unwrap_or(u32::MAX)
}

/// Sperr-Dictionaries (`/Action`, `/Fields`) eines **signierten** Felds: `/Lock` am Feld
/// (ISO 32000-1 Tabelle 233) und FieldMDP-`/TransformParams` im Signaturdictionary
/// (`/Reference`, ISO 32000-1 §12.8.2.4) – Validatoren werten die Transformation aus, auch wenn
/// `/Lock` fehlt.
fn lock_dicts<'a>(src: &'a SourcePdf, f: &SigField) -> Vec<&'a Dictionary> {
    let dict_of = |o: &'a Object| src.resolve(o).ok().and_then(|o| o.as_dict().ok());
    let mut out = Vec::new();
    let Some(fd) = src.get_object(f.field_id).ok().and_then(|o| o.as_dict().ok()) else { return out };
    if let Some(lock) = fd.get(b"Lock").ok().and_then(dict_of) {
        out.push(lock);
    }
    let sig = f.value_id.and_then(|id| src.get_object(id).ok()).or_else(|| fd.get(b"V").ok());
    if let Some(Object::Array(refs)) =
        sig.and_then(dict_of).and_then(|s| s.get(b"Reference").ok()).and_then(|o| src.resolve(o).ok())
    {
        for r in refs.iter().filter_map(dict_of) {
            if r.get(b"TransformMethod").ok().and_then(|o| o.as_name().ok()) == Some(b"FieldMDP")
                && let Some(tp) = r.get(b"TransformParams").ok().and_then(dict_of)
            {
                out.push(tp);
            }
        }
    }
    out
}

/// Voll qualifizierte Namen der Felder, die vorhandene Signaturen sperren (`/Action /All |
/// /Include | /Exclude`, `/Fields` voll qualifizierte Namen; ein Name sperrt auch alle
/// Nachfahren). Gemeinsame Grundlage von [`analyze`] und `prepare`.
pub(crate) fn locked_field_names(src: &SourcePdf, fields: &[SigField]) -> BTreeSet<String> {
    let all: Vec<&str> = fields.iter().map(|f| f.qualified_name.as_str()).collect();
    let mut locked = BTreeSet::new();
    for f in fields.iter().filter(|f| f.signed) {
        for lock in lock_dicts(src, f) {
            let action = lock.get(b"Action").ok().and_then(|o| src.resolve(o).ok()).and_then(|o| o.as_name().ok());
            let names: Vec<String> = match lock.get(b"Fields").ok().and_then(|o| src.resolve(o).ok()) {
                Some(Object::Array(a)) => a
                    .iter()
                    .filter_map(|o| src.resolve(o).ok())
                    .filter_map(pdfover_pdf_cos::decode_text_string)
                    .collect(),
                _ => Vec::new(),
            };
            let matches = |q: &str| names.iter().any(|n| q == n || q.starts_with(&format!("{n}.")));
            for q in &all {
                let hit = match action {
                    Some(b"All") => true,
                    Some(b"Include") => matches(q),
                    Some(b"Exclude") => !matches(q),
                    _ => false,
                };
                if hit && *q != f.qualified_name {
                    locked.insert((*q).to_string());
                }
            }
        }
    }
    locked
}

/// Seed Values (ISO 32000-1 Tabelle 234) mit dem Klon erfüllbar? `/Ff`-Bits: 1 Filter,
/// 2 SubFilter, 4 Reasons, 6 AddRevInfo, 7 DigestMethod (Pflicht, wenn gesetzt).
fn seed_value_compatible(src: &SourcePdf, sv: &Dictionary) -> bool {
    let ff = sv.get(b"Ff").ok().and_then(|o| crate::util::int_of(src, o)).unwrap_or(0);
    if ff & 1 != 0
        && let Some(filter) = sv.get(b"Filter").ok().and_then(|o| src.resolve(o).ok()).and_then(|o| o.as_name().ok())
        && filter != b"Adobe.PPKLite"
    {
        return false;
    }
    // Pflicht-Begründungen: `/Reason` kommt aus dem Profil und ist vorab nicht abgleichbar
    if ff & 8 != 0
        && matches!(sv.get(b"Reasons").ok().and_then(|o| src.resolve(o).ok()), Some(Object::Array(a)) if !a.is_empty())
    {
        return false;
    }
    let names = |key: &[u8]| -> Option<Vec<Vec<u8>>> {
        match sv.get(key).ok().and_then(|o| src.resolve(o).ok())? {
            Object::Array(a) => {
                Some(a.iter().filter_map(|o| src.resolve(o).ok()?.as_name().ok().map(<[u8]>::to_vec)).collect())
            }
            _ => None,
        }
    };
    if ff & 2 != 0
        && let Some(sf) = names(b"SubFilter")
        && !sf.iter().any(|n| n == b"ETSI.CAdES.detached")
    {
        return false;
    }
    if ff & 64 != 0
        && let Some(dm) = names(b"DigestMethod")
        && !dm.iter().any(|n| matches!(n.as_slice(), b"SHA256" | b"SHA384" | b"SHA512"))
    {
        return false;
    }
    if ff & 32 != 0
        && matches!(sv.get(b"AddRevInfo").ok().and_then(|o| src.resolve(o).ok()), Some(Object::Boolean(true)))
    {
        return false; // LTV-Daten verlangt (B-LT), nicht im v1-Umfang
    }
    // Zeitstempel verlangt (B-T) bzw. Zertifizierung verlangt
    if let Some(Object::Dictionary(ts)) = sv.get(b"TimeStamp").ok().and_then(|o| src.resolve(o).ok())
        && ts.get(b"Ff").ok().and_then(|o| crate::util::int_of(src, o)).unwrap_or(0) & 1 != 0
    {
        return false;
    }
    if let Some(Object::Dictionary(mdp)) = sv.get(b"MDP").ok().and_then(|o| src.resolve(o).ok())
        && mdp.get(b"P").ok().and_then(|o| crate::util::int_of(src, o)).unwrap_or(0) > 0
    {
        return false;
    }
    if let Some(Object::Dictionary(cert)) = sv.get(b"Cert").ok().and_then(|o| src.resolve(o).ok())
        && cert.get(b"Ff").ok().and_then(|o| crate::util::int_of(src, o)).unwrap_or(0) != 0
    {
        return false; // Zertifikatsvorgaben kann der Klon vorab nicht prüfen
    }
    true
}

/// Liest die Seed-Value-Dictionaries leerer Felder (`/SV`).
fn seed_values(src: &SourcePdf, fields: &[SigField]) -> Vec<bool> {
    fields
        .iter()
        .filter(|f| !f.signed && f.has_seed_value)
        .filter_map(|f| {
            let fd = src.get_object(f.field_id).ok()?.as_dict().ok()?;
            let sv = src.resolve(fd.get(b"SV").ok()?).ok()?.as_dict().ok()?;
            Some(seed_value_compatible(src, sv))
        })
        .collect()
}

/// Revision, an deren Ende `end` liegt (genau am Offset nach `%%EOF` oder dazwischen).
fn revision_ending_at(src: &SourcePdf, end: u64) -> Option<usize> {
    let bytes = src.bytes();
    src.revisions().iter().position(|r| {
        if r.end_offset == end {
            return true;
        }
        // `%%EOF` ohne bzw. mit anderem EOL: zwischen `end` und `end_offset` nur Zeilenende
        end < r.end_offset
            && r.end_offset - end <= 2
            && bytes
                .get(end as usize..r.end_offset as usize)
                .is_some_and(|t| t.iter().all(|b| matches!(b, b'\r' | b'\n')))
            && bytes.get(..end as usize).is_some_and(|h| h.ends_with(b"%%EOF"))
    })
}

/// Analysiert ein geöffnetes Dokument. Fehler beim Öffnen (Passwort, Public-Key, 0 Seiten) bildet
/// der Aufrufer über [`AcceptanceClass::K5Rejected`] ab ([`reject_open_error`]).
///
/// Vorhandene Signaturen werden kryptografisch geprüft (Status für I4); das hasht je Signatur die
/// abgedeckten Bytes.
pub fn analyze(src: &SourcePdf) -> Result<DocReport, PadesError> {
    use AcceptanceClass::*;
    let mut findings: Vec<Finding> = Vec::new();
    let mut fields = src.signature_fields()?;

    // Vorhandene Signaturen
    let mut signatures = Vec::new();
    for f in fields.iter().filter(|f| f.signed) {
        let Some(info) = sigs::read_sig_dict(src, f) else {
            // `/V` ohne lesbares Signaturdictionary: zählt als (ungültige) Vorsignatur, damit
            // `is_signed` mit `prepare` übereinstimmt (keine neue Seite, Plan §6.1)
            signatures.push(ExistingSignature {
                field_name: f.qualified_name.clone(),
                sub_filter: None,
                byte_range: None,
                contents: Vec::new(),
                location: None,
                covers_revision: None,
                status: PriorStatus::Invalid,
                status_code: Some(SIGNATURE_DICTIONARY_UNREADABLE.into()),
                covers_whole_file: false,
            });
            continue;
        };
        let (status, status_code) = sigs::signature_status(src.bytes(), &info);
        let end = info.covered_end();
        let covers_revision = end.and_then(|e| revision_ending_at(src, e));
        let covers_whole_file = covers_revision.is_some_and(|i| i + 1 == src.revisions().len());
        signatures.push(ExistingSignature {
            field_name: f.qualified_name.clone(),
            sub_filter: info.sub_filter.clone(),
            byte_range: info.byte_range(),
            contents: info.der(),
            location: info.location.clone(),
            covers_revision,
            status,
            status_code,
            covers_whole_file,
        });
    }
    let signed = !signatures.is_empty();
    if signatures.iter().any(|s| s.status == PriorStatus::Invalid) {
        findings.push(Finding::new(codes::PRIOR_SIGNATURE_INVALID, K1Regular, true));
    }
    if signatures.iter().any(|s| s.status == PriorStatus::NotVerifiable) {
        findings.push(Finding::new(codes::PRIOR_SIGNATURE_UNVERIFIABLE, K1Regular, false));
    }

    // Querverweis und Dateiende: K2 nur ohne Vorsignaturen und nur nach Zustimmung (Warnung),
    // mit Vorsignaturen K5 (Plan §6.1)
    let k2 = |code: &str| {
        if signed { Finding::new(code, K5Rejected, false) } else { Finding::new(code, K2Repairable, true) }
    };
    match src.xref_health() {
        XrefHealth::Intact => {}
        XrefHealth::Repaired => findings.push(k2(codes::XREF_REPAIRED)),
        XrefHealth::Reconstructed => findings.push(Finding::new(codes::XREF_RECONSTRUCTED, K5Rejected, false)),
    }
    if src.tail().kind == TailKind::Garbage {
        findings.push(k2(codes::TAIL_GARBAGE));
    }

    // Verschlüsselung und Rechte
    let rights = src.rights();
    let encrypted = src.encryption().is_some();
    let may_sign = rights.may_sign();
    if !may_sign {
        findings.push(Finding::new(codes::PERMISSIONS, K5Rejected, false));
    } else if encrypted {
        findings.push(Finding::new(codes::ENCRYPTED, K3Encrypted, false));
    }

    // DocMDP
    let doc_mdp = src.doc_mdp()?.map(|d| DocMdp { permission: d.permission });
    match doc_mdp.map(|d| d.permission) {
        Some(1) => findings.push(Finding::new(codes::DOCMDP_P1, K5Rejected, false)),
        Some(2) => findings.push(Finding::new(codes::DOCMDP_P2, K4Restricted, false)),
        Some(3) => findings.push(Finding::new(codes::DOCMDP_P3, K4Restricted, false)),
        _ => {}
    }

    // /Lock: gesperrte leere Felder als ReadOnly markieren
    let locked = locked_field_names(src, &fields);
    let mut any_locked = false;
    for f in fields.iter_mut().filter(|f| !f.signed && locked.contains(&f.qualified_name)) {
        f.field_flags |= 1;
        any_locked = true;
    }
    if any_locked {
        findings.push(Finding::new(codes::FIELD_LOCKED, K4Restricted, false));
    }
    let sv = seed_values(src, &fields);
    if sv.iter().any(|ok| !ok) {
        findings.push(Finding::new(codes::SEED_VALUE_INCOMPATIBLE, K4Restricted, true));
    } else if !sv.is_empty() {
        findings.push(Finding::new(codes::SEED_VALUE, K4Restricted, false));
    }

    // XFA, Usage Rights, NeedAppearances
    let acro = src.acroform_info()?;
    let xfa = if src.needs_rendering() {
        XfaKind::Dynamic
    } else if acro.as_ref().is_some_and(|a| a.has_xfa) {
        XfaKind::Hybrid
    } else {
        XfaKind::None
    };
    match xfa {
        XfaKind::Dynamic => findings.push(Finding::new(codes::XFA_DYNAMIC, K5Rejected, false)),
        XfaKind::Hybrid => findings.push(Finding::new(codes::XFA_HYBRID, K1Regular, true)),
        XfaKind::None => {}
    }
    let usage_rights = src.has_usage_rights();
    if usage_rights {
        findings.push(Finding::new(codes::USAGE_RIGHTS, K1Regular, true));
    }
    let need_appearances = acro.as_ref().is_some_and(|a| a.need_appearances);
    let pdfa = src.pdfa_identification()?.map(|p| PdfaId { part: p.part, conformance: p.conformance });
    if need_appearances && pdfa.is_some() {
        findings.push(Finding::new(codes::PDFA_NEED_APPEARANCES, K1Regular, true));
    }

    let class = findings.iter().map(|f| f.class).max_by_key(|c| c.severity()).unwrap_or(K1Regular);
    Ok(DocReport {
        class,
        findings,
        page_count: src.page_count(),
        xref_kind: src.xref_kind(),
        tail: src.tail().kind,
        encrypted,
        may_sign,
        next_field_number: next_field_number(&fields, FIELD_PREFIX),
        fields,
        signatures,
        doc_mdp,
        pdfa,
        tagged: src.is_tagged(),
        xfa,
        usage_rights,
        need_appearances,
    })
}

/// Befund (K5) für einen Fehler beim Öffnen mit `SourcePdf::open` (Benutzerpasswort,
/// Public-Key-Security, 0 Seiten, keine PDF-Datei …).
pub fn reject_open_error(e: &CosError) -> Finding {
    let code = match e {
        CosError::PasswordRequired => codes::USER_PASSWORD,
        CosError::UnsupportedSecurityHandler(h) if h.contains("PubSec") => codes::PUBLIC_KEY,
        CosError::UnsupportedSecurityHandler(_) => codes::UNSUPPORTED_SECURITY,
        CosError::NoPages => codes::NO_PAGES,
        CosError::NotAPdf => codes::NOT_A_PDF,
        CosError::Parse(_) | CosError::InvalidStructure(_) | CosError::MissingObject(_) => codes::UNREADABLE,
        CosError::LimitExceeded { .. } => codes::LIMIT_EXCEEDED,
        CosError::Decryption(_) => codes::DECRYPTION,
        CosError::Io(_) => codes::IO,
    };
    Finding::new(code, AcceptanceClass::K5Rejected, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfover_pdf_cos::{Rect, SigWidget};

    fn field(name: &str, depth: usize) -> SigField {
        SigField {
            qualified_name: name.into(),
            partial_name: Some(name.into()),
            field_id: (1, 0),
            depth,
            widgets: vec![SigWidget { id: (1, 0), page_index: Some(0), rect: Rect::default(), annot_flags: 0 }],
            signed: true,
            value_id: None,
            has_lock: false,
            has_seed_value: false,
            field_flags: 0,
        }
    }

    #[test]
    fn field_numbering_like_count_signatures() {
        let f = |n: &str| field(n, 0);
        assert_eq!(next_field_number(&[], FIELD_PREFIX), 1);
        assert_eq!(next_field_number(&[f("PDF-AS Signatur1"), f("PDF-AS Signatur3")], FIELD_PREFIX), 4);
        // kein Integer → ignoriert; fremde Namen ignoriert; tiefere Ebenen ignoriert
        assert_eq!(next_field_number(&[f("PDF-AS Signatur1a"), f("Signature7")], FIELD_PREFIX), 1);
        assert_eq!(next_field_number(&[field("PDF-AS Signatur9", 1)], FIELD_PREFIX), 1);
        // Java parseInt: Vorzeichen, führende Nullen
        assert_eq!(next_field_number(&[f("PDF-AS Signatur+05")], FIELD_PREFIX), 6);
        assert_eq!(next_field_number(&[f("PDF-AS Signatur-5")], FIELD_PREFIX), 1);
        // String.replace entfernt alle Vorkommen
        assert_eq!(next_field_number(&[f("PDF-AS Signatur2PDF-AS Signatur")], FIELD_PREFIX), 3);
        assert_eq!(java_parse_int("2147483648"), None);
        assert_eq!(java_parse_int(" 1"), None);
    }

    #[test]
    fn severity_order() {
        use AcceptanceClass::*;
        let mut v = vec![K5Rejected, K1Regular, K2Repairable, K4Restricted, K3Encrypted];
        v.sort_by_key(|c| c.severity());
        assert_eq!(v, vec![K1Regular, K3Encrypted, K4Restricted, K2Repairable, K5Rejected]);
        assert!(!K5Rejected.is_signable() && K4Restricted.is_signable());
    }

    #[test]
    fn open_errors_map_to_k5() {
        let f = reject_open_error(&CosError::PasswordRequired);
        assert_eq!((f.code.as_str(), f.class), (codes::USER_PASSWORD, AcceptanceClass::K5Rejected));
        assert_eq!(
            reject_open_error(&CosError::UnsupportedSecurityHandler("Adobe.PubSec".into())).code,
            codes::PUBLIC_KEY
        );
        assert_eq!(reject_open_error(&CosError::NoPages).code, codes::NO_PAGES);
    }

    #[test]
    fn existing_signature_debug_is_redacted() {
        let s = ExistingSignature {
            field_name: "Max Mustermann".into(),
            sub_filter: None,
            byte_range: None,
            contents: b"geheim".to_vec(),
            location: None,
            covers_revision: None,
            status: PriorStatus::Valid,
            status_code: None,
            covers_whole_file: false,
        };
        let d = format!("{s:?}");
        assert!(!d.contains("Mustermann") && !d.contains("geheim"), "{d}");
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("contents"), "{json}");
    }
}

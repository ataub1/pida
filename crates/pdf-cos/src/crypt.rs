//! Standard-Security-Handler: Befund und Verschlüsselung neuer Objekte (Plan §6.3).
//!
//! Die Schlüsselableitung (Algorithmen 2/2.A, RC4/AES-128/AES-256, R2–R6) übernimmt lopdf beim
//! Laden mit leerem Benutzerpasswort. `Encryptor` verwendet denselben Dateischlüssel und dieselben
//! Crypt-Filter für neue Objekte; welche Werte ausgenommen sind (`/Contents` des
//! Signaturdictionarys, xref-Streams, `/ID`), entscheidet der Writer.

use std::sync::Arc;

use lopdf::encryption::crypt_filters::{CryptFilter, IdentityCryptFilter, Rc4CryptFilter};
use lopdf::{Dictionary, EncryptionState, Object, ObjectId};

use crate::{CosError, CryptAlgorithm, CryptTarget, EncryptionInfo, Permissions, Result, SecurityHandler};

fn int(d: &Dictionary, key: &[u8]) -> Option<i64> {
    d.get(key).ok().and_then(|o| o.as_i64().ok())
}

fn name<'a>(d: &'a Dictionary, key: &[u8]) -> Option<&'a [u8]> {
    d.get(key).ok().and_then(|o| o.as_name().ok())
}

/// `/P` als Bitfeld (negative Werte im Zweierkomplement, ISO 32000-1 Tabelle 22).
pub(crate) fn permissions_from(p: Option<i64>) -> Permissions {
    match p {
        Some(v) => Permissions((v & 0xFFFF_FFFF) as u32),
        None => Permissions(0),
    }
}

/// Verfahren und Schlüssellänge eines Crypt-Filters (V4/V5).
fn crypt_filter_algorithm(encrypt: &Dictionary, filter_name: Option<&[u8]>, v: i64) -> (CryptAlgorithm, u16) {
    let fname = filter_name.unwrap_or(b"Identity");
    if fname == b"Identity" {
        return (CryptAlgorithm::Identity, 0);
    }
    let cf = encrypt.get(b"CF").ok().and_then(|o| o.as_dict().ok()).and_then(|cf| cf.get(fname).ok()?.as_dict().ok());
    let cfm = cf.and_then(|d| name(d, b"CFM")).unwrap_or(b"None");
    let length = cf.and_then(|d| int(d, b"Length")).map(|l| if l <= 32 { l * 8 } else { l });
    match cfm {
        b"V2" => (CryptAlgorithm::Rc4_128, length.unwrap_or(128).clamp(40, 128) as u16),
        b"AESV2" => (CryptAlgorithm::Aes128, 128),
        b"AESV3" => (CryptAlgorithm::Aes256, 256),
        b"None" if v >= 4 => (CryptAlgorithm::Identity, 0),
        other => (CryptAlgorithm::Unknown(String::from_utf8_lossy(other).into_owned()), length.unwrap_or(0) as u16),
    }
}

/// Befund aus dem Encrypt-Dictionary.
pub(crate) fn encryption_info(encrypt: &Dictionary, encrypt_dict_id: Option<ObjectId>) -> EncryptionInfo {
    let handler = match name(encrypt, b"Filter") {
        Some(b"Standard") => SecurityHandler::Standard,
        Some(b"Adobe.PubSec") => SecurityHandler::PublicKey,
        Some(other) => SecurityHandler::Other(String::from_utf8_lossy(other).into_owned()),
        None => SecurityHandler::Other(String::new()),
    };
    let v = int(encrypt, b"V").unwrap_or(0);
    let r = int(encrypt, b"R").unwrap_or(0);
    let (algorithm, length_bits) = match v {
        0 | 1 => (CryptAlgorithm::Rc4_40, 40),
        2 | 3 => {
            let len = int(encrypt, b"Length").unwrap_or(40).clamp(40, 128);
            (if len > 40 { CryptAlgorithm::Rc4_128 } else { CryptAlgorithm::Rc4_40 }, len as u16)
        }
        4 | 5 => {
            let stm = name(encrypt, b"StmF");
            crypt_filter_algorithm(encrypt, stm, v)
        }
        other => (CryptAlgorithm::Unknown(format!("V{other}")), int(encrypt, b"Length").unwrap_or(0) as u16),
    };
    let length_bits = if v == 5 && length_bits == 0 { 256 } else { length_bits };
    EncryptionInfo {
        handler,
        v: v.clamp(0, 255) as u8,
        r: r.clamp(0, 255) as u8,
        length_bits,
        algorithm,
        permissions: permissions_from(int(encrypt, b"P")),
        encrypt_metadata: encrypt.get(b"EncryptMetadata").ok().and_then(|o| o.as_bool().ok()).unwrap_or(true),
        encrypt_dict_id,
    }
}

/// Befund aus lopdfs Zustand (Rückfall, wenn das Encrypt-Dictionary nicht über die eigene
/// Querverweisanalyse erreichbar ist, z. B. bei rekonstruiertem Querverweis).
pub(crate) fn info_from_state(state: &EncryptionState) -> EncryptionInfo {
    let v = state.version();
    let algorithm = match v {
        0 | 1 => CryptAlgorithm::Rc4_40,
        2 | 3 if state.key_length().unwrap_or(40) > 40 => CryptAlgorithm::Rc4_128,
        2 | 3 => CryptAlgorithm::Rc4_40,
        _ => match select_filter(state, state.default_stream_filter()).method() {
            b"V2" => CryptAlgorithm::Rc4_128,
            b"AESV2" => CryptAlgorithm::Aes128,
            b"AESV3" => CryptAlgorithm::Aes256,
            b"Identity" => CryptAlgorithm::Identity,
            other => CryptAlgorithm::Unknown(String::from_utf8_lossy(other).into_owned()),
        },
    };
    let length_bits = match &algorithm {
        CryptAlgorithm::Rc4_40 => 40,
        CryptAlgorithm::Rc4_128 | CryptAlgorithm::Aes128 => 128,
        CryptAlgorithm::Aes256 => 256,
        _ => 0,
    };
    EncryptionInfo {
        handler: SecurityHandler::Standard,
        v: v.clamp(0, 255) as u8,
        r: state.revision().clamp(0, 255) as u8,
        length_bits,
        algorithm,
        permissions: Permissions((state.permissions().bits() & 0xFFFF_FFFF) as u32),
        encrypt_metadata: state.encrypt_metadata(),
        encrypt_dict_id: state.encrypt_object_id(),
    }
}

/// `true`, wenn das leere Passwort zugleich das **Owner**-Passwort ist (ISO 32000-2 Algorithmus 7
/// bzw. 12). PDFBox prüft beim Laden zuerst das Owner-Passwort und gewährt dann Owner-Rechte
/// (`AccessPermission.getOwnerAccessPermission()`), unabhängig von `/P` – PDF-AS
/// `PdfBoxUtils.checkPDFPermissions` lässt das Signieren dann immer zu.
///
/// `encrypt` ist das rohe Encrypt-Dictionary, `trailer` der Rohtrailer (für `/ID[0]`, R2–R4).
pub(crate) fn empty_password_is_owner(encrypt: &Dictionary, trailer: &Dictionary) -> bool {
    let mut tmp = lopdf::Document::new();
    let id = (1, 0);
    tmp.objects.insert(id, Object::Dictionary(encrypt.clone()));
    tmp.trailer.set("Encrypt", Object::Reference(id));
    if let Ok(v) = trailer.get(b"ID") {
        tmp.trailer.set("ID", v.clone());
    }
    tmp.authenticate_raw_owner_password(b"").is_ok()
}

/// `true`, wenn lopdf diese Kombination aus V/R entschlüsseln kann (ISO 32000-1/-2).
pub(crate) fn standard_supported(info: &EncryptionInfo) -> bool {
    matches!((info.v, info.r), (1, 2) | (2, 3) | (3, 3) | (4, 4) | (5, 5) | (5, 6) | (1, 3) | (2, 2) | (4, 3))
}

/// Interner Aufbau des [`crate::Encryptor`].
#[derive(Clone)]
pub(crate) struct EncryptorInner {
    state: EncryptionState,
    string_filter: Arc<dyn CryptFilter>,
    stream_filter: Arc<dyn CryptFilter>,
    pub(crate) algorithm: CryptAlgorithm,
}

fn select_filter(state: &EncryptionState, filter_name: &[u8]) -> Arc<dyn CryptFilter> {
    if state.version() < 4 {
        return Arc::new(Rc4CryptFilter);
    }
    // ISO 32000-1 Tabelle 20: StmF/StrF fehlen → /Identity.
    if filter_name.is_empty() || filter_name == b"Identity" {
        return Arc::new(IdentityCryptFilter);
    }
    state.crypt_filters().get(filter_name).cloned().unwrap_or_else(|| Arc::new(Rc4CryptFilter))
}

impl EncryptorInner {
    pub(crate) fn new(state: EncryptionState, algorithm: CryptAlgorithm) -> Self {
        let string_filter = select_filter(&state, state.default_string_filter());
        let stream_filter = select_filter(&state, state.default_stream_filter());
        Self { state, string_filter, stream_filter, algorithm }
    }

    fn filter(&self, target: CryptTarget) -> &Arc<dyn CryptFilter> {
        match target {
            CryptTarget::String => &self.string_filter,
            CryptTarget::Stream => &self.stream_filter,
        }
    }

    pub(crate) fn encrypt(&self, id: ObjectId, data: &[u8], target: CryptTarget) -> Result<Vec<u8>> {
        let f = self.filter(target);
        let key =
            f.compute_key(self.state.file_encryption_key(), id).map_err(|e| CosError::Decryption(e.to_string()))?;
        f.encrypt(&key, data).map_err(|e| CosError::Decryption(e.to_string()))
    }

    pub(crate) fn decrypt(&self, id: ObjectId, data: &[u8], target: CryptTarget) -> Result<Vec<u8>> {
        let f = self.filter(target);
        let key =
            f.compute_key(self.state.file_encryption_key(), id).map_err(|e| CosError::Decryption(e.to_string()))?;
        f.decrypt(&key, data).map_err(|e| CosError::Decryption(e.to_string()))
    }
}

/// Entschlüsselt die Strings eines Signaturdictionarys außer `/Contents` (dieser Wert ist nach
/// ISO 32000-2 §7.6.1 nie verschlüsselt). `raw` ist das unentschlüsselte Dictionary aus der Datei.
pub(crate) fn decrypt_signature_dict(state: &EncryptionState, id: ObjectId, raw: &Dictionary) -> Dictionary {
    let mut out = Dictionary::new();
    for (k, v) in raw.iter() {
        let mut v = v.clone();
        if k.as_slice() != b"Contents" {
            let _ = lopdf::encryption::decrypt_object(state, id, &mut v);
        }
        out.set(k.clone(), v);
    }
    out
}

/// `true` für Signatur- bzw. Zeitstempel-Dictionaries (`/ByteRange` + `/Contents`).
pub(crate) fn is_signature_dict(d: &Dictionary) -> bool {
    d.has(b"ByteRange")
        && matches!(d.get(b"Contents"), Ok(Object::String(..)))
        && (d.has_type(b"Sig") || d.has_type(b"DocTimeStamp") || d.has(b"Filter") || !d.has(b"Type"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(pairs: &[(&str, Object)]) -> Dictionary {
        let mut d = Dictionary::new();
        for (k, v) in pairs {
            d.set(k.as_bytes().to_vec(), v.clone());
        }
        d
    }

    #[test]
    fn info_variants() {
        let rc4 = dict(&[
            ("Filter", Object::Name(b"Standard".to_vec())),
            ("V", Object::Integer(2)),
            ("R", Object::Integer(3)),
            ("Length", Object::Integer(128)),
            ("P", Object::Integer(-1084)),
        ]);
        let i = encryption_info(&rc4, Some((12, 0)));
        assert_eq!(i.handler, SecurityHandler::Standard);
        assert_eq!(i.algorithm, CryptAlgorithm::Rc4_128);
        assert_eq!(i.length_bits, 128);
        assert!(i.permissions.can_fill_in_form());
        assert!(!i.permissions.can_modify());
        assert!(!i.permissions.can_modify_annotations());
        assert!(standard_supported(&i));

        let mut cf = Dictionary::new();
        cf.set(
            "StdCF",
            Object::Dictionary(dict(&[("CFM", Object::Name(b"AESV2".to_vec())), ("Length", Object::Integer(16))])),
        );
        let aes = dict(&[
            ("Filter", Object::Name(b"Standard".to_vec())),
            ("V", Object::Integer(4)),
            ("R", Object::Integer(4)),
            ("CF", Object::Dictionary(cf)),
            ("StmF", Object::Name(b"StdCF".to_vec())),
            ("StrF", Object::Name(b"StdCF".to_vec())),
            ("P", Object::Integer(-4)),
            ("EncryptMetadata", Object::Boolean(false)),
        ]);
        let i = encryption_info(&aes, None);
        assert_eq!(i.algorithm, CryptAlgorithm::Aes128);
        assert_eq!(i.length_bits, 128);
        assert!(!i.encrypt_metadata);

        let pubsec = dict(&[("Filter", Object::Name(b"Adobe.PubSec".to_vec())), ("V", Object::Integer(4))]);
        assert_eq!(encryption_info(&pubsec, None).handler, SecurityHandler::PublicKey);
    }

    #[test]
    fn permission_bits() {
        assert_eq!(permissions_from(Some(-4)).0, 0xFFFF_FFFC);
        assert_eq!(permissions_from(Some(4294966212)).0, (-1084i32) as u32);
    }
}

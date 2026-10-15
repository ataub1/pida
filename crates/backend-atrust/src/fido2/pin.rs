//! CTAP2 `authenticatorClientPIN` (FIDO CTAP 2.1 §6.5): PIN/UV-Auth-Protokoll 1 und 2 für den
//! CTAP2-HID-Provider (Welle 3).
//!
//! Ablauf (§6.5.5.7, §6.5.5.8): `getKeyAgreement` → ECDH P-256 mit einem flüchtigen Schlüssel der
//! Plattform → gemeinsames Geheimnis → `pinHashEnc = encrypt(LEFT(SHA-256(PIN), 16))` →
//! `getPinToken` (CTAP 2.0) bzw. `getPinUvAuthTokenUsingPinWithPermissions` mit Recht `ga` und
//! `rpId` (CTAP 2.1, Option `pinUvAuthToken`) → `pinUvAuthToken` entschlüsseln →
//! `pinUvAuthParam = authenticate(token, clientDataHash)` für `authenticatorGetAssertion`.
//!
//! | | Protokoll 1 (§6.5.6) | Protokoll 2 (§6.5.7) |
//! |---|---|---|
//! | KDF | SHA-256(Z) | HKDF-SHA-256(Salt 32 × 0, „CTAP2 HMAC key“ ‖ „CTAP2 AES key“) |
//! | encrypt | AES-256-CBC, IV = 0 | AES-256-CBC, zufälliger IV vorangestellt |
//! | authenticate | HMAC-SHA-256, erste 16 Byte | HMAC-SHA-256 (32 Byte, HMAC-Schlüssel) |
//!
//! Nur RustCrypto (`p256` ECDH, `aes`, `cbc`, `hmac`, `sha2`, `rand_core::OsRng`). Geheimnisse
//! (PIN-Hash, gemeinsames Geheimnis, Token) liegen in [`Zeroizing`] und erscheinen in keinem
//! `Debug` (ADR 0001 Regel 9).
//!
//! Vor dem ersten Versuch fragt der Client `getPINRetries` ab: bei 0 meldet er
//! [`Fido2Error::Locked`], ohne einen Versuch zu verbrauchen. PINs mit weniger als 4 Zeichen oder
//! mehr als 63 Byte (§6.5.1) kann der Schlüssel nie angenommen haben; sie gelten ohne Gerätezugriff
//! als falsch. Die PIN wird nicht NFC-normalisiert (Ziffern-PINs sind davon nicht betroffen).

use std::time::Duration;

use cbc::cipher::block_padding::NoPadding;
use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use hmac::{Hmac, Mac};
use p256::PublicKey;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use rand_core::RngCore;
use sha2::Digest as _;
use zeroize::Zeroizing;

use super::Fido2Error;
use super::ctap::{AuthenticatorInfo, Cbor, CtapHidDevice, HidConnection, cbor_decode, map_status, status};

/// `authenticatorClientPIN`
pub const CTAP2_CLIENT_PIN: u8 = 0x06;

/// Unterkommandos von `authenticatorClientPIN` (§6.5.5).
pub mod sub {
    /// `getPINRetries`
    pub const GET_PIN_RETRIES: u64 = 0x01;
    /// `getKeyAgreement`
    pub const GET_KEY_AGREEMENT: u64 = 0x02;
    /// `getPinToken` (CTAP 2.0)
    pub const GET_PIN_TOKEN: u64 = 0x05;
    /// `getPinUvAuthTokenUsingPinWithPermissions` (CTAP 2.1)
    pub const GET_PIN_UV_AUTH_TOKEN_USING_PIN_WITH_PERMISSIONS: u64 = 0x09;
}

/// Recht `ga` (`authenticatorGetAssertion`) eines `pinUvAuthToken` (§6.5.5.7).
pub const PERMISSION_GET_ASSERTION: u64 = 0x02;

/// Zeitlimit eines `clientPIN`-Kommandos (keine Nutzeraktion am Schlüssel nötig).
const PIN_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

type HmacSha256 = Hmac<sha2::Sha256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// PIN/UV-Auth-Protokoll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinProtocol {
    /// Protokoll 1 (CTAP 2.0)
    One,
    /// Protokoll 2 (CTAP 2.1)
    Two,
}

impl PinProtocol {
    /// Kennung im CBOR (`pinUvAuthProtocol`).
    pub fn id(self) -> u64 {
        match self {
            PinProtocol::One => 1,
            PinProtocol::Two => 2,
        }
    }

    /// Protokoll zur Kennung.
    pub fn from_id(id: u64) -> Option<Self> {
        match id {
            1 => Some(PinProtocol::One),
            2 => Some(PinProtocol::Two),
            _ => None,
        }
    }

    /// Wählt aus `pinUvAuthProtocols` (GetInfo 0x06) wie ein Browser das bevorzugte Protokoll
    /// (2 vor 1). Ohne Angabe gilt Protokoll 1 (CTAP 2.0 kennt das Feld nicht).
    pub fn select(supported: &[u64]) -> Option<Self> {
        if supported.is_empty() {
            return Some(PinProtocol::One);
        }
        [PinProtocol::Two, PinProtocol::One].into_iter().find(|p| supported.contains(&p.id()))
    }
}

/// HMAC-SHA-256.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC akzeptiert jede Schlüssellänge");
    mac.update(message);
    mac.finalize().into_bytes().into()
}

/// HKDF-SHA-256 (RFC 5869) für genau einen Ausgabeblock (32 Byte).
fn hkdf_sha256_32(salt: &[u8], ikm: &[u8], info: &[u8]) -> Zeroizing<[u8; 32]> {
    let prk = Zeroizing::new(hmac_sha256(salt, ikm));
    let mut t = Vec::with_capacity(info.len() + 1);
    t.extend_from_slice(info);
    t.push(1);
    Zeroizing::new(hmac_sha256(prk.as_ref(), &t))
}

fn aes256_cbc_encrypt(key: &[u8; 32], iv: [u8; 16], plaintext: &[u8]) -> Result<Vec<u8>, Fido2Error> {
    if !plaintext.len().is_multiple_of(16) {
        return Err(Fido2Error::Device("PIN-Protokoll: Länge kein Vielfaches von 16".into()));
    }
    let mut buf = plaintext.to_vec();
    let len = buf.len();
    Aes256CbcEnc::new(key.into(), &iv.into())
        .encrypt_padded::<NoPadding>(&mut buf, len)
        .map_err(|_| Fido2Error::Device("PIN-Protokoll: Verschlüsselung fehlgeschlagen".into()))?;
    Ok(buf)
}

fn aes256_cbc_decrypt(key: &[u8; 32], iv: [u8; 16], ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, Fido2Error> {
    if !ciphertext.len().is_multiple_of(16) {
        return Err(Fido2Error::Device("PIN-Protokoll: Länge kein Vielfaches von 16".into()));
    }
    let mut buf = Zeroizing::new(ciphertext.to_vec());
    Aes256CbcDec::new(key.into(), &iv.into())
        .decrypt_padded::<NoPadding>(&mut buf)
        .map_err(|_| Fido2Error::Device("PIN-Protokoll: Entschlüsselung fehlgeschlagen".into()))?;
    Ok(buf)
}

/// Gemeinsames Geheimnis eines PIN-Protokolls (32 Byte bei Protokoll 1, HMAC- ‖ AES-Schlüssel mit
/// 64 Byte bei Protokoll 2). `Debug` ohne Inhalt.
#[derive(Clone)]
pub struct SharedSecret {
    protocol: PinProtocol,
    bytes: Zeroizing<Vec<u8>>,
}

impl std::fmt::Debug for SharedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedSecret").field("protocol", &self.protocol).field("len", &self.bytes.len()).finish()
    }
}

impl SharedSecret {
    /// KDF aus der x-Koordinate `z` des ECDH-Ergebnisses (§6.5.6 bzw. §6.5.7).
    pub fn derive(protocol: PinProtocol, z: &[u8; 32]) -> Self {
        let bytes = match protocol {
            PinProtocol::One => Zeroizing::new(sha2::Sha256::digest(z).to_vec()),
            PinProtocol::Two => {
                let salt = [0u8; 32];
                let mut out = Zeroizing::new(Vec::with_capacity(64));
                out.extend_from_slice(hkdf_sha256_32(&salt, z, b"CTAP2 HMAC key").as_ref());
                out.extend_from_slice(hkdf_sha256_32(&salt, z, b"CTAP2 AES key").as_ref());
                out
            }
        };
        SharedSecret { protocol, bytes }
    }

    /// Protokoll.
    pub fn protocol(&self) -> PinProtocol {
        self.protocol
    }

    fn aes_key(&self) -> &[u8; 32] {
        let k = match self.protocol {
            PinProtocol::One => &self.bytes[..32],
            PinProtocol::Two => &self.bytes[32..64],
        };
        k.try_into().expect("32 Byte")
    }

    /// HMAC-Schlüssel: das ganze Geheimnis (Protokoll 1) bzw. dessen erste Hälfte (Protokoll 2).
    fn hmac_key(&self) -> &[u8] {
        &self.bytes[..32]
    }

    /// `encrypt(sharedSecret, plaintext)`; Protokoll 2 mit zufälligem IV (vorangestellt).
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Fido2Error> {
        let mut iv = [0u8; 16];
        if self.protocol == PinProtocol::Two {
            rand_core::OsRng.fill_bytes(&mut iv);
        }
        self.encrypt_with_iv(iv, plaintext)
    }

    /// Wie [`Self::encrypt`] mit vorgegebenem IV (Protokoll 1 verlangt IV = 0).
    pub fn encrypt_with_iv(&self, iv: [u8; 16], plaintext: &[u8]) -> Result<Vec<u8>, Fido2Error> {
        match self.protocol {
            PinProtocol::One => aes256_cbc_encrypt(self.aes_key(), [0u8; 16], plaintext),
            PinProtocol::Two => {
                let mut out = iv.to_vec();
                out.extend(aes256_cbc_encrypt(self.aes_key(), iv, plaintext)?);
                Ok(out)
            }
        }
    }

    /// `decrypt(sharedSecret, ciphertext)`.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, Fido2Error> {
        match self.protocol {
            PinProtocol::One => aes256_cbc_decrypt(self.aes_key(), [0u8; 16], ciphertext),
            PinProtocol::Two => {
                if ciphertext.len() < 16 {
                    return Err(Fido2Error::Device("PIN-Protokoll: IV fehlt".into()));
                }
                let iv: [u8; 16] = ciphertext[..16].try_into().expect("16");
                aes256_cbc_decrypt(self.aes_key(), iv, &ciphertext[16..])
            }
        }
    }

    /// `authenticate(sharedSecret, message)` (Protokoll 1: 16 Byte, Protokoll 2: 32 Byte).
    pub fn authenticate(&self, message: &[u8]) -> Vec<u8> {
        authenticate(self.protocol, self.hmac_key(), message)
    }
}

/// `authenticate(key, message)` mit einem `pinUvAuthToken` bzw. HMAC-Schlüssel.
pub fn authenticate(protocol: PinProtocol, key: &[u8], message: &[u8]) -> Vec<u8> {
    let mac = hmac_sha256(key, message);
    match protocol {
        PinProtocol::One => mac[..16].to_vec(),
        PinProtocol::Two => mac.to_vec(),
    }
}

/// `LEFT(SHA-256(PIN), 16)`.
pub fn pin_hash(pin: &str) -> Zeroizing<[u8; 16]> {
    let digest = Zeroizing::new(<[u8; 32]>::from(sha2::Sha256::digest(pin.as_bytes())));
    Zeroizing::new(digest[..16].try_into().expect("16"))
}

/// Kann `pin` überhaupt eine gültige PIN sein (§6.5.1: mindestens 4 Unicode-Zeichen, höchstens
/// 63 Byte UTF-8)?
pub fn pin_format_ok(pin: &str) -> bool {
    pin.chars().count() >= 4 && pin.len() <= 63
}

/// Öffentlicher Schlüssel als COSE_Key (§6.5.6 `getPublicKey`): kty EC2, alg -25
/// (ECDH-ES+HKDF-256, für beide Protokolle so vorgeschrieben), crv P-256, x, y.
pub fn cose_key(public: &PublicKey) -> Cbor {
    let point = public.to_encoded_point(false);
    let x = point.x().map(|x| x.to_vec()).unwrap_or_default();
    let y = point.y().map(|y| y.to_vec()).unwrap_or_default();
    Cbor::Map(vec![
        (Cbor::Unsigned(1), Cbor::Unsigned(2)),
        (Cbor::Unsigned(3), Cbor::Negative(24)),
        (Cbor::Negative(0), Cbor::Unsigned(1)),
        (Cbor::Negative(1), Cbor::Bytes(x)),
        (Cbor::Negative(2), Cbor::Bytes(y)),
    ])
}

/// COSE_Key des Authenticators → öffentlicher Schlüssel (Punkt auf der Kurve geprüft).
pub fn parse_cose_key(value: &Cbor) -> Result<PublicKey, Fido2Error> {
    let bad = |m: &str| Fido2Error::Device(format!("PIN-Protokoll: keyAgreement {m}"));
    let Cbor::Map(entries) = value else { return Err(bad("ist keine Map")) };
    let get = |k: &Cbor| entries.iter().find(|(key, _)| key == k).map(|(_, v)| v);
    if get(&Cbor::Unsigned(1)) != Some(&Cbor::Unsigned(2)) || get(&Cbor::Negative(0)) != Some(&Cbor::Unsigned(1)) {
        return Err(bad("ist kein P-256-Schlüssel"));
    }
    let coord = |k: u64| match get(&Cbor::Negative(k)) {
        Some(Cbor::Bytes(b)) if b.len() == 32 => Ok(b.clone()),
        _ => Err(bad("ohne gültige Koordinaten")),
    };
    let mut sec1 = vec![0x04];
    sec1.extend(coord(1)?);
    sec1.extend(coord(2)?);
    PublicKey::from_sec1_bytes(&sec1).map_err(|_| bad("liegt nicht auf der Kurve"))
}

/// Flüchtiger Schlüssel der Plattform für eine PIN-Abfrage. `Debug` ohne Schlüssel.
pub struct PlatformKeyAgreement {
    secret: p256::ecdh::EphemeralSecret,
}

impl std::fmt::Debug for PlatformKeyAgreement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlatformKeyAgreement").finish_non_exhaustive()
    }
}

impl Default for PlatformKeyAgreement {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformKeyAgreement {
    /// Neuer zufälliger Schlüssel (`OsRng`).
    pub fn new() -> Self {
        PlatformKeyAgreement { secret: p256::ecdh::EphemeralSecret::random(&mut rand_core::OsRng) }
    }

    /// Eigener öffentlicher Schlüssel als COSE_Key (Parameter `keyAgreement`).
    pub fn cose_key(&self) -> Cbor {
        cose_key(&self.secret.public_key())
    }

    /// ECDH mit dem Schlüssel des Authenticators und KDF des Protokolls.
    pub fn shared_secret(&self, protocol: PinProtocol, peer: &PublicKey) -> SharedSecret {
        let shared = self.secret.diffie_hellman(peer);
        let z: Zeroizing<[u8; 32]> = Zeroizing::new((*shared.raw_secret_bytes()).into());
        SharedSecret::derive(protocol, &z)
    }
}

/// `pinUvAuthToken` (nur im RAM). `Debug` ohne Inhalt.
#[derive(Clone)]
pub struct PinToken {
    protocol: PinProtocol,
    token: Zeroizing<Vec<u8>>,
}

impl std::fmt::Debug for PinToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinToken").field("protocol", &self.protocol).field("len", &self.token.len()).finish()
    }
}

impl PinToken {
    /// Protokoll, mit dem das Token geholt wurde (`pinUvAuthProtocol` der Assertion).
    pub fn protocol(&self) -> PinProtocol {
        self.protocol
    }

    /// `pinUvAuthParam` für `authenticatorGetAssertion` = `authenticate(token, clientDataHash)`.
    pub fn auth_param(&self, client_data_hash: &[u8; 32]) -> Vec<u8> {
        authenticate(self.protocol, &self.token, client_data_hash)
    }
}

/// Parameter von `authenticatorClientPIN` (kanonisch nach Schlüssel sortiert).
fn client_pin_params(protocol: PinProtocol, sub_command: u64, extra: Vec<(u64, Cbor)>) -> Cbor {
    let mut entries = vec![(1, Cbor::Unsigned(protocol.id())), (2, Cbor::Unsigned(sub_command))];
    entries.extend(extra);
    entries.sort_by_key(|(k, _)| *k);
    Cbor::Map(entries.into_iter().map(|(k, v)| (Cbor::Unsigned(k), v)).collect())
}

/// Fehlerabbildung für `clientPIN` (Status ≠ OK).
fn pin_status(code: u8) -> Fido2Error {
    match code {
        status::PIN_INVALID => Fido2Error::PinInvalid,
        status::PIN_POLICY_VIOLATION => Fido2Error::PinInvalid,
        _ => map_status(code),
    }
}

fn client_pin<C: HidConnection>(device: &mut CtapHidDevice<C>, params: &Cbor) -> Result<Cbor, Fido2Error> {
    let (code, data) = device.cbor(CTAP2_CLIENT_PIN, Some(params), PIN_COMMAND_TIMEOUT, None)?;
    if code != status::OK {
        return Err(pin_status(code));
    }
    if data.is_empty() {
        return Ok(Cbor::Map(Vec::new()));
    }
    cbor_decode(&data)
}

/// `getPINRetries`: verbleibende PIN-Versuche.
pub fn pin_retries<C: HidConnection>(
    device: &mut CtapHidDevice<C>,
    protocol: PinProtocol,
) -> Result<Option<u64>, Fido2Error> {
    let resp = client_pin(device, &client_pin_params(protocol, sub::GET_PIN_RETRIES, Vec::new()))?;
    Ok(match resp.get_uint_key(3) {
        Some(Cbor::Unsigned(n)) => Some(*n),
        _ => None,
    })
}

/// Holt ein `pinUvAuthToken` mit der PIN (§6.5.5.7). Voraussetzung: GetInfo meldet `clientPin`
/// = `true`. Fehler: [`Fido2Error::PinInvalid`] (falsche PIN, der Schlüssel zählt einen Versuch
/// herunter), [`Fido2Error::Locked`] (keine Versuche mehr bzw. drei Fehlversuche in Folge – dann
/// ab- und wieder anstecken), [`Fido2Error::NoUserVerification`] (keine PIN gesetzt).
pub fn get_pin_token<C: HidConnection>(
    device: &mut CtapHidDevice<C>,
    info: &AuthenticatorInfo,
    pin: &str,
    rp_id: &str,
) -> Result<PinToken, Fido2Error> {
    match info.client_pin {
        Some(true) => {}
        Some(false) => return Err(Fido2Error::NoUserVerification),
        None => {
            return Err(Fido2Error::Device("Sicherheitsschlüssel unterstützt keine PIN (clientPin)".into()));
        }
    }
    let protocol = PinProtocol::select(&info.pin_protocols)
        .ok_or_else(|| Fido2Error::Device("kein unterstütztes PIN-Protokoll (1 oder 2)".into()))?;
    if !pin_format_ok(pin) {
        // kann nie die PIN sein; kein Versuch am Schlüssel verbrauchen
        return Err(Fido2Error::PinInvalid);
    }
    // Vorabprüfung, ohne einen Versuch zu verbrauchen. Beantwortet ein Schlüssel `getPINRetries`
    // nicht, entscheidet `getPinToken` selbst (`PIN_BLOCKED`); Transportfehler zeigen sich dann
    // beim nächsten Kommando.
    if let Ok(Some(0)) = pin_retries(device, protocol) {
        return Err(Fido2Error::Locked);
    }
    let ka = client_pin(device, &client_pin_params(protocol, sub::GET_KEY_AGREEMENT, Vec::new()))?;
    let peer = parse_cose_key(ka.get_uint_key(1).ok_or_else(|| cbor_err("keyAgreement fehlt"))?)?;
    let platform = PlatformKeyAgreement::new();
    let shared = platform.shared_secret(protocol, &peer);
    let pin_hash_enc = shared.encrypt(pin_hash(pin).as_ref())?;
    let mut extra = vec![(3, platform.cose_key()), (6, Cbor::Bytes(pin_hash_enc))];
    let sub_command = if info.pin_uv_auth_token == Some(true) {
        extra.push((9, Cbor::Unsigned(PERMISSION_GET_ASSERTION)));
        extra.push((10, Cbor::Text(rp_id.to_owned())));
        sub::GET_PIN_UV_AUTH_TOKEN_USING_PIN_WITH_PERMISSIONS
    } else {
        sub::GET_PIN_TOKEN
    };
    let resp = client_pin(device, &client_pin_params(protocol, sub_command, extra))?;
    let encrypted = match resp.get_uint_key(2) {
        Some(Cbor::Bytes(b)) => b,
        _ => return Err(cbor_err("pinUvAuthToken fehlt")),
    };
    let token = shared.decrypt(encrypted)?;
    let ok_len = match protocol {
        PinProtocol::One => token.len() == 16 || token.len() == 32,
        PinProtocol::Two => token.len() == 32,
    };
    if !ok_len {
        return Err(cbor_err("pinUvAuthToken hat eine ungültige Länge"));
    }
    Ok(PinToken { protocol, token })
}

fn cbor_err(msg: &str) -> Fido2Error {
    Fido2Error::Device(format!("ungültige CBOR-Antwort: {msg}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn primitives_match_published_vectors() {
        // RFC 4231 Testfall 2
        assert_eq!(
            hmac_sha256(b"Jefe", b"what do ya want for nothing?").to_vec(),
            hex("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843")
        );
        // RFC 5869 A.1 (erste 32 Byte der OKM)
        let okm = hkdf_sha256_32(&hex("000102030405060708090a0b0c"), &[0x0b; 22], &hex("f0f1f2f3f4f5f6f7f8f9"));
        assert_eq!(okm.to_vec(), hex("3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"));
        // NIST SP 800-38A F.2.5 (CBC-AES256.Encrypt, erster Block)
        let key: [u8; 32] = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4").try_into().unwrap();
        let iv: [u8; 16] = hex("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
        let ct = aes256_cbc_encrypt(&key, iv, &hex("6bc1bee22e409f96e93d7e117393172a")).unwrap();
        assert_eq!(ct, hex("f58c4c04d6e5f1ba779eabfb5f7bfbd6"));
        assert_eq!(aes256_cbc_decrypt(&key, iv, &ct).unwrap().to_vec(), hex("6bc1bee22e409f96e93d7e117393172a"));
        assert!(aes256_cbc_encrypt(&key, iv, &[0; 15]).is_err());
        assert!(aes256_cbc_decrypt(&key, iv, &[0; 17]).is_err());
        // LEFT(SHA-256("1234"), 16)
        assert_eq!(pin_hash("1234").to_vec(), hex("03ac674216f3e15c761ee1a5e255f067"));
    }

    #[test]
    fn protocol_selection_and_kdf() {
        assert_eq!(PinProtocol::select(&[]), Some(PinProtocol::One));
        assert_eq!(PinProtocol::select(&[1]), Some(PinProtocol::One));
        assert_eq!(PinProtocol::select(&[1, 2]), Some(PinProtocol::Two));
        assert_eq!(PinProtocol::select(&[2]), Some(PinProtocol::Two));
        assert_eq!(PinProtocol::select(&[3]), None);
        assert_eq!(PinProtocol::from_id(2).map(PinProtocol::id), Some(2));
        let z = [7u8; 32];
        let one = SharedSecret::derive(PinProtocol::One, &z);
        assert_eq!(one.bytes.to_vec(), sha2::Sha256::digest(z).to_vec());
        let two = SharedSecret::derive(PinProtocol::Two, &z);
        assert_eq!(two.bytes.len(), 64);
        assert_eq!(&two.bytes[..32], hkdf_sha256_32(&[0; 32], &z, b"CTAP2 HMAC key").as_ref());
        assert_eq!(&two.bytes[32..], hkdf_sha256_32(&[0; 32], &z, b"CTAP2 AES key").as_ref());
        // Protokoll 1: IV 0, 16-Byte-MAC; Protokoll 2: IV vorangestellt, 32-Byte-MAC
        let c1 = one.encrypt(&[1; 16]).unwrap();
        assert_eq!(c1.len(), 16);
        assert_eq!(c1, aes256_cbc_encrypt(one.aes_key(), [0; 16], &[1; 16]).unwrap());
        assert_eq!(one.decrypt(&c1).unwrap().to_vec(), vec![1; 16]);
        assert_eq!(one.authenticate(b"m").len(), 16);
        let c2 = two.encrypt(&[1; 32]).unwrap();
        assert_eq!(c2.len(), 48);
        assert_eq!(two.decrypt(&c2).unwrap().to_vec(), vec![1; 32]);
        assert_ne!(two.encrypt(&[1; 32]).unwrap()[..16], c2[..16], "zufälliger IV");
        assert_eq!(two.authenticate(b"m"), hmac_sha256(&two.bytes[..32], b"m").to_vec());
        assert!(two.decrypt(&[0; 8]).is_err());
        // kein Geheimnis im Debug
        let dbg = format!("{one:?} {two:?}");
        assert!(dbg.contains("len: 32") && dbg.contains("len: 64"), "{dbg}");
    }

    #[test]
    fn cose_key_roundtrip_and_validation() {
        let platform = PlatformKeyAgreement::new();
        let cose = platform.cose_key();
        let enc = super::super::ctap::cbor_encode(&cose);
        // kanonisch: {1: 2, 3: -25, -1: 1, -2: x, -3: y}
        assert_eq!(&enc[..6], &[0xa5, 0x01, 0x02, 0x03, 0x38, 0x18]);
        assert_eq!(&enc[6..8], &[0x20, 0x01]);
        let pk = parse_cose_key(&cose).unwrap();
        assert_eq!(cose_key(&pk), cose);
        // ECDH ist symmetrisch
        let other = PlatformKeyAgreement::new();
        let peer_other = parse_cose_key(&other.cose_key()).unwrap();
        for p in [PinProtocol::One, PinProtocol::Two] {
            let a = platform.shared_secret(p, &peer_other);
            let b = other.shared_secret(p, &pk);
            assert_eq!(a.bytes.to_vec(), b.bytes.to_vec());
        }
        // Punkt nicht auf der Kurve, falsche Kurve, kurze Koordinaten
        let mut off_curve = cose.clone();
        if let Cbor::Map(e) = &mut off_curve {
            e[4].1 = Cbor::Bytes(vec![1; 32]);
        }
        assert!(parse_cose_key(&off_curve).is_err());
        let mut wrong_crv = cose.clone();
        if let Cbor::Map(e) = &mut wrong_crv {
            e[2].1 = Cbor::Unsigned(2);
        }
        assert!(parse_cose_key(&wrong_crv).is_err());
        let mut short = cose;
        if let Cbor::Map(e) = &mut short {
            e[3].1 = Cbor::Bytes(vec![1; 31]);
        }
        assert!(parse_cose_key(&short).is_err());
        assert!(parse_cose_key(&Cbor::Null).is_err());
        assert!(!format!("{platform:?}").is_empty());
    }

    #[test]
    fn pin_format_rules() {
        assert!(pin_format_ok("1234"));
        assert!(pin_format_ok("äöüß"), "4 Unicode-Zeichen, 8 Byte");
        assert!(!pin_format_ok("123"));
        assert!(!pin_format_ok(""));
        assert!(pin_format_ok(&"1".repeat(63)));
        assert!(!pin_format_ok(&"1".repeat(64)));
        let p =
            client_pin_params(PinProtocol::Two, sub::GET_PIN_TOKEN, vec![(6, Cbor::Bytes(vec![])), (3, Cbor::Null)]);
        let keys: Vec<Cbor> = match p {
            Cbor::Map(e) => e.into_iter().map(|(k, _)| k).collect(),
            _ => unreachable!(),
        };
        assert_eq!(keys, [1, 2, 3, 6].map(Cbor::Unsigned));
    }
}

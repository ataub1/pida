//! CTAP2 über CTAPHID, transportunabhängig (FIDO CTAP 2.1, §8.1 USB-HID, §6.2 GetAssertion,
//! §6.4 GetInfo). Der Plattform-Transport ([`HidConnection`]) liefert nur 64-Byte-Reports; alles
//! andere (Kanal-Allokation, Fragmentierung, Keepalive, Abbruch, CBOR) liegt hier und ist gegen
//! einen simulierten Authenticator getestet.
//!
//! PIN (Welle 3): Verlangt ein Authenticator eine PIN, meldet [`get_assertion`]
//! [`Fido2Error::PinRequired`] (Code `fido2-pin-required`); das Backend fragt sie über
//! `SecretKind::Fido2Pin` ab und wiederholt mit [`get_assertion_with_pin`], das über
//! `authenticatorClientPIN` ([`super::pin`], Protokoll 1 und 2) ein `pinUvAuthToken` holt und
//! `pinUvAuthParam` mitsendet. Windows erledigt die PIN über `webauthn.dll` selbst.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sha2::Digest as _;

use super::{AssertionOptions, Fido2Assertion, Fido2Error, UserVerification, client_data_json, credential_id_string};

/// Größe eines HID-Reports (ohne Report-ID).
pub const REPORT_LEN: usize = 64;
/// Broadcast-Kanal.
pub const BROADCAST_CID: u32 = 0xffff_ffff;

const TYPE_INIT: u8 = 0x80;
/// `CTAPHID_PING`
pub const CMD_PING: u8 = 0x01;
/// `CTAPHID_INIT`
pub const CMD_INIT: u8 = 0x06;
/// `CTAPHID_CBOR`
pub const CMD_CBOR: u8 = 0x10;
/// `CTAPHID_CANCEL`
pub const CMD_CANCEL: u8 = 0x11;
/// `CTAPHID_KEEPALIVE`
pub const CMD_KEEPALIVE: u8 = 0x3b;
/// `CTAPHID_ERROR`
pub const CMD_ERROR: u8 = 0x3f;

/// `authenticatorGetAssertion`
pub const CTAP2_GET_ASSERTION: u8 = 0x02;
/// `authenticatorGetInfo`
pub const CTAP2_GET_INFO: u8 = 0x04;

/// Ausgewählte CTAP2-Statuscodes.
pub mod status {
    /// Erfolg
    pub const OK: u8 = 0x00;
    /// ungültige Option
    pub const UNSUPPORTED_OPTION: u8 = 0x2b;
    /// ungültige Option (2.1)
    pub const INVALID_OPTION: u8 = 0x2c;
    /// Abbruch per `CTAPHID_CANCEL`
    pub const KEEPALIVE_CANCEL: u8 = 0x2d;
    /// keine passenden Credentials
    pub const NO_CREDENTIALS: u8 = 0x2e;
    /// Zeitüberschreitung bei der Nutzeraktion
    pub const USER_ACTION_TIMEOUT: u8 = 0x2f;
    /// Nutzerin hat abgelehnt
    pub const OPERATION_DENIED: u8 = 0x27;
    /// PIN falsch
    pub const PIN_INVALID: u8 = 0x31;
    /// PIN gesperrt
    pub const PIN_BLOCKED: u8 = 0x32;
    /// `pinUvAuthParam` ungültig
    pub const PIN_AUTH_INVALID: u8 = 0x33;
    /// drei Fehlversuche in Folge: erst nach erneutem Anstecken wieder möglich
    pub const PIN_AUTH_BLOCKED: u8 = 0x34;
    /// keine PIN gesetzt
    pub const PIN_NOT_SET: u8 = 0x35;
    /// PIN erforderlich (2.1: `PUAT_REQUIRED`)
    pub const PIN_REQUIRED: u8 = 0x36;
    /// PIN verletzt die Richtlinie (z. B. zu kurz)
    pub const PIN_POLICY_VIOLATION: u8 = 0x37;
    /// UV gesperrt
    pub const UV_BLOCKED: u8 = 0x3c;
    /// Aktion abgelaufen
    pub const ACTION_TIMEOUT: u8 = 0x3a;
}

/// Transport: 64-Byte-Reports schreiben/lesen (Report-ID 0 ergänzt der Plattform-Code). Nicht `Send`:
/// unter macOS ist der Kanal an die Run-Loop des öffnenden Threads gebunden.
pub trait HidConnection {
    /// Schreibt einen Report.
    fn write_report(&mut self, report: &[u8; REPORT_LEN]) -> std::io::Result<()>;
    /// Liest einen Report; `Ok(None)` nach `timeout` ohne Daten.
    fn read_report(&mut self, timeout: Duration) -> std::io::Result<Option<[u8; REPORT_LEN]>>;
}

// ------------------------------------------------------------------------------------------------
// CBOR (RFC 8949), nur was CTAP2 braucht: definite Längen, kanonische Kodierung der eigenen Maps.

/// CBOR-Wert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cbor {
    /// Major 0
    Unsigned(u64),
    /// Major 1 (Wert = -1 - n)
    Negative(u64),
    /// Major 2
    Bytes(Vec<u8>),
    /// Major 3
    Text(String),
    /// Major 4
    Array(Vec<Cbor>),
    /// Major 5 (Reihenfolge wie kodiert)
    Map(Vec<(Cbor, Cbor)>),
    /// `true`/`false`
    Bool(bool),
    /// `null`/`undefined`
    Null,
}

impl Cbor {
    /// Ganzzahliger Schlüssel einer Map (`Unsigned`).
    pub fn get_uint_key(&self, key: u64) -> Option<&Cbor> {
        match self {
            Cbor::Map(entries) => entries.iter().find(|(k, _)| *k == Cbor::Unsigned(key)).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Textschlüssel einer Map.
    pub fn get_text_key(&self, key: &str) -> Option<&Cbor> {
        match self {
            Cbor::Map(entries) => entries.iter().find(|(k, _)| matches!(k, Cbor::Text(t) if t == key)).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Cbor::Bytes(b) => Some(b),
            _ => None,
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            Cbor::Text(t) => Some(t),
            _ => None,
        }
    }
}

fn encode_head(out: &mut Vec<u8>, major: u8, value: u64) {
    let m = major << 5;
    match value {
        0..=23 => out.push(m | value as u8),
        24..=0xff => out.extend_from_slice(&[m | 24, value as u8]),
        0x100..=0xffff => {
            out.push(m | 25);
            out.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(m | 26);
            out.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            out.push(m | 27);
            out.extend_from_slice(&value.to_be_bytes());
        }
    }
}

/// Kodiert einen Wert (kürzeste Kopfform).
pub fn cbor_encode(value: &Cbor) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(&mut out, value);
    out
}

fn encode_into(out: &mut Vec<u8>, value: &Cbor) {
    match value {
        Cbor::Unsigned(n) => encode_head(out, 0, *n),
        Cbor::Negative(n) => encode_head(out, 1, *n),
        Cbor::Bytes(b) => {
            encode_head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Cbor::Text(t) => {
            encode_head(out, 3, t.len() as u64);
            out.extend_from_slice(t.as_bytes());
        }
        Cbor::Array(items) => {
            encode_head(out, 4, items.len() as u64);
            for item in items {
                encode_into(out, item);
            }
        }
        Cbor::Map(entries) => {
            encode_head(out, 5, entries.len() as u64);
            for (k, v) in entries {
                encode_into(out, k);
                encode_into(out, v);
            }
        }
        Cbor::Bool(false) => out.push(0xf4),
        Cbor::Bool(true) => out.push(0xf5),
        Cbor::Null => out.push(0xf6),
    }
}

/// Dekodiert genau einen Wert (Rest wird ignoriert); Tiefe ≤ 16.
pub fn cbor_decode(input: &[u8]) -> Result<Cbor, Fido2Error> {
    let mut pos = 0;
    decode_value(input, &mut pos, 0)
}

fn cbor_err(msg: &str) -> Fido2Error {
    Fido2Error::Device(format!("ungültige CBOR-Antwort: {msg}"))
}

fn take<'a>(input: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], Fido2Error> {
    let end = pos.checked_add(n).filter(|e| *e <= input.len()).ok_or_else(|| cbor_err("zu kurz"))?;
    let s = &input[*pos..end];
    *pos = end;
    Ok(s)
}

fn decode_head(input: &[u8], pos: &mut usize) -> Result<(u8, u8, u64), Fido2Error> {
    let first = take(input, pos, 1)?[0];
    let major = first >> 5;
    let info = first & 0x1f;
    let value = match info {
        0..=23 => info as u64,
        24 => take(input, pos, 1)?[0] as u64,
        25 => u16::from_be_bytes(take(input, pos, 2)?.try_into().expect("2")) as u64,
        26 => u32::from_be_bytes(take(input, pos, 4)?.try_into().expect("4")) as u64,
        27 => u64::from_be_bytes(take(input, pos, 8)?.try_into().expect("8")),
        _ => return Err(cbor_err("unbestimmte Länge oder reserviert")),
    };
    Ok((major, info, value))
}

fn decode_value(input: &[u8], pos: &mut usize, depth: usize) -> Result<Cbor, Fido2Error> {
    if depth > 16 {
        return Err(cbor_err("zu tief geschachtelt"));
    }
    let (major, info, value) = decode_head(input, pos)?;
    let len = |v: u64| usize::try_from(v).map_err(|_| cbor_err("Länge"));
    Ok(match major {
        0 => Cbor::Unsigned(value),
        1 => Cbor::Negative(value),
        2 => Cbor::Bytes(take(input, pos, len(value)?)?.to_vec()),
        3 => Cbor::Text(
            String::from_utf8(take(input, pos, len(value)?)?.to_vec()).map_err(|_| cbor_err("Text kein UTF-8"))?,
        ),
        4 => {
            let n = len(value)?;
            let mut items = Vec::with_capacity(n.min(64));
            for _ in 0..n {
                items.push(decode_value(input, pos, depth + 1)?);
            }
            Cbor::Array(items)
        }
        5 => {
            let n = len(value)?;
            let mut entries = Vec::with_capacity(n.min(64));
            for _ in 0..n {
                let k = decode_value(input, pos, depth + 1)?;
                let v = decode_value(input, pos, depth + 1)?;
                entries.push((k, v));
            }
            Cbor::Map(entries)
        }
        6 => decode_value(input, pos, depth + 1)?, // Tag ignorieren
        7 => match info {
            20 => Cbor::Bool(false),
            21 => Cbor::Bool(true),
            22 | 23 => Cbor::Null,
            _ => return Err(cbor_err("Gleitkomma/Simple-Wert nicht unterstützt")),
        },
        _ => unreachable!("major < 8"),
    })
}

// ------------------------------------------------------------------------------------------------
// CTAPHID

/// Geöffneter CTAPHID-Kanal.
#[derive(Debug)]
pub struct CtapHidDevice<C: HidConnection> {
    conn: C,
    cid: u32,
    /// Fähigkeiten aus `CTAPHID_INIT` (Bit 2 = CBOR).
    pub capabilities: u8,
}

/// Erzeugt eine zufällige 8-Byte-Nonce (ohne eigene Zufallsquelle: `RandomState` + Zeit).
pub fn random_nonce() -> [u8; 8] {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(Instant::now().elapsed().as_nanos());
    if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        h.write_u128(d.as_nanos());
    }
    h.finish().to_be_bytes()
}

impl<C: HidConnection> CtapHidDevice<C> {
    /// `CTAPHID_INIT` auf dem Broadcast-Kanal (§11.2.9.1.3).
    pub fn init(conn: C, nonce: [u8; 8]) -> Result<Self, Fido2Error> {
        let mut dev = CtapHidDevice { conn, cid: BROADCAST_CID, capabilities: 0 };
        dev.send(CMD_INIT, &nonce)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let (cmd, payload) = dev.receive(deadline, None)?;
            if cmd == CMD_INIT && payload.len() >= 17 && payload[..8] == nonce {
                dev.cid = u32::from_be_bytes(payload[8..12].try_into().expect("4"));
                dev.capabilities = payload[16];
                if dev.cid == BROADCAST_CID || dev.cid == 0 {
                    return Err(Fido2Error::Device("ungültiger CTAPHID-Kanal".into()));
                }
                return Ok(dev);
            }
        }
    }

    /// Kanal-ID.
    pub fn cid(&self) -> u32 {
        self.cid
    }

    /// Unterstützt CBOR (CTAP2).
    pub fn supports_cbor(&self) -> bool {
        self.capabilities & 0x04 != 0
    }

    fn io(e: std::io::Error) -> Fido2Error {
        Fido2Error::Device(format!("HID: {e}"))
    }

    /// Sendet eine Nachricht, fragmentiert in Init- und Fortsetzungspakete.
    pub fn send(&mut self, cmd: u8, payload: &[u8]) -> Result<(), Fido2Error> {
        if payload.len() > 57 + 128 * 59 {
            return Err(Fido2Error::Device("Nachricht zu groß".into()));
        }
        let mut report = [0u8; REPORT_LEN];
        report[..4].copy_from_slice(&self.cid.to_be_bytes());
        report[4] = TYPE_INIT | cmd;
        report[5..7].copy_from_slice(&(payload.len() as u16).to_be_bytes());
        let first = payload.len().min(57);
        report[7..7 + first].copy_from_slice(&payload[..first]);
        self.conn.write_report(&report).map_err(Self::io)?;
        for (seq, chunk) in payload[first..].chunks(59).enumerate() {
            let mut report = [0u8; REPORT_LEN];
            report[..4].copy_from_slice(&self.cid.to_be_bytes());
            report[4] = seq as u8;
            report[5..5 + chunk.len()].copy_from_slice(chunk);
            self.conn.write_report(&report).map_err(Self::io)?;
        }
        Ok(())
    }

    /// Empfängt die nächste Nachricht für diesen Kanal (Keepalive wird übersprungen). Wird
    /// `cancel` gesetzt, sendet die Funktion einmal `CTAPHID_CANCEL` und wartet auf die Antwort.
    pub fn receive(&mut self, deadline: Instant, cancel: Option<&AtomicBool>) -> Result<(u8, Vec<u8>), Fido2Error> {
        let mut cancel_sent = false;
        loop {
            if let Some(flag) = cancel
                && !cancel_sent
                && flag.load(Ordering::SeqCst)
            {
                self.send(CMD_CANCEL, &[])?;
                cancel_sent = true;
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(Fido2Error::Device("Zeitüberschreitung am Sicherheitsschlüssel".into()));
            }
            let slice = (deadline - now).min(Duration::from_millis(100));
            let Some(report) = self.conn.read_report(slice).map_err(Self::io)? else { continue };
            let cid = u32::from_be_bytes(report[..4].try_into().expect("4"));
            if cid != self.cid || report[4] & TYPE_INIT == 0 {
                continue; // fremder Kanal oder verwaistes Fortsetzungspaket
            }
            let cmd = report[4] & !TYPE_INIT;
            let total = u16::from_be_bytes([report[5], report[6]]) as usize;
            let mut payload = Vec::with_capacity(total);
            payload.extend_from_slice(&report[7..7 + total.min(57)]);
            let mut seq = 0u8;
            while payload.len() < total {
                let now = Instant::now();
                if now >= deadline {
                    return Err(Fido2Error::Device("Zeitüberschreitung beim Lesen".into()));
                }
                let Some(cont) =
                    self.conn.read_report((deadline - now).min(Duration::from_millis(500))).map_err(Self::io)?
                else {
                    continue;
                };
                let ccid = u32::from_be_bytes(cont[..4].try_into().expect("4"));
                if ccid != self.cid {
                    continue;
                }
                if cont[4] & TYPE_INIT != 0 || cont[4] != seq {
                    return Err(Fido2Error::Device("CTAPHID-Fortsetzung außer der Reihe".into()));
                }
                let need = (total - payload.len()).min(59);
                payload.extend_from_slice(&cont[5..5 + need]);
                seq = seq.wrapping_add(1);
            }
            match cmd {
                CMD_KEEPALIVE => continue,
                CMD_ERROR => {
                    return Err(Fido2Error::Device(format!(
                        "CTAPHID-Fehler 0x{:02x}",
                        payload.first().copied().unwrap_or(0)
                    )));
                }
                _ => return Ok((cmd, payload)),
            }
        }
    }

    /// CTAP2-Kommando über `CTAPHID_CBOR`; liefert den Status und die CBOR-Nutzdaten.
    pub fn cbor(
        &mut self,
        command: u8,
        params: Option<&Cbor>,
        timeout: Duration,
        cancel: Option<&AtomicBool>,
    ) -> Result<(u8, Vec<u8>), Fido2Error> {
        let mut payload = vec![command];
        if let Some(p) = params {
            payload.extend_from_slice(&cbor_encode(p));
        }
        self.send(CMD_CBOR, &payload)?;
        let deadline = Instant::now() + timeout;
        loop {
            let (cmd, response) = self.receive(deadline, cancel)?;
            if cmd != CMD_CBOR {
                continue;
            }
            let status = *response.first().ok_or_else(|| Fido2Error::Device("leere CTAP2-Antwort".into()))?;
            return Ok((status, response[1..].to_vec()));
        }
    }

    /// `authenticatorGetInfo`.
    pub fn get_info(&mut self) -> Result<AuthenticatorInfo, Fido2Error> {
        let (status, data) = self.cbor(CTAP2_GET_INFO, None, Duration::from_secs(5), None)?;
        if status != status::OK {
            return Err(Fido2Error::Device(format!("GetInfo: CTAP2-Status 0x{status:02x}")));
        }
        AuthenticatorInfo::from_cbor(&cbor_decode(&data)?)
    }
}

/// Relevante Teile von `authenticatorGetInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthenticatorInfo {
    /// `versions` (`FIDO_2_0`, `FIDO_2_1`, `U2F_V2` …)
    pub versions: Vec<String>,
    /// Option `uv` (eingebaute Nutzerverifikation): `Some(true)` = eingerichtet
    pub uv: Option<bool>,
    /// Option `clientPin`: `Some(true)` = PIN gesetzt, `Some(false)` = unterstützt, nicht gesetzt
    pub client_pin: Option<bool>,
    /// `pinUvAuthProtocols` (0x06; leer bei CTAP 2.0 = Protokoll 1). Additiv (Welle 3).
    pub pin_protocols: Vec<u64>,
    /// Option `pinUvAuthToken` (CTAP 2.1: `getPinUvAuthTokenUsingPinWithPermissions`). Additiv (Welle 3).
    pub pin_uv_auth_token: Option<bool>,
}

impl AuthenticatorInfo {
    fn from_cbor(value: &Cbor) -> Result<Self, Fido2Error> {
        let versions = match value.get_uint_key(1) {
            Some(Cbor::Array(items)) => items.iter().filter_map(|v| v.as_text().map(str::to_owned)).collect(),
            _ => return Err(cbor_err("GetInfo ohne versions")),
        };
        let option = |name: &str| match value.get_uint_key(4).and_then(|o| o.get_text_key(name)) {
            Some(Cbor::Bool(b)) => Some(*b),
            _ => None,
        };
        let pin_protocols = match value.get_uint_key(6) {
            Some(Cbor::Array(items)) => items
                .iter()
                .filter_map(|v| match v {
                    Cbor::Unsigned(n) => Some(*n),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(AuthenticatorInfo {
            versions,
            uv: option("uv"),
            client_pin: option("clientPin"),
            pin_protocols,
            pin_uv_auth_token: option("pinUvAuthToken"),
        })
    }
}

fn text(s: &str) -> Cbor {
    Cbor::Text(s.to_owned())
}

/// Baut die Parameter von `authenticatorGetAssertion` (kanonisch sortierte Schlüssel).
pub fn get_assertion_params(options: &AssertionOptions, client_data_hash: &[u8; 32], uv: bool) -> Cbor {
    get_assertion_params_with_pin(options, client_data_hash, uv, None)
}

/// Wie [`get_assertion_params`], mit `pinUvAuthParam` (0x06) und `pinUvAuthProtocol` (0x07) aus
/// einem PIN-Token. Mit PIN entfällt die Option `uv` (CTAP 2.1 §6.2.2). Additiv (Welle 3).
pub fn get_assertion_params_with_pin(
    options: &AssertionOptions,
    client_data_hash: &[u8; 32],
    uv: bool,
    pin_auth: Option<(Vec<u8>, u64)>,
) -> Cbor {
    let mut entries =
        vec![(Cbor::Unsigned(1), text(&options.rp_id)), (Cbor::Unsigned(2), Cbor::Bytes(client_data_hash.to_vec()))];
    if !options.allow_credentials.is_empty() {
        let list = options
            .allow_credentials
            .iter()
            .map(|c| Cbor::Map(vec![(text("id"), Cbor::Bytes(c.id.clone())), (text("type"), text("public-key"))]))
            .collect();
        entries.push((Cbor::Unsigned(3), Cbor::Array(list)));
    }
    if uv && pin_auth.is_none() {
        entries.push((Cbor::Unsigned(5), Cbor::Map(vec![(text("uv"), Cbor::Bool(true))])));
    }
    if let Some((param, protocol)) = pin_auth {
        entries.push((Cbor::Unsigned(6), Cbor::Bytes(param)));
        entries.push((Cbor::Unsigned(7), Cbor::Unsigned(protocol)));
    }
    Cbor::Map(entries)
}

/// Fehlerabbildung der CTAP2-Statuscodes (Codes aus dem Katalog: `fido2-no-credential`,
/// `fido2-timeout`, `fido2-locked`, `fido2-pin-auth-blocked`, `fido2-no-user-verification`; Welle 3).
pub(crate) fn map_status(code: u8) -> Fido2Error {
    match code {
        status::KEEPALIVE_CANCEL | status::OPERATION_DENIED => Fido2Error::Cancelled,
        status::NO_CREDENTIALS => Fido2Error::NoCredential,
        status::USER_ACTION_TIMEOUT | status::ACTION_TIMEOUT => Fido2Error::Timeout,
        // Code `fido2-pin-required`; ohne PIN-Unterstützung des Providers zeigt das Backend den
        // Hinweis „bitte App oder SMS verwenden“ (Welle 2)
        status::PIN_REQUIRED => Fido2Error::PinRequired,
        status::PIN_INVALID => Fido2Error::PinInvalid,
        status::PIN_BLOCKED | status::UV_BLOCKED => Fido2Error::Locked,
        // vorübergehend: ab- und wieder anstecken genügt (nicht zurücksetzen!)
        status::PIN_AUTH_BLOCKED => Fido2Error::PinAuthBlocked,
        status::PIN_NOT_SET => Fido2Error::NoUserVerification,
        status::PIN_AUTH_INVALID => Fido2Error::Device("PIN-Prüfung am Sicherheitsschlüssel fehlgeschlagen".into()),
        other => Fido2Error::Device(format!("CTAP2-Status 0x{other:02x}")),
    }
}

/// Holt eine Assertion über einen geöffneten Kanal ohne PIN (siehe [`get_assertion_with_pin`]).
pub fn get_assertion<C: HidConnection>(
    device: &mut CtapHidDevice<C>,
    options: &AssertionOptions,
    origin: &str,
    cancel: &AtomicBool,
) -> Result<Fido2Assertion, Fido2Error> {
    get_assertion_with_pin(device, options, origin, cancel, None)
}

/// Holt eine Assertion über einen geöffneten Kanal: GetInfo → UV-Entscheidung → (mit PIN:
/// `pinUvAuthToken`, [`super::pin::get_pin_token`]) → GetAssertion. Additiv (Welle 3).
///
/// Ohne PIN, wie ein Browser: `required`/`preferred` → eingebaute UV (`uv`-Option), wenn
/// eingerichtet; sonst bei gesetzter PIN [`Fido2Error::PinRequired`] (das Backend fragt die PIN ab
/// und ruft erneut auf); `required` ohne PIN und Biometrie → [`Fido2Error::NoUserVerification`];
/// `preferred` ohne beides bzw. `discouraged` → nur Anwesenheit. Mit PIN wird immer das Token
/// verwendet (die PIN wird nur nach `PinRequired` übergeben, auch wenn der Schlüssel sie z. B.
/// wegen `alwaysUv` im GetAssertion selbst verlangt hat).
pub fn get_assertion_with_pin<C: HidConnection>(
    device: &mut CtapHidDevice<C>,
    options: &AssertionOptions,
    origin: &str,
    cancel: &AtomicBool,
    pin: Option<&str>,
) -> Result<Fido2Assertion, Fido2Error> {
    if !device.supports_cbor() {
        return Err(Fido2Error::U2fOnly);
    }
    let info = device.get_info()?;
    let builtin_uv = info.uv == Some(true);
    let pin_set = info.client_pin == Some(true);
    let uv = match (pin, options.user_verification) {
        (Some(_), _) => true,
        (None, UserVerification::Required | UserVerification::Preferred) if builtin_uv => true,
        (None, UserVerification::Required | UserVerification::Preferred) if pin_set => {
            return Err(map_status(status::PIN_REQUIRED));
        }
        (None, UserVerification::Required) => return Err(Fido2Error::NoUserVerification),
        (None, UserVerification::Preferred | UserVerification::Discouraged) => false,
    };
    let client_data = client_data_json(&options.challenge, origin);
    let hash: [u8; 32] = sha2::Sha256::digest(&client_data).into();
    let pin_auth = match pin {
        Some(p) => {
            let token = super::pin::get_pin_token(device, &info, p, &options.rp_id)?;
            Some((token.auth_param(&hash), token.protocol().id()))
        }
        None => None,
    };
    let params = get_assertion_params_with_pin(options, &hash, uv, pin_auth);
    let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(120_000).clamp(10_000, 600_000) as u64);
    let (code, data) = device.cbor(CTAP2_GET_ASSERTION, Some(&params), timeout, Some(cancel))?;
    if code != status::OK {
        return Err(map_status(code));
    }
    let response = cbor_decode(&data)?;
    let authenticator_data =
        response.get_uint_key(2).and_then(Cbor::as_bytes).ok_or_else(|| cbor_err("authData fehlt"))?.to_vec();
    let signature =
        response.get_uint_key(3).and_then(Cbor::as_bytes).ok_or_else(|| cbor_err("signature fehlt"))?.to_vec();
    let raw_id = match response.get_uint_key(1).and_then(|c| c.get_text_key("id")).and_then(Cbor::as_bytes) {
        Some(id) => id.to_vec(),
        // CTAP2: bei genau einem Eintrag in allowList darf `credential` fehlen
        None if options.allow_credentials.len() == 1 => options.allow_credentials[0].id.clone(),
        None => return Err(cbor_err("credential fehlt")),
    };
    let user_handle =
        response.get_uint_key(4).and_then(|u| u.get_text_key("id")).and_then(Cbor::as_bytes).map(<[u8]>::to_vec);
    Ok(Fido2Assertion {
        id: credential_id_string(&raw_id),
        raw_id,
        kind: "public-key".into(),
        authenticator_data,
        client_data_json: client_data,
        signature,
        user_handle,
    })
}

#[cfg(test)]
pub(crate) mod sim {
    //! Simulierter CTAP2-Authenticator für Tests: feste Assertion-Antworten (keine echte
    //! Signatur), aber ein vollständiges `authenticatorClientPIN` (Protokoll 1 und 2, echtes
    //! ECDH/AES/HMAC aus Sicht des Authenticators, PIN-Zähler, Sperre nach drei Fehlversuchen in
    //! Folge bzw. bei 0 Versuchen, Prüfung von `pinUvAuthParam` und Rechten).
    use std::collections::VecDeque;

    use super::super::pin::{self, PinProtocol, SharedSecret};
    use super::*;

    /// `CTAP2_ERR_INVALID_PARAMETER`
    const INVALID_PARAMETER: u8 = 0x02;
    /// `CTAP2_ERR_MISSING_PARAMETER`
    const MISSING_PARAMETER: u8 = 0x14;
    /// `CTAP2_ERR_INVALID_SUBCOMMAND`
    const INVALID_SUBCOMMAND: u8 = 0x3e;
    /// `CTAP2_ERR_UNAUTHORIZED_PERMISSION`
    const UNAUTHORIZED_PERMISSION: u8 = 0x40;

    /// Verhalten des Simulators.
    #[derive(Debug, Clone)]
    pub struct SimConfig {
        pub uv: Option<bool>,
        pub client_pin: Option<bool>,
        pub status: u8,
        pub keepalives: usize,
        pub omit_credential: bool,
        pub user_handle: Option<Vec<u8>>,
        pub cbor_capable: bool,
        pub noise_other_channel: bool,
        /// gesetzte PIN (GetInfo `clientPin` steuert weiterhin `client_pin`)
        pub pin: Option<String>,
        /// verbleibende PIN-Versuche
        pub pin_retries: u64,
        /// `pinUvAuthProtocols` (leer = CTAP 2.0, nur Protokoll 1)
        pub pin_protocols: Vec<u64>,
        /// Option `pinUvAuthToken` (CTAP 2.1)
        pub pin_uv_auth_token: Option<bool>,
        /// `alwaysUv`: GetAssertion ohne `pinUvAuthParam` → `PIN_REQUIRED`
        pub always_uv: bool,
    }

    impl SimConfig {
        /// Schlüssel mit gesetzter PIN und den angegebenen PIN-Protokollen.
        pub fn with_pin(pin: &str, protocols: &[u64]) -> Self {
            SimConfig {
                uv: None,
                client_pin: Some(true),
                pin: Some(pin.to_owned()),
                pin_protocols: protocols.to_vec(),
                ..SimConfig::default()
            }
        }
    }

    impl Default for SimConfig {
        fn default() -> Self {
            Self {
                uv: Some(false),
                client_pin: Some(false),
                status: 0,
                keepalives: 2,
                omit_credential: false,
                user_handle: Some(vec![7, 7]),
                cbor_capable: true,
                noise_other_channel: true,
                pin: None,
                pin_retries: 8,
                pin_protocols: Vec::new(),
                pin_uv_auth_token: None,
                always_uv: false,
            }
        }
    }

    /// Simulator mit Protokoll der empfangenen Nachrichten.
    #[derive(Debug)]
    pub struct SimDevice {
        pub config: SimConfig,
        pub cid: u32,
        inbox: Vec<u8>,
        expect: Option<(u32, u8, usize, u8)>,
        pub outbox: VecDeque<[u8; REPORT_LEN]>,
        pub received: Vec<(u8, Vec<u8>)>,
        pub cancel_received: bool,
        pending_assertion: Option<Vec<u8>>,
        /// Schlüssel für `getKeyAgreement` (wird nach einer falschen PIN neu erzeugt)
        key_agreement: p256::SecretKey,
        /// Fehlversuche in Folge (3 → `PIN_AUTH_BLOCKED` bis zum „Neustecken“)
        pub consecutive_fails: u8,
        /// ausgegebenes Token: (Protokoll, Token, Rechte, RP-ID)
        token: Option<(PinProtocol, Vec<u8>, u64, Option<String>)>,
        /// letzte Assertion mit gesetztem UV-Flag
        pub last_uv: bool,
    }

    impl SimDevice {
        pub fn new(config: SimConfig) -> Self {
            Self {
                config,
                cid: 0x0102_0304,
                inbox: Vec::new(),
                expect: None,
                outbox: VecDeque::new(),
                received: Vec::new(),
                cancel_received: false,
                pending_assertion: None,
                key_agreement: p256::SecretKey::random(&mut rand_core::OsRng),
                consecutive_fails: 0,
                token: None,
                last_uv: false,
            }
        }

        /// Schlüssel ab- und wieder angesteckt (setzt `PIN_AUTH_BLOCKED` zurück).
        pub fn power_cycle(&mut self) {
            self.consecutive_fails = 0;
            self.token = None;
            self.key_agreement = p256::SecretKey::random(&mut rand_core::OsRng);
        }

        fn protocol_of(&self, params: &Cbor) -> Result<PinProtocol, u8> {
            let id = match params.get_uint_key(1) {
                Some(Cbor::Unsigned(n)) => *n,
                _ => return Err(MISSING_PARAMETER),
            };
            let supported = if self.config.pin_protocols.is_empty() { &[1][..] } else { &self.config.pin_protocols };
            if !supported.contains(&id) {
                return Err(INVALID_PARAMETER);
            }
            PinProtocol::from_id(id).ok_or(INVALID_PARAMETER)
        }

        /// `authenticatorClientPIN` aus Sicht des Authenticators (CTAP 2.1 §6.5.5).
        fn client_pin(&mut self, params: &Cbor) -> Result<Cbor, u8> {
            if self.config.client_pin.is_none() {
                return Err(INVALID_SUBCOMMAND);
            }
            let protocol = self.protocol_of(params)?;
            let sub_command = match params.get_uint_key(2) {
                Some(Cbor::Unsigned(n)) => *n,
                _ => return Err(MISSING_PARAMETER),
            };
            match sub_command {
                pin::sub::GET_PIN_RETRIES => {
                    Ok(Cbor::Map(vec![(Cbor::Unsigned(3), Cbor::Unsigned(self.config.pin_retries))]))
                }
                pin::sub::GET_KEY_AGREEMENT => {
                    Ok(Cbor::Map(vec![(Cbor::Unsigned(1), pin::cose_key(&self.key_agreement.public_key()))]))
                }
                pin::sub::GET_PIN_TOKEN | pin::sub::GET_PIN_UV_AUTH_TOKEN_USING_PIN_WITH_PERMISSIONS => {
                    let with_permissions = sub_command == pin::sub::GET_PIN_UV_AUTH_TOKEN_USING_PIN_WITH_PERMISSIONS;
                    if with_permissions && self.config.pin_uv_auth_token != Some(true) {
                        return Err(INVALID_SUBCOMMAND);
                    }
                    let (permissions, rp_id) = if with_permissions {
                        let perm = match params.get_uint_key(9) {
                            Some(Cbor::Unsigned(n)) if *n != 0 => *n,
                            _ => return Err(MISSING_PARAMETER),
                        };
                        (perm, params.get_uint_key(10).and_then(Cbor::as_text).map(str::to_owned))
                    } else {
                        // CTAP 2.0-Token: Rechte mc|ga ohne RP-Bindung
                        (0x03, None)
                    };
                    let Some(pin_value) = self.config.pin.clone() else { return Err(status::PIN_NOT_SET) };
                    if self.config.pin_retries == 0 {
                        return Err(status::PIN_BLOCKED);
                    }
                    if self.consecutive_fails >= 3 {
                        return Err(status::PIN_AUTH_BLOCKED);
                    }
                    let peer = pin::parse_cose_key(params.get_uint_key(3).ok_or(MISSING_PARAMETER)?)
                        .map_err(|_| INVALID_PARAMETER)?;
                    let shared = p256::ecdh::diffie_hellman(self.key_agreement.to_nonzero_scalar(), peer.as_affine());
                    let z: [u8; 32] = (*shared.raw_secret_bytes()).into();
                    let secret = SharedSecret::derive(protocol, &z);
                    let enc = params.get_uint_key(6).and_then(Cbor::as_bytes).ok_or(MISSING_PARAMETER)?;
                    let hash = secret.decrypt(enc).map_err(|_| INVALID_PARAMETER)?;
                    self.config.pin_retries -= 1;
                    if hash.as_slice() != pin::pin_hash(&pin_value).as_ref() {
                        self.consecutive_fails += 1;
                        self.key_agreement = p256::SecretKey::random(&mut rand_core::OsRng);
                        return Err(if self.config.pin_retries == 0 {
                            status::PIN_BLOCKED
                        } else if self.consecutive_fails >= 3 {
                            status::PIN_AUTH_BLOCKED
                        } else {
                            status::PIN_INVALID
                        });
                    }
                    self.config.pin_retries = 8;
                    self.consecutive_fails = 0;
                    let mut token = vec![0u8; 32];
                    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut token);
                    let encrypted = secret.encrypt(&token).map_err(|_| INVALID_PARAMETER)?;
                    self.token = Some((protocol, token, permissions, rp_id));
                    Ok(Cbor::Map(vec![(Cbor::Unsigned(2), Cbor::Bytes(encrypted))]))
                }
                _ => Err(INVALID_SUBCOMMAND),
            }
        }

        /// Prüft `pinUvAuthParam` einer GetAssertion; `Ok(true)` = UV durch PIN-Token.
        fn check_pin_auth(&self, params: &Cbor) -> Result<bool, u8> {
            let Some(param) = params.get_uint_key(6).and_then(Cbor::as_bytes) else {
                return if self.config.always_uv && self.config.uv != Some(true) {
                    Err(status::PIN_REQUIRED)
                } else {
                    Ok(false)
                };
            };
            let protocol = self.protocol_of(&Cbor::Map(vec![(
                Cbor::Unsigned(1),
                params.get_uint_key(7).cloned().unwrap_or(Cbor::Null),
            )]))?;
            let Some((token_protocol, token, permissions, rp_id)) = &self.token else {
                return Err(status::PIN_AUTH_INVALID);
            };
            let hash = params.get_uint_key(2).and_then(Cbor::as_bytes).ok_or(MISSING_PARAMETER)?;
            if *token_protocol != protocol || pin::authenticate(protocol, token, hash) != param {
                return Err(status::PIN_AUTH_INVALID);
            }
            let rp = params.get_uint_key(1).and_then(Cbor::as_text);
            if permissions & pin::PERMISSION_GET_ASSERTION == 0 || rp_id.as_deref().is_some_and(|r| Some(r) != rp) {
                return Err(UNAUTHORIZED_PERMISSION);
            }
            if params.get_uint_key(5).and_then(|o| o.get_text_key("uv")).is_some() {
                return Err(INVALID_PARAMETER); // `uv` neben pinUvAuthParam ist ein Fehler der Plattform
            }
            Ok(true)
        }

        fn reply(&mut self, cid: u32, cmd: u8, payload: &[u8]) {
            let mut report = [0u8; REPORT_LEN];
            report[..4].copy_from_slice(&cid.to_be_bytes());
            report[4] = TYPE_INIT | cmd;
            report[5..7].copy_from_slice(&(payload.len() as u16).to_be_bytes());
            let first = payload.len().min(57);
            report[7..7 + first].copy_from_slice(&payload[..first]);
            self.outbox.push_back(report);
            for (seq, chunk) in payload[first..].chunks(59).enumerate() {
                let mut r = [0u8; REPORT_LEN];
                r[..4].copy_from_slice(&cid.to_be_bytes());
                r[4] = seq as u8;
                r[5..5 + chunk.len()].copy_from_slice(chunk);
                self.outbox.push_back(r);
            }
        }

        fn handle(&mut self, cid: u32, cmd: u8, payload: Vec<u8>) {
            self.received.push((cmd, payload.clone()));
            match cmd {
                CMD_INIT => {
                    let mut resp = payload[..8].to_vec();
                    resp.extend_from_slice(&self.cid.to_be_bytes());
                    resp.extend_from_slice(&[2, 5, 4, 0, if self.config.cbor_capable { 0x04 | 0x01 } else { 0x01 }]);
                    if self.config.noise_other_channel {
                        self.reply(0x0a0b_0c0d, CMD_INIT, &[0u8; 17]);
                    }
                    self.reply(BROADCAST_CID, CMD_INIT, &resp);
                }
                CMD_CANCEL => {
                    self.cancel_received = true;
                    if self.pending_assertion.take().is_some() {
                        self.reply(cid, CMD_CBOR, &[status::KEEPALIVE_CANCEL]);
                    }
                }
                CMD_CBOR => match payload[0] {
                    CTAP2_GET_INFO => {
                        let mut opts = Vec::new();
                        if let Some(t) = self.config.pin_uv_auth_token {
                            opts.push((Cbor::Text("pinUvAuthToken".into()), Cbor::Bool(t)));
                        }
                        if let Some(uv) = self.config.uv {
                            opts.push((Cbor::Text("uv".into()), Cbor::Bool(uv)));
                        }
                        if let Some(p) = self.config.client_pin {
                            opts.push((Cbor::Text("clientPin".into()), Cbor::Bool(p)));
                        }
                        let mut info = vec![
                            (Cbor::Unsigned(1), Cbor::Array(vec![Cbor::Text("FIDO_2_0".into())])),
                            (Cbor::Unsigned(3), Cbor::Bytes(vec![0; 16])),
                            (Cbor::Unsigned(4), Cbor::Map(opts)),
                        ];
                        if !self.config.pin_protocols.is_empty() {
                            let list = self.config.pin_protocols.iter().map(|p| Cbor::Unsigned(*p)).collect();
                            info.push((Cbor::Unsigned(6), Cbor::Array(list)));
                        }
                        let info = Cbor::Map(info);
                        let mut resp = vec![0];
                        resp.extend(cbor_encode(&info));
                        self.reply(cid, CMD_CBOR, &resp);
                    }
                    CTAP2_GET_ASSERTION => {
                        for _ in 0..self.config.keepalives {
                            self.reply(cid, CMD_KEEPALIVE, &[2]);
                        }
                        if self.config.status != 0 {
                            self.reply(cid, CMD_CBOR, &[self.config.status]);
                            return;
                        }
                        let params = cbor_decode(&payload[1..]).unwrap();
                        let pin_uv = match self.check_pin_auth(&params) {
                            Ok(v) => v,
                            Err(code) => {
                                self.reply(cid, CMD_CBOR, &[code]);
                                return;
                            }
                        };
                        let builtin_uv =
                            params.get_uint_key(5).and_then(|o| o.get_text_key("uv")) == Some(&Cbor::Bool(true));
                        self.last_uv = pin_uv || builtin_uv;
                        let cred_id = match params.get_uint_key(3) {
                            Some(Cbor::Array(list)) => list[0].get_text_key("id").cloned().unwrap(),
                            _ => Cbor::Bytes(vec![9, 9, 9]),
                        };
                        let mut map = Vec::new();
                        if !self.config.omit_credential {
                            map.push((
                                Cbor::Unsigned(1),
                                Cbor::Map(vec![
                                    (Cbor::Text("id".into()), cred_id),
                                    (Cbor::Text("type".into()), Cbor::Text("public-key".into())),
                                ]),
                            ));
                        }
                        // authData: rpIdHash(32) + flags + counter(4)
                        let mut auth = vec![0xaa; 32];
                        auth.extend_from_slice(&[if self.last_uv { 0x05 } else { 0x01 }, 0, 0, 0, 5]);
                        map.push((Cbor::Unsigned(2), Cbor::Bytes(auth)));
                        map.push((Cbor::Unsigned(3), Cbor::Bytes(vec![0x30; 70])));
                        if let Some(uh) = &self.config.user_handle {
                            map.push((
                                Cbor::Unsigned(4),
                                Cbor::Map(vec![(Cbor::Text("id".into()), Cbor::Bytes(uh.clone()))]),
                            ));
                        }
                        let mut resp = vec![0];
                        resp.extend(cbor_encode(&Cbor::Map(map)));
                        self.pending_assertion = Some(resp);
                    }
                    pin::CTAP2_CLIENT_PIN => {
                        let params = cbor_decode(&payload[1..]).unwrap_or(Cbor::Null);
                        match self.client_pin(&params) {
                            Ok(resp) => {
                                let mut r = vec![0];
                                r.extend(cbor_encode(&resp));
                                self.reply(cid, CMD_CBOR, &r);
                            }
                            Err(code) => self.reply(cid, CMD_CBOR, &[code]),
                        }
                    }
                    _ => self.reply(cid, CMD_CBOR, &[0x01]),
                },
                _ => self.reply(cid, CMD_ERROR, &[0x01]),
            }
        }
    }

    impl HidConnection for SimDevice {
        fn write_report(&mut self, report: &[u8; REPORT_LEN]) -> std::io::Result<()> {
            let cid = u32::from_be_bytes(report[..4].try_into().unwrap());
            if report[4] & TYPE_INIT != 0 {
                let cmd = report[4] & !TYPE_INIT;
                let total = u16::from_be_bytes([report[5], report[6]]) as usize;
                self.inbox = report[7..7 + total.min(57)].to_vec();
                if self.inbox.len() >= total {
                    let p = std::mem::take(&mut self.inbox);
                    self.handle(cid, cmd, p);
                } else {
                    self.expect = Some((cid, cmd, total, 0));
                }
            } else if let Some((ecid, cmd, total, seq)) = self.expect {
                assert_eq!(ecid, cid);
                assert_eq!(report[4], seq);
                let need = (total - self.inbox.len()).min(59);
                self.inbox.extend_from_slice(&report[5..5 + need]);
                if self.inbox.len() >= total {
                    self.expect = None;
                    let p = std::mem::take(&mut self.inbox);
                    self.handle(cid, cmd, p);
                } else {
                    self.expect = Some((ecid, cmd, total, seq + 1));
                }
            }
            Ok(())
        }

        fn read_report(&mut self, _timeout: Duration) -> std::io::Result<Option<[u8; REPORT_LEN]>> {
            if let Some(r) = self.outbox.pop_front() {
                return Ok(Some(r));
            }
            if !self.cancel_received
                && let Some(resp) = self.pending_assertion.take()
            {
                let cid = self.cid;
                self.reply(cid, CMD_CBOR, &resp);
                return Ok(self.outbox.pop_front());
            }
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sim::*;
    use super::*;
    use crate::fido2::CredentialDescriptor;

    fn options(uv: UserVerification, allow: usize) -> AssertionOptions {
        AssertionOptions {
            challenge: vec![1, 2, 3, 4],
            rp_id: "a-trust.at".into(),
            allow_credentials: (0..allow)
                .map(|i| CredentialDescriptor { id: vec![i as u8; 64], transports: vec!["usb".into()] })
                .collect(),
            user_verification: uv,
            timeout_ms: Some(30_000),
        }
    }

    #[test]
    fn cbor_roundtrip_and_canonical_heads() {
        let v = Cbor::Map(vec![
            (Cbor::Unsigned(1), Cbor::Text("a-trust.at".into())),
            (Cbor::Unsigned(2), Cbor::Bytes(vec![0; 300])),
            (
                Cbor::Negative(24),
                Cbor::Array(vec![Cbor::Bool(true), Cbor::Bool(false), Cbor::Null, Cbor::Unsigned(70000)]),
            ),
            (Cbor::Text("x".into()), Cbor::Unsigned(u64::MAX)),
        ]);
        let enc = cbor_encode(&v);
        assert_eq!(&enc[..3], &[0xa4, 0x01, 0x6a]);
        assert_eq!(cbor_decode(&enc).unwrap(), v);
        assert_eq!(cbor_encode(&Cbor::Unsigned(23)), [0x17]);
        assert_eq!(cbor_encode(&Cbor::Unsigned(24)), [0x18, 24]);
        assert_eq!(cbor_encode(&Cbor::Unsigned(256)), [0x19, 1, 0]);
        assert_eq!(cbor_encode(&Cbor::Negative(0)), [0x20]);
        assert!(cbor_decode(&[0x5f]).is_err(), "unbestimmte Länge");
        assert!(cbor_decode(&[0x42, 1]).is_err(), "zu kurz");
        assert!(cbor_decode(&[0xf9, 0, 0]).is_err(), "Gleitkomma");
        assert_eq!(cbor_decode(&[0xc6, 0x01]).unwrap(), Cbor::Unsigned(1), "Tag wird übersprungen");
        let deep: Vec<u8> = std::iter::repeat_n(0x81u8, 40).chain([0x01]).collect();
        assert!(cbor_decode(&deep).is_err());
    }

    #[test]
    fn get_assertion_params_are_canonical() {
        let p = get_assertion_params(&options(UserVerification::Required, 1), &[0x11; 32], true);
        let enc = cbor_encode(&p);
        // {1: rpId, 2: hash, 3: [{"id":…, "type":"public-key"}], 5: {"uv": true}}
        assert_eq!(enc[0], 0xa4);
        let back = cbor_decode(&enc).unwrap();
        assert_eq!(back.get_uint_key(1), Some(&Cbor::Text("a-trust.at".into())));
        let opts = back.get_uint_key(5).unwrap();
        assert_eq!(opts.get_text_key("uv"), Some(&Cbor::Bool(true)));
        let p = get_assertion_params(&options(UserVerification::Discouraged, 0), &[0x11; 32], false);
        assert_eq!(cbor_encode(&p)[0], 0xa2, "ohne allowList und ohne Optionen");
    }

    #[test]
    fn full_flow_against_simulator() {
        let sim = SimDevice::new(SimConfig::default());
        let mut dev = CtapHidDevice::init(sim, [1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert_eq!(dev.cid(), 0x0102_0304);
        assert!(dev.supports_cbor());
        let cancel = AtomicBool::new(false);
        let opts = options(UserVerification::Preferred, 3);
        let a = get_assertion(&mut dev, &opts, "https://service.a-trust.at", &cancel).unwrap();
        assert_eq!(a.raw_id, vec![0u8; 64], "erstes Credential der allowList (Nachricht > 1 Paket)");
        assert_eq!(a.id, credential_id_string(&a.raw_id));
        assert_eq!(a.kind, "public-key");
        assert_eq!(a.authenticator_data.len(), 37);
        assert_eq!(a.signature.len(), 70);
        assert_eq!(a.user_handle, Some(vec![7, 7]));
        let cd: serde_json::Value = serde_json::from_slice(&a.client_data_json).unwrap();
        assert_eq!(cd["origin"], "https://service.a-trust.at");
        assert_eq!(cd["challenge"], "AQIDBA");
        // clientDataHash im Request = SHA-256(clientDataJSON)
        let sent = &dev.conn.received.iter().find(|(c, p)| *c == CMD_CBOR && p[0] == CTAP2_GET_ASSERTION).unwrap().1;
        let params = cbor_decode(&sent[1..]).unwrap();
        let expected: [u8; 32] = sha2::Sha256::digest(&a.client_data_json).into();
        assert_eq!(params.get_uint_key(2), Some(&Cbor::Bytes(expected.to_vec())));
        assert!(params.get_uint_key(5).is_none(), "preferred ohne eingebaute UV → keine Optionen");
    }

    #[test]
    fn uv_decisions() {
        let cancel = AtomicBool::new(false);
        let origin = "https://service.a-trust.at";
        // eingebaute UV wird bei required genutzt
        let sim = SimDevice::new(SimConfig { uv: Some(true), ..SimConfig::default() });
        let mut dev = CtapHidDevice::init(sim, [9; 8]).unwrap();
        get_assertion(&mut dev, &options(UserVerification::Required, 1), origin, &cancel).unwrap();
        let sent = &dev.conn.received.iter().rev().find(|(c, _)| *c == CMD_CBOR).unwrap().1;
        assert_eq!(
            cbor_decode(&sent[1..]).unwrap().get_uint_key(5).unwrap().get_text_key("uv"),
            Some(&Cbor::Bool(true))
        );
        // nur PIN eingerichtet → klarer Hinweis
        let sim = SimDevice::new(SimConfig { uv: None, client_pin: Some(true), ..SimConfig::default() });
        let mut dev = CtapHidDevice::init(sim, [9; 8]).unwrap();
        let e = get_assertion(&mut dev, &options(UserVerification::Required, 1), origin, &cancel).unwrap_err();
        assert_eq!(e, Fido2Error::PinRequired);
        assert_eq!(e.code(), "fido2-pin-required");
        // nichts eingerichtet
        let sim = SimDevice::new(SimConfig { uv: None, client_pin: None, ..SimConfig::default() });
        let mut dev = CtapHidDevice::init(sim, [9; 8]).unwrap();
        assert!(get_assertion(&mut dev, &options(UserVerification::Required, 1), origin, &cancel).is_err());
        // nur U2F
        let sim = SimDevice::new(SimConfig { cbor_capable: false, ..SimConfig::default() });
        let mut dev = CtapHidDevice::init(sim, [9; 8]).unwrap();
        assert!(get_assertion(&mut dev, &options(UserVerification::Discouraged, 1), origin, &cancel).is_err());
    }

    #[test]
    fn status_mapping_and_missing_credential() {
        let cancel = AtomicBool::new(false);
        let origin = "https://service.a-trust.at";
        for (code, cancelled) in
            [(status::OPERATION_DENIED, true), (status::NO_CREDENTIALS, false), (status::PIN_REQUIRED, false)]
        {
            let sim = SimDevice::new(SimConfig { status: code, ..SimConfig::default() });
            let mut dev = CtapHidDevice::init(sim, [3; 8]).unwrap();
            let e = get_assertion(&mut dev, &options(UserVerification::Discouraged, 1), origin, &cancel).unwrap_err();
            assert_eq!(e == Fido2Error::Cancelled, cancelled, "{code:#x}: {e:?}");
        }
        // credential fehlt bei genau einem allowList-Eintrag → dessen ID
        let sim = SimDevice::new(SimConfig { omit_credential: true, user_handle: None, ..SimConfig::default() });
        let mut dev = CtapHidDevice::init(sim, [3; 8]).unwrap();
        let a = get_assertion(&mut dev, &options(UserVerification::Discouraged, 1), origin, &cancel).unwrap();
        assert_eq!(a.raw_id, vec![0u8; 64]);
        assert_eq!(a.user_handle, None);
    }

    #[test]
    fn cancel_sends_ctaphid_cancel() {
        let sim = SimDevice::new(SimConfig { keepalives: 0, ..SimConfig::default() });
        let mut dev = CtapHidDevice::init(sim, [4; 8]).unwrap();
        let cancel = AtomicBool::new(true);
        let e =
            get_assertion(&mut dev, &options(UserVerification::Discouraged, 1), "https://service.a-trust.at", &cancel)
                .unwrap_err();
        assert_eq!(e, Fido2Error::Cancelled);
        assert!(dev.conn.cancel_received);
    }

    // --- clientPIN (Welle 3) ----------------------------------------------------------------------

    const ORIGIN: &str = "https://service.a-trust.at";

    fn pin_device(config: SimConfig) -> CtapHidDevice<SimDevice> {
        CtapHidDevice::init(SimDevice::new(config), [6; 8]).unwrap()
    }

    /// Parameter der zuletzt gesendeten CTAP2-Nachricht mit Kommando `command`.
    fn last_sent(dev: &CtapHidDevice<SimDevice>, command: u8) -> Option<Cbor> {
        dev.conn
            .received
            .iter()
            .rev()
            .find(|(c, p)| *c == CMD_CBOR && p.first() == Some(&command))
            .map(|(_, p)| cbor_decode(&p[1..]).unwrap())
    }

    fn sub_commands(dev: &CtapHidDevice<SimDevice>) -> Vec<u64> {
        dev.conn
            .received
            .iter()
            .filter(|(c, p)| *c == CMD_CBOR && p.first() == Some(&crate::fido2::pin::CTAP2_CLIENT_PIN))
            .filter_map(|(_, p)| match cbor_decode(&p[1..]).unwrap().get_uint_key(2) {
                Some(Cbor::Unsigned(n)) => Some(*n),
                _ => None,
            })
            .collect()
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn pin_protocols_one_and_two_against_simulator() {
        use crate::fido2::pin::sub;
        let cancel = AtomicBool::new(false);
        for (protocols, permissions, protocol, param_len, token_sub) in [
            (vec![], None, 1, 16, sub::GET_PIN_TOKEN),
            (vec![1], None, 1, 16, sub::GET_PIN_TOKEN),
            (vec![2, 1], None, 2, 32, sub::GET_PIN_TOKEN),
            (vec![1, 2], Some(true), 2, 32, sub::GET_PIN_UV_AUTH_TOKEN_USING_PIN_WITH_PERMISSIONS),
        ] {
            let label = format!("{protocols:?} {permissions:?}");
            let mut dev =
                pin_device(SimConfig { pin_uv_auth_token: permissions, ..SimConfig::with_pin("1234", &protocols) });
            // ohne PIN: wie ein Browser bei required und preferred die PIN verlangen
            for uv in [UserVerification::Required, UserVerification::Preferred] {
                let e = get_assertion(&mut dev, &options(uv, 1), ORIGIN, &cancel).unwrap_err();
                assert_eq!(e, Fido2Error::PinRequired, "{label}");
            }
            // discouraged: nur Anwesenheit, kein clientPIN
            get_assertion(&mut dev, &options(UserVerification::Discouraged, 1), ORIGIN, &cancel).unwrap();
            assert!(!dev.conn.last_uv && sub_commands(&dev).is_empty(), "{label}");
            // mit PIN
            let a = get_assertion_with_pin(
                &mut dev,
                &options(UserVerification::Required, 1),
                ORIGIN,
                &cancel,
                Some("1234"),
            )
            .unwrap();
            assert!(dev.conn.last_uv, "{label}");
            assert_eq!(a.authenticator_data[32] & 0x04, 0x04, "UV-Flag ({label})");
            let sent = last_sent(&dev, CTAP2_GET_ASSERTION).unwrap();
            assert_eq!(sent.get_uint_key(7), Some(&Cbor::Unsigned(protocol)), "{label}");
            assert!(matches!(sent.get_uint_key(6), Some(Cbor::Bytes(b)) if b.len() == param_len), "{label}");
            assert!(sent.get_uint_key(5).is_none(), "keine uv-Option neben pinUvAuthParam ({label})");
            assert_eq!(sub_commands(&dev), [sub::GET_PIN_RETRIES, sub::GET_KEY_AGREEMENT, token_sub], "{label}");
            let token_req = last_sent(&dev, crate::fido2::pin::CTAP2_CLIENT_PIN).unwrap();
            if token_sub == sub::GET_PIN_UV_AUTH_TOKEN_USING_PIN_WITH_PERMISSIONS {
                assert_eq!(
                    token_req.get_uint_key(9),
                    Some(&Cbor::Unsigned(crate::fido2::pin::PERMISSION_GET_ASSERTION))
                );
                assert_eq!(token_req.get_uint_key(10), Some(&Cbor::Text("a-trust.at".into())));
            } else {
                assert!(token_req.get_uint_key(9).is_none() && token_req.get_uint_key(10).is_none());
            }
            // weder PIN noch PIN-Hash im Klartext auf der Leitung
            let hash = crate::fido2::pin::pin_hash("1234");
            for (_, payload) in &dev.conn.received {
                assert!(!contains(payload, b"1234") && !contains(payload, hash.as_ref()), "{label}");
            }
        }
    }

    #[test]
    fn wrong_pin_counts_down_and_locks() {
        use crate::fido2::pin::sub;
        let cancel = AtomicBool::new(false);
        let opts = options(UserVerification::Required, 1);
        let mut dev = pin_device(SimConfig { pin_retries: 5, ..SimConfig::with_pin("1234", &[2, 1]) });
        let e = get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("4321")).unwrap_err();
        assert_eq!(e, Fido2Error::PinInvalid);
        assert_eq!(e.code(), "fido2-pin-invalid");
        assert_eq!(dev.conn.config.pin_retries, 4, "ein Versuch verbraucht");
        get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("1234")).unwrap();
        assert_eq!(dev.conn.config.pin_retries, 8, "richtige PIN setzt den Zähler zurück");
        // drei Fehlversuche in Folge: PIN_AUTH_BLOCKED – auch die richtige PIN hilft erst nach dem
        // erneuten Anstecken
        for _ in 0..2 {
            assert_eq!(
                get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("0000")).unwrap_err(),
                Fido2Error::PinInvalid
            );
        }
        let e = get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("0000")).unwrap_err();
        assert_eq!(e, Fido2Error::PinAuthBlocked, "vorübergehend, nicht dauerhaft gesperrt");
        assert_eq!(e.code(), "fido2-pin-auth-blocked");
        assert_eq!(
            get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("1234")).unwrap_err(),
            Fido2Error::PinAuthBlocked
        );
        dev.conn.power_cycle();
        get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("1234")).unwrap();
        // letzter Versuch falsch → PIN_BLOCKED; danach prüft der Client getPINRetries und
        // verbraucht keinen Versuch mehr (kein getKeyAgreement/getPinToken)
        let mut dev = pin_device(SimConfig { pin_retries: 1, ..SimConfig::with_pin("1234", &[1]) });
        assert_eq!(
            get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("9999")).unwrap_err(),
            Fido2Error::Locked
        );
        assert_eq!(dev.conn.config.pin_retries, 0);
        let before = sub_commands(&dev).len();
        assert_eq!(
            get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("1234")).unwrap_err(),
            Fido2Error::Locked
        );
        assert_eq!(sub_commands(&dev)[before..], [sub::GET_PIN_RETRIES]);
        // gesperrt bereits beim Öffnen
        let mut dev = pin_device(SimConfig { pin_retries: 0, ..SimConfig::with_pin("1234", &[2]) });
        assert_eq!(
            get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("1234")).unwrap_err(),
            Fido2Error::Locked
        );
    }

    #[test]
    fn pin_not_set_unsupported_or_malformed() {
        let cancel = AtomicBool::new(false);
        let required = options(UserVerification::Required, 1);
        // PIN unterstützt, aber nicht gesetzt: required ohne Biometrie → klarer Code
        let mut dev = pin_device(SimConfig { uv: None, client_pin: Some(false), ..SimConfig::default() });
        let e = get_assertion(&mut dev, &required, ORIGIN, &cancel).unwrap_err();
        assert_eq!(e, Fido2Error::NoUserVerification);
        assert_eq!(e.code(), "fido2-no-user-verification");
        assert_eq!(
            get_assertion_with_pin(&mut dev, &required, ORIGIN, &cancel, Some("1234")).unwrap_err(),
            Fido2Error::NoUserVerification
        );
        // preferred ohne PIN/Biometrie → nur Anwesenheit
        get_assertion(&mut dev, &options(UserVerification::Preferred, 1), ORIGIN, &cancel).unwrap();
        assert!(!dev.conn.last_uv);
        // GetInfo meldet clientPin = true, der Schlüssel hat aber keine PIN (PIN_NOT_SET)
        let mut dev = pin_device(SimConfig { uv: None, client_pin: Some(true), pin: None, ..SimConfig::default() });
        assert_eq!(
            get_assertion_with_pin(&mut dev, &required, ORIGIN, &cancel, Some("1234")).unwrap_err(),
            Fido2Error::NoUserVerification
        );
        // kein clientPIN überhaupt
        let mut dev = pin_device(SimConfig { uv: None, client_pin: None, ..SimConfig::default() });
        assert!(matches!(
            get_assertion_with_pin(&mut dev, &required, ORIGIN, &cancel, Some("1234")),
            Err(Fido2Error::Device(_))
        ));
        // zu kurze bzw. zu lange PIN: falsch, ohne einen Versuch am Schlüssel zu verbrauchen
        let mut dev = pin_device(SimConfig::with_pin("1234", &[2]));
        for pin in ["123", &"1".repeat(64)] {
            assert_eq!(
                get_assertion_with_pin(&mut dev, &required, ORIGIN, &cancel, Some(pin)).unwrap_err(),
                Fido2Error::PinInvalid
            );
        }
        assert!(sub_commands(&dev).is_empty());
        assert_eq!(dev.conn.config.pin_retries, 8);
        // nur unbekannte Protokolle
        let mut dev = pin_device(SimConfig::with_pin("1234", &[7]));
        assert!(matches!(
            get_assertion_with_pin(&mut dev, &required, ORIGIN, &cancel, Some("1234")),
            Err(Fido2Error::Device(m)) if m.contains("PIN-Protokoll")
        ));
    }

    #[test]
    fn always_uv_key_requests_the_pin_in_get_assertion() {
        let cancel = AtomicBool::new(false);
        let opts = options(UserVerification::Discouraged, 1);
        let mut dev = pin_device(SimConfig { always_uv: true, ..SimConfig::with_pin("1234", &[1]) });
        assert_eq!(get_assertion(&mut dev, &opts, ORIGIN, &cancel).unwrap_err(), Fido2Error::PinRequired);
        get_assertion_with_pin(&mut dev, &opts, ORIGIN, &cancel, Some("1234")).unwrap();
        assert!(dev.conn.last_uv);
        // gefälschtes pinUvAuthParam weist der Schlüssel ab
        let hash = [0x11; 32];
        let params = get_assertion_params_with_pin(&opts, &hash, false, Some((vec![0; 16], 1)));
        let (code, _) = dev.cbor(CTAP2_GET_ASSERTION, Some(&params), Duration::from_secs(1), None).unwrap();
        assert_eq!(code, status::PIN_AUTH_INVALID);
        assert!(matches!(map_status(code), Fido2Error::Device(_)));
    }

    #[test]
    fn status_codes_from_the_catalog() {
        assert_eq!(map_status(status::NO_CREDENTIALS), Fido2Error::NoCredential);
        assert_eq!(map_status(status::USER_ACTION_TIMEOUT), Fido2Error::Timeout);
        assert_eq!(map_status(status::ACTION_TIMEOUT), Fido2Error::Timeout);
        for code in [status::PIN_BLOCKED, status::UV_BLOCKED] {
            assert_eq!(map_status(code), Fido2Error::Locked);
        }
        assert_eq!(map_status(status::PIN_AUTH_BLOCKED), Fido2Error::PinAuthBlocked);
        assert_eq!(map_status(status::PIN_NOT_SET), Fido2Error::NoUserVerification);
        let sim = SimDevice::new(SimConfig { cbor_capable: false, ..SimConfig::default() });
        let mut dev = CtapHidDevice::init(sim, [9; 8]).unwrap();
        let e = get_assertion(&mut dev, &options(UserVerification::Discouraged, 1), ORIGIN, &AtomicBool::new(false))
            .unwrap_err();
        assert_eq!(e, Fido2Error::U2fOnly);
    }

    #[test]
    fn get_info_reads_pin_fields() {
        let mut dev = pin_device(SimConfig { pin_uv_auth_token: Some(true), ..SimConfig::with_pin("1234", &[2, 1]) });
        let info = dev.get_info().unwrap();
        assert_eq!(info.pin_protocols, [2, 1]);
        assert_eq!(info.pin_uv_auth_token, Some(true));
        assert_eq!(info.client_pin, Some(true));
        let mut dev = pin_device(SimConfig::default());
        let info = dev.get_info().unwrap();
        assert!(info.pin_protocols.is_empty());
        assert_eq!(info.pin_uv_auth_token, None);
    }

    /// Gerät, das vorgegebene (auch bösartige) Reports liefert.
    struct ScriptedReports(std::collections::VecDeque<[u8; REPORT_LEN]>);

    impl HidConnection for ScriptedReports {
        fn write_report(&mut self, _report: &[u8; REPORT_LEN]) -> std::io::Result<()> {
            Ok(())
        }
        fn read_report(&mut self, _timeout: Duration) -> std::io::Result<Option<[u8; REPORT_LEN]>> {
            Ok(self.0.pop_front())
        }
    }

    fn device_with(reports: Vec<[u8; REPORT_LEN]>) -> CtapHidDevice<ScriptedReports> {
        CtapHidDevice { conn: ScriptedReports(reports.into()), cid: 7, capabilities: 0x04 }
    }

    fn init_report(cid: u32, cmd: u8, total: u16, data: &[u8]) -> [u8; REPORT_LEN] {
        let mut r = [0u8; REPORT_LEN];
        r[..4].copy_from_slice(&cid.to_be_bytes());
        r[4] = TYPE_INIT | cmd;
        r[5..7].copy_from_slice(&total.to_be_bytes());
        r[7..7 + data.len()].copy_from_slice(data);
        r
    }

    fn cont_report(cid: u32, seq: u8) -> [u8; REPORT_LEN] {
        let mut r = [0xabu8; REPORT_LEN];
        r[..4].copy_from_slice(&cid.to_be_bytes());
        r[4] = seq;
        r
    }

    #[test]
    fn malicious_reports_never_panic() {
        let deadline = || Instant::now() + Duration::from_millis(300);
        // angekündigte Länge 65535: die Fortsetzungen laufen nach 128 Paketen in das Init-Bit
        // (Sequenz 0x80) und werden als Protokollfehler erkannt
        let mut reports = vec![init_report(7, CMD_CBOR, u16::MAX, &[0; 57])];
        reports.extend((0..=128u8).map(|seq| cont_report(7, seq)));
        assert!(device_with(reports).receive(deadline(), None).is_err());
        // falsche Sequenznummer
        let reports = vec![init_report(7, CMD_CBOR, 100, &[0; 57]), cont_report(7, 1)];
        assert!(device_with(reports).receive(deadline(), None).is_err());
        // fremde Kanäle und verwaiste Fortsetzungen werden übersprungen, dann Zeitüberschreitung
        let reports = vec![init_report(8, CMD_CBOR, 1, &[0]), cont_report(7, 0)];
        assert!(device_with(reports).receive(deadline(), None).is_err());
        // leere CBOR-Antwort und CTAPHID_ERROR
        let mut dev = device_with(vec![init_report(7, CMD_CBOR, 0, &[])]);
        assert!(dev.cbor(CTAP2_GET_INFO, None, Duration::from_millis(300), None).is_err());
        let mut dev = device_with(vec![init_report(7, CMD_ERROR, 1, &[0x7f])]);
        assert!(matches!(dev.receive(deadline(), None), Err(Fido2Error::Device(m)) if m.contains("0x7f")));
        // GetInfo mit Müll statt CBOR
        let mut dev = device_with(vec![init_report(7, CMD_CBOR, 4, &[0, 0xff, 0xff, 0xff])]);
        assert!(dev.get_info().is_err());
        // INIT-Antwort mit Broadcast-Kanal wird abgelehnt
        let mut payload = [0u8; 17];
        payload[..8].copy_from_slice(&[5; 8]);
        payload[8..12].copy_from_slice(&BROADCAST_CID.to_be_bytes());
        let conn = ScriptedReports(vec![init_report(BROADCAST_CID, CMD_INIT, 17, &payload)].into());
        assert!(CtapHidDevice::init(conn, [5; 8]).is_err());
    }

    #[test]
    fn cbor_decoder_survives_random_input() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..20_000 {
            let len = (next() % 48) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            let _ = cbor_decode(&bytes);
        }
        // riesige angekündigte Längen ohne Daten
        assert!(cbor_decode(&[0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).is_err());
        assert!(cbor_decode(&[0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).is_err());
        assert!(cbor_decode(&[0xbb, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0x01]).is_err());
    }
}

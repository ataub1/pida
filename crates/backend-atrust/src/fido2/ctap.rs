//! CTAP2 über CTAPHID, transportunabhängig (FIDO CTAP 2.1, §8.1 USB-HID, §6.2 GetAssertion,
//! §6.4 GetInfo). Der Plattform-Transport ([`HidConnection`]) liefert nur 64-Byte-Reports; alles
//! andere (Kanal-Allokation, Fragmentierung, Keepalive, Abbruch, CBOR) liegt hier und ist gegen
//! einen simulierten Authenticator getestet.
//!
//! Nicht enthalten (offener Punkt): `clientPIN`/PIN-Protokoll. Verlangt ein Authenticator eine
//! PIN, meldet [`get_assertion`] [`Fido2Error::PinRequired`] (Code `fido2-pin-required`; Windows
//! erledigt die PIN über `webauthn.dll` selbst). Der Weg zur PIN steht bereit
//! (`SecretKind::Fido2Pin`, `Fido2Provider::get_assertion_with_pin`), der HID-Provider meldet aber
//! noch `supports_pin() == false`.

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
    /// PIN erforderlich
    pub const PIN_REQUIRED: u8 = 0x36;
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
    /// Option `clientPin`: `Some(true)` = PIN gesetzt
    pub client_pin: Option<bool>,
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
        Ok(AuthenticatorInfo { versions, uv: option("uv"), client_pin: option("clientPin") })
    }
}

fn text(s: &str) -> Cbor {
    Cbor::Text(s.to_owned())
}

/// Baut die Parameter von `authenticatorGetAssertion` (kanonisch sortierte Schlüssel).
pub fn get_assertion_params(options: &AssertionOptions, client_data_hash: &[u8; 32], uv: bool) -> Cbor {
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
    if uv {
        entries.push((Cbor::Unsigned(5), Cbor::Map(vec![(text("uv"), Cbor::Bool(true))])));
    }
    Cbor::Map(entries)
}

/// Fehlerabbildung der CTAP2-Statuscodes.
fn map_status(code: u8) -> Fido2Error {
    match code {
        status::KEEPALIVE_CANCEL | status::OPERATION_DENIED => Fido2Error::Cancelled,
        status::NO_CREDENTIALS => {
            Fido2Error::Device("Auf diesem Sicherheitsschlüssel ist keine ID-Austria-Registrierung vorhanden".into())
        }
        status::USER_ACTION_TIMEOUT | status::ACTION_TIMEOUT => {
            Fido2Error::Device("Zeitüberschreitung: Sicherheitsschlüssel wurde nicht berührt".into())
        }
        // Code `fido2-pin-required`; ohne PIN-Unterstützung des Providers zeigt das Backend den
        // Hinweis „bitte App oder SMS verwenden“ (Welle 2)
        status::PIN_REQUIRED => Fido2Error::PinRequired,
        status::PIN_INVALID => Fido2Error::PinInvalid,
        status::PIN_BLOCKED | status::UV_BLOCKED => Fido2Error::Device("Sicherheitsschlüssel ist gesperrt".into()),
        other => Fido2Error::Device(format!("CTAP2-Status 0x{other:02x}")),
    }
}

/// Holt eine Assertion über einen geöffneten Kanal: GetInfo → UV-Entscheidung → GetAssertion.
///
/// UV: `required` → eingebaute UV (`uv`-Option) verwenden; ist nur eine PIN eingerichtet, kommt
/// ein Hinweis (siehe Modul-Doku). `preferred` → eingebaute UV, wenn eingerichtet, sonst nur
/// Anwesenheit. `discouraged` → nur Anwesenheit.
pub fn get_assertion<C: HidConnection>(
    device: &mut CtapHidDevice<C>,
    options: &AssertionOptions,
    origin: &str,
    cancel: &AtomicBool,
) -> Result<Fido2Assertion, Fido2Error> {
    if !device.supports_cbor() {
        return Err(Fido2Error::Device("Sicherheitsschlüssel unterstützt kein FIDO2 (nur U2F)".into()));
    }
    let info = device.get_info()?;
    let builtin_uv = info.uv == Some(true);
    let uv = match options.user_verification {
        UserVerification::Required if builtin_uv => true,
        UserVerification::Required if info.client_pin == Some(true) => return Err(map_status(status::PIN_REQUIRED)),
        UserVerification::Required => {
            return Err(Fido2Error::Device(
                "Sicherheitsschlüssel kann die Nutzerin nicht verifizieren (keine PIN/Biometrie eingerichtet)".into(),
            ));
        }
        UserVerification::Preferred => builtin_uv,
        UserVerification::Discouraged => false,
    };
    let client_data = client_data_json(&options.challenge, origin);
    let hash: [u8; 32] = sha2::Sha256::digest(&client_data).into();
    let params = get_assertion_params(options, &hash, uv);
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
    //! Simulierter CTAP2-Authenticator für Tests (kein echtes Krypto, feste Antworten).
    use std::collections::VecDeque;

    use super::*;

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
            }
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
                        if let Some(uv) = self.config.uv {
                            opts.push((Cbor::Text("uv".into()), Cbor::Bool(uv)));
                        }
                        if let Some(p) = self.config.client_pin {
                            opts.push((Cbor::Text("clientPin".into()), Cbor::Bool(p)));
                        }
                        let info = Cbor::Map(vec![
                            (Cbor::Unsigned(1), Cbor::Array(vec![Cbor::Text("FIDO_2_0".into())])),
                            (Cbor::Unsigned(3), Cbor::Bytes(vec![0; 16])),
                            (Cbor::Unsigned(4), Cbor::Map(opts)),
                        ]);
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
                        auth.extend_from_slice(&[0x01, 0, 0, 0, 5]);
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

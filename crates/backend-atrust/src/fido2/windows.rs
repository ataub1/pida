//! Windows-WebAuthn-API (`webauthn.dll`, ab Windows 10 1903, API-Version ≥ 1) – Feature
//! `fido2-windows`.
//!
//! Begründung für `unsafe` (ADR 0001 §4 Regel 8): Die Plattform-API ist nur als C-Schnittstelle
//! verfügbar. Der Aufrufer übergibt `clientDataJSON` selbst (`WEBAUTHN_CLIENT_DATA`) und kann so die
//! Origin `https://service.a-trust.at` setzen (Faktencheck 9a); Windows berechnet den Hash, zeigt
//! den Systemdialog (PIN, Windows Hello, Sicherheitsschlüssel) und liefert die Assertion. Das
//! `unsafe` ist auf diese Datei beschränkt: dynamisches Laden (`LoadLibraryExW` nur aus `System32`,
//! `GetProcAddress`), Strukturen nach `webauthn.h`, Kopieren der Ergebnis-Puffer vor
//! `WebAuthNFreeAssertion`. Ohne Windows (oder ohne DLL) ist der Provider „nicht verfügbar“.
//!
//! Strukturversionen: `WEBAUTHN_CLIENT_DATA` v1, `WEBAUTHN_CREDENTIAL` v1,
//! `WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS` v3 (mit Abbruch-ID; v1, wenn die DLL keine
//! Abbruch-ID liefert), Ergebnis `WEBAUTHN_ASSERTION` – gelesen werden nur die Felder der Version 1.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use pida_backend_api::WindowHandle;

use super::{
    AssertionOptions, Fido2Assertion, Fido2Error, Fido2Provider, Fido2Request, UserVerification, client_data_json,
    credential_id_string,
};

type Dword = u32;
type Hresult = i32;
type Bool = i32;
type Pcwstr = *const u16;
type Hwnd = *mut c_void;

/// `GUID`
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct ClientData {
    dw_version: Dword,
    cb_client_data_json: Dword,
    pb_client_data_json: *const u8,
    pwsz_hash_alg_id: Pcwstr,
}

#[repr(C)]
struct Credential {
    dw_version: Dword,
    cb_id: Dword,
    pb_id: *const u8,
    pwsz_credential_type: Pcwstr,
}

#[repr(C)]
struct Credentials {
    c_credentials: Dword,
    p_credentials: *const Credential,
}

#[repr(C)]
struct Extensions {
    c_extensions: Dword,
    p_extensions: *const c_void,
}

/// `WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS` bis Version 3.
#[repr(C)]
struct GetAssertionOptions {
    dw_version: Dword,
    dw_timeout_milliseconds: Dword,
    credential_list: Credentials,
    extensions: Extensions,
    dw_authenticator_attachment: Dword,
    dw_user_verification_requirement: Dword,
    dw_flags: Dword,
    // Version 2
    pwsz_u2f_app_id: Pcwstr,
    pb_u2f_app_id: *mut Bool,
    // Version 3
    p_cancellation_id: *mut Guid,
}

/// `WEBAUTHN_ASSERTION`, Felder der Version 1.
#[repr(C)]
struct Assertion {
    dw_version: Dword,
    cb_authenticator_data: Dword,
    pb_authenticator_data: *const u8,
    cb_signature: Dword,
    pb_signature: *const u8,
    credential: Credential,
    cb_user_id: Dword,
    pb_user_id: *const u8,
}

const CLIENT_DATA_VERSION: Dword = 1;
const CREDENTIAL_VERSION: Dword = 1;
const GET_ASSERTION_OPTIONS_VERSION_1: Dword = 1;
const GET_ASSERTION_OPTIONS_VERSION_3: Dword = 3;
const ATTACHMENT_ANY: Dword = 0;
const UV_REQUIRED: Dword = 1;
const UV_PREFERRED: Dword = 2;
const UV_DISCOURAGED: Dword = 3;

const NTE_USER_CANCELLED: Hresult = 0x8009_0036_u32 as Hresult;
const NTE_NOT_FOUND: Hresult = 0x8009_0011_u32 as Hresult;
const HRESULT_ERROR_CANCELLED: Hresult = 0x8007_04C7_u32 as Hresult;
const NTE_TOKEN_TIMEOUT: Hresult = 0x8009_0035_u32 as Hresult;

// nur beim Laden der DLL (`cfg(windows)`) gebraucht; die übrigen Signaturen stehen in `Api`
#[cfg(windows)]
type FnGetApiVersion = unsafe extern "system" fn() -> Dword;
type FnGetAssertion = unsafe extern "system" fn(
    Hwnd,
    Pcwstr,
    *const ClientData,
    *const GetAssertionOptions,
    *mut *mut Assertion,
) -> Hresult;
type FnFreeAssertion = unsafe extern "system" fn(*mut Assertion);
type FnGetCancellationId = unsafe extern "system" fn(*mut Guid) -> Hresult;
type FnCancel = unsafe extern "system" fn(*const Guid) -> Hresult;
type FnGetErrorName = unsafe extern "system" fn(Hresult) -> Pcwstr;

/// Funktionszeiger aus `webauthn.dll`.
struct Api {
    version: Dword,
    get_assertion: FnGetAssertion,
    free_assertion: FnFreeAssertion,
    get_cancellation_id: Option<FnGetCancellationId>,
    cancel: Option<FnCancel>,
    error_name: Option<FnGetErrorName>,
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(load).as_ref()
}

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;

    pub type Hmodule = *mut c_void;
    pub const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> Hmodule;
        pub fn GetProcAddress(module: Hmodule, name: *const u8) -> *mut c_void;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetForegroundWindow() -> *mut c_void;
    }
}

#[cfg(windows)]
fn load() -> Option<Api> {
    let name = wide("webauthn.dll");
    // SAFETY: `name` ist NUL-terminiert; Suche nur in System32 (keine DLL-Planting-Lücke).
    let module = unsafe { sys::LoadLibraryExW(name.as_ptr(), std::ptr::null_mut(), sys::LOAD_LIBRARY_SEARCH_SYSTEM32) };
    if module.is_null() {
        return None;
    }
    let symbol = |n: &[u8]| -> *mut c_void {
        debug_assert_eq!(n.last(), Some(&0));
        // SAFETY: `module` ist gültig, `n` NUL-terminiert.
        unsafe { sys::GetProcAddress(module, n.as_ptr()) }
    };
    let get_version = symbol(b"WebAuthNGetApiVersionNumber\0");
    let get_assertion = symbol(b"WebAuthNAuthenticatorGetAssertion\0");
    let free_assertion = symbol(b"WebAuthNFreeAssertion\0");
    if get_version.is_null() || get_assertion.is_null() || free_assertion.is_null() {
        return None;
    }
    // SAFETY: Signaturen nach webauthn.h; die DLL bleibt bis Prozessende geladen.
    unsafe {
        let version = std::mem::transmute::<*mut c_void, FnGetApiVersion>(get_version)();
        let optional = |p: *mut c_void| (!p.is_null()).then_some(p);
        Some(Api {
            version,
            get_assertion: std::mem::transmute::<*mut c_void, FnGetAssertion>(get_assertion),
            free_assertion: std::mem::transmute::<*mut c_void, FnFreeAssertion>(free_assertion),
            get_cancellation_id: optional(symbol(b"WebAuthNGetCancellationId\0"))
                .map(|p| std::mem::transmute::<*mut c_void, FnGetCancellationId>(p)),
            cancel: optional(symbol(b"WebAuthNCancelCurrentOperation\0"))
                .map(|p| std::mem::transmute::<*mut c_void, FnCancel>(p)),
            error_name: optional(symbol(b"WebAuthNGetErrorName\0"))
                .map(|p| std::mem::transmute::<*mut c_void, FnGetErrorName>(p)),
        })
    }
}

#[cfg(not(windows))]
fn load() -> Option<Api> {
    None
}

#[cfg(windows)]
fn foreground_window() -> Hwnd {
    // SAFETY: parameterlos, liefert ggf. NULL.
    unsafe { sys::GetForegroundWindow() }
}

#[cfg(not(windows))]
fn foreground_window() -> Hwnd {
    std::ptr::null_mut()
}

/// UTF-16 mit abschließendem NUL.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Liest einen NUL-terminierten UTF-16-String (höchstens 1024 Zeichen).
///
/// # Safety
/// `p` ist NULL oder zeigt auf einen gültigen, NUL-terminierten String.
unsafe fn from_wide(p: Pcwstr) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: laut Vertrag NUL-terminiert; harte Obergrenze gegen Fehlbelegung.
    while len < 1024 && unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` Elemente sind gelesen worden.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}

/// Kopiert einen Ergebnis-Puffer.
///
/// # Safety
/// `p` ist NULL oder zeigt auf mindestens `len` lesbare Bytes.
unsafe fn copy(p: *const u8, len: Dword) -> Vec<u8> {
    if p.is_null() || len == 0 {
        return Vec::new();
    }
    // SAFETY: laut Vertrag gültig für `len` Bytes.
    unsafe { std::slice::from_raw_parts(p, len as usize) }.to_vec()
}

fn uv_requirement(uv: UserVerification) -> Dword {
    match uv {
        UserVerification::Required => UV_REQUIRED,
        UserVerification::Preferred => UV_PREFERRED,
        UserVerification::Discouraged => UV_DISCOURAGED,
    }
}

fn map_hresult(api: &Api, hr: Hresult) -> Fido2Error {
    match hr {
        NTE_USER_CANCELLED | HRESULT_ERROR_CANCELLED => Fido2Error::Cancelled,
        NTE_NOT_FOUND => Fido2Error::Device("Kein passender Sicherheitsschlüssel für ID Austria gefunden".into()),
        NTE_TOKEN_TIMEOUT => Fido2Error::Device("Zeitüberschreitung am Sicherheitsschlüssel".into()),
        _ => {
            let name = api.error_name.map(|f| {
                // SAFETY: liefert einen statischen, NUL-terminierten String der DLL.
                unsafe { from_wide(f(hr)) }
            });
            Fido2Error::Device(format!("{} (0x{:08x})", name.unwrap_or_default(), hr as u32))
        }
    }
}

/// FIDO2 über `webauthn.dll`.
#[derive(Debug, Default)]
pub struct WebAuthnProvider {
    current: Mutex<Option<Guid>>,
}

impl WebAuthnProvider {
    /// Neuer Provider (lädt die DLL beim ersten Gebrauch).
    pub fn new() -> Self {
        Self::default()
    }

    /// API-Version der DLL (`None` ohne `webauthn.dll`).
    pub fn api_version() -> Option<u32> {
        api().map(|a| a.version)
    }
}

impl Fido2Provider for WebAuthnProvider {
    fn is_available(&self) -> bool {
        api().is_some_and(|a| a.version >= 1)
    }

    fn get_assertion(
        &self,
        request: &Fido2Request,
        parent: Option<WindowHandle>,
    ) -> Result<Fido2Assertion, Fido2Error> {
        let api = api().ok_or(Fido2Error::Unavailable)?;
        let options = AssertionOptions::parse(&request.options_json, &request.origin)?;
        let client_data = client_data_json(&options.challenge, &request.origin);
        let rp_id = wide(&options.rp_id);
        let hash_alg = wide("SHA-256");
        let public_key = wide("public-key");
        let credentials: Vec<Credential> = options
            .allow_credentials
            .iter()
            .map(|c| Credential {
                dw_version: CREDENTIAL_VERSION,
                cb_id: c.id.len() as Dword,
                pb_id: c.id.as_ptr(),
                pwsz_credential_type: public_key.as_ptr(),
            })
            .collect();

        let mut guid = Guid::default();
        let cancellable = api.get_cancellation_id.is_some_and(|f| {
            // SAFETY: `guid` ist ein gültiger Zielpuffer.
            unsafe { f(&mut guid) >= 0 }
        });
        if cancellable {
            *self.current.lock().expect("poisoned") = Some(guid);
        }
        let assertion_options = GetAssertionOptions {
            dw_version: if cancellable { GET_ASSERTION_OPTIONS_VERSION_3 } else { GET_ASSERTION_OPTIONS_VERSION_1 },
            dw_timeout_milliseconds: options.timeout_ms.unwrap_or(120_000),
            credential_list: Credentials {
                c_credentials: credentials.len() as Dword,
                p_credentials: if credentials.is_empty() { std::ptr::null() } else { credentials.as_ptr() },
            },
            extensions: Extensions { c_extensions: 0, p_extensions: std::ptr::null() },
            dw_authenticator_attachment: ATTACHMENT_ANY,
            dw_user_verification_requirement: uv_requirement(options.user_verification),
            dw_flags: 0,
            pwsz_u2f_app_id: std::ptr::null(),
            pb_u2f_app_id: std::ptr::null_mut(),
            p_cancellation_id: if cancellable { &mut guid } else { std::ptr::null_mut() },
        };
        let cd = ClientData {
            dw_version: CLIENT_DATA_VERSION,
            cb_client_data_json: client_data.len() as Dword,
            pb_client_data_json: client_data.as_ptr(),
            pwsz_hash_alg_id: hash_alg.as_ptr(),
        };
        let hwnd = parent.map(|w| w.0 as usize as Hwnd).filter(|h| !h.is_null()).unwrap_or_else(foreground_window);
        let mut out: *mut Assertion = std::ptr::null_mut();
        // SAFETY: alle Zeiger zeigen auf lebende lokale Puffer (bis zum Ende dieser Funktion);
        // Strukturlayout nach webauthn.h; `out` wird von der DLL belegt.
        let hr = unsafe { (api.get_assertion)(hwnd, rp_id.as_ptr(), &cd, &assertion_options, &mut out) };
        *self.current.lock().expect("poisoned") = None;
        if hr < 0 || out.is_null() {
            return Err(map_hresult(api, hr));
        }
        // SAFETY: `out` ist eine gültige WEBAUTHN_ASSERTION der DLL; die Puffer werden vor dem
        // Freigeben kopiert.
        let result = unsafe {
            let a = &*out;
            let raw_id = copy(a.credential.pb_id, a.credential.cb_id);
            let kind = from_wide(a.credential.pwsz_credential_type);
            let assertion = Fido2Assertion {
                id: credential_id_string(&raw_id),
                raw_id,
                kind: if kind.is_empty() { "public-key".into() } else { kind },
                authenticator_data: copy(a.pb_authenticator_data, a.cb_authenticator_data),
                client_data_json: client_data.clone(),
                signature: copy(a.pb_signature, a.cb_signature),
                user_handle: (a.cb_user_id > 0).then(|| copy(a.pb_user_id, a.cb_user_id)),
            };
            (api.free_assertion)(out);
            assertion
        };
        Ok(result)
    }

    fn cancel(&self) {
        let Some(api) = api() else { return };
        let Some(cancel) = api.cancel else { return };
        if let Some(guid) = *self.current.lock().expect("poisoned") {
            // SAFETY: `guid` stammt aus WebAuthNGetCancellationId.
            let _ = unsafe { cancel(&guid) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_webauthn_h() {
        // 64-Bit-Layout (Windows x64/ARM64): Zeiger 8 Byte, DWORD 4 Byte, natürliche Ausrichtung.
        if std::mem::size_of::<usize>() == 8 {
            assert_eq!(std::mem::size_of::<ClientData>(), 24);
            assert_eq!(std::mem::size_of::<Credential>(), 24);
            assert_eq!(std::mem::size_of::<Credentials>(), 16);
            assert_eq!(std::mem::size_of::<Extensions>(), 16);
            assert_eq!(std::mem::size_of::<GetAssertionOptions>(), 80);
            assert_eq!(std::mem::offset_of!(GetAssertionOptions, credential_list), 8);
            assert_eq!(std::mem::offset_of!(GetAssertionOptions, dw_authenticator_attachment), 40);
            assert_eq!(std::mem::offset_of!(GetAssertionOptions, pwsz_u2f_app_id), 56);
            assert_eq!(std::mem::offset_of!(GetAssertionOptions, p_cancellation_id), 72);
            assert_eq!(std::mem::offset_of!(Assertion, pb_signature), 24);
            assert_eq!(std::mem::offset_of!(Assertion, credential), 32);
            assert_eq!(std::mem::offset_of!(Assertion, pb_user_id), 64);
            assert_eq!(std::mem::size_of::<Assertion>(), 72);
        }
        assert_eq!(std::mem::size_of::<Guid>(), 16);
    }

    #[test]
    fn helpers() {
        assert_eq!(wide("ab"), vec![97, 98, 0]);
        let w = wide("SHA-256");
        // SAFETY: `w` ist NUL-terminiert.
        assert_eq!(unsafe { from_wide(w.as_ptr()) }, "SHA-256");
        // SAFETY: NULL ist erlaubt.
        assert_eq!(unsafe { from_wide(std::ptr::null()) }, "");
        // SAFETY: NULL ist erlaubt.
        assert!(unsafe { copy(std::ptr::null(), 5) }.is_empty());
        assert_eq!(uv_requirement(UserVerification::Required), 1);
        assert_eq!(uv_requirement(UserVerification::Discouraged), 3);
        assert_eq!(NTE_USER_CANCELLED as u32, 0x8009_0036);
    }

    #[test]
    fn availability_matches_platform() {
        let p = WebAuthnProvider::new();
        if cfg!(windows) {
            // auf Windows-Runnern ohne webauthn.dll (Server Core) darf es auch `false` sein
            let _ = p.is_available();
        } else {
            assert!(!p.is_available());
            let r = p.get_assertion(
                &Fido2Request { options_json: "{}".into(), origin: crate::WEBAUTHN_ORIGIN.into() },
                None,
            );
            assert_eq!(r.unwrap_err(), Fido2Error::Unavailable);
            p.cancel();
        }
    }
}

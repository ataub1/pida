//! macOS-Transport über den IOKit-HID-Manager.
//!
//! Begründung für `unsafe` (ADR 0001 §4 Regel 8): FIDO-HID-Geräte sind unter macOS nur über die
//! C-APIs von IOKit/CoreFoundation erreichbar (die AuthenticationServices-API scheidet aus, weil sie
//! Associated Domains für `a-trust.at` verlangt, docs/analyse/02 §7.2). Das `unsafe` ist auf diese
//! Datei beschränkt: Geräte suchen (Usage-Page `0xF1D0`), öffnen, Output-Reports senden,
//! Input-Reports über einen Callback in der Run-Loop des aufrufenden Threads empfangen. Alle
//! CF-Objekte werden mit `CFRetain`/`CFRelease` verwaltet, Puffer und Warteschlange liegen in
//! eigenen Heap-Blöcken mit stabiler Adresse, die erst nach dem Abmelden des Callbacks freigegeben
//! werden.
#![allow(unsafe_code)]

use std::collections::VecDeque;
use std::ffi::{c_char, c_void};
use std::time::{Duration, Instant};

use super::{FIDO_USAGE_CTAPHID, FIDO_USAGE_PAGE, HidDeviceInfo};
use crate::fido2::Fido2Error;
use crate::fido2::ctap::{HidConnection, REPORT_LEN};

type CfTypeRef = *const c_void;
type CfStringRef = *const c_void;
type CfMutableDictionaryRef = *mut c_void;
type CfSetRef = *const c_void;
type CfRunLoopRef = *mut c_void;
type CfIndex = isize;
type IoHidManagerRef = *mut c_void;
type IoHidDeviceRef = *mut c_void;
type IoReturn = i32;

type ReportCallback = unsafe extern "C" fn(
    context: *mut c_void,
    result: IoReturn,
    sender: *mut c_void,
    report_type: u32,
    report_id: u32,
    report: *mut u8,
    report_length: CfIndex,
);

/// Nur als Adresse verwendete CF-Konstanten.
#[repr(C)]
struct Opaque {
    _private: [u8; 0],
}

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const CF_NUMBER_SINT32: CfIndex = 3;
const CF_NUMBER_SINT64: CfIndex = 4;
const IOHID_REPORT_TYPE_OUTPUT: u32 = 1;
const IOHID_OPTIONS_NONE: u32 = 0;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: CfStringRef;
    static kCFTypeDictionaryKeyCallBacks: Opaque;
    static kCFTypeDictionaryValueCallBacks: Opaque;
    fn CFDictionaryCreateMutable(
        allocator: *const c_void,
        capacity: CfIndex,
        key_callbacks: *const Opaque,
        value_callbacks: *const Opaque,
    ) -> CfMutableDictionaryRef;
    fn CFDictionarySetValue(dict: CfMutableDictionaryRef, key: *const c_void, value: *const c_void);
    fn CFNumberCreate(allocator: *const c_void, the_type: CfIndex, value: *const c_void) -> CfTypeRef;
    fn CFNumberGetValue(number: CfTypeRef, the_type: CfIndex, value: *mut c_void) -> u8;
    fn CFNumberGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFGetTypeID(cf: CfTypeRef) -> usize;
    fn CFStringCreateWithCString(allocator: *const c_void, c_str: *const c_char, encoding: u32) -> CfStringRef;
    fn CFStringGetCString(s: CfStringRef, buffer: *mut c_char, size: CfIndex, encoding: u32) -> u8;
    fn CFSetGetCount(set: CfSetRef) -> CfIndex;
    fn CFSetGetValues(set: CfSetRef, values: *mut *const c_void);
    fn CFRetain(cf: CfTypeRef) -> CfTypeRef;
    fn CFRelease(cf: CfTypeRef);
    fn CFRunLoopGetCurrent() -> CfRunLoopRef;
    fn CFRunLoopRunInMode(mode: CfStringRef, seconds: f64, return_after_source_handled: u8) -> i32;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDManagerCreate(allocator: *const c_void, options: u32) -> IoHidManagerRef;
    fn IOHIDManagerSetDeviceMatching(manager: IoHidManagerRef, matching: *const c_void);
    fn IOHIDManagerCopyDevices(manager: IoHidManagerRef) -> CfSetRef;
    fn IOHIDDeviceGetProperty(device: IoHidDeviceRef, key: CfStringRef) -> CfTypeRef;
    fn IOHIDDeviceOpen(device: IoHidDeviceRef, options: u32) -> IoReturn;
    fn IOHIDDeviceClose(device: IoHidDeviceRef, options: u32) -> IoReturn;
    fn IOHIDDeviceSetReport(
        device: IoHidDeviceRef,
        report_type: u32,
        report_id: CfIndex,
        report: *const u8,
        report_length: CfIndex,
    ) -> IoReturn;
    fn IOHIDDeviceRegisterInputReportCallback(
        device: IoHidDeviceRef,
        report: *mut u8,
        report_length: CfIndex,
        callback: Option<ReportCallback>,
        context: *mut c_void,
    );
    fn IOHIDDeviceScheduleWithRunLoop(device: IoHidDeviceRef, run_loop: CfRunLoopRef, mode: CfStringRef);
    fn IOHIDDeviceUnscheduleFromRunLoop(device: IoHidDeviceRef, run_loop: CfRunLoopRef, mode: CfStringRef);
}

/// Eigene CF-Referenz (gibt beim Drop frei).
struct Cf(CfTypeRef);

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: eigene (+1) Referenz.
            unsafe { CFRelease(self.0) };
        }
    }
}

fn cf_string(s: &str) -> Cf {
    let c = std::ffi::CString::new(s).expect("ohne NUL");
    // SAFETY: gültiger C-String; NULL-Allocator = Standard.
    Cf(unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), CF_STRING_ENCODING_UTF8) })
}

fn cf_i32(v: i32) -> Cf {
    // SAFETY: Zeiger auf einen lokalen i32.
    Cf(unsafe { CFNumberCreate(std::ptr::null(), CF_NUMBER_SINT32, (&v as *const i32).cast()) })
}

/// Matching-Dictionary `{DeviceUsagePage: 0xF1D0, DeviceUsage: 1}`.
fn matching() -> Cf {
    // SAFETY: Callbacks sind die CF-Standardtypen; Schlüssel/Werte werden vom Dictionary retained.
    unsafe {
        let dict = CFDictionaryCreateMutable(
            std::ptr::null(),
            2,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        if dict.is_null() {
            return Cf(std::ptr::null());
        }
        let page_key = cf_string("DeviceUsagePage");
        let usage_key = cf_string("DeviceUsage");
        let page = cf_i32(FIDO_USAGE_PAGE as i32);
        let usage = cf_i32(FIDO_USAGE_CTAPHID as i32);
        CFDictionarySetValue(dict, page_key.0, page.0);
        CFDictionarySetValue(dict, usage_key.0, usage.0);
        Cf(dict as CfTypeRef)
    }
}

/// Liest eine Eigenschaft als String bzw. Zahl.
fn property_string(device: IoHidDeviceRef, key: &str) -> Option<String> {
    let key = cf_string(key);
    // SAFETY: `device` ist gültig; „Get“-Regel: keine eigene Referenz.
    unsafe {
        let value = IOHIDDeviceGetProperty(device, key.0);
        if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
            return None;
        }
        let mut buf = [0 as c_char; 256];
        (CFStringGetCString(value, buf.as_mut_ptr(), buf.len() as CfIndex, CF_STRING_ENCODING_UTF8) != 0)
            .then(|| std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

fn property_i64(device: IoHidDeviceRef, key: &str) -> Option<i64> {
    let key = cf_string(key);
    // SAFETY: wie oben; Zielpuffer ist ein lokaler i64.
    unsafe {
        let value = IOHIDDeviceGetProperty(device, key.0);
        if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
            return None;
        }
        let mut out: i64 = 0;
        (CFNumberGetValue(value, CF_NUMBER_SINT64, (&mut out as *mut i64).cast()) != 0).then_some(out)
    }
}

/// Kennung eines Geräts für [`HidDeviceInfo::path`] (Location-ID + Produkt-ID).
fn device_path(device: IoHidDeviceRef) -> String {
    format!(
        "iokit:{:x}:{:x}:{:x}",
        property_i64(device, "LocationID").unwrap_or(0),
        property_i64(device, "VendorID").unwrap_or(0),
        property_i64(device, "ProductID").unwrap_or(0)
    )
}

/// Ruft `f` mit allen FIDO-Geräten auf (Referenzen gelten nur während des Aufrufs).
fn with_devices<T>(f: impl FnOnce(IoHidManagerRef, &[IoHidDeviceRef]) -> T) -> T {
    // SAFETY: Manager und Set werden nach `f` freigegeben; Geräte nur während `f` benutzt.
    unsafe {
        let manager = IOHIDManagerCreate(std::ptr::null(), IOHID_OPTIONS_NONE);
        if manager.is_null() {
            return f(std::ptr::null_mut(), &[]);
        }
        let manager = Cf(manager as CfTypeRef);
        let matching = matching();
        IOHIDManagerSetDeviceMatching(manager.0 as IoHidManagerRef, matching.0);
        let set = Cf(IOHIDManagerCopyDevices(manager.0 as IoHidManagerRef));
        let mut devices: Vec<IoHidDeviceRef> = Vec::new();
        if !set.0.is_null() {
            let n = CFSetGetCount(set.0).max(0) as usize;
            let mut values: Vec<*const c_void> = vec![std::ptr::null(); n];
            CFSetGetValues(set.0, values.as_mut_ptr());
            devices = values.into_iter().map(|v| v as IoHidDeviceRef).collect();
        }
        f(manager.0 as IoHidManagerRef, &devices)
    }
}

/// FIDO-Geräte über IOKit.
pub fn enumerate() -> Vec<HidDeviceInfo> {
    let mut found = with_devices(|_, devices| {
        devices
            .iter()
            .map(|&d| HidDeviceInfo { path: device_path(d), product: property_string(d, "Product") })
            .collect::<Vec<_>>()
    });
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// Geöffnetes IOKit-HID-Gerät, gebunden an die Run-Loop des öffnenden Threads.
#[derive(Debug)]
pub struct IokitConnection {
    manager: CfTypeRef,
    device: IoHidDeviceRef,
    run_loop: CfRunLoopRef,
    buffer: *mut [u8; REPORT_LEN],
    queue: *mut VecDeque<[u8; REPORT_LEN]>,
}

unsafe extern "C" fn on_input_report(
    context: *mut c_void,
    _result: IoReturn,
    _sender: *mut c_void,
    _report_type: u32,
    _report_id: u32,
    report: *mut u8,
    report_length: CfIndex,
) {
    if context.is_null() || report.is_null() || report_length <= 0 {
        return;
    }
    // SAFETY: `context` ist die Warteschlange dieser Verbindung (eigener Heap-Block, lebt bis nach
    // dem Abmelden des Callbacks); `report` zeigt auf `report_length` Bytes im separaten Puffer.
    unsafe {
        let queue = &mut *(context as *mut VecDeque<[u8; REPORT_LEN]>);
        let data = std::slice::from_raw_parts(report, report_length as usize);
        let mut out = [0u8; REPORT_LEN];
        let n = data.len().min(REPORT_LEN);
        out[..n].copy_from_slice(&data[..n]);
        if queue.len() < 256 {
            queue.push_back(out);
        }
    }
}

/// Öffnet das Gerät mit dieser Kennung.
pub fn open(info: &HidDeviceInfo) -> Result<IokitConnection, Fido2Error> {
    with_devices(|manager, devices| {
        let device = devices
            .iter()
            .copied()
            .find(|&d| device_path(d) == info.path)
            .ok_or_else(|| Fido2Error::Device("Sicherheitsschlüssel nicht mehr angeschlossen".into()))?;
        // SAFETY: `device`/`manager` sind gültig; eigene Referenzen per CFRetain, Puffer auf dem
        // Heap mit stabiler Adresse; Registrierung und Planung in der aktuellen Run-Loop.
        unsafe {
            let result = IOHIDDeviceOpen(device, IOHID_OPTIONS_NONE);
            if result != 0 {
                return Err(Fido2Error::Device(format!(
                    "Sicherheitsschlüssel lässt sich nicht öffnen (IOReturn 0x{:08x})",
                    result as u32
                )));
            }
            CFRetain(device as CfTypeRef);
            CFRetain(manager as CfTypeRef);
            let buffer = Box::into_raw(Box::new([0u8; REPORT_LEN]));
            let queue = Box::into_raw(Box::new(VecDeque::<[u8; REPORT_LEN]>::new()));
            IOHIDDeviceRegisterInputReportCallback(
                device,
                buffer.cast(),
                REPORT_LEN as CfIndex,
                Some(on_input_report),
                queue.cast(),
            );
            let run_loop = CFRunLoopGetCurrent();
            IOHIDDeviceScheduleWithRunLoop(device, run_loop, kCFRunLoopDefaultMode);
            Ok(IokitConnection { manager: manager as CfTypeRef, device, run_loop, buffer, queue })
        }
    })
}

impl HidConnection for IokitConnection {
    fn write_report(&mut self, report: &[u8; REPORT_LEN]) -> std::io::Result<()> {
        // SAFETY: Gerät offen, Puffer gültig für 64 Bytes.
        let result = unsafe {
            IOHIDDeviceSetReport(self.device, IOHID_REPORT_TYPE_OUTPUT, 0, report.as_ptr(), REPORT_LEN as CfIndex)
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::other(format!("IOHIDDeviceSetReport 0x{:08x}", result as u32)))
        }
    }

    fn read_report(&mut self, timeout: Duration) -> std::io::Result<Option<[u8; REPORT_LEN]>> {
        let deadline = Instant::now() + timeout;
        loop {
            // SAFETY: der Callback läuft nur innerhalb von CFRunLoopRunInMode auf diesem Thread;
            // hier besteht keine gleichzeitige Referenz.
            if let Some(report) = unsafe { (*self.queue).pop_front() } {
                return Ok(Some(report));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let slice = (deadline - now).min(Duration::from_millis(50)).as_secs_f64();
            // SAFETY: Standard-Modus der aktuellen Run-Loop (dieselbe, in der das Gerät geplant ist).
            unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, slice, 1) };
        }
    }
}

impl Drop for IokitConnection {
    fn drop(&mut self) {
        // SAFETY: Reihenfolge: Callback abmelden, aus der Run-Loop nehmen, schließen, Referenzen
        // freigeben – erst danach die Heap-Blöcke.
        unsafe {
            IOHIDDeviceRegisterInputReportCallback(
                self.device,
                self.buffer.cast(),
                REPORT_LEN as CfIndex,
                None,
                std::ptr::null_mut(),
            );
            IOHIDDeviceUnscheduleFromRunLoop(self.device, self.run_loop, kCFRunLoopDefaultMode);
            IOHIDDeviceClose(self.device, IOHID_OPTIONS_NONE);
            CFRelease(self.device as CfTypeRef);
            CFRelease(self.manager);
            drop(Box::from_raw(self.queue));
            drop(Box::from_raw(self.buffer));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumeration_does_not_crash() {
        // Auf Testrechnern ist meist kein Schlüssel angeschlossen; jedes Ergebnis ist zulässig,
        // aber die Pfade müssen eindeutig und stabil sein.
        let a = enumerate();
        let b = enumerate();
        assert_eq!(a, b);
        assert!(a.iter().all(|d| d.path.starts_with("iokit:")));
    }

    #[test]
    fn matching_dictionary_is_created() {
        let m = matching();
        assert!(!m.0.is_null());
        let s = cf_string("x");
        assert!(!s.0.is_null());
    }

    #[test]
    fn open_unknown_device_fails_cleanly() {
        let e = open(&HidDeviceInfo { path: "iokit:0:0:0-gibt-es-nicht".into(), product: None }).unwrap_err();
        assert!(matches!(e, Fido2Error::Device(_)));
    }
}

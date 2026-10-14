//! FIDO2 über USB-HID (CTAP2/CTAPHID) für macOS und Linux – Feature `fido2-hid`.
//!
//! Eigene Implementierung statt `ctap-hid-fido2`/libfido2 (keine neuen Abhängigkeiten, keine
//! Zusatzinstallation): Geräte mit HID-Usage-Page `0xF1D0` (FIDO Alliance), Usage `0x01`.
//! - Linux: `/sys/class/hidraw/*/device/report_descriptor` auswerten, `/dev/hidrawN` lesen/schreiben
//!   (ohne `unsafe`; braucht Zugriffsrechte über udev/`uaccess`, unter Flatpak `--device=all`).
//! - macOS: IOKit-HID-Manager ([`macos`], FFI mit lokalem `allow(unsafe_code)`).
//!
//! Protokoll und Abbruch liegen in [`super::ctap`]. Verlangt der Schlüssel eine PIN, kommt ein
//! klarer Hinweis (PIN-Protokoll ist ein offener Punkt). Mehrere Schlüssel: der erste gefundene
//! wird verwendet.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pdfover_backend_api::WindowHandle;

use super::ctap::{CtapHidDevice, HidConnection, get_assertion, random_nonce};
use super::{AssertionOptions, Fido2Assertion, Fido2Error, Fido2Provider, Fido2Request};

#[cfg(unix)]
pub mod hidraw;
#[cfg(target_os = "macos")]
pub mod macos;

/// FIDO-Usage-Page.
pub const FIDO_USAGE_PAGE: u32 = 0xF1D0;
/// FIDO-Usage „CTAPHID“.
pub const FIDO_USAGE_CTAPHID: u32 = 0x01;

/// Prüft einen HID-Report-Deskriptor auf Usage-Page `0xF1D0` mit Usage `0x01`
/// (kurze Items nach HID 1.11 §6.2.2.2; lange Items werden übersprungen).
pub fn is_fido_report_descriptor(descriptor: &[u8]) -> bool {
    let mut usage_page: u32 = 0;
    let mut i = 0;
    while i < descriptor.len() {
        let prefix = descriptor[i];
        if prefix == 0xFE {
            // langes Item: 0xFE, bDataSize, bLongItemTag, Daten
            let size = *descriptor.get(i + 1).unwrap_or(&0) as usize;
            i += 3 + size;
            continue;
        }
        let size = match prefix & 0x03 {
            3 => 4,
            n => n as usize,
        };
        let Some(data) = descriptor.get(i + 1..i + 1 + size) else { return false };
        let value = data.iter().rev().fold(0u32, |acc, b| (acc << 8) | *b as u32);
        let item_type = (prefix >> 2) & 0x03;
        let tag = prefix >> 4;
        match (item_type, tag) {
            (1, 0) => usage_page = value, // Global: Usage Page
            (2, 0) => {
                // Local: Usage (4 Byte = erweiterte Usage mit Page im oberen Wort)
                let (page, usage) = if size == 4 { (value >> 16, value & 0xFFFF) } else { (usage_page, value) };
                if page == FIDO_USAGE_PAGE && usage == FIDO_USAGE_CTAPHID {
                    return true;
                }
            }
            _ => {}
        }
        i += 1 + size;
    }
    false
}

/// Ein gefundener FIDO-Sicherheitsschlüssel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDeviceInfo {
    /// Plattformpfad (`/dev/hidraw3`, IOKit-Registry-ID …)
    pub path: String,
    /// Produktname, falls bekannt
    pub product: Option<String>,
}

/// Alle angeschlossenen FIDO-HID-Geräte.
pub fn enumerate() -> Vec<HidDeviceInfo> {
    #[cfg(target_os = "macos")]
    {
        macos::enumerate()
    }
    #[cfg(target_os = "linux")]
    {
        hidraw::enumerate()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

/// Öffnet ein Gerät.
fn open(device: &HidDeviceInfo) -> Result<Box<dyn HidConnection>, Fido2Error> {
    #[cfg(target_os = "macos")]
    {
        macos::open(device).map(|c| Box::new(c) as Box<dyn HidConnection>)
    }
    #[cfg(target_os = "linux")]
    {
        hidraw::open(device).map(|c| Box::new(c) as Box<dyn HidConnection>)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = device;
        Err(Fido2Error::Unavailable)
    }
}

impl HidConnection for Box<dyn HidConnection> {
    fn write_report(&mut self, report: &[u8; super::ctap::REPORT_LEN]) -> std::io::Result<()> {
        (**self).write_report(report)
    }

    fn read_report(&mut self, timeout: std::time::Duration) -> std::io::Result<Option<[u8; super::ctap::REPORT_LEN]>> {
        (**self).read_report(timeout)
    }
}

/// CTAP2-HID-Provider (macOS/Linux).
#[derive(Debug, Default)]
pub struct HidFido2Provider {
    cancel: Arc<AtomicBool>,
}

impl HidFido2Provider {
    /// Neuer Provider.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Fido2Provider for HidFido2Provider {
    fn is_available(&self) -> bool {
        !enumerate().is_empty()
    }

    fn get_assertion(
        &self,
        request: &Fido2Request,
        _parent: Option<WindowHandle>,
    ) -> Result<Fido2Assertion, Fido2Error> {
        self.cancel.store(false, Ordering::SeqCst);
        let options = AssertionOptions::parse(&request.options_json, &request.origin)?;
        let device = enumerate().into_iter().next().ok_or(Fido2Error::Unavailable)?;
        tracing::debug!(product = ?device.product, "FIDO2-HID-Gerät");
        let connection = open(&device)?;
        let mut ctap = CtapHidDevice::init(connection, random_nonce())?;
        get_assertion(&mut ctap, &options, &request.origin, &self.cancel)
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Report-Deskriptor eines typischen FIDO-Schlüssels (Usage Page 0xF1D0, Usage 1, 64-Byte-Reports).
    const FIDO_DESCRIPTOR: &[u8] = &[
        0x06, 0xD0, 0xF1, // Usage Page (FIDO Alliance)
        0x09, 0x01, // Usage (CTAPHID)
        0xA1, 0x01, // Collection (Application)
        0x09, 0x20, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x40, 0x81, 0x02, // Input
        0x09, 0x21, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x40, 0x91, 0x02, // Output
        0xC0, // End Collection
    ];

    #[test]
    fn descriptor_detection() {
        assert!(is_fido_report_descriptor(FIDO_DESCRIPTOR));
        // Tastatur (Generic Desktop, Usage 6)
        assert!(!is_fido_report_descriptor(&[0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0xC0]));
        // Usage 1 auf anderer Page
        assert!(!is_fido_report_descriptor(&[0x05, 0x01, 0x09, 0x01]));
        // erweiterte 4-Byte-Usage
        assert!(is_fido_report_descriptor(&[0x0B, 0x01, 0x00, 0xD0, 0xF1]));
        // langes Item wird übersprungen, abgeschnittener Deskriptor ist kein FIDO-Gerät
        assert!(is_fido_report_descriptor(&[0xFE, 0x02, 0x10, 0xAA, 0xBB, 0x06, 0xD0, 0xF1, 0x09, 0x01]));
        assert!(!is_fido_report_descriptor(&[0x06, 0xD0]));
        assert!(!is_fido_report_descriptor(&[]));
    }

    #[test]
    fn provider_without_devices_is_unavailable() {
        let p = HidFido2Provider::new();
        if enumerate().is_empty() {
            assert!(!p.is_available());
            let r = p.get_assertion(
                &Fido2Request { options_json: r#"{"challenge":"AQ"}"#.into(), origin: crate::WEBAUTHN_ORIGIN.into() },
                None,
            );
            assert_eq!(r.unwrap_err(), Fido2Error::Unavailable);
        }
        p.cancel();
    }
}

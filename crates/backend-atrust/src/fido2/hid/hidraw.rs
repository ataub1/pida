//! Linux-`hidraw`-Transport ohne `unsafe`: Deskriptor aus sysfs, Reports über `/dev/hidrawN`
//! (nicht blockierend geöffnet, Lesen mit kurzem Polling). Wird auf allen Unix-Systemen übersetzt
//! (Tests), aber nur unter Linux verwendet.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{HidDeviceInfo, is_fido_report_descriptor};
use crate::fido2::Fido2Error;
use crate::fido2::ctap::{HidConnection, REPORT_LEN};

/// `O_NONBLOCK` der Zielplattform.
#[cfg(all(target_os = "linux", any(target_arch = "mips", target_arch = "mips64")))]
const O_NONBLOCK: i32 = 0o200;
#[cfg(all(target_os = "linux", any(target_arch = "sparc", target_arch = "sparc64")))]
const O_NONBLOCK: i32 = 0x4000;
#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "mips", target_arch = "mips64", target_arch = "sparc", target_arch = "sparc64"))
))]
const O_NONBLOCK: i32 = 0o4000;
#[cfg(not(target_os = "linux"))]
const O_NONBLOCK: i32 = 0x0004;

/// Durchsucht `sysfs_root` (`/sys/class/hidraw`) nach FIDO-Geräten.
pub fn enumerate_in(sysfs_root: &Path, dev_root: &Path) -> Vec<HidDeviceInfo> {
    let Ok(entries) = std::fs::read_dir(sysfs_root) else { return Vec::new() };
    let mut found: Vec<HidDeviceInfo> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let descriptor = std::fs::read(entry.path().join("device/report_descriptor")).ok()?;
            if !is_fido_report_descriptor(&descriptor) {
                return None;
            }
            let product = std::fs::read_to_string(entry.path().join("device/uevent"))
                .ok()
                .and_then(|u| u.lines().find_map(|l| l.strip_prefix("HID_NAME=").map(str::to_owned)));
            Some(HidDeviceInfo { path: dev_root.join(&name).to_string_lossy().into_owned(), product })
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// FIDO-Geräte unter `/sys/class/hidraw`.
pub fn enumerate() -> Vec<HidDeviceInfo> {
    enumerate_in(Path::new("/sys/class/hidraw"), Path::new("/dev"))
}

/// Geöffnetes `hidraw`-Gerät.
#[derive(Debug)]
pub struct HidrawConnection {
    file: File,
    path: PathBuf,
}

/// Öffnet `/dev/hidrawN` lesend und schreibend (nicht blockierend).
pub fn open(device: &HidDeviceInfo) -> Result<HidrawConnection, Fido2Error> {
    let path = PathBuf::from(&device.path);
    let file = OpenOptions::new().read(true).write(true).custom_flags(O_NONBLOCK).open(&path).map_err(|e| {
        if e.kind() == ErrorKind::PermissionDenied {
            Fido2Error::Device(format!(
                "kein Zugriff auf {} – udev-Regel (uaccess) für FIDO-Sicherheitsschlüssel fehlt",
                path.display()
            ))
        } else {
            Fido2Error::Device(format!("{}: {e}", path.display()))
        }
    })?;
    Ok(HidrawConnection { file, path })
}

impl HidConnection for HidrawConnection {
    fn write_report(&mut self, report: &[u8; REPORT_LEN]) -> std::io::Result<()> {
        // Report-ID 0 voranstellen (FIDO-Geräte nutzen keine Report-IDs)
        let mut buf = [0u8; REPORT_LEN + 1];
        buf[1..].copy_from_slice(report);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.file.write(&buf) {
                Ok(n) if n == buf.len() => return Ok(()),
                Ok(n) => {
                    return Err(std::io::Error::other(format!("nur {n} Bytes an {} geschrieben", self.path.display())));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn read_report(&mut self, timeout: Duration) -> std::io::Result<Option<[u8; REPORT_LEN]>> {
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; REPORT_LEN + 1];
        loop {
            match self.file.read(&mut buf) {
                Ok(n) if n == REPORT_LEN => {
                    let mut report = [0u8; REPORT_LEN];
                    report.copy_from_slice(&buf[..REPORT_LEN]);
                    return Ok(Some(report));
                }
                Ok(n) if n == REPORT_LEN + 1 => {
                    // mit vorangestellter Report-ID
                    let mut report = [0u8; REPORT_LEN];
                    report.copy_from_slice(&buf[1..]);
                    return Ok(Some(report));
                }
                Ok(0) => return Err(std::io::Error::new(ErrorKind::UnexpectedEof, "Gerät getrennt")),
                Ok(_) => continue, // unvollständiger Report – verwerfen
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => {
                    if Instant::now() >= deadline {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysfs_enumeration() {
        let root = std::env::temp_dir().join(format!("pdfover-hidraw-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mk = |name: &str, descriptor: &[u8], uevent: &str| {
            let dir = root.join(name).join("device");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("report_descriptor"), descriptor).unwrap();
            std::fs::write(dir.join("uevent"), uevent).unwrap();
        };
        mk("hidraw0", &[0x05, 0x01, 0x09, 0x06], "HID_NAME=Tastatur\n");
        mk("hidraw3", &[0x06, 0xD0, 0xF1, 0x09, 0x01], "HID_ID=0003:00001050:00000407\nHID_NAME=Test Security Key\n");
        mk("hidraw1", &[0x06, 0xD0, 0xF1, 0x09, 0x01], "");
        let found = enumerate_in(&root, Path::new("/dev"));
        assert_eq!(
            found,
            [
                HidDeviceInfo { path: "/dev/hidraw1".into(), product: None },
                HidDeviceInfo { path: "/dev/hidraw3".into(), product: Some("Test Security Key".into()) },
            ]
        );
        assert!(enumerate_in(&root.join("fehlt"), Path::new("/dev")).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn open_reports_missing_device() {
        let e = open(&HidDeviceInfo { path: "/dev/pdfover-gibt-es-nicht".into(), product: None }).unwrap_err();
        assert!(matches!(e, Fido2Error::Device(_)));
    }
}

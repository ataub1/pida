// PDF-Over NG – sicherer Speicher der iOS-Seite (Keychain), EUPL-1.2.
//
// Werte liegen als `kSecClassGenericPassword` unter dem Dienst `<Bundle-ID>.secrets`, Konto = Schlüssel
// der Rust-Seite (`PDF-Over NG/<SecretId::account>`). Zugriff nur bei entsperrtem Gerät und nur auf diesem
// Gerät (`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`): kein iCloud-Schlüsselbund, kein Backup auf ein
// anderes Gerät. Werte, Schlüssel und Statuscodes erscheinen nie in Fehlermeldungen oder Logs.
//
// Keychain-Einträge überleben das Löschen der App. Beim ersten Start nach einer (Neu-)Installation – die
// Markierung in `UserDefaults` fehlt dann – werden deshalb alle Einträge des Dienstes entfernt, damit
// keine Passwörter einer früheren Installation zurückkommen ([`SecureStore.purgeAfterReinstall`]).

import Foundation
import Security

/// Fehler der nativen Seite mit kurzem Code für `invoke.reject(_, code:)` – ohne Pfade, Schlüssel oder
/// Werte (die Rust-Seite übernimmt nur den Code, `Error::from_native_code`).
struct NativeError: Error {
  let code: String

  init(code: String) {
    self.code = code
  }

  /// Keychain-Status → Code.
  init(status: OSStatus) {
    switch status {
    case errSecInteractionNotAllowed:
      code = "keychain-locked"
    case errSecMissingEntitlement, errSecNotAvailable:
      code = "keychain-unavailable"
    default:
      code = "keychain-error"
    }
  }
}

enum SecureStore {
  /// Markierung in `UserDefaults`: Keychain dieser Installation bereits bereinigt.
  private static let installedMarker = "at.pdf-over-ng.secure-store.installed"

  static var service: String {
    (Bundle.main.bundleIdentifier ?? "at.pdf-over-ng") + ".secrets"
  }

  private static func query(_ key: String) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: service,
      kSecAttrAccount as String: key,
      kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
    ]
  }

  /// Wert zu `key`; `nil` = nicht vorhanden.
  static func get(_ key: String) throws -> String? {
    var q = query(key)
    q[kSecReturnData as String] = kCFBooleanTrue
    q[kSecMatchLimit as String] = kSecMatchLimitOne
    var out: CFTypeRef?
    let status = SecItemCopyMatching(q as CFDictionary, &out)
    switch status {
    case errSecSuccess:
      guard let data = out as? Data, let value = String(data: data, encoding: .utf8) else {
        throw NativeError(code: "keychain-bad-data")
      }
      return value
    case errSecItemNotFound:
      return nil
    default:
      throw NativeError(status: status)
    }
  }

  /// Schreibt bzw. ersetzt den Wert zu `key`.
  static func set(_ key: String, _ value: String) throws {
    let data = Data(value.utf8)
    let attributes: [String: Any] = [
      kSecValueData as String: data,
      kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
    ]
    var status = SecItemUpdate(query(key) as CFDictionary, attributes as CFDictionary)
    if status == errSecItemNotFound {
      var add = query(key)
      add.merge(attributes) { _, new in new }
      status = SecItemAdd(add as CFDictionary, nil)
    }
    guard status == errSecSuccess else { throw NativeError(status: status) }
  }

  /// Löscht den Wert zu `key`; ein fehlender Eintrag ist kein Fehler.
  static func delete(_ key: String) throws {
    let status = SecItemDelete(query(key) as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw NativeError(status: status)
    }
  }

  /// Erster Start nach der Installation: Einträge einer früheren Installation entfernen.
  static func purgeAfterReinstall() {
    let defaults = UserDefaults.standard
    guard !defaults.bool(forKey: installedMarker) else { return }
    let all: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: service,
    ]
    let status = SecItemDelete(all as CFDictionary)
    // gesperrtes Gerät (Start im Hintergrund): beim nächsten Start erneut versuchen
    if status == errSecSuccess || status == errSecItemNotFound {
      defaults.set(true, forKey: installedMarker)
    }
  }
}

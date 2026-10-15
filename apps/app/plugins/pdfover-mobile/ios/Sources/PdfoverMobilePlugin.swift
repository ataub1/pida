// PDF-Over NG – native iOS-Seite des Plugins `pdfover-mobile` (EUPL-1.2).
//
// LEERE HÜLLE (Welle Mobile, Paket M0): Jede Methode lehnt mit dem Code `not-implemented` ab; die
// Rust-Seite fällt dann auf sichere Rückfälle zurück (Secrets nur im RAM, kein Teilen/Exportieren).
// Paket M-iOS füllt die Methoden; Vertrag (Argumente, Antworten, Fehlercodes) siehe
// `apps/app/plugins/pdfover-mobile/src/lib.rs` und docs/integration/mobile.md.
//
// Regeln: keine Pfade, Schlüssel oder Werte in Fehlermeldungen oder Logs; Werte des sicheren Speichers
// nur in der Keychain (kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly, kein iCloud-Sync);
// Kopien eingehender Dokumente nur in den übergebenen Zielordner (`destination`).

import Tauri
import UIKit
import WebKit

struct KeyArgs: Decodable {
  let key: String
}

struct SetArgs: Decodable {
  let key: String
  let value: String
}

struct ShareArgs: Decodable {
  let path: String
  let mimeType: String
}

struct ExportArgs: Decodable {
  let path: String
  let fileName: String
  let mimeType: String
}

struct ImportArgs: Decodable {
  let kind: String
  let multiple: Bool
  let destination: String
}

struct PendingArgs: Decodable {
  let urls: [String]
  let destination: String
}

class PdfoverMobilePlugin: Plugin {
  private func notImplemented(_ invoke: Invoke) {
    invoke.reject("pdfover-mobile: native Seite noch nicht umgesetzt (M-iOS)", code: "not-implemented")
  }

  /// `{ key }` → `{ value: string | null }` (Keychain, kSecClassGenericPassword)
  @objc public func secureStoreGet(_ invoke: Invoke) {
    notImplemented(invoke)
  }

  /// `{ key, value }` → `{}`
  @objc public func secureStoreSet(_ invoke: Invoke) {
    notImplemented(invoke)
  }

  /// `{ key }` → `{}` (fehlender Eintrag ist kein Fehler)
  @objc public func secureStoreDelete(_ invoke: Invoke) {
    notImplemented(invoke)
  }

  /// `{ path, mimeType }` → `{}` (UIActivityViewController)
  @objc public func shareFile(_ invoke: Invoke) {
    notImplemented(invoke)
  }

  /// `{ path, fileName, mimeType }` → `{ saved: bool }` (UIDocumentPickerViewController forExporting)
  @objc public func exportFile(_ invoke: Invoke) {
    notImplemented(invoke)
  }

  /// `{ kind: "pdf" | "pkcs12", multiple, destination }` → `{ files: [{ name, path }] }`
  @objc public func importFile(_ invoke: Invoke) {
    notImplemented(invoke)
  }

  /// `{ urls, destination }` → `{ files: [{ name, path }] }` („Öffnen in“/Share, Security-Scoped URLs)
  @objc public func takePendingOpenDocuments(_ invoke: Invoke) {
    notImplemented(invoke)
  }
}

@_cdecl("init_plugin_pdfover_mobile")
func initPlugin() -> Plugin {
  return PdfoverMobilePlugin()
}

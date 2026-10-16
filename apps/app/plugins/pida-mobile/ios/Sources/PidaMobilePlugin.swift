// PIDA – native iOS-Seite des Plugins `pida-mobile` (EUPL-1.2), Paket M-iOS.
//
// Vertrag (Argumente, Antworten, Fehlercodes): `apps/app/plugins/pida-mobile/src/lib.rs` und
// docs/integration/mobile.md §2; iOS-Details in docs/integration/mobile-ios.md.
//
// - `secureStore*`: Keychain (SecureStore.swift, `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`).
// - `shareFile`: `UIActivityViewController`; `openFile`: QuickLook (`QLPreviewController`, Ergebnis
//   „Öffnen“ – `opener` öffnet auf iOS keine Pfade); `exportFile`: `UIDocumentPickerViewController(forExporting:)`;
//   `importFile`: `UIDocumentPickerViewController(forOpeningContentTypes:asCopy:)` mit Kopie nach
//   `destination` (Presentation.swift, SandboxFiles.swift).
// - `takePendingOpenDocuments`: „Öffnen in“/Teilen/Dateien-App inkl. Security-Scoped URLs
//   (OpenedDocuments.swift – Hooks vor den Delegates von tao).
// - Schriftgröße: Dynamic Type als Grundschrift der Oberfläche (TextSize.swift).
// - Lebenszyklus: kommt ohne Zutun dieses Plugins bei Rust an (tao: `sceneWillResignActive` bzw.
//   `applicationWillResignActive` → `WindowEvent::Suspended`, `sceneWillEnterForeground` bzw.
//   `applicationWillEnterForeground` → `WindowEvent::Resumed`; die Shell macht daraus
//   `UiEvent::Backgrounded`/`Foregrounded`). Ein eigener Weg würde die Ereignisse doppeln.
//
// Die Methoden laufen auf der seriellen IPC-Warteschlange von Tauri (nicht auf dem Hauptthread); UIKit nur
// über `DispatchQueue.main`, Dateikopien auf einer eigenen Warteschlange, damit der sichere Speicher nicht
// hinter einer langen Kopie (iCloud-Download) wartet. Regeln: keine Pfade, Schlüssel oder Werte in
// Fehlermeldungen oder Logs; Kopien nur in den übergebenen Zielordner.

import QuickLook
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

/// `{ value: string | null }` (auch `null` ausdrücklich, wie im Vertrag).
struct ValueResponse: Encodable {
  let value: String?

  enum CodingKeys: String, CodingKey { case value }

  func encode(to encoder: Encoder) throws {
    var c = encoder.container(keyedBy: CodingKeys.self)
    try c.encode(value, forKey: .value)
  }
}

struct EmptyResponse: Encodable {}

/// `{ saved, name? }` – `name`: tatsächlicher Dateiname am Ziel (der Picker nummeriert bei „Beide behalten“)
struct ExportResponse: Encodable {
  let saved: Bool
  let name: String?
}

class PidaMobilePlugin: Plugin {
  /// Laufender Dokument-Picker (nur Hauptthread) – höchstens einer zugleich.
  private var activePicker: PickerSession?
  /// Laufende QuickLook-Vorschau (nur Hauptthread; `dataSource` hält QuickLook nur schwach).
  private var activePreview: PreviewSession?
  /// Dateikopien (eingehende Dokumente, Import).
  private let io = DispatchQueue(label: "at.pida.mobile.io", qos: .userInitiated)

  override init() {
    super.init()
    // vor `UIApplicationMain` (Tauri lädt die Plugins beim Bau der App): Hooks für eingehende Dokumente
    // stehen, bevor iOS die erste Szene verbindet bzw. `application:openURL:options:` ruft
    OpenURLHooks.install()
    SecureStore.purgeAfterReinstall()
    SandboxFiles.excludeImportedKeystoresFromBackup()
  }

  /// WebView angelegt: Systemschriftgröße (Dynamic Type) übernehmen (TextSize.swift).
  override func load(webview: WKWebView) {
    TextSize.attach(to: webview)
  }

  // MARK: Hilfen

  private func reject(_ invoke: Invoke, _ code: String) {
    invoke.reject("pida-mobile: \(code)", code: code)
  }

  private func reject(_ invoke: Invoke, error: Error) {
    reject(invoke, (error as? NativeError)?.code ?? "native")
  }

  private func args<T: Decodable>(_ invoke: Invoke, _ type: T.Type) -> T? {
    do {
      return try invoke.parseArgs(type)
    } catch {
      reject(invoke, "bad-args")
      return nil
    }
  }

  /// Datei der App (innerhalb der Sandbox), die geteilt bzw. exportiert werden darf.
  private func ownFile(_ path: String) -> URL? {
    let url = URL(fileURLWithPath: path, isDirectory: false)
    var isDir: ObjCBool = false
    guard path.hasPrefix("/"), FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir), !isDir.boolValue,
      SandboxFiles.isInside(url, SandboxFiles.containerRoot)
    else { return nil }
    return url
  }

  /// Zeigt `controller` über der WebView; `false`, wenn es nichts zum Darüberlegen gibt.
  private func present(_ controller: UIViewController, completion: (() -> Void)? = nil) -> Bool {
    guard let presenter = Presentation.presenter(from: manager.viewController) else { return false }
    Presentation.anchor(controller, in: presenter)
    presenter.present(controller, animated: true, completion: completion)
    return true
  }

  // MARK: Sicherer Speicher

  /// `{ key }` → `{ value: string | null }`
  @objc public func secureStoreGet(_ invoke: Invoke) {
    guard let a = args(invoke, KeyArgs.self) else { return }
    do {
      invoke.resolve(ValueResponse(value: try SecureStore.get(a.key)))
    } catch {
      reject(invoke, error: error)
    }
  }

  /// `{ key, value }` → `{}`
  @objc public func secureStoreSet(_ invoke: Invoke) {
    guard let a = args(invoke, SetArgs.self) else { return }
    do {
      try SecureStore.set(a.key, a.value)
      invoke.resolve(EmptyResponse())
    } catch {
      reject(invoke, error: error)
    }
  }

  /// `{ key }` → `{}` (fehlender Eintrag ist kein Fehler)
  @objc public func secureStoreDelete(_ invoke: Invoke) {
    guard let a = args(invoke, KeyArgs.self) else { return }
    do {
      try SecureStore.delete(a.key)
      invoke.resolve(EmptyResponse())
    } catch {
      reject(invoke, error: error)
    }
  }

  // MARK: Teilen, Exportieren, Importieren

  /// `{ path, mimeType }` → `{}`, sobald das Share-Sheet gezeigt wird (UIActivityViewController)
  @objc public func shareFile(_ invoke: Invoke) {
    guard let a = args(invoke, ShareArgs.self) else { return }
    guard let url = ownFile(a.path) else { return reject(invoke, "file-missing") }
    DispatchQueue.main.async {
      let sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
      if !self.present(sheet, completion: { invoke.resolve(EmptyResponse()) }) {
        self.reject(invoke, "no-view")
      }
    }
  }

  /// `{ path, mimeType }` → `{}`, sobald die Vorschau gezeigt wird (QuickLook; darin auch Teilen/Drucken)
  @objc public func openFile(_ invoke: Invoke) {
    guard let a = args(invoke, ShareArgs.self) else { return }
    guard let url = ownFile(a.path) else { return reject(invoke, "file-missing") }
    DispatchQueue.main.async {
      guard QLPreviewController.canPreview(url as NSURL) else { return self.reject(invoke, "no-preview") }
      let session = PreviewSession(url: url) { self.activePreview = nil }
      let preview = QLPreviewController()
      preview.dataSource = session
      preview.delegate = session
      self.activePreview = session
      if !self.present(preview, completion: { invoke.resolve(EmptyResponse()) }) {
        self.activePreview = nil
        self.reject(invoke, "no-view")
      }
    }
  }

  /// `{ path, fileName, mimeType }` → `{ saved, name? }` (UIDocumentPickerViewController forExporting, als Kopie);
  /// Abbruch → Code `cancelled`
  @objc public func exportFile(_ invoke: Invoke) {
    guard let a = args(invoke, ExportArgs.self) else { return }
    guard let source = ownFile(a.path) else { return reject(invoke, "file-missing") }
    // Der Picker übernimmt den Dateinamen der Quelle: bei abweichendem Namen eine benannte Kopie
    let name = SandboxFiles.safeName(a.fileName, fallback: source.lastPathComponent)
    var staging: URL?
    var exported = source
    if name != source.lastPathComponent {
      let dir = FileManager.default.temporaryDirectory.appendingPathComponent("export-\(UUID().uuidString)", isDirectory: true)
      do {
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        exported = dir.appendingPathComponent(name)
        try FileManager.default.copyItem(at: source, to: exported)
        staging = dir
      } catch {
        try? FileManager.default.removeItem(at: dir)
        return reject(invoke, "io-error")
      }
    }
    let cleanup = { if let dir = staging { try? FileManager.default.removeItem(at: dir) } }
    DispatchQueue.main.async {
      guard self.activePicker == nil else {
        cleanup()
        return self.reject(invoke, "busy")
      }
      let picker = UIDocumentPickerViewController(forExporting: [exported], asCopy: true)
      let session = PickerSession(
        onPick: { urls in
          self.activePicker = nil
          cleanup()
          invoke.resolve(ExportResponse(saved: !urls.isEmpty, name: urls.first?.lastPathComponent))
        },
        onCancel: {
          self.activePicker = nil
          cleanup()
          self.reject(invoke, "cancelled")
        })
      picker.delegate = session
      picker.presentationController?.delegate = session
      picker.shouldShowFileExtensions = true
      self.activePicker = session
      if !self.present(picker) {
        self.activePicker = nil
        cleanup()
        self.reject(invoke, "no-view")
      }
    }
  }

  /// `{ kind: "pdf" | "pkcs12", multiple, destination }` → `{ files: [{ name, path }] }`; Abbruch → `cancelled`
  @objc public func importFile(_ invoke: Invoke) {
    guard let a = args(invoke, ImportArgs.self) else { return }
    guard let types = Presentation.contentTypes(kind: a.kind) else { return reject(invoke, "bad-args") }
    let destination: URL
    do {
      destination = try SandboxFiles.destination(a.destination)
    } catch {
      return reject(invoke, error: error)
    }
    DispatchQueue.main.async {
      guard self.activePicker == nil else { return self.reject(invoke, "busy") }
      // asCopy: iOS legt eine eigene Kopie in tmp ab (kein Security Scope nötig); sie wird nach dem
      // Übernehmen gelöscht
      let picker = UIDocumentPickerViewController(forOpeningContentTypes: types, asCopy: true)
      picker.allowsMultipleSelection = a.multiple
      picker.shouldShowFileExtensions = true
      let session = PickerSession(
        onPick: { urls in
          self.activePicker = nil
          self.io.async {
            var files: [CopiedFile] = []
            let tmp = FileManager.default.temporaryDirectory
            for url in urls {
              if let file = try? SandboxFiles.copyIn(url, to: destination) { files.append(file) }
              if SandboxFiles.isInside(url, tmp) { try? FileManager.default.removeItem(at: url) }
            }
            invoke.resolve(FilesResponse(files: files))
          }
        },
        onCancel: {
          self.activePicker = nil
          self.reject(invoke, "cancelled")
        })
      picker.delegate = session
      picker.presentationController?.delegate = session
      self.activePicker = session
      if !self.present(picker) {
        self.activePicker = nil
        self.reject(invoke, "no-view")
      }
    }
  }

  // MARK: Eingehende Dokumente

  /// `{ urls, destination }` → `{ files: [{ name, path }] }` („Öffnen in“/Teilen, Security-Scoped URLs)
  @objc public func takePendingOpenDocuments(_ invoke: Invoke) {
    guard let a = args(invoke, PendingArgs.self) else { return }
    let destination: URL
    do {
      destination = try SandboxFiles.destination(a.destination)
    } catch {
      return reject(invoke, error: error)
    }
    io.async {
      invoke.resolve(FilesResponse(files: OpenedDocuments.shared.take(requested: a.urls, destination: destination)))
    }
  }
}

@_cdecl("init_plugin_pida_mobile")
func initPlugin() -> Plugin {
  return PidaMobilePlugin()
}

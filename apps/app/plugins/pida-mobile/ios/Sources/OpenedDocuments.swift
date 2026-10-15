// PIDA – eingehende Dokumente („Öffnen in“, Teilen, Dateien-App) auf iOS, EUPL-1.2.
//
// Die App erklärt `LSSupportsOpeningDocumentsInPlace` (nötig, damit `Documents` in der Dateien-App
// erscheint). iOS übergibt dann die **Original-URL** mit einer Sandbox-Erweiterung; lesbar ist sie nur
// über genau dieses `URL`-Objekt (`startAccessingSecurityScopedResource`). tao (Tauri) wandelt die URL in
// einen String für `RunEvent::Opened` – die Erweiterung geht dabei verloren, und beim Kaltstart im
// Scene-Lebenszyklus meldet tao 0.37 die URLs aus den Verbindungsoptionen gar nicht.
//
// Deshalb hängt sich dieses Modul **vor** die Delegates von tao (`TaoSceneDelegate`, `AppDelegate`):
// Es merkt sich die URL-Objekte und ruft danach die ursprüngliche Methode auf (tao bleibt unverändert).
//
// - Kaltstart (`scene:willConnectToSession:options:`): tao meldet nichts → die URLs liefert
//   `takePendingOpenDocuments` beim ersten Aufruf von selbst („auto“).
// - Laufende App (`scene:openURLContexts:`, `application:openURL:options:`): tao meldet die URL als String;
//   kann die Rust-Seite sie nicht lesen, reicht sie sie an `takePendingOpenDocuments` weiter, und dieses
//   Modul öffnet sie über das gemerkte Objekt. Liest die Rust-Seite die Datei selbst (Simulator, Kopien in
//   `Documents/Inbox`), verfällt der Eintrag nach [`OpenedDocuments.retention`].
//
// Fehlt eine der Klassen bzw. Methoden (andere tao-Version), bleibt es beim Verhalten ohne Hook; im Log
// steht dann „pida-mobile: hook … nicht installiert“.

import Foundation
import ObjectiveC
import UIKit

final class OpenedDocuments {
  static let shared = OpenedDocuments()

  /// Wie lange eine von tao gemeldete URL auf die Anfrage der Rust-Seite wartet.
  static let retention: TimeInterval = 10 * 60

  private struct Entry {
    let url: URL
    /// tao meldet diese URL nicht (Kaltstart) → von selbst liefern
    let auto: Bool
    let at: Date
  }

  private let lock = NSLock()
  private var entries: [Entry] = []

  /// Vergleichsschlüssel: Pfad nach Standardisierung und Auflösen von Symlinks (unabhängig von der
  /// Prozentkodierung des Strings, den tao bzw. `url::Url` erzeugen).
  static func key(_ url: URL) -> String? {
    guard url.isFileURL else { return nil }
    return url.standardizedFileURL.resolvingSymlinksInPath().path
  }

  func capture(_ urls: [URL], auto: Bool) {
    let fileURLs = urls.filter { $0.isFileURL }
    guard !fileURLs.isEmpty else { return }
    lock.lock()
    defer { lock.unlock() }
    let now = Date()
    for url in fileURLs {
      entries.append(Entry(url: url, auto: auto, at: now))
    }
  }

  /// Kopien für `takePendingOpenDocuments`: alle „auto“-Einträge und die angefragten URLs (über das
  /// gemerkte Objekt, sonst die URL aus dem String); danach ist die Warteschlange leer bzw. bereinigt.
  func take(requested: [String], destination: URL) -> [CopiedFile] {
    let now = Date()
    var sources: [URL] = []
    lock.lock()
    var seen = Set<String>()
    var keep: [Entry] = []
    let requestedKeys = requested.compactMap { URL(string: $0) }.compactMap { OpenedDocuments.key($0) }
    var unresolved = Set(requestedKeys)
    for entry in entries {
      guard let key = OpenedDocuments.key(entry.url) else { continue }
      if entry.auto || unresolved.contains(key) {
        if seen.insert(key).inserted { sources.append(entry.url) }
        unresolved.remove(key)
      } else if now.timeIntervalSince(entry.at) < OpenedDocuments.retention {
        keep.append(entry)
      }
    }
    entries = keep
    lock.unlock()
    // angefragt, aber nie über einen Hook gesehen: die URL aus dem String versuchen (ohne Erweiterung
    // nur lesbar, wenn sie ohnehin zugänglich ist)
    for string in requested {
      guard let url = URL(string: string), let key = OpenedDocuments.key(url), unresolved.contains(key),
        seen.insert(key).inserted
      else { continue }
      sources.append(url)
    }

    var files: [CopiedFile] = []
    for source in sources {
      do {
        files.append(try SandboxFiles.copyIn(source, to: destination))
        // eigene Kopie von iOS (AirDrop, Mail): nach dem Übernehmen entfernen
        if SandboxFiles.isInside(source, SandboxFiles.documentsInbox) {
          try? FileManager.default.removeItem(at: source)
        }
      } catch {
        NSLog("pida-mobile: eingehendes Dokument nicht übernommen (%@)", (error as? NativeError)?.code ?? "io-error")
      }
    }
    SandboxFiles.purgeInbox(olderThan: OpenedDocuments.retention)
    return files
  }
}

/// Hooks vor den Delegates von tao (siehe Moduldoku).
enum OpenURLHooks {
  private static var installed = false

  /// Einmalig beim Laden des Plugins (Hauptthread, vor `UIApplicationMain` – tao registriert seine
  /// Klassen beim Anlegen der Ereignisschleife, Tauri initialisiert die Plugins danach).
  static func install() {
    guard !installed else { return }
    installed = true
    hookSceneWillConnect()
    hookSceneOpenURLContexts()
    hookApplicationOpenURL()
  }

  /// Methode, die `cls` selbst implementiert (nicht nur erbt) – sonst würde der Hook die Oberklasse ändern.
  private static func ownMethod(_ cls: AnyClass, _ sel: Selector) -> Method? {
    var count: UInt32 = 0
    guard let list = class_copyMethodList(cls, &count) else { return nil }
    defer { free(list) }
    for i in 0..<Int(count) where method_getName(list[i]) == sel {
      return list[i]
    }
    return nil
  }

  private static func lookup(_ className: String, _ selector: String) -> (Method, Selector)? {
    let sel = NSSelectorFromString(selector)
    guard let cls = NSClassFromString(className), let method = ownMethod(cls, sel) else {
      NSLog("pida-mobile: hook %@ %@ nicht installiert", className, selector)
      return nil
    }
    return (method, sel)
  }

  private static func hookSceneWillConnect() {
    guard let (method, sel) = lookup("TaoSceneDelegate", "scene:willConnectToSession:options:") else { return }
    typealias Fn = @convention(c) (AnyObject, Selector, UIScene, UISceneSession, UIScene.ConnectionOptions) -> Void
    let original = unsafeBitCast(method_getImplementation(method), to: Fn.self)
    let block: @convention(block) (AnyObject, UIScene, UISceneSession, UIScene.ConnectionOptions) -> Void = {
      this, scene, session, options in
      OpenedDocuments.shared.capture(options.urlContexts.map { $0.url }, auto: true)
      original(this, sel, scene, session, options)
    }
    method_setImplementation(method, imp_implementationWithBlock(block))
  }

  private static func hookSceneOpenURLContexts() {
    guard let (method, sel) = lookup("TaoSceneDelegate", "scene:openURLContexts:") else { return }
    typealias Fn = @convention(c) (AnyObject, Selector, UIScene, NSSet) -> Void
    let original = unsafeBitCast(method_getImplementation(method), to: Fn.self)
    let block: @convention(block) (AnyObject, UIScene, NSSet) -> Void = { this, scene, contexts in
      OpenedDocuments.shared.capture(contexts.compactMap { ($0 as? UIOpenURLContext)?.url }, auto: false)
      original(this, sel, scene, contexts)
    }
    method_setImplementation(method, imp_implementationWithBlock(block))
  }

  private static func hookApplicationOpenURL() {
    guard let (method, sel) = lookup("AppDelegate", "application:openURL:options:") else { return }
    typealias Fn = @convention(c) (AnyObject, Selector, UIApplication, NSURL, NSDictionary) -> ObjCBool
    let original = unsafeBitCast(method_getImplementation(method), to: Fn.self)
    let block: @convention(block) (AnyObject, UIApplication, NSURL, NSDictionary) -> ObjCBool = {
      this, app, url, options in
      OpenedDocuments.shared.capture([url as URL], auto: false)
      return original(this, sel, app, url, options)
    }
    method_setImplementation(method, imp_implementationWithBlock(block))
  }
}

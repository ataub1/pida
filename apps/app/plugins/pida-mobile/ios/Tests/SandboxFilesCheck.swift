// PIDA – Prüfung der reinen Dateilogik der iOS-Seite auf dem Mac (ohne Simulator), EUPL-1.2.
//
// Übersetzt zusammen mit `Sources/SandboxFiles.swift` und `Sources/SecureStore.swift` (nur Foundation/Security);
// läuft über `cargo test -p tauri-plugin-pida-mobile --test ios_logic` (macOS mit Xcode, sonst übersprungen).
// Mehrfachauswahl (ADR 0004 Nachtrag 17): `copyAll` kopiert alle Quellen in ihrer Reihenfolge, gleiche Namen in
// verschiedene Unterordner (nichts überschreibt sich), nicht kopierbare zählen als `skipped`.

import Foundation

@main
struct SandboxFilesCheck {
  static func check(_ ok: Bool, _ what: String) {
    if !ok {
      FileHandle.standardError.write("FEHLER: \(what)\n".data(using: .utf8)!)
      exit(1)
    }
  }

  static func main() throws {
    let fm = FileManager.default
    let root = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
    let source = root.appendingPathComponent("quelle", isDirectory: true)
    let dest = root.appendingPathComponent("import", isDirectory: true)
    try fm.createDirectory(at: source, withIntermediateDirectories: true)
    try fm.createDirectory(at: dest, withIntermediateDirectories: true)
    func file(_ sub: String, _ name: String, _ text: String) throws -> URL {
      let dir = source.appendingPathComponent(sub, isDirectory: true)
      try fm.createDirectory(at: dir, withIntermediateDirectories: true)
      let url = dir.appendingPathComponent(name)
      try text.data(using: .utf8)!.write(to: url)
      return url
    }
    let vertrag1 = try file("a", "Vertrag.pdf", "%PDF a")
    let fehlt = source.appendingPathComponent("b/weg.pdf")
    let ordner = source.appendingPathComponent("c", isDirectory: true)
    try fm.createDirectory(at: ordner, withIntermediateDirectories: true)
    let vertrag2 = try file("d", "Vertrag.pdf", "%PDF d")
    let angebot = try file("e", "Angebot.pdf", "%PDF e")
    var cleaned: [String] = []
    let result = SandboxFiles.copyAll([vertrag1, fehlt, ordner, vertrag2, angebot], to: dest) { cleaned.append($0.lastPathComponent) }
    let rel = result.files.map { URL(fileURLWithPath: $0.path).pathComponents.suffix(2).joined(separator: "/") }
    // erster freier Unterordner je Name (`uniqueTarget`): gleiche Namen nebeneinander, nichts überschreibt sich
    check(rel == ["1/Vertrag.pdf", "2/Vertrag.pdf", "1/Angebot.pdf"], "Reihenfolge und eigene Unterordner: \(rel)")
    check(result.files.map(\.name) == ["Vertrag.pdf", "Vertrag.pdf", "Angebot.pdf"], "Namen: \(result.files.map(\.name))")
    check(result.skipped == 2, "fehlende Datei und Ordner nicht übernommen: \(result.skipped)")
    check(cleaned == ["Vertrag.pdf", "weg.pdf", "c", "Vertrag.pdf", "Angebot.pdf"], "Aufräumen je Quelle: \(cleaned)")
    let texts = try result.files.map { try String(contentsOfFile: $0.path, encoding: .utf8) }
    check(texts == ["%PDF a", "%PDF d", "%PDF e"], "Inhalte: \(texts)")
    let json = String(data: try JSONEncoder().encode(result), encoding: .utf8) ?? ""
    check(json.contains("\"skipped\":2") && json.contains("\"files\""), "Antwort an die Rust-Seite: \(json)")
    let empty = String(data: try JSONEncoder().encode(FilesResponse(files: [])), encoding: .utf8) ?? ""
    check(empty.contains("\"skipped\":0"), "ohne Fehler skipped = 0: \(empty)")
    print("SandboxFilesCheck ok")
  }
}

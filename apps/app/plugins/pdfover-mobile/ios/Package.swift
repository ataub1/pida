// swift-tools-version:5.3
// PDF-Over NG – native iOS-Seite des Plugins `pdfover-mobile` (EUPL-1.2).
// Die Tauri-API (`.tauri/tauri-api`) kopiert das Build-Skript von `tauri-plugin` hierher.
//
// Mindestversion des Pakets iOS 14 (UTType, Dokument-Picker mit Inhaltstypen); die App selbst verlangt
// iOS 16 (tauri.conf.json `bundle.iOS.minimumSystemVersion`, gen/apple/project.yml).

import PackageDescription

let package = Package(
  name: "tauri-plugin-pdfover-mobile",
  platforms: [
    .macOS(.v10_13),
    .iOS(.v14),
  ],
  products: [
    .library(
      name: "tauri-plugin-pdfover-mobile",
      type: .static,
      targets: ["tauri-plugin-pdfover-mobile"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-pdfover-mobile",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)

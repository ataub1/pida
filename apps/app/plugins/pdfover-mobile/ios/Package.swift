// swift-tools-version:5.3
// PDF-Over NG – native iOS-Seite des Plugins `pdfover-mobile` (EUPL-1.2).
// Die Tauri-API (`.tauri/tauri-api`) kopiert das Build-Skript von `tauri-plugin` hierher.

import PackageDescription

let package = Package(
  name: "tauri-plugin-pdfover-mobile",
  platforms: [
    .macOS(.v10_13),
    .iOS(.v13),
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

# PDF-Over NG

Funktional gleichwertiger Klon von [PDF-Over](https://technology.a-sit.at/pdf-over/) (A-SIT, EUPL-1.2)
**ohne Java**: ein UI-unabhängiger Rust-Kern (PAdES-Engine, Signatur-Backends, Workflow) mit einer dünnen
Tauri-2-Shell für Windows, macOS, Linux, iOS und Android. Arbeitstitel „PDF-Over NG“, Lizenz
**EUPL-1.2**.

ID Austria läuft über den nativen A-Trust-Ablauf (Szenario A, freigegeben); lokale BKU und PKCS#12-Keystore
sind weitere Signiermittel. Details und Begründungen: [Architektur- und Umsetzungsplan](docs/00-architektur-und-umsetzungsplan.md)
und [ADR 0001](docs/adr/0001-workspace-und-schnittstellen.md).

## Stand

Fundament: Workspace, verbindliche Schnittstellen aller Crates (Stubs mit `todo!()`), Testdaten, CI.
Die Umsetzung erfolgt in sechs parallelen Paketen (A–F) und der anschließenden Integration
(Ownership und Regeln: ADR 0001 §4).

## Struktur

```
crates/
  java-properties   Java-Properties-Parser/-Writer (PDF-Over.config, PDF-AS-Profile, messages*.properties)
  i18n              Fluent-Kataloge de/en
  pdf-cos           lopdf-Fassade: SourcePdf, Revisionen, Verschlüsselung, Seiten, Felder
  pdf-incremental   Append-only-Writer, ByteRange/Contents-Platzhalter
  cades             CMS/CAdES, Zertifikate, Verifier
  sigblock          PDF-AS-Profile, Layout, Fonts, Emblem, Appearance
  placement         Positionierung, Auto-Position, QR-Platzhalter, Signaturfelder
  render            Renderer-Trait (PDFium)
  backend-api       Backend-Traits, Capabilities, InteractionBroker
  sl12              Security Layer 1.2 (XML)
  net               HTTP-Client (Cookies, Redirects, Proxy, OS-Truststore)
  backend-atrust    ID Austria / A-Trust (Szenario A)
  backend-localbku  lokale BKU (127.0.0.1:3495)
  backend-keystore  PKCS#12
  pades             Analyse, Prepared (Zweiphasenmodell), Invarianten I1–I6
  workflow          Reducer (State/Event/Effect), Ausgaberegeln
  config            Settings (TOML), Legacy-Import, Overlay, Richtlinien, Secrets
  core-api          Fassade für Shell und CLI (dispatch/subscribe)
apps/
  cli               pdfover-cli (kompatibel zu den PDF-Over-Argumenten)
  app               Tauri-2-Shell (folgt in der Integration)
docs/               Plan, Analysen, Recherchen, ADRs
testdata/           gemeinsame Testdaten (Test-Keystores NUR TEST, PDF-AS-PDFs, qpdf-Varianten)
tools/              env.sh, make-testdata.sh
reference/          Originalquellen (nur lesen, nicht im Git)
```

## Bauen

```sh
source tools/env.sh            # rustup (keg-only) und .venv in den PATH
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p pdfover-cli -- --help
tools/make-testdata.sh         # Testdaten erzeugen/aktualisieren (openssl, qpdf)
```

Toolchain: Rust 1.99 stable, Edition 2024. Abhängigkeiten sind in `[workspace.dependencies]` gepinnt
(RustCrypto-Generation `cms` 0.2 / `der` 0.7, `lopdf` 0.45.x, `pdfium-render` 0.9.4).

## Lizenz

EUPL-1.2. Profile, Texte und Testdokumente stammen aus PDF-Over/PDF-AS (EUPL). Drittlizenzen siehe
`deny.toml` (Allowlist nach Plan §2.5).

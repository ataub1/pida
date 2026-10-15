# PIDA

**PIDA – PDF-Signatur mit ID Austria** (PDF · ID · Austria)

Funktional gleichwertiger Klon von [PDF-Over](https://technology.a-sit.at/pdf-over/) (A-SIT, EUPL-1.2)
**ohne Java**: ein UI-unabhängiger Rust-Kern (PAdES-Engine, Signatur-Backends, Workflow) mit einer dünnen
Tauri-2-Shell für Windows, macOS, Linux, iOS und Android. Name **PIDA**
(bis 15.10.2026 Arbeitstitel „PDF-Over NG“, [ADR 0002](docs/adr/0002-name-pida.md)), Lizenz
**EUPL-1.2**.

ID Austria läuft über den nativen A-Trust-Ablauf (Szenario A, freigegeben); lokale BKU und PKCS#12-Keystore
sind weitere Signiermittel. Details und Begründungen: [Architektur- und Umsetzungsplan](docs/00-architektur-und-umsetzungsplan.md)
und [ADR 0001](docs/adr/0001-workspace-und-schnittstellen.md).

## Stand

Welle 3 (Integration): Rust-Kern mit PAdES-Engine, Backends (ID Austria Szenario A, lokale BKU,
PKCS#12), Workflow und Kern-Fassade; Kommandozeile `pida-cli`; Desktop-App (Tauri 2, React) mit
denselben PDF-Over-Startargumenten, Bundles für macOS (Windows/Linux über die CI). Mobile folgt.
Abnahme der App: [docs/integration/app-abnahme.md](docs/integration/app-abnahme.md); Arbeitsweise,
CI und Lizenzen: [docs/entwicklung.md](docs/entwicklung.md). Ownership und Regeln: ADR 0001 §4,
verbindliche Präzisierungen aus dem Schnittstellen-Review: §8.

## Mitarbeit und Sicherheit

Beiträge sind willkommen – siehe [CONTRIBUTING.md](CONTRIBUTING.md). Sicherheitslücken bitte vertraulich melden,
wie in [SECURITY.md](SECURITY.md) beschrieben (nicht als öffentliches Issue).

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
  render            Renderer-Trait, PDFium-Implementierung hinter Feature `pdfium`
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
  cli               pida-cli (kompatibel zu den PDF-Over-Argumenten; Parser auch für die GUI)
  app               Desktop-App: Oberfläche (React 19, React Aria) + Tauri-2-Shell (src-tauri)
docs/               Plan, Analysen, Recherchen, ADRs
testdata/           gemeinsame Testdaten (Test-Keystores NUR TEST, PDF-AS-PDFs, qpdf-Varianten)
tools/              env.sh, fetch-pdfium.*, third-party-notices.sh, npm-licenses.mjs, make-testdata.sh …
reference/          Originalquellen (nur lesen, nicht im Git)
```

## Bauen

```sh
source tools/env.sh            # rustup (keg-only), .venv und PIDA_REFERENCE (auch in git worktrees)
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p pida-cli -- --help
tools/make-testdata.sh         # Testdaten erzeugen/aktualisieren (openssl, qpdf, python3)

cd apps/app && npm ci && npm run build && npm test        # Oberfläche
eval "$(tools/fetch-pdfium.sh --print-env)"               # PDFium (im Repo-Wurzelordner)
npm run notices && npm run bundle:mac:debug               # macOS-App mit PDFium und Drittlizenzen
```

Toolchain: Rust 1.99 stable, Edition 2024. Abhängigkeiten sind in `[workspace.dependencies]` gepinnt
(RustCrypto-Generation `cms` 0.2 / `der` 0.7, `lopdf` 0.45.x, `pdfium-render` 0.9.4).

## Lizenz

EUPL-1.2. Profile, Texte und Testdokumente stammen aus PDF-Over/PDF-AS (EUPL). Drittlizenzen: Allowlist
in `deny.toml` (Plan §2.5); `tools/third-party-notices.sh` erzeugt die THIRD-PARTY-NOTICES, die jedem
App-Bundle beiliegen.

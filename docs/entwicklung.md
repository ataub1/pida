# Entwicklung – PDF-Over NG

Praktische Anleitung für Setup, Tests, CI, Lizenzen und Releases. Verbindlich bleiben
[Plan](00-architektur-und-umsetzungsplan.md) und [ADR 0001](adr/0001-workspace-und-schnittstellen.md);
dieses Dokument beschreibt, *wie* man damit arbeitet. Stand: 15.10.2026 (Welle 2, Paket I4).

## 1. Schnellstart

```sh
source tools/env.sh                          # rustup, .venv (pyHanko), PDFOVER_REFERENCE – auch in Worktrees
eval "$(tools/fetch-pdfium.sh --print-env)"  # PDFium chromium/8100 laden, PDFOVER_PDFIUM_DIR setzen
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo run -p pdfover-cli -- --help
```

Unter Windows (PowerShell): `Invoke-Expression (tools\fetch-pdfium.ps1 -PrintEnv)`, danach dieselben
`cargo`-Befehle.

## 2. Werkzeuge und Umgebung

| Werkzeug | Version | Zweck | Bezug |
|---|---|---|---|
| Rust | **1.99.0** stable, Edition 2024 (`rust-version` im Workspace) | Bauen | macOS: `brew install rustup` (keg-only, `/opt/homebrew/opt/rustup/bin`), dann `rustup toolchain install 1.99.0 -c clippy,rustfmt`; sonst rustup.rs |
| PDFium | chromium/8100 (pdfium-render 0.9.4 = API 7881) | Renderer, Tests | `tools/fetch-pdfium.sh` / `.ps1` (§3) |
| qpdf | ≥ 11.7 (lokal 12) | Orakel `qpdf --check`, Testdaten | `brew install qpdf`, `apt install qpdf` |
| OpenSSL | ≥ 3.2 (`openssl cms -cades`; lokal 4) | CMS-Orakel | macOS: `brew install openssl@3` (das System-`openssl` ist LibreSSL) |
| pyHanko | 0.37.0 (+ cli 0.5.0, certvalidator 0.32.1), Pillow 12.3.0 | PAdES-/ICC-Orakel | `python3 -m venv .venv && .venv/bin/pip install -r tools/python-oracles.txt` |
| Java | 21 (nur Test-Orakel, nie im Produkt) | Differentialtests (Properties, Konfiguration, i18n, Keystore-Generator) | `brew install openjdk@21` bzw. Temurin 21 |
| xmllint | beliebig | SL-1.2-XML-Orakel | macOS vorhanden, `apt install libxml2-utils` |
| cargo-deny | **0.20.2** | Lizenz-/Lieferketten-Gate (§6) | `brew install cargo-deny` oder `cargo install --locked cargo-deny@0.20.2` |
| cargo-about | **0.9.2** | THIRD-PARTY-NOTICES (§6) | `brew install cargo-about` oder `cargo install --locked cargo-about@0.9.2` |
| Node | 22 (npm, Lockfile) | Tauri-Frontend `apps/app` | `brew install node@22` |
| Xcode | 27 | iOS-Ziele, ring für iOS | App Store |

**`tools/env.sh`** setzt `PATH` (rustup keg-only, `~/.cargo/bin`, `.venv/bin`), `PDFOVER_ROOT`,
`PDFOVER_MAIN` (Hauptcheckout, auch aus einem `git worktree`) und `PDFOVER_REFERENCE` (Referenzquellen,
nur lesen; Tests lesen nie daraus). Lehnt eine Sandbox `source tools/env.sh` ab, genügen die
gleichwertigen Exporte:

```sh
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:<hauptcheckout>/.venv/bin:$PATH"
export PDFOVER_REFERENCE="<hauptcheckout>/reference"
```

**Umgebungsvariablen der Tests** (alle optional; ohne Werkzeug/Variable überspringen sich Tests mit
einer Meldung auf stderr):

| Variable | Wirkung |
|---|---|
| `PDFOVER_PDFIUM_DIR` | Verzeichnis mit `libpdfium.dylib`/`.so` bzw. `pdfium.dll` (sonst PDFium-Tests übersprungen) |
| `PDFOVER_PYTHON` | Python mit pyHanko (sonst `python3` neben `pyhanko`, dann `python3` im PATH) |
| `PDFOVER_I18N_BLESS=1` | erzeugt `crates/i18n/locales/*/pdfover.ftl` neu (`--test catalogs`) |
| `PDFOVER_UPDATE_RESULTS=1`, `PDFOVER_E2E_OUT=<dir>` | `testdata/pdf-incremental/RESULTS.md` aktualisieren bzw. Ausgaben behalten |
| `PDFOVER_FUZZ_ITERS`, `PDFOVER_FUZZ_SEED`, `PDFOVER_FUZZ_FILE`, `PDFOVER_FUZZ_TRACE` | längere/gezielte Robustheitsläufe (pdf-cos, pdf-incremental) |
| `PDFOVER_TEST_KEYCHAIN=1` | Test gegen den echten Schlüsselbund (config) |

Grundregeln für Tests (ADR 0001 §4 Regel 6): kein Netzverkehr, keine echten Zugangsdaten, Testdaten
nur aus `testdata/` bzw. Crate-Assets.

## 3. PDFium

- `tools/pdfium.lock` hält Release (`chromium/8100`) und SHA-256 der 12 Archive von
  [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries): `mac-arm64`, `mac-x64`,
  `linux-x64`, `linux-arm64`, `win-x64`, `win-arm64`, `ios-device-arm64`, `ios-simulator-arm64`,
  `ios-simulator-x64`, `android-arm64`, `android-arm`, `android-x64`. Alle Hashes wurden am 15.10.2026
  gegen die Digests der GitHub-Release-API geprüft; `mac-arm64` ist bitgleich mit der bisher genutzten Kopie.
- `tools/fetch-pdfium.sh [plattform…|--all]` (macOS, Linux, Git Bash) bzw. `tools/fetch-pdfium.ps1`
  (Windows PowerShell 5.1/7) laden nach `tools/pdfium/<plattform>/` (nicht versioniert), prüfen den
  Hash, verwerfen abweichende Archive, lehnen Archive mit absoluten oder `..`-Pfaden ab und laden
  nichts neu, solange die entpackte Version zum Lock passt. Die Bibliothek liegt in `lib/`, unter
  Windows in `bin/` (`pdfium.dll`). `--print-dir`, `--print-env`, `--github-env` liefern
  `PDFOVER_PDFIUM_DIR`.
- Neue Release: `PDFIUM_RELEASE=chromium/<n> tools/fetch-pdfium.sh --update-lock --all`, Hashes gegen
  die Release-Seite bzw. `gh attestation verify <archiv> --repo bblanchon/pdfium-binaries` abgleichen,
  dann `pdfium-render`/`pdfium_latest` (Root-`Cargo.toml`) und ADR 0060 nachziehen.
- Suchreihenfolge zur Laufzeit und Sicherheitsregeln (kein Arbeitsverzeichnis): ADR 0060 §1. Unter iOS
  wird die dylib später in ein xcframework verpackt (Plan §2.1).

## 4. Bauen und Testen

```sh
cargo check --workspace --all-targets --locked
cargo check -p pdfover-render -p pdfover-placement -p pdfover-pades --no-default-features --all-targets --locked
cargo test --workspace --locked                     # mit PDFOVER_PDFIUM_DIR und Orakeln im PATH
cargo test -p pdfover-backend-atrust --features fido2-hid      # macOS/Linux (fido2-windows unter Windows)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
```

- `--locked` wie in der CI: `Cargo.lock` wird mit committet; kein `cargo update` ohne Absprache.
- FIDO2 unter Linux braucht Zugriff auf `/dev/hidraw*` (udev-Regel mit `uaccess`, Flatpak `--device=all`).
- Die Tauri-Shell (`apps/app`, Paket I3) braucht unter Linux `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `librsvg2-dev`, `libayatana-appindicator3-dev`,
  `libxdo-dev` und vor dem Rust-Build ein gebautes Frontend (`npm ci && npm run build` in `apps/app`).

## 5. Lints und Formatierung

- `cargo fmt --all` (rustfmt.toml: Breite 120). Pakete formatieren nur ihre eigenen Crates (`cargo fmt -p …`).
- Workspace-Lints (Root-`Cargo.toml`, `[workspace.lints]`): `unsafe_code = deny` (FFI nur in eigenen
  Modulen mit lokalem `allow` und `SAFETY`-Kommentar), `missing_debug_implementations`, `dead_code`,
  `unused_variables`, `unused_lifetimes`, `redundant_lifetimes`, `unused_import_braces`; Clippy
  zusätzlich `todo`, `unimplemented`, `dbg_macro`, `mem_forget`, `rc_buffer`, `lossy_float_literal`,
  `verbose_file_reads`, `unnecessary_safety_comment`, `infinite_loop`, `rest_pat_in_fully_bound_structs`.
  Aufgenommen wurde nur, was im Bestand ohne Befund ist; `trivial_numeric_casts` bewusst nicht
  (FFI-Typen wie `c_ulong` sind je Plattform verschieden).
- Die CI prüft `cargo clippy --no-deps --all-targets -- -D warnings` für alle Crates außer der Liste
  `CLIPPY_LENIENT` in `.github/workflows/ci.yml`; für diese laufen dieselben Lints ohne Build-Bruch.
  `--no-deps` ist nötig, weil `-D warnings` sonst auch für mitübersetzte Workspace-Abhängigkeiten gilt.
  Lokal gleichwertig:
  `cargo clippy --workspace --exclude <je Eintrag aus CLIPPY_LENIENT> --no-deps --all-targets --locked -- -D warnings`.
- `CLIPPY_LENIENT` (Stand Welle 2): `pdfover-pades`, `pdfover-workflow`, `pdfover-core-api`,
  `pdfover-cli` (Stubs, Pakete I1/I2) sowie `pdfover-pdf-cos`, `pdfover-sigblock`,
  `pdfover-backend-keystore` wegen je einer `dead_code`-Warnung in fremder Ownership:
  `crates/pdf-cos/src/lexer.rs:459` (Feld `end`), `crates/sigblock/src/fonts.rs:169` (Feld `scaling`),
  `crates/backend-keystore/tests/backend.rs:30` (Funktion `copy_to`). Behebung: entfernen oder mit
  `#[allow(dead_code, reason = "…")]` begründen, dann das Crate aus der Liste streichen. Ziel ist eine
  leere Liste.

## 6. Abhängigkeiten, Lizenzen, Lieferkette

**Neue Abhängigkeiten** (ADR 0001 §4 Regel 2): Steht sie in `[workspace.dependencies]`, mit
`workspace = true` einbinden (eigene `features` erlaubt). Sonst zunächst mit exakter Version im
Crate-`Cargo.toml` und Kommentar; die Integration hebt sie in den Workspace (Welle 2: `encoding_rs`,
`hyper`, `tokio-rustls`, `pkcs5`, `rand_core`). Versionen nicht nebenbei ändern, kein `cargo update`.

**cargo-deny** (`deny.toml`, CI-Job `deny`, blockierend):

```sh
cargo deny --locked check          # advisories bans licenses sources
```

- *Lizenzen*: Allowlist nach Plan §2.5/§10.2 (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, MPL-2.0,
  Unicode-3.0, BSL-1.0, OFL-1.1/Bitstream-Vera), `confidence-threshold = 0.93`, keine Ausnahmen
  (`webpki-root-certs`/CDLA zieht rustls-platform-verifier nur unter wasm32). Ausgewertet werden genau
  die 12 Auslieferungsziele und alle Features.
- *Advisories*: Lücken, `yanked`, `unmaintained` und `unsound` brechen den Build (auch transitiv);
  einzige Ausnahme RUSTSEC-2023-0071 (rsa/Marvin, nur Entschlüsseln betroffen, Plan §2.1).
- *Bans*: doppelte Versionen = Fehler; die 26 bekannten Duplikate stehen mit Begründung in `skip`
  (vor allem die zweite RustCrypto-Generation über p12-keystore/lopdf/secret-service, ADR 0001 §5.2).
  Verboten: GPL/AGPL-PDF-Crates (`pdfium`, `mupdf`, `pdf_signer`, `pdfluent`), `openssl`/`native-tls`,
  `aws-lc-*`; rustls ohne `aws_lc_rs`/`fips`, reqwest ohne `default-tls`/`native-tls`/`rustls`.
- *Quellen*: nur crates.io.
- Bringt eine Änderung ein neues Duplikat oder eine neue Lizenz, wird sie bewertet und begründet in
  `deny.toml` eingetragen – nie pauschal (`skip-tree`, globale Lizenz) freigeschaltet.
- **Erwartet mit der Tauri-Shell** (Probe 15.10.2026: Workspace + `tauri` 2.12.4/`tauri-build` 2.7.3
  ohne Plugins): eine Lizenz `Apache-2.0 WITH LLVM-exception` (`target-lexicon` 0.12 über
  `cfg-expr`/`system-deps`, nur Build-Zeit unter Linux → Ausnahme nur für dieses Crate) und 25 weitere
  doppelte Crates (`alloc-no-stdlib`, `alloc-stdlib`, `base64`, `bitflags`, `brotli-decompressor`,
  `getrandom`, `hashbrown`, `heck`, `indexmap`, `jni`, `jni-sys`, `png`, `proc-macro-crate`,
  `serde_spanned`, `syn`, `thiserror`, `thiserror-impl`, `toml`, `toml_datetime`, `toml_edit`,
  `windows-sys`, `windows-targets`, `windows_aarch64_msvc`, `windows_x86_64_msvc`, `winnow`). Sie
  werden bei der Aufnahme von `apps/app/src-tauri` mit Begründung in `skip` eingetragen (die genauen
  Versionen nennt `cargo deny check bans`).

**THIRD-PARTY-NOTICES** (Plan §10.2; CI-Job `notices` lädt das Ergebnis als Artefakt hoch):

```sh
tools/third-party-notices.sh            # → target/notices/THIRD-PARTY-NOTICES.md
```

Enthält PDFium samt mitgebauten Bibliotheken (aus dem PDFium-Paket), die Font-/AFM-Lizenzen aus
`crates/sigblock/assets/licenses`, alle ausgelieferten Rust-Crates über cargo-about
(`tools/licenses/about.toml`, `about.hbs`; Dev-Abhängigkeiten ausgenommen, Build-Abhängigkeiten
enthalten) mit den NOTICE-Dateien von Apache-2.0-Crates und – sobald `apps/app/package-lock.json`
existiert – die npm-Laufzeitabhängigkeiten. Die Datei gehört in das Über-Fenster und jedes Paket.

**npm** (Plan §2.5: dieselbe Allowlist für die UI): `node tools/npm-licenses.mjs apps/app/package-lock.json`
prüft das Lockfile – Laufzeitpakete gegen die Allowlist, reine Build-Werkzeuge (`dev`) zusätzlich gegen
übliche permissive Daten-/Werkzeuglizenzen (CC0, CC-BY, 0BSD, BlueOak, Python-2.0 …); Copyleft und
fehlende Angaben brechen ab. Mit `--notices <datei>` entstehen die Hinweise.

## 7. i18n: Codes und Kataloge

- Kataloge: `crates/i18n/locales/<de|en>/pdfover.ftl` (erzeugt, nie von Hand) und `pdfover-ng.ftl`
  (von Hand; ADR 0050).
- Stabile Codes der Crates (`BackendError::code`, `CmsError::code`, `KeystoreError::code`,
  `Fido2Error::code`, `SigBlockError::code`, `RenderError::code`, `PlacementError::code`,
  `PlacementNotice::code`, `pades::Finding::code`, `ConfigError::message_id`, `LegacyWarning::code` …)
  stehen in `pdfover-ng.ftl` unter **ihrem Code als ID**; Codes mit Punkt (`render.failed`) werden mit
  `pdfover_i18n::fluent_id_for_java_key` abgebildet (`render-failed`). Variablen heißen wie die Felder
  der Variante (`$detail` für Freitext, `$status`, `$code` …).
- `NoticeCode`/`FormProblem` (backend-api) tragen keinen Code-String; die Zuordnung zu Katalog-IDs
  steht in `crates/i18n/tests/code_catalog.rs` (`NOTICE_CODE_IDS`, `FORM_PROBLEM_IDS`).
- Neuer Code: im Crate einführen, DE- und EN-Text mit denselben Variablen in beide `pdfover-ng.ftl`
  eintragen, `cargo test -p pdfover-i18n` ausführen. Der Test `code_catalog` findet jeden Code im
  Quelltext (`fn code`/`fn message_id`, `NOTICE_*`, Literale der Familien `ng-`, `cms-`, `keystore-`,
  `backend-`, `fido2-`, `atrust-`, `sigblock-`, `pdf-`, `pades-`, `workflow-`, `core-`, `render.`,
  `placement.` …) und prüft DE/EN, gleiche Variablen, die Welle-1-Liste
  (`testdata/i18n/wave1-message-ids.json`, wortgleich) und IDs in `t!`/`translate`-Aufrufen.

## 8. CI (`.github/workflows/ci.yml`)

| Job | Inhalt | blockierend |
|---|---|---|
| `fmt` | `cargo fmt --all --check` | ja |
| `build` (ubuntu, macOS, Windows) | PDFium laden (Cache über `tools/pdfium.lock`), Orakel (pyHanko aus `tools/python-oracles.txt`, Java 21, qpdf/xmllint unter Linux, qpdf/OpenSSL 3 unter macOS), ggf. Frontend und Tauri-Systembibliotheken, `cargo check`, Check ohne PDFium, Clippy `--no-deps -D warnings` (außer `CLIPPY_LENIENT`), FIDO2 (`fido2-windows` unter Windows, sonst `fido2-hid`), `cargo test`, unter Linux Doc-Build mit `RUSTDOCFLAGS=-D warnings` | ja |
| `deny` | `cargo deny --locked check` (cargo-deny 0.20.2) | ja |
| `notices` | `tools/third-party-notices.sh`, Artefakt `THIRD-PARTY-NOTICES` | ja |
| `mobile` | `cargo check` für `aarch64-apple-ios` und `aarch64-linux-android` (NDK des Runners) | verifizierte Crates ja, ganzer Workspace informativ |
| `app` | nur wenn `apps/app/package.json` existiert: Lockfile-Pflicht, npm-Lizenzprüfung, `npm ci`, `lint`/`build`/`test` (falls vorhanden) | ja |

Caching: `Swatinem/rust-cache` (Speichern nur auf `main`), PDFium, pip, npm. Actions sind auf
Commit-SHAs gepinnt (Kommentar = Release-Tag); `actionlint` prüft die Datei lokal
(`brew install actionlint shellcheck && actionlint`).

## 9. Cross-Checks (lokal, 15.10.2026, macOS arm64, Rust 1.99.0)

`cargo check --locked --target <ziel> -p <crate>` je Crate (Skript-Ergebnis):

| Ziel | Ergebnis |
|---|---|
| `x86_64-pc-windows-msvc` | ok: java-properties, i18n, pdf-cos, pdf-incremental, cades, sigblock, placement, render, sl12, backend-api, config, backend-keystore, pades, workflow. Fehlschlag nur am C-Build von `ring` (kein MSVC/Windows-SDK auf macOS): net, backend-localbku, backend-atrust (`fido2-windows`), core-api, cli → CI `windows-latest` |
| `x86_64-unknown-linux-gnu` | wie Windows: alle reinen Rust-Crates ok, `ring`-abhängige nur nativ (CI `ubuntu-latest`) |
| `aarch64-apple-ios` | ok: alle Crates außer config/workflow/core-api/cli (ring baut mit Xcode). **Blocker:** `keyring` 4 → `apple-native-keyring-store` 1.0.2 verlangt unter iOS das Feature `protected` (`compile_error!`). Behoben in Welle 2: `time` ohne `local-offset` (zog `num_threads` 0.1.7, das für iOS nicht übersetzt) |
| `aarch64-linux-android` | ok: alle reinen Rust-Crates inkl. config/workflow; `ring`-abhängige brauchen das NDK (`CC_aarch64_linux_android`), CI-Job `mobile` setzt es |

Gefunden und behoben: `trivial_numeric_casts` schlug nur für Windows an (`c_ulong` in render/ffi) und
ist deshalb nicht Teil der Workspace-Lints. Offen für Mobile: `pdfover-config` sollte `keyring` nur auf
Desktop-Zielen einbinden (`[target.'cfg(not(any(target_os = "ios", target_os = "android")))'.dependencies]`
plus `cfg` um `KeyringSecretStore`); Mobile nutzt ohnehin das `secure-store`-Plugin (Plan §3.1).

## 10. Release-Grundgerüst

Noch kein Release-Workflow; die Bausteine stehen fest:

1. **Version** in `[workspace.package] version` (SemVer), Tag `vX.Y.Z` auf `main`; Changelog-Eintrag.
2. **Build** je Plattform mit `--release --locked` (Profil: `lto = "thin"`, `codegen-units = 1`,
   `strip = "symbols"`): CLI `pdfover-cli` und Tauri-Bundles (`apps/app`, Tauri-CLI 2.12.x: NSIS/MSI,
   DMG Universal, deb/rpm/Flatpak, iOS/Android) – Plan §9.1.
3. **PDFium beilegen**: `tools/fetch-pdfium.sh <plattform>` (Hash aus `tools/pdfium.lock`); `lib/`
   bzw. `bin/` neben die Binary, macOS `Frameworks/`, iOS als xcframework (ADR 0060 §1).
4. **Hinweise**: `tools/third-party-notices.sh` → `THIRD-PARTY-NOTICES.md` in jedes Paket und ins
   Über-Fenster; dazu SBOM (CycloneDX) und `cargo deny check` als Gate (Plan §2.5).
5. **Signieren nur in der CI** (Plan §9.3): Windows Artifact Signing/HSM, Apple `notarytool`/`stapler`
   (PDFium-dylib mitsigniert), Linux-Repo-Schlüssel offline; Mobile über die Stores.
6. **Updates** (Plan §9.4): Tauri-Updater mit Ed25519-Signatur und eigenem Manifest; Linux über
   Paketquellen/Flathub.
7. **Abnahme**: Größen-Gate ≤ 25 MB je Desktop-Installer (Plan §9.1), Smoke-Test je Paket.

Ein `.github/workflows/release.yml` folgt, sobald die Tauri-Shell (Paket I3) bündelt; Signier-Secrets
gehören dem Auftraggeber (Plan §9.3, E-13).

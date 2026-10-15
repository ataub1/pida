# ADR 0002 – Name PIDA (Umbenennung von „PDF-Over NG“)

| | |
|---|---|
| **Status** | angenommen (15.10.2026) |
| **Kontext** | Der Arbeitstitel „PDF-Over NG“ (Plan §1, ADR 0001: Entscheidung E-05 „Markenklärung“) wird ersetzt. |
| **Entscheidung** | Auftraggeber, 15.10.2026: **alles** heißt **PIDA** (PDF · ID · Austria). |
| **Umsetzung** | `tools/rename-to-pida.py` (wiederholbar, idempotent, nur Python-Standardbibliothek) |

## 1. Entscheidung

- Produkt- und Anzeigename: **PIDA**. Untertitel/Beschreibung: **„PIDA – PDF-Signatur mit ID Austria“**,
  englisch **„PIDA – PDF signing with ID Austria“** (CLI-Hilfe `ng-cli-help-title`, Bundle-Kurzbeschreibung,
  README).
- Technische Namen folgen einheitlich: Bundle-ID `at.pida`, GUI-Binary `pida`, CLI `pida-cli`, Crates
  `pida-<x>`, Umgebungsvariablen `PIDA_*`, Custom Protocol `pida-render://`.
- Bezüge auf das **Original PDF-Over** (A-SIT) bleiben wörtlich erhalten (§3) – PIDA bleibt ein
  funktional gleichwertiger Nachfolger, der PDF-Over-Einstellungen übernimmt und dessen CLI-Argumente versteht.
- Die Umbenennung ist **kein Handbetrieb**, sondern das Skript `tools/rename-to-pida.py`. Es wird nach jeder
  Zusammenführung mit Ständen unter dem alten Namen (insbesondere der Mobile-Welle) erneut ausgeführt (§5).

## 2. Mapping-Tabelle

Reihenfolge wie im Skript (spezifische und längere Muster zuerst); `python3 tools/rename-to-pida.py --rules`
gibt die verbindliche Fassung aus.

### 2.1 Feste Formulierungen (vor allen anderen Regeln)

| Alt | Neu | Wo |
|---|---|---|
| `PDF-Over NG – PDF-Dokumente elektronisch signieren` | `PIDA – PDF-Signatur mit ID Austria` | CLI-Hilfe (DE), Meldungs-ID-Liste |
| `PDF-Over NG – sign PDF documents electronically` | `PIDA – PDF signing with ID Austria` | CLI-Hilfe (EN), Meldungs-ID-Liste |
| `"shortDescription": "PDF-Dokumente elektronisch signieren"` | `"shortDescription": "PIDA – PDF-Signatur mit ID Austria"` | `tauri.conf.json` |
| `"longDescription": "PDF-Over NG signiert …` | `"longDescription": "PIDA (PDF · ID · Austria) signiert …` | `tauri.conf.json` |
| `s.contains("pdf-over-ng") \|\| s.contains("pdf-over ng")` | `s.contains("pida")` | Test in `crates/config/src/paths.rs` (sonst `clippy::eq_op`) |
| `#[command(name = "pdfover-cli", version, …` | `#[command(name = "pida-cli", display_name = "PIDA", version, …` | `pida-cli --version` gibt `PIDA <Version>` aus (wie die App); Test in `apps/cli/tests/cli.rs` |
| `Kennung at.pdf-over-ng: „-“ ist im Android-Paketnamen unzulässig → at.pdf_over_ng (Tauri).` | `Kennung at.pida = Android-Paket …` | Kommentar in `gen/android` (Mobile-Welle) |
| `# PDF-Over NG` (Titel), `Arbeitstitel „PDF-Over NG“, Lizenz` | `# PIDA` + Untertitel, `Name **PIDA** (bis 15.10.2026 Arbeitstitel …)` | `README.md` |
| `Arbeitstitel „PDF-Over NG“.` | `… , seit 15.10.2026 **PIDA** (ADR 0002).` | ADR 0001 |

### 2.2 Namen und Kennungen

| Alt | Neu | Beispiele |
|---|---|---|
| `PDF-Over NG`, `PDF-Over-NG`, `PDFOverNG`, `PdfOverNg` | `PIDA` | Fenster-/Dokumenttitel, `productName`, User-Agent `PIDA v<Version>`, Keychain-Dienst `PIDA`, Windows-Ordner `%APPDATA%\PIDA`, `PIDA.app`, DMG `PIDA_<version>_aarch64.dmg`, Multipart-Boundary, Texte in `pida.ftl` |
| `pdf-over-ng` | `pida` | Bundle-ID `at.pida`, `mainBinaryName`/`[[bin]]` `pida`, Ordner `at.pida` (macOS) bzw. `pida` (Linux), Log `pida.log`, `/etc/pida/policy.toml`, GitHub `ataub1/pida`, CI-Artefakte `pida-debug-*`/`pida-release-*` |
| `pdf_over_ng` | `pida` | Android-Paket/`applicationId` `at.pida` (aus der Bundle-ID), Single-Instance-Socket `/tmp/at_pida_si.sock` |
| `pdf-overng` | `pida` | Schreibweise von `directories` |
| `pdfover-ng`, `pdfover_ng`, `pdfoverng`, `PDFOVER_NG` | `pida` bzw. `PIDA` | Katalog `locales/<lang>/pida.ftl` (Datei umbenannt), npm-Paket `pida-app`, localStorage `pida.theme`, Kotlin-Pakete `at.pdfoverng.*` → `at.pida.*` |
| `PDFOVER_` | `PIDA_` | alle Umgebungs- und Shell-Variablen (§2.4) |
| `Pdfover` | `Pida` | `PdfoverMobilePlugin` → `PidaMobilePlugin` (Swift/Kotlin-Klasse und Datei), `PdfoverMobile`, `PdfoverMobileExt` |
| `pdfover-` | `pida-` | Crates `pida-<x>`, `pida-cli`, `pida-app`, Plugin `pida-mobile` bzw. `tauri-plugin-pida-mobile` (inkl. Verzeichnis `apps/app/plugins/pida-mobile`), `pida-render://`, Präfix `pida-smoke:`, Thread `pida-core`, Temp-Namen |
| `pdfover_` | `pida_` | Rust-Pfade `pida_<x>`, `pida_app_lib`, `pida_mobile()`, `init_plugin_pida_mobile`, Shell-Hilfsvariablen |
| `pdfover:` | `pida:` | Tauri-Ereignisse `pida:view`, `pida:emblem-drop` |
| `pdfover=` | `pida=` | Log-Filter `pida=debug` |
| `pdfover.<name>` (außer `pdfover.ftl` und Java-Paketen) | `pida.<name>` | Mobile: FileProvider `${applicationId}.pida.files`, Intent-Extra `pida.smoke`, Gradle-Property `pida.pdfiumJniLibs` |
| `NG` (Kurzform im Text) | `PIDA` | „NG-Ergänzung“ → „PIDA-Ergänzung“, „NG signiert …“ (nur Inhalte, nur als eigenes Wort) |

Interne Meldungs-IDs (`ng-*`, `ng-ui-*`) bleiben unverändert (keine ID-Umbenennung).

### 2.3 Crates und Binaries

| Verzeichnis (unverändert) | Paket alt → neu | Rust-Pfad |
|---|---|---|
| `crates/<x>` (18 Crates) | `pdfover-<x>` → `pida-<x>` | `pdfover_<x>` → `pida_<x>` |
| `apps/cli` | `pdfover-cli` → `pida-cli` (Binary `pida-cli`) | `pida_cli` |
| `apps/app/src-tauri` | `pdfover-app` → `pida-app` (Binary `pida`, Lib `pida_app_lib`) | `pida_app_lib` |
| `apps/app` (npm) | `pdfover-ng-app` → `pida-app` | – |
| `apps/app/plugins/pdfover-mobile` → `apps/app/plugins/pida-mobile` (Mobile-Welle) | `tauri-plugin-pdfover-mobile` → `tauri-plugin-pida-mobile` | `tauri_plugin_pida_mobile` |

### 2.4 Umgebungsvariablen

`PIDA_HOME`, `PIDA_PDFIUM_DIR`, `PIDA_SMOKE`, `PIDA_LOG`, `PIDA_ROOT`, `PIDA_MAIN`, `PIDA_REFERENCE`,
`PIDA_BLESS`, `PIDA_I18N_BLESS`, `PIDA_UPDATE_RESULTS`, `PIDA_KEEP`, `PIDA_E2E_OUT`, `PIDA_PYTHON`, `PIDA_JAVA`,
`PIDA_FUZZ_*` (`ITERS`, `SEED`, `FILE`, `TRACE`, `DUMP`, `THREADS`, `VERBOSE`, `PREPARE_ITERS`),
`PIDA_VERAPDF_JAR`, `PIDA_VERAPDF_KEEP`, `PIDA_PDFAS_LIB`, `PIDA_GOLDEN_CACHE`, `PIDA_GOLDEN_REPORT_DIR`,
`PIDA_GOLDEN_DEBUG_DIR`, `PIDA_NOTICES_DIR`, `PIDA_TEST_KEYCHAIN`, `PIDA_DEBUG_FILE`, `PIDA_DEBUG_OBJ` – jeweils
vorher `PDFOVER_*`. Der Code liest die alten Namen **nicht** mehr. `tools/env.sh` übernimmt beim Übergang
die Werte von `PDFOVER_{PDFIUM_DIR, REFERENCE, HOME, PYTHON, JAVA, VERAPDF_JAR, PDFAS_LIB, NOTICES_DIR,
GOLDEN_CACHE, LOG}` aus der aufrufenden Umgebung (sofern die neue Variable leer ist) und entfernt danach
alle `PDFOVER_*` – nach dem Laden gibt es nur noch `PIDA_*`.

### 2.5 Anwendungsdaten je Betriebssystem

| Zweck | Windows | macOS | Linux/BSD |
|---|---|---|---|
| Konfiguration | `%APPDATA%\PIDA\config` | `~/Library/Application Support/at.pida` | `$XDG_CONFIG_HOME/pida` |
| Daten | `%APPDATA%\PIDA\data` | wie Konfiguration | `$XDG_DATA_HOME/pida` |
| Cache | `%LOCALAPPDATA%\PIDA\cache` | `~/Library/Caches/at.pida` | `$XDG_CACHE_HOME/pida` |
| Logs | `%LOCALAPPDATA%\PIDA\data\logs` | `~/Library/Logs/at.pida` | `$XDG_STATE_HOME/pida/logs` |
| Richtlinien (Datei) | `%ProgramData%\PIDA\policy.toml` | `/Library/Application Support/at.pida/policy.toml` | `/etc/pida/policy.toml` |

## 3. Bewusst unverändert

Das Skript lässt folgende Dateien und Stellen aus (`--rules` listet die Muster; `--check` meldet sie nicht):

- **Historische Dokumente:** `reference/` (Originalquellen), `docs/analyse/**`, `docs/recherche/**` – dort
  ändert sich nichts. `docs/00-architektur-und-umsetzungsplan.md` erhält nur einen Hinweis-Kasten oben
  („Arbeitstitel PDF-Over NG heißt seit 15.10.2026 PIDA“), der Rest bleibt wörtlich.
- **Original-PDF-Over-Bezüge:** Legacy-Import aus `~/.pdf-over`, Datei `PDF-Over.config`, Legacy-Keys,
  PDF-Over-kompatible CLI-Argumente und Hilfetexte, Texte wie „Einstellungen aus PDF-Over übernehmen“,
  der aus PDF-Over übernommene Datenschutztext, Protokoll `import-pdf-over.log`, Java-Pakete
  `at.asit.pdfover.*` und Pfade `pdf-over-gui/…`, `pdf-over-commons/…`, `pdf-over-signer/…`. Diese Muster
  enthalten nie „NG“ und werden von keiner Regel getroffen (`PDF-Over` allein wird nie ersetzt).
- **Aus PDF-Over erzeugter Katalog:** `locales/<lang>/pdfover.ftl` behält den Dateinamen (Inhalt = Meldungen
  aus `messages*.properties` von PDF-Over), ebenso `convert_pdfover_messages`/`prepare_pdfover_messages`,
  `pdfover_header`, `PDFOVER_PROFILE_IDS` (die 12 Profil-IDs von PDF-Over), `PDFOVER_UNUSED_KEYS`,
  `PasswordStorage::{from,as}_pdfover`, Testnamen `…_like_pdf_over`, `all_pdfover_profile…`.
  Die eigenen Texte stehen jetzt in `locales/<lang>/pida.ftl`.
- **Testdaten-Inhalte:** `testdata/**` außer Dokumentation (`README.md`, `RESULTS.md`), Generatoren
  (`*.sh`, `*.py`, `*.java`) und der Kopie `testdata/i18n/wave1-message-ids.json`. Unverändert bleiben damit
  PDFs, Golden-Manifeste (`testdata/golden/pdfas-4.4.2/**`, Positionsschlüssel `"pdfover"`, Fall-IDs
  `…__pdfover`), Altkonfigurationen (`*.config`, `*.properties`, Pfad `/tmp/pdfover-nicht-vorhanden/…`) und
  Zertifikate. Die Namen der Testzertifikate und -inhalte („PDF-Over NG Test RSA (NUR TEST)“,
  „PDF-Over NG NUR TEST …“, „PDF-Over NG Testinhalt …“) bleiben überall – auch in Tests und Generatoren –,
  weil sie so in den binären Testdaten stehen.
- **Abgeleitete Dateien** (`Cargo.lock`, `apps/app/package-lock.json`, `generated.ts`, `view-states.json`)
  werden nicht textuell umgeschrieben, sondern neu erzeugt (`--regenerate`); dazu gehört `cargo fmt --all`,
  weil die kürzeren Namen umbrochene Zeilen wieder in die Breite 120 passen lassen.
- Zeilen mit dem Pragma `rename-to-pida:keep` (Übergangsblock in `tools/env.sh`) und historische Hinweise
  „Arbeitstitel … PDF-Over NG“.

## 4. Folgen und Hinweise

- **Keine Migration der Pre-Release-Ordner.** `at.pdf-over-ng` (macOS: Application Support, Caches, Logs),
  `pdf-over-ng` (Linux: XDG-Ordner, `/etc/pdf-over-ng`), `%APPDATA%\PDF-Over NG` / `%LOCALAPPDATA%\PDF-Over NG`
  (Windows) werden nicht übernommen – es gibt nur einen Testnutzer. PIDA startet dort mit leeren
  Einstellungen (bzw. bietet den Import aus `~/.pdf-over` erneut an); die alten Ordner können von Hand
  gelöscht werden. Ebenso verfallen Schlüsselbund-Einträge unter dem Dienst „PDF-Over NG“, der
  Fensterzustand und die WebView-Daten (localStorage `pdfover-ng.*`, an die Bundle-ID gebunden).
- **User-Agent gegenüber A-Trust** lautet jetzt `PIDA v<Version>` (vorher `PDF-Over NG v<Version>`). Plan
  L10 verlangt für eine geänderte Kennung die Abstimmung mit A-Trust; vor dem nächsten Abnahmelauf gegen
  A-Trust bestätigen, dass die Seitenauslieferung unverändert ist.
- **GitHub-Repository** `ataub1/pdf-over-ng` wird in `ataub1/pida` umbenannt (Orchestrator; GitHub leitet
  alte URLs weiter). Das Repository selbst enthält derzeit keinen Verweis darauf (`repository` in
  `Cargo.toml` ist der Platzhalter `https://example.invalid/pida`); künftige Verweise erfasst die Regel
  `pdf-over-ng` → `pida`. Die lokale Remote-URL (`git remote set-url`) passt man nach der Umbenennung an.
- **Single-Instance:** Eine laufende alte Instanz (Socket `/tmp/at_pdf_over_ng_si.sock`) wird nicht erkannt.
- **Mobile-Projekte:** Sind `gen/android` bzw. `gen/apple` versioniert (Mobile-Welle), benennt das Skript sie
  mit um: Android-Paket `at/pdf_over_ng/…` → `at/pida/…` (auch `buildSrc`), Theme `Theme.pdfover_app` →
  `Theme.pida_app`; das Xcode-Projekt heißt nach dem Crate der Shell (`tauri ios init`):
  `pdfover-app.xcodeproj` → `pida-app.xcodeproj`, `pdfover-app_iOS` → `pida-app_iOS`, `Sources/pdfover-app` →
  `Sources/pida-app` (ein aus der Bundle-ID erzeugtes `pdf-over-ng.xcodeproj` würde zu `pida.xcodeproj`).
  Sind sie nicht versioniert, nach der Umbenennung neu erzeugen (`npm run tauri android init`,
  `npm run tauri ios init`), damit Paket `at.pida` und Projektnamen passen. Daten der Mobile-App
  (SharedPreferences `pdfover_secure_store`, Keychain-Dienst `at.pdf-over-ng.secrets`, FileProvider
  `…pdfover.files`) gehen wie auf dem Desktop ohne Migration verloren (Paket bzw. Bundle-ID ändern sich ohnehin).
- **Liegen gebliebene Build-Ausgaben:** `git mv` verschiebt nur versionierte Dateien. Ignorierte Ausgaben
  bleiben in den Altordnern, z. B. `gen/android/app/src/main/java/at/pdf_over_ng/generated/` (von Tauri erzeugte
  Kotlin-Quellen – Gradle übersetzt sie sonst weiter mit), `gen/android/app/src/main/jniLibs/*/libpdfover_app_lib.so`
  (landet sonst zusätzlich im APK), `plugins/pdfover-mobile/{.tauri,android/.tauri,android/build}`. Das Skript
  löscht sie nicht, meldet sie aber nach dem Lauf und bei `--check` als **Hinweis** (ohne Einfluss auf den
  Exit-Code), ebenso nicht versionierte, nicht ignorierte Dateien mit Altnamen (die das Skript nicht bearbeitet –
  erst `git add`, dann erneut ausführen). `target/`, `node_modules/`, `build/`, `dist/`, `.claude/` und andere
  Repositories/Worktrees werden dabei nicht durchsucht.
- Lokale Build-Ausgaben (`target/…/pdf-over-ng`, `bundle/macos/PDF-Over NG.app`) bleiben liegen, bis
  `target/` geleert wird.

## 5. Erneuter Lauf (nach dem Mobile-Merge oder anderen Zusammenführungen)

Das Skript ist idempotent: ein zweiter Lauf ändert nichts, neue Dateien unter dem alten Namen (z. B.
`apps/app/plugins/pdfover-mobile/**`, Kotlin `at/pdfoverng/**`, `PdfoverMobilePlugin.swift`,
`gen/apple`, `gen/android`) erfasst es beim nächsten Lauf. Ablauf auf `main`:

```sh
# 1. Mobile-Welle (o. a.) zusammenführen; Konflikte dabei zugunsten des neuen Inhalts lösen – die Namen
#    korrigiert erst Schritt 3.
# 2. Skript und Dokumente dieses Pakets übernehmen (falls noch nicht auf main):
git checkout <pida-branch> -- tools/rename-to-pida.py docs/adr/0002-name-pida.md
# 3. Umbenennen, abgeleitete Dateien erzeugen, prüfen:
python3 tools/rename-to-pida.py --self-test
python3 tools/rename-to-pida.py --dry-run -v | less      # Sichtprüfung
python3 tools/rename-to-pida.py                         # Inhalte + git mv + Hinweis-Kästen
python3 tools/rename-to-pida.py --regenerate            # Cargo.lock, cargo fmt, package-lock.json, pdfover.ftl, generated.ts, view-states.json, ui-events.json
python3 tools/rename-to-pida.py --check                 # Exit 0 = keine Altnamen außerhalb der Ausnahmen
# 4. Prüfen wie in docs/entwicklung.md §4 (cargo check/test/clippy, cargo deny, npm ci/build/test,
#    Bundle mit PIDA_SMOKE=1, actionlint)
```

`--check` meldet (Exit 1) ausstehende Ersetzungen, umzubenennende Pfade, veraltete abgeleitete Dateien und
Restfundstellen von „PDF-Over NG“/„pdfover“, die keine Regel erfasst. Neue legitime Original-Bezüge, die das
Skript fälschlich meldet, gehören in die Ausnahmeliste (`PROTECT`, `ALLOWED`, `SKIP`) des Skripts – nicht in
Handkorrekturen, sonst gehen sie beim nächsten Lauf verloren. Gleiches gilt für neue feste Formulierungen
(Stufe `phrase`).

## 6. Das Skript

- Nur Git-verfolgte reguläre Dateien; Binärdateien (NUL-Byte oder kein UTF-8), Symlinks und Submodule
  bleiben unberührt; Zeilenenden und Dateirechte bleiben erhalten.
- Ablauf je Datei: feste Formulierungen → geschützte Stellen maskieren → Regeln in Tabellenreihenfolge →
  Maskierung aufheben → Hinweis-Kästen (nur wenn ihre Markierung fehlt). Pfade: dieselben Regeln mit
  Pfad-Schutzliste, Umbenennung mit `git mv`, leere Altordner werden entfernt.
- Optionen: `--dry-run [-v]`, `--check`, `--regenerate`, `--rules`, `--self-test`, `--root <ordner>`.
- `--self-test` baut ein synthetisches Git-Repository mit Desktop- **und** Mobile-Dateien (Plugin
  `pdfover-mobile` mit Kotlin-Paket `at/pdfoverng/mobile`, Swift-Klassen, `AndroidManifest.xml` mit FileProvider,
  `res/xml/pdfover_share_paths.xml`, `SecureStore`; `gen/android` mit `at/pdf_over_ng` inkl. `buildSrc`,
  Gradle-Dateien, Theme, `gradlew` (Ausführungsrecht) und `gradlew.bat` (CRLF); `gen/apple` mit
  `pdfover-app.xcodeproj`/`pdfover-app_iOS`/`Sources/pdfover-app` sowie `pdf-over-ng.xcodeproj`/`pdf-over-ng_iOS`,
  `project.yml`, `Info.plist`; `tauri.android.conf.json`/`tauri.ios.conf.json`), Ausnahmen (Analyse-Dokument,
  Legacy-Pfade, Testzertifikat, Golden-Manifest, `pdfover.ftl`), einer Binärdatei, einer unverfolgten Datei und
  ignorierten Build-Ausgaben in umzubenennenden Ordnern und prüft: `--check` vorher Exit 1, `--dry-run` ändert
  nichts, Ergebnis je Datei, `git mv` im Index, Zeilenenden und Dateirechte erhalten, Hinweise auf liegen
  gebliebene Ausgaben, zweiter Lauf ohne Änderung, `--check` danach Exit 0 (mit Hinweisen), Restsuche meldet
  nicht erfasste Altnamen.
- Validierung beim Anlegen (15.10.2026): (1) Auf einem Abzug von `main` (`ab7a274`) erzeugen Skript und
  `--regenerate` (Lockfiles, `cargo fmt --all`) exakt den Stand des Umbenennungs-Commits – Handarbeit steckt
  nur in diesem ADR und im Skript selbst. (2) Auf einem Abzug des Mobile-Zweigs
  (`worktree-wf_bcedef89-63a-1`: Plugin, Kotlin/Swift, versionierte `gen/android` und `gen/apple`) ändert es
  333 Dateien und benennt 36 Pfade um; `--check` meldet danach nur noch die abgeleiteten Dateien. Dabei
  gefundene Mobile-Kennungen (`pdfover.files`, `pdfover.smoke`, Log-Tags `PdfOverNg`) sind Regeln geworden.
- Review (15.10.2026): (3) Auf einem Abzug von `main` (`0091403`) ergeben Skript, `cargo fmt --all`,
  `cargo update --workspace` und `npm install --package-lock-only` bis auf die beiden Dateien aus `0091403`
  exakt den Stand dieses Zweigs; der Commit „Positionierung: Abschnitt Position entfernt“ lässt sich darauf
  mit `git am` ohne Konflikt anwenden. (4) Auf dem Mobile-Zweig einschließlich nicht committeter Änderungen
  (u. a. `tauri.android.conf.json`) und ignorierter Build-Ausgaben: 333 Dateien, 36 Pfade, danach `--check`
  ohne Befund außer den abgeleiteten Dateien; `cargo update --workspace --offline` erzeugt ein bis auf die
  Namen gleiches `Cargo.lock` (736 Pakete); die liegen gebliebenen Ordner werden als Hinweis gemeldet.

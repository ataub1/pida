# Mobile (iOS/Android) – Fundament M0 und Aufgaben für M-iOS/M-Android

Stand: 15.10.2026, Welle Mobile, Paket **M0** (plattformneutrales Fundament). Grundlage: Plan
§5 (Plattform-Matrix, §5.2 Mobile 1.0, §5.3 Plattformspezifika), §3.5 (Lebenszyklus), §4.7 (FIDO2),
§7.7 (Sicherheit); ADR 0001; docs/analyse/02 (A-Trust auf Mobile: kein FIDO2, App-Wechsel).

Mobile 1.0 (Plan §5.2): ID Austria nativ (Szenario A: Anmeldung, SMS-TAN, Bestätigung in der App
„ID Austria“ mit App-Wechsel; **kein FIDO2** – ausgegraut mit Hinweis), PKCS#12 nur nach Import in die
App-Sandbox, **keine lokale BKU, keine CLI**. Dokumente über Share-Sheet/„Öffnen in“ (iOS) bzw.
Intents `VIEW`/`SEND` `application/pdf` (Android) und die System-Dateiauswahl; Ausgabe in die
App-Dokumente plus Teilen/„Sichern unter“; Positionieren per Touch, die Zahlenfelder bleiben.

M0 liefert alles, was **beide** Plattformen brauchen (Kern, Shell, Plugin-Rust-Seite, Oberfläche,
PDFium-Pakete); die nativen Seiten des Plugins sind **leere Hüllen**. M-iOS und M-Android arbeiten danach
parallel in diesem Worktree (Zuständigkeiten siehe §9).

## 1. Architektur

```
React-Oberfläche (apps/app/src)            ── ViewState / UiEvent (Tauri-IPC, unverändert) ──┐
  isMobile(view): Teilen/Exportieren, PKCS#12-Assistent, „Signatur fortsetzen“, FIDO2-Hinweis   │
                                                                                                ▼
Tauri-Shell (apps/app/src-tauri, Crate pida-app)                                pida-core-api (Kern)
  cfg(mobile): Sandbox-Pfade, Lebenszyklus, Dokument-Eingang,  ── CoreOptions ──▶  Workflow-Reducer (State::Resuming)
  mobile_pick_files, ShellHooks share/export, SecretStore                         resume.rs: Prepared::persist/restore
        │ Rust-API (keine WebView-Befehle)                                          Long-Poll nach Rückkehr, Backend-Liste
        ▼
Plugin tauri-plugin-pida-mobile (apps/app/plugins/pida-mobile)
  src/ (fertig): PidaMobile, Fehlercodes, Pfadprüfung, tls.rs (Android: rustls-platform-verifier)
  ios/Sources/PidaMobilePlugin.swift      ← M-iOS füllt (Keychain, Share, Document Picker, Inbox)
  android/src/main/java/…/PidaMobilePlugin.kt ← M-Android füllt (Keystore, ACTION_SEND, SAF, Intents)
```

Die Oberfläche hält keinen Fachzustand (Plan §3.1). Plattform kennt sie aus `ViewState.platform`
(`{ os, mobile }`, vom Kern aus `CoreConfig::platform`).

## 2. Plugin `pida-mobile`

Crate `tauri-plugin-pida-mobile`, Tauri-Name `pida-mobile`, registriert auf **allen** Plattformen
(Desktop: ohne native Seite, jede Methode liefert `Error::Unsupported`). Das Plugin hat **keine Befehle
für die WebView** (`COMMANDS = []`, keine Berechtigungen): der sichere Speicher darf aus der Oberfläche
nicht erreichbar sein. Nur die Rust-Shell ruft es auf – nie vom Hauptthread (iOS führt die Methoden dort
aus; die Shell nutzt Hintergrund-Threads, der sichere Speicher zusätzlich eine Frist von 5 s).

Rust-API (`PidaMobileExt::pida_mobile()` auf `AppHandle`):

| Methode (Rust) | nativ | Argumente (JSON, camelCase) | Antwort | Umsetzung iOS / Android |
|---|---|---|---|---|
| `secure_store_get(key)` | `secureStoreGet` | `{ key }` | `{ value: string \| null }` | Keychain `kSecClassGenericPassword`, `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, nicht synchronisiert / AES-256-GCM-Schlüssel im Android Keystore, verschlüsselte Werte in privaten SharedPreferences (ohne Sicherung) |
| `secure_store_set(key, value)` | `secureStoreSet` | `{ key, value }` | `{}` | ersetzt vorhandene Werte |
| `secure_store_delete(key)` | `secureStoreDelete` | `{ key }` | `{}` | fehlender Eintrag ist kein Fehler |
| `share_file(path, mime)` | `shareFile` | `{ path, mimeType }` | `{}` sobald gezeigt | `UIActivityViewController` / `ACTION_SEND` mit `FileProvider`-URI + `FLAG_GRANT_READ_URI_PERMISSION` |
| `open_file(path, mime)` | `openFile` | `{ path, mimeType }` | `{}` sobald die Anzeige startet | QuickLook (`QLPreviewController`, ohne Bearbeiten) / `ACTION_VIEW` mit `FileProvider`-URI an eine fremde PDF-App – Ergebnis „Öffnen“ (`ShellHooks::open_path`; nur `not-implemented` fällt auf `opener` zurück) |
| `export_file(path, name, mime)` → `bool` | `exportFile` | `{ path, fileName, mimeType }` | `{ saved }` | `UIDocumentPickerViewController(forExporting:asCopy:)` / SAF `ACTION_CREATE_DOCUMENT` + Kopie |
| `import_file(kind, multiple, dest)` → Dateien | `importFile` | `{ kind: "pdf"\|"pkcs12", multiple, destination }` | `{ files: [{ name, path }] }` | `UIDocumentPickerViewController(forOpeningContentTypes:asCopy:true)` / SAF `ACTION_OPEN_DOCUMENT`; **Bytes** nach `destination` kopieren |
| `take_pending_open_documents(urls, dest)` → Dateien | `takePendingOpenDocuments` | `{ urls: [string], destination }` | `{ files: [{ name, path }] }` | vorgemerkte Dokumente („Öffnen in“/Share bzw. Intents `VIEW`/`SEND`/`SEND_MULTIPLE`) **und** die übergebenen URLs (Security-Scoped `file://`, `content://`) nach `destination` kopieren, Warteschlange leeren |

Fehler: `invoke.reject(message, code)` – `not-implemented` (Hülle), `cancelled` (Nutzerin hat
abgebrochen; Export/Import liefern dann `false` bzw. `[]`), sonst ein kurzer eigener Code. Die Nachricht
wird verworfen (kann Pfade enthalten); Rust kennt `ng-mobile-unsupported`, `ng-mobile-not-implemented`,
`ng-mobile-cancelled`, `ng-mobile-timeout`, `ng-mobile-native` (übersetzt in `pida.ftl`).
Zurückgegebene Pfade müssen **innerhalb von `destination`** liegen und Dateien sein – andere verwirft die
Rust-Seite (`within`, Schutz gegen fehlerhafte native Seiten).

Tests der Rust-Seite: `cargo test -p tauri-plugin-pida-mobile` (Vertrag mit einer Attrappe der
nativen Seite: Argumente, Antworten, Abbruch, Pfadprüfung, Fehlercodes).

## 3. Lebenszyklus (Plan §3.5)

- **Ereignisse:** `WindowEvent::Suspended` (iOS `applicationWillResignActive`, Android `onPause`) →
  `UiEvent::Backgrounded`; `WindowEvent::Resumed` (iOS `applicationWillEnterForeground`, Android
  `onResume`) → `UiEvent::Foregrounded` (`apps/app/src-tauri/src/lib.rs`, `mobile::lifecycle_event`).
- **Kein Abbruch:** Im Reducer ändern `Backgrounded`/`Foregrounded` weder Zustand noch Effekte (Tabellentest
  in jedem Zustand, Property-Test `reducer_is_total_and_keeps_queue_consistent`). Auf Mobile gehen
  `CloseRequested`/`ExitRequested` **nicht** als `Quit` an den Kern (das hätte den Vorgang abgebrochen und den
  persistierten Zustand gelöscht).
- **Long-Poll nach Rückkehr:** Nach `Foregrounded` (nur wenn vorher `Backgrounded`) beantwortet der Kern eine
  offene Maske `WaitForApp`/`QrCode` mit „weiter“ (`CoreBroker::restart_long_poll`); das A-Trust-Backend lädt
  die Seite neu (R7) und startet einen frischen Long-Poll – die alte Verbindung hat das System im
  Hintergrund oft getrennt (sonst erst nach 10-s-Backoff bzw. 300-s-Frist). Ist die Bestätigung inzwischen
  angekommen, geht es sofort weiter. `backend-atrust` ist dafür unverändert.
- **App-Kill → „Signatur fortsetzen“:** Mit `CoreOptions::persist_prepared` (Standard auf iOS/Android)
  sichert der Kern **nach `prepare`** in `Paths::temp_dir/resume/` (App-Sandbox, nicht der Cache;
  Ordner 0700, Dateien 0600): `resume.json` (Name, Signiermittel, Position, wartende Dokumente,
  Zeitpunkt, SHA-256), `document/<Originalname>` (Kopie des Originals) und
  `pades::Prepared::persist` (Platzhalter-PDF + Begleitdatei). Beim nächsten Start bietet der Kern
  im Schritt `open` `ViewState.resume` an (höchstens 60 min alt, sonst Hinweis `ng-resume-expired`).
  `resume_signature` → `Event::ResumeRequested` → `State::Resuming`: Dokument laden, **ohne**
  Platzhalterfrage/Positionierung, gespeichertes Signiermittel („Signatur fortsetzen“ ist die
  Nutzeraktion; kein Zustimmungsschritt, das Angebot zeigt bei ID Austria den Kurzhinweis, ADR 0003). Sitzung, Anmeldung und Bestätigung laufen neu (das ID-Austria-Passwort wird nie
  persistiert); `Effect::Prepare` wird durch `Prepared::restore` ersetzt – Signaturzeitpunkt `/M`, Block
  und Position bleiben die des ersten Versuchs. Scheitert die Wiederherstellung (Prüfsumme, Original),
  wird mit Hinweis `ng-resume-restore-failed` neu vorbereitet. `discard_resume` löscht den Zustand.
- **Löschen:** `Effect::DeletePersisted` nach Erfolg, Abbruch und Fehler löscht nur den Zustand des
  **laufenden** Vorgangs (ein noch nicht angenommenes Angebot übersteht den Abbruch eines anderen Vorgangs
  vor `prepare`; ein neues `prepare` ersetzt es).

## 4. Dokumente, Ausgabe, Secrets, Signiermittel

- **Eingang:** `RunEvent::Opened` liefert URLs. `mobile::ingest_urls` liest `file:`-URLs als **Bytes** und
  legt Kopien in `<app_cache_dir>/inbox/<n>/<Name>` ab (Name bleibt → Ausgabename `<Name>_signed.pdf`);
  nicht lesbare URLs (Android `content://`, Security-Scoped) und Intents holt die Shell über
  `take_pending_open_documents` – beim Start und nach jeder Rückkehr in den Vordergrund. Die
  Dateiauswahl der Oberfläche läuft über den Befehl `mobile_pick_files` (`kind` `pdf`/`pkcs12`, Kopien in
  `<app_cache_dir>/import/`).
- **Ausgabe:** `CoreOptions::default_output_dir` = iOS `Documents` (Tauri `document_dir`, in der Dateien-App
  sichtbar), Android `filesDir/Documents` (`mobile::ANDROID_OUTPUT_DIR`; **nicht** `getExternalFilesDir`, den unter
  Android 9/10 Apps mit Speicher-Berechtigung mitlesen), dann ohne Speichern-Dialog; vorhandene Datei → Rückfrage
  „Überschreiben?“. Im Ergebnis „Teilen“ (`share_result`) und „Sichern unter …“ (`export_result`) –
  Effekte `Effect::ShareResult`/`ExportResult` → `ShellHooks::share_path`/`export_path` → Plugin.
- **Secrets:** `pida_config::ShellSecretStore` über `mobile::PluginSecureStorage` (Schlüssel
  `PIDA/<SecretId::account>`); Verfügbarkeit beim ersten Zugriff (Probe `secureStoreGet`). Solange
  die native Seite leer ist: nur RAM, `is_persistent() == false` (UI-Hinweis `ng-secrets-not-persistent`).
  Das ID-Austria-Passwort hat keinen `SecretId` und erreicht den Speicher nie.
- **Signiermittel:** keine lokale BKU (Registry nur auf dem Desktop); ID Austria mit
  `BackendOptionView.notes = ["ng-fido2-unavailable-mobile"]` – die Oberfläche zeigt FIDO2 ausgegraut mit
  Hinweis (Auswahl, Masken mit `can_switch_to_fido2`, Einstellungen › FIDO2).
- **PKCS#12-Import:** `import_keystore { path, password, remember }` – der Kern prüft Format und Passwort
  (`backend-keystore::list_aliases`), kopiert die Datei nach `Paths::data_dir/keystores/` (0600), setzt
  Einstellungen › Keystore (aktiviert, `PKCS12`, Alias bei genau einem Schlüssel) und legt das Passwort
  mit `remember` im sicheren Speicher ab (Speicherart „Schlüsselbund“); Ergebnis in
  `ViewState.keystore_import` (`ng-keystore-imported`, `keystore-wrong-password`,
  `ng-keystore-import-format`, `ng-keystore-import-busy` während eines Signiervorgangs …).

## 5. PDFium

- **iOS:** `tools/fetch-pdfium.sh --xcframework` → `tools/pdfium/apple/PDFium.xcframework` mit den Slices
  `ios-arm64` und `ios-arm64_x86_64-simulator` (je `pdfium.framework/pdfium`, install name
  `@rpath/pdfium.framework/pdfium`, ad-hoc signiert). Verwendet `xcodebuild -create-xcframework`; scheitert
  `xcodebuild` (auf dem Entwicklungs-Mac: „failed to load a required plug-in“ ohne
  `xcodebuild -runFirstLaunch`), stellt das Skript dieselbe Struktur direkt zusammen. Im Xcode-Projekt
  „Embed & Sign“. Der Kern bindet `Frameworks/pdfium.framework/pdfium` im Bundle
  (`pida_render::library_paths_in`, `pdfium::mobile_dir`); keine Suche nach dem bloßen Dateinamen.
- **Mindest-iOS:** Die `pdfium-binaries` chromium/8100 für iOS sind mit `minos 17.0` gekennzeichnet; M-iOS setzt
  die Kennzeichnung mit `vtool` auf **16.0** (Plan §5.1, `PDFIUM_IOS_MIN`, docs/integration/mobile-ios.md §3) –
  auf iOS 16 selbst ungetestet.
- **Android:** `tools/fetch-pdfium.sh --jnilibs` → `tools/pdfium/android/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libpdfium.so`
  (keine Abhängigkeit von `libc++_shared`). Der Kern lädt über `dlopen("libpdfium.so")` aus
  `nativeLibraryDir` (wie `System.loadLibrary`); `bundle.android.minSdkVersion = 28` (Plan: Android 9).

## 6. TLS unter Android

`rustls-platform-verifier` braucht JavaVM, Context und Class-Loader. Das Plugin initialisiert ihn beim
Start aus `ndk-context`, das tao beim Anlegen der Activity setzt (`tls::init_platform_verifier`, einziges
`unsafe` mit begründeten SAFETY-Kommentaren). Zusätzlich muss die Kotlin-Komponente ins APK
(Aufgabe M-Android): Maven-Repository
`https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/` und
`implementation("org.rustls:rustls-platform-verifier:<Version von rustls-platform-verifier-android aus Cargo.lock, derzeit 0.2.0>")` (Gruppe `org.rustls`, nicht `rustls` wie in der README des Crates; umgesetzt in M-Android);
die Proguard-Regel liefert `android/consumer-rules.pro`. Ohne Komponente scheitert die erste
A-Trust-Verbindung.

## 7. Oberfläche

- `ViewState.platform.mobile` (`isMobile`) schaltet: Durchsuchen über `mobile_pick_files`; Ergebnis mit
  „Teilen“/„Sichern unter …“ statt „Im Ordner anzeigen“; Einstellungen ohne Ausgabeordner, Proxy,
  Update-Prüfung und Log-Ordner; Keystore-Tab mit Import-Assistent (`features/settings/KeystoreImport.tsx`, auch
  aus der Signiermittel-Auswahl); „Signatur fortsetzen“-Dialog (`app/ResumeDialog.tsx`, Verwerfen nur
  ausdrücklich); FIDO2 ausgegraut mit Hinweis.
- Positionieren per Touch: außerhalb des Blocks angetippt steht der Block mittig **über** dem Finger
  (`TOUCH_OFFSET_PT` 12 pt), ein zweiter Finger verschiebt nichts, kurze Haptik (Android), größere
  Trefferfläche; Zahlenfelder gibt es seit 15.10.2026 nicht mehr (D1). Pinch-Zoom ist optional und offen.
- Safe Areas (`viewport-fit=cover`, `env(safe-area-inset-*)`), Tap-Ziele ≥ 44 px bei `pointer: coarse`,
  (bis E0: Hauptleiste auf schmalen Telefonen mit Namen nur des aktuellen Schritts; seit E1 ohne Hauptleiste).
- Capabilities: `capabilities/mobile.json` (`platforms: ["iOS", "android"]`) – Ereignisse, Kern-Bridge,
  `allow-mobile-pick-files`, `opener` nur `https:`/`mailto:`; **kein** Dateidialog, Log-Ordner, Dateimanager,
  keine DevTools. `capabilities/default.json` gilt nur noch für den Desktop.
- Befunde des Orchestrators behoben: (a) Links im Markup sind echte Inline-Links (`role="link"`), der
  Satzpunkt bleibt am Link (vorher: `<button>` als atomares Inline-Element, Punkt allein in der nächsten
  Zeile); (b) Live-Region und Felder nennen dieselben Kanten (`linke Kante … von links, Oberkante … von
  unten`; `announce` liefert `top` statt `bottom`).

- **Seit E1 (ADR 0004, „Arbeitsbereich“):** ein Bildschirm statt der Schritte; unter 760 px Inspektor als
  Bottom-Sheet (eingeklappt: Konto-Chip, „Signieren“ und die Überdeckungswarnung), Miniaturen eingeklappt;
  Sticker per Touch mit Fingerversatz (Block mittig **über** dem Finger), ein zweiter Finger verschiebt nicht,
  Doppeltippen setzt den Block, Haptik beim Einrasten; Zahlenfelder gibt es nicht mehr (D1). Signierkarte als
  Vollbild-Overlay über der Seitenansicht (App-Wechsel zur ID-Austria-App ist kein Abbruch), FIDO2 ausgegraut
  mit Hinweis; „Fertig“ mit „Öffnen“, „Teilen“, „Sichern unter …“ (kein Ordner), kleinere Vorschau, damit die
  Aktionen ohne Scrollen sichtbar bleiben; PKCS#12-Import aus dem Menü des Konto-Chips; „Signatur fortsetzen“
  mit Kurzhinweis bei ID Austria. Nachweise: `src/app/Mobile.test.tsx` (`?platform=ios|android` im Mock),
  Screenshots `docs/integration/screenshots/stage1/mobil-*.png` (375 × 812 @2x, hell/dunkel). Offen [H]:
  Simulator/Emulator (Safe Areas, App-Wechsel bei der Bestätigung, Touch-Ziehen auf dem Gerät).
- **Seit E2 (ADR 0004 §3.8, Stapel):** ab zwei geteilten bzw. gewählten PDFs die Stapelübersicht (unter 760 px
  einspaltig, „Alle signieren“ über die volle Breite); Ausgabe in die App-Dokumente ohne Ordnerwahl und ohne
  „Im Ordner zeigen“; Stapelkarte als Bottom-Sheet mit „Bestätigung k von n · Name“, Überspringen, Pausen und
  Abbruch-Rückfrage; App-Wechsel je Bestätigung ist kein Abbruch. App-Kill im Lauf: „Signatur fortsetzen“ setzt
  nur das unterbrochene Dokument fort, danach die Übersicht mit dem Rest „Bereit“ – der Lauf geht nicht von selbst
  weiter (Kern: `crates/core-api/tests/mobile.rs`). Nachweise: `features/batch/batch.test.tsx` (Mobile, schmal,
  axe), Screenshots `docs/integration/screenshots/stage2/mobil-*.png`; 360 px ohne waagrechtes Scrollen. Offen
  [H]: Simulator/Emulator (App-Wechsel je Bestätigung, App-Kill im Lauf).
- **Seit E3 (ADR 0004 §3.10/§3.11, Nachtrag 5):** Einstellungen als Vollbild mit waagrechter Bereichsleiste und
  Suche, sofort wirksam; ohne Proxy, Ausgabeordner, Update-Prüfung, Protokolle, Chipkarte und Bildmarken-Zeile
  (der Kern lehnt `proxy.*`, `output.folder`, `general.update_check`, `keystore.file` auf Mobilgeräten mit
  `ng-settings-unavailable-mobile` ab); Keystore nur über den PKCS#12-Import-Assistenten (neuer Stil); Bereich
  „Dokument“ (PDF/A, Platzhaltersuche) wie am Desktop, Hinweise im ausgeklappten Bottom-Sheet; „Zuletzt
  signiert“ ohne Ordneranzeige. Nachweise: `features/settings/{settings,a11y}.test.tsx` (Mobile), Kern
  `crates/core-api/tests/settings.rs::mobile_restrictions`, Screenshots `docs/integration/screenshots/stage3/mobil-*.png`
  (375 × 812 @2x); 375 px ohne waagrechtes Scrollen der Seite. Offen [H]: Simulator/Emulator.

## 8. Bauen und Prüfen

Umgebung wie `tools/env.sh` (Rust 1.99, JDK 21, Android-SDK, NDK 29.0.14206865). Pfad mit Leerzeichen:
`cargo check` für iOS/Android funktioniert direkt; scheitern Gradle/Xcode-Skripte daran, über einen
Symlink ohne Leerzeichen bauen (`ln -s "<worktree>" /tmp/pida-mobile` und dort `npm run tauri …`).

```sh
# PDFium für Mobile (einmalig)
tools/fetch-pdfium.sh --xcframework --jnilibs

# Kern, Shell, Plugin (Desktop-Tests)
cargo test -p pida-workflow -p pida-core-api -p pida-app -p tauri-plugin-pida-mobile -p pida-config
cd apps/app && npm test

# Typprüfung Mobile (iOS kompiliert dabei das Swift-Paket des Plugins)
cargo check -p pida-app --target aarch64-apple-ios-sim
cargo check -p pida-app --target aarch64-apple-ios
NDK=$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin
CC_aarch64_linux_android=$NDK/aarch64-linux-android24-clang AR_aarch64_linux_android=$NDK/llvm-ar \
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK/aarch64-linux-android24-clang \
  cargo check -p pida-app --target aarch64-linux-android
```

Projekte anlegen (M-iOS/M-Android, je in `apps/app`): `npm run tauri ios init` (braucht `xcodegen`:
`brew install xcodegen`; auf dem Entwicklungs-Mac zusätzlich einmal `sudo xcodebuild -runFirstLaunch`),
`npm run tauri android init`. Danach `apps/app/src-tauri/.gitignore` anpassen (`/gen/apple`, `/gen/android`
werden versioniert, nur Build-Ausgaben ignorieren). Kennung `at.pida`: unter Android ist `-` im
Paketnamen unzulässig – Tauri leitet `at.pida` ab; prüfen und in `tauri.conf.json` ggf. per
`bundle.android`/Plattform-Konfiguration festlegen.

Nachweise M0 (15.10.2026): Workspace-Tests grün (`cargo test --workspace`), `npm test` 264 Tests,
`cargo clippy -D warnings` (Desktop, `aarch64-apple-ios`, `aarch64-apple-ios-sim`,
`aarch64-linux-android` mit NDK) für Shell und Plugin; `cargo check -p pida-app` für iOS-Simulator und
Android grün (verbleibende Warnungen nur `dead_code` in `pdf-cos`, `sigblock`, `pades` – Felder, die auf
den Mobil-Zielen ungelesen bleiben; außerhalb von M0, `pdf-cos`/`sigblock` gehören zu T2). Nicht geprüft (Aufgabe M-iOS/M-Android): Start im Simulator/Emulator, native Seiten,
Gradle-Build der Kotlin-Hülle.

## 9. Aufgaben

Gemeinsame Regeln: Kern, Shell-Rust, Plugin-Rust und Oberfläche sind von M0 fertig; Änderungen daran nur
mit Absprache (Integration), Vertrag in §2 nicht einseitig ändern. Keine echten Zugangsdaten, nur
Testdaten (`testdata/pdf`, `testdata/keystores`, Passwort `test`), kein Netzverkehr in Tests.

**M-iOS** (Dateien: `apps/app/plugins/pida-mobile/ios/**`, `apps/app/src-tauri/gen/apple/**`):

1. `tauri ios init`; Projekt versionieren; `PDFium.xcframework` einbinden („Embed & Sign“), Deployment
   Target 17.0.
2. `Info.plist`: `CFBundleDocumentTypes` (`com.adobe.pdf`, Rolle Viewer/Editor), `LSSupportsOpeningDocumentsInPlace`
   (Entscheidung: Kopie in `Inbox` vs. in place mit Security Scope), `UIFileSharingEnabled` (Ausgabe in
   „Dateien“ sichtbar), `ITSAppUsesNonExemptEncryption` (Exportkontrolle), ATS ohne Ausnahmen.
3. Swift: Keychain (`secureStore*`), `UIActivityViewController` (`shareFile`), Dokument-Picker
   (`exportFile`, `importFile` mit Kopie nach `destination`), „Öffnen in“/Share-Eingang inkl.
   Security-Scoped URLs (`takePendingOpenDocuments`).
4. Im Simulator nachweisen: Start, PDFium gebunden (Vorschau), Dokument über „Teilen“ öffnen, Keystore-
   Import + Signatur mit `test-rsa2048.p12`, Ausgabe teilen/sichern, App-Wechsel während einer
   (simulierten) Wartemaske, App beenden → „Signatur fortsetzen“; Screenshots.
5. QuickLook für „Öffnen“ des Ergebnisses (`opener` öffnet auf iOS keine Pfade) – ggf. `ShellHooks::open_path`
   über das Plugin (Vertrag mit M0/Integration abstimmen).

**M-Android** (Dateien: `apps/app/plugins/pida-mobile/android/**`, `apps/app/src-tauri/gen/android/**`) –
**umgesetzt** (15.10.2026), Stand, Nachweise und Befunde: docs/integration/mobile-android.md:

1. `tauri android init`; Projekt versionieren; `jniLibs` aus `tools/pdfium/android/jniLibs` einbinden
   (`sourceSets.main.jniLibs.srcDirs`), `minSdk 28`.
2. Manifest: Intent-Filter `VIEW`, `SEND`, `SEND_MULTIPLE` für `application/pdf` (Activity
   `singleTask`, `configChanges` für Drehen/Dark Mode), `FileProvider` (Authority
   `${applicationId}.pida.files`) für „Teilen“.
3. Kotlin: Android Keystore (`secureStore*`), `ACTION_SEND` (`shareFile`), SAF `ACTION_CREATE_DOCUMENT`
   (`exportFile`), `ACTION_OPEN_DOCUMENT` (`importFile`), Intents in `load()`/`onNewIntent` vormerken und
   über `ContentResolver` nach `destination` kopieren (`takePendingOpenDocuments`).
4. `rustls-platform-verifier`-Komponente (Maven, Version aus `Cargo.lock`, §6) einbinden.
5. Im Emulator `pida_api35` nachweisen wie M-iOS Punkt 4; zusätzlich Drehen während der Wartemaske,
   „Nicht behalten“-Entwickleroption (Activity neu) und Zurück-Taste.
6. „Öffnen“ des Ergebnisses über `ACTION_VIEW` (FileProvider) statt `opener`-Pfad.

**Offen für die Integration:** Pinch-Zoom der Vorschau (optional), Bildmarke aus der Fotomediathek
(HEIC, Plan §5.2), Managed App Configuration/Restrictions (Richtlinien), CI-Jobs mit Simulator/Emulator,
Mindest-iOS (§5).

## 10. Adversarialer Review der Mobile-Welle (15.10.2026)

Geprüft: `git diff main...HEAD` (M0, M-iOS, M-Android) auf Sicherheit (Plan §7.7), Funktion (§5.2/§3.5) und
Belege; beide Plattformen neu gebaut und im Simulator bzw. Emulator durchgespielt. Behobene Befunde:

| # | Schwere | Befund | Behebung |
|---|---|---|---|
| R-1 | hoch | Einstellungen speichern absolute Pfade; iOS vergibt bei jedem Update einen neuen Container (`…/Application/<UUID>/`). Danach war der importierte Keystore „nicht auf diesem Gerät“, und mit nur ID Austria als verfügbarem Signiermittel startete der Kern **ohne Rückfrage** eine ID-Austria-Sitzung (anonymer InfoboxRead bei A-Trust – Ursache der beiden Vorfälle aus M-iOS/M-Android) | (a) `workflow::auto_choice` F-54 wie PDF-Over (`KEYSTORE_ENABLED`): ein **eingeschalteter** Keystore verhindert die automatische Wahl auch dann, wenn die Datei fehlt – es erscheint die Auswahl. (b) `core-api` `relocate_sandbox_files` (nur Mobile): fehlt `[keystore] file` bzw. `signature.emblem`, liegt aber unter gleichem Namen in `data_dir/keystores` bzw. `data_dir/emblem`, wird der Pfad umgestellt und die Passwörter im sicheren Speicher (Konto = Pfad) übertragen (`/var` ↔ `/private/var` berücksichtigt). Im Simulator belegt: Update-Installation → neue Container-UUID → Log `importierter Keystore nach Umzug der Sandbox neu zugeordnet secrets=2`, Signatur ohne Passwortabfrage, pyHanko *VALID* |
| R-2 | hoch | `app.exit` auf Mobile: `Finished` des Kerns (z. B. „Abbrechen“ im Fehlerschritt beim Start, `SKIP_FINISH`) und „Beenden“ ohne Kern riefen `app.exit` – unter iOS Panik in tao + `abort`, unter Android `exit()` gegen laufende Systemthreads | Shell `finish_on_mobile`: Mobile beendet sich nie selbst, sondern startet einen neuen Kern (wieder „Dokument öffnen“); `DispatchOutcome::Quit` ebenso |
| R-3 | mittel | Android-Ausgabe in `getExternalFilesDir(Documents)` – unter Android 9/10 für Apps mit Speicher-Berechtigung lesbar | Ausgabe in `filesDir/Documents` (Datei 0600), FileProvider des Plugins `files-path Documents/`; Teilen, „Sichern unter“, „Öffnen“ im Emulator belegt |
| R-4 | mittel | Kopien im Cache blieben liegen, darunter ein **zweites Exemplar des importierten PKCS#12** (`cache/import/<n>/*.p12`) | Kern entfernt nach erfolgreichem Import die Kopie der Dateiauswahl (nur innerhalb von `Paths::cache_dir`, nach falschem Passwort bleibt sie für den nächsten Versuch); Shell räumt beim Start Kopien in `inbox/` und `import/` älter als 1 h (`STALE_COPY_AGE`, = Höchstalter „Signatur fortsetzen“) weg |
| R-5 | mittel | iOS: importierter Keystore unter `Library/Application Support` landete in iCloud-/Geräte-Sicherungen (Android: `allowBackup=false`, Keychain: `…ThisDeviceOnly`) | Plugin legt `…/data/keystores` beim Laden an (0700) und setzt `isExcludedFromBackup` (geprüft: xattr `com_apple_backup_excludeItem`) |
| R-6 | mittel | Android: von Tauri erzeugter Intent-Filter (`VIEW`/`SEND` mit `BROWSABLE`, implizit `file:`) breiter als nötig | `tauri.android.conf.json` setzt `bundle.fileAssociations = null`, der erzeugte Block im Manifest ist leer; `cmd package query-activities` löst `BROWSABLE`- und `file:`-`VIEW` nicht mehr auf, `content:`-`VIEW`/`SEND` weiter |
| R-7 | mittel | `tauri-plugin-dialog` auch im Mobile-Binary (iOS: Photos/PhotosUI-Bezüge, App-Store-Zweckangaben; Android: zusätzliche Bibliothek), obwohl Mobile nur den Dokument-Picker nutzt | Abhängigkeit und Registrierung nur Desktop (`otool -L`: kein Photos-Framework mehr) |
| R-8 | niedrig | Oberfläche: Überschreiben-Rückfrage mit vollem Sandbox-Pfad; Zugriffstasten-Unterstreichungen auf Touch-Geräten (auch in Dialogen, die als Portal außerhalb von `.app` liegen); „Browse for PDF on your computer“ | nur Dateiname; `data-mobile` auch an `<html>`, CSS blendet `.mnemonic` aus; eigener Text `ng-ui-browse-mobile` („Dokument wählen …“) |
| R-9 | niedrig | Android-Secure-Store verwarf leere Werte (Längenprüfung `>` statt `>=`); `hooks::open_path` fiel unter iOS bei jedem Fehler auf `opener` zurück | korrigiert; Rückfall nur noch bei `not-implemented` |
| R-10 | niedrig | Doku: Keychain-Zugriffsklasse, `openFile` unter iOS, Mindest-iOS veraltet | korrigiert (§2, §5, Plugin-Moduldoku) |

Geprüft und **in Ordnung**: WebView erreicht das Plugin nicht (Tauri prüft die ACL vor `run_mobile_plugin`;
`mobile.json` ohne `pida-mobile:*`), CSP/Capabilities minimal, ID-Austria-Passwort ohne `SecretId` (nur RAM der
Sitzung), Fehler/Logs ohne Pfade, URIs oder Werte, Pfadprüfungen bei Import/Eingang (Rust `within`, Swift
`isInside`, Kotlin `Documents.within`), `resume.json` nur mit relativen Pfaden und Prüfsummen, Keychain
`WhenUnlockedThisDeviceOnly` ohne Synchronisierung, Android-Keystore-AES-GCM mit Schlüssel als AAD, Backup-Regeln,
Lebenszyklus (Hintergrund ≠ Abbruch, Long-Poll-Neustart), FIDO2 ausgegraut, keine lokale BKU.

**Nachweise (diese Prüfung):** iOS-Simulator iPhone 17/iOS 27 – Startprüfung `pida-smoke: ok step=open
pdfium=true`, Keystore-Import über den Dokument-Picker (Kopie im Cache danach entfernt, Datei 0600,
Sicherungsausschluss), Positionieren per Touch, Signiermittel-Auswahl (FIDO2 ausgegraut, Keystore wählbar), Signatur
ohne Passwortabfrage, Teilen, „Sichern unter“ (bitgleich), QuickLook, Überschreiben-Rückfrage nur mit Dateinamen,
Lebenszyklus `suspended=true/false`, Update-Installation mit Umzug des Keystores; drei signierte Dokumente laut
pyHanko *VALID* (`--trust testdata/keystores/test-rsa2048.crt.pem`), `qpdf --check` ohne Fehler. Netzschutz: Start
mit `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY=http://127.0.0.1:9` (System-Proxy des Kerns) – keine Verbindung zu A-Trust.
Android-Emulator `pida_api35` im Flugmodus – Startprüfung `pdfium=true`, Keystore-Import über SAF (Cache-Kopie
entfernt, Secrets nur als `v1:`-Chiffrat), Dokument per `VIEW`-Intent (genau einmal), Ziehen des Blocks, Hintergrund
und zurück ohne Zustandsverlust, Signatur ohne Passwortabfrage, Ausgabe in `files/Documents` (0600), Teilen,
„Sichern unter“ (bitgleich nach `Download/`), „Öffnen“ über den FileProvider; pyHanko *VALID*. Screenshots in
`docs/integration/screenshots/{ios,android}/`.

Desktop ohne Regression: `cargo test --workspace --no-fail-fast` grün (1129 Tests, danach 2 weitere in
`security_config`), `npm test` 266 Tests, `cargo clippy -D warnings` für Shell, Plugin, Kern und Workflow (Desktop,
`aarch64-apple-ios-sim`, `aarch64-linux-android`), Gradle `testDebugUnitTest` 5/5; macOS-Debug-Bundle-Startprüfung
dreimal `pida-smoke: ok step=open pdfium=true` (464/400/384 ms). Hinweis: Läuft bereits eine andere Instanz von
PIDA (Single-Instance-Socket `/tmp/at_pida_si.sock`), übergibt die Startprüfung an sie und gibt nichts
aus – für die Prüfung deshalb mit `--config '{"identifier":"at.pida.smoketest"}'` gebaut.

Offen (nicht behoben, siehe auch mobile-ios.md §7 und mobile-android.md §7): Android beendet den Prozess, wenn das
System die Activity ohne Konfigurationsänderung zerstört (Tauri/wry, upstream); Startprüfung unter Android endet in
`exit()` (FORTIFY-Meldung möglich, nur Debug-Startprüfung); App-Wechsel zur App „ID Austria“ und „Signatur fortsetzen“
nach App-Kill nur durch Kern-Tests belegt (brauchen A-Trust); `CFBundleShortVersionString`/`versionName` 1.0.0 statt
0.1.0; iOS bietet im Share-Sheet die App selbst und „Markup“ an.

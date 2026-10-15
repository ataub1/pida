# Android – Paket M-Android (Projekt, native Plugin-Seite, Nachweise)

Stand: 15.10.2026, Welle Mobile, Paket **M-Android**. Grundlage: docs/integration/mobile.md (Fundament M0,
Vertrag des Plugins §2, Lebenszyklus §3), Plan §5.2/§5.3/§7.7, ADR 0001.

Ergebnis: Die App läuft im Emulator `pida_api35` (Android 15, arm64). PDFium wird zur Laufzeit
geladen, Dokumente kommen über `VIEW`/`SEND` (content://) und die System-Dateiauswahl in die App, der
PKCS#12-Keystore wird importiert, das Passwort liegt verschlüsselt mit einem Android-Keystore-Schlüssel,
ein Test-PDF wird in der App mit dem Keystore signiert, über „Sichern unter …“ (SAF) exportiert und ist laut
pyHanko gültig (§6).

## 1. Projekt `apps/app/src-tauri/gen/android`

Angelegt mit `npm run tauri android init --ci` (Tauri CLI 2.12.3) und versioniert (vorher in
`apps/app/src-tauri/.gitignore` ausgeschlossen; Build-Ausgaben ignorieren die `.gitignore` des Projekts).

| Punkt | Wert / Umsetzung |
|---|---|
| `applicationId`, `namespace` | `at.pida` (Kennung `at.pida`: „-“ ist im Paketnamen unzulässig, Tauri ersetzt es durch „_“). Die Plugin-Klassen bleiben in `at.pida.mobile`. |
| SDK | `minSdk 28` (Android 9, Plan E-15), `targetSdk 35`, `compileSdk 36` (die Tauri-API-Bibliothek `.tauri/tauri-api` verlangt 36; Plattform 36 und Build-Tools 36 lädt AGP beim ersten Build selbst). |
| Werkzeuge | AGP 9.3.1, Kotlin 2.2.10, Gradle 9.6.1 mit `distributionSha256Sum` (Prüfsumme von services.gradle.org) und `networkTimeout=60000` – der Wrapper brach den Download sonst nach 10 s ab. |
| Manifest | Intent-Filter `VIEW` (`content://`, `application/pdf`), `SEND` und `SEND_MULTIPLE` (`application/pdf`); Activity `singleTask` mit `configChanges` für Drehen, Dark Mode, Sprache, Schriftgröße, Tastatur; `windowSoftInputMode=adjustResize`. Berechtigungen nur `INTERNET` und `VIBRATE` (Haptik beim Positionieren), **keine** Speicher-Berechtigungen. |
| Klartext | `usesCleartextTraffic=false` im Release **und** Debug; nur `tauri android dev` (Dev-Server im LAN) braucht `PIDA_ANDROID_DEV_CLEARTEXT=1` beim Build. |
| Sicherung | `allowBackup=false`, `fullBackupContent` und `dataExtractionRules` schließen alle Bereiche aus (Cloud-Sicherung und Gerätewechsel): Einstellungen, importierter Keystore, verschlüsselte Secrets bleiben auf dem Gerät. |
| FileProvider | App: `${applicationId}.fileprovider` (wry, nur `Pictures/` für Kameraaufnahmen der WebView – die Vorlage gab den ganzen externen Speicher frei). Plugin: `${applicationId}.pida.files` (`PidaFileProvider`, nur `files-path Documents/` = Ausgabeordner `filesDir/Documents`, Review R-3) für „Teilen“ und „Öffnen“. Beide nicht exportiert, Lesezugriff nur per Einzelfreigabe. |
| Datei-Zuordnung | `tauri.android.conf.json` setzt `bundle.fileAssociations = null`: der von Tauri erzeugte Filter (`VIEW`/`SEND` mit `BROWSABLE`, implizit `file:`) entfällt, der Block zwischen den Markierungen im Manifest bleibt leer (Review R-6). |
| PDFium | `sourceSets.main.jniLibs.srcDir(tools/pdfium/android/jniLibs)` (aus `tools/fetch-pdfium.sh --jnilibs`, SHA-256 in `tools/pdfium.lock`); anderer Ort mit `-Ppida.pdfiumJniLibs=<pfad>`. Fehlt die Bibliothek, warnt Gradle; die App startet dann ohne Vorschau. Der Kern lädt `libpdfium.so` mit `dlopen` aus `nativeLibraryDir`. |
| TLS | Kotlin-Komponente `org.rustls:rustls-platform-verifier:0.2.0` (Maven-Archiv des Projekts auf GitHub, Repository mit `includeGroup("org.rustls")` im Wurzelprojekt). **Achtung:** Die README des Crates nennt die Gruppe `rustls`, im Archiv heißt sie `org.rustls`. Das Plugin bricht den Build ab, wenn `rustls-platform-verifier-android` in `Cargo.lock` eine andere Version hat. SHA-256 der AAR (Gradle-Cache): `aa021794230fbc2f0be355999e2cf67398dd563de066a2e17001ffbd0b69101b`. |
| Symbole, Farben | Icons aus `apps/app/src-tauri/icons/android` (adaptiv), Fensterfarbe wie die Oberfläche (`#F4F6F9`, dunkel `#11161D`); Standardvorlage (Tauri-Logo, Hello-World-Layout, Android-TV-Eintrag) entfernt. |

### `MainActivity.kt`

- **Eingehende Intents prüfen**, bevor tao (`RunEvent::Opened`) und das Plugin sie sehen: nur `VIEW`/`SEND`/
  `SEND_MULTIPLE`, deren URIs **alle** `content://` fremder Anbieter sind. `file://`, `data:` (Text-`SEND`)
  und URIs der eigenen Provider werden verworfen (ACTION_MAIN) – sonst könnte ein expliziter Intent einer
  anderen App eigene Dateien der Sandbox als „Dokument“ einschleusen (die eigene App darf ihre Provider
  ohne Freigabe lesen; die Shell liest `file:`-URLs selbst).
- **Keine doppelte Auslieferung nach Neuerstellung:** Mit `savedInstanceState` oder
  `FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY` wird der ursprüngliche Intent durch ACTION_MAIN ersetzt (sonst öffnete
  ein Neustart aus „Zuletzt verwendet“ das alte Dokument erneut).
- **Systemleisten:** Android 15 (targetSdk 35) zeichnet immer randlos; die WebView meldet Status- und
  Navigationsleiste nicht als `env(safe-area-inset-*)` – im Querformat lag die Statusleiste über der
  Kopfzeile. Die Activity legt Statusleiste, Navigationsleiste, Display-Ausschnitt und Tastatur als Abstand
  um den Inhalt (Screenshot 07).
- **Zurück-Taste/-Geste:** schließt den obersten Dialog der Oberfläche (Esc an React Aria), sonst
  `moveTaskToBack` – die Activity wird **nicht** beendet. Unter Tauri beendet das Zerstören der einzigen
  Activity auch den Prozess (§7.1) und damit einen laufenden Signiervorgang.
- **Startprüfung (nur Debug-Builds):** Intent-Extra `pida.smoke` setzt `PIDA_SMOKE=1` vor dem Start des
  Kerns (`apps/app/src-tauri/src/smoke.rs`).

## 2. Native Plugin-Seite (`apps/app/plugins/pida-mobile/android`)

Dateien: `PidaMobilePlugin.kt` (Befehle), `SecureStore.kt`, `Documents.kt` (reine Logik, JVM-Tests in
`src/test`), Manifest (FileProvider, `<queries>` für PDF-Anzeige-Apps), `res/xml/pida_share_paths.xml`.
Befehle laufen auf dem UI-Thread (tauri `run_mobile_plugin` → Looper); Dateizugriffe und Verschlüsselung
laufen auf einem eigenen Thread (`pida-mobile-io`), die Antwort kommt von dort. Fehler nur als kurze
Codes, nie mit Pfaden, URIs, Schlüsseln oder Werten (ADR 0001 Regel 9).

| Methode | Android-Umsetzung | Fehlercodes |
|---|---|---|
| `secureStoreGet/Set/Delete` | AES-256-GCM, Schlüssel `pida-secure-store-v1` im Android Keystore (StrongBox, sonst TEE; nicht exportierbar). Werte als `v1:` + Base64(IV ‖ Chiffrat) in den privaten SharedPreferences `pida_secure_store`; Eintragsname SHA-256 des Schlüssels, Schlüssel zusätzlich als AAD. Unlesbare Einträge (Schlüssel verloren) gelten als fehlend und werden entfernt. `EncryptedSharedPreferences` ist abgekündigt und wird nicht verwendet. | `keystore-failed`, `store-write-failed` |
| `shareFile` | `ACTION_SEND` mit FileProvider-URI, `ClipData` und `FLAG_GRANT_READ_URI_PERMISSION` im System-Auswahldialog; PIDA selbst ist ausgeschlossen (`EXTRA_EXCLUDE_COMPONENTS`). | `not-found`, `not-shareable`, `no-app` |
| `openFile` (**neu**, §3) | `ACTION_VIEW` mit FileProvider-URI an eine fremde PDF-App (Ergebnis „Öffnen“, F-87). | `not-found`, `not-shareable`, `no-app` |
| `exportFile` | SAF `ACTION_CREATE_DOCUMENT` (Vorschlag = Dateiname), danach Kopie der Bytes über den `ContentResolver` (`"wt"`); halb geschriebene Dokumente werden gelöscht. Abbruch → `cancelled` → Rust `Ok(false)`. | `cancelled`, `export-failed`, `too-large`, `no-picker` |
| `importFile` | SAF `ACTION_OPEN_DOCUMENT` (`pdf`: `application/pdf`; `pkcs12`: `application/x-pkcs12`, `application/pkcs12`, `application/octet-stream` – der Kern prüft das Format), Kopie nach `destination/<n>/<Anzeigename>`, höchstens 512 MiB. | `cancelled`, `bad-kind`, `destination`, `no-picker` |
| `takePendingOpenDocuments` | Intents aus dem Konstruktor (Kaltstart) und `onNewIntent` werden vorgemerkt; die URIs kommen zusätzlich über tao als Argument. `PendingDocuments`: jede Übergabe **genau einmal** – URIs aus selbst gesehenen Intents nur aus der Warteschlange, übergebene URIs nur, wenn sie hier unbekannt sind (die Reihenfolge von Start-, Rückkehr- und `Opened`-Aufruf ist nicht festgelegt). Nur `content://` fremder Anbieter; Kopie wie `importFile`. | (Einzeldateien werden übersprungen) |

Laufende SAF-Auswahlen sind je Aufruf (`invoke.id`) gemerkt – ein verlorenes Ergebnis blockiert keine
späteren Auswahlen. Lebenszyklus: `onPause`/`onResume` meldet tao der Rust-Seite
(`WindowEvent::Suspended`/`Resumed` → `UiEvent::Backgrounded`/`Foregrounded`, M0); im Log
`Lebenszyklus suspended=true/false` bei jeder Dateiauswahl, ohne dass der Vorgang abbricht.

## 3. Erweiterung des Vertrags (gemeinsame Dateien, minimal)

„Öffnen“ des Ergebnisses ging unter Android nicht: `opener` übergibt `file://`-Pfade, die Android anderen
Apps verweigert (Hinweis „Konnte signiertes Dokument nicht öffnen“). Deshalb:

- `apps/app/plugins/pida-mobile/src/lib.rs`: `PidaMobile::open_file(path, mime)` → native Methode
  `openFile` `{ path, mimeType }` → `{}` (Tabelle in der Moduldoku, Vertragstest `open_file_contract`).
- `apps/app/src-tauri/src/hooks.rs` `TauriHooks::open_path`: auf Mobile zuerst das Plugin; unter Android ist
  dessen Fehler endgültig, **unter iOS** fällt die Shell bei jedem Fehler (Methode fehlt in der Swift-Seite)
  auf `opener` zurück – M-iOS kann `openFile` (QuickLook) ergänzen, ohne die Rust-Seite zu ändern.
- `apps/app/plugins/pida-mobile/src/tls.rs`: nur Doku (Maven-Gruppe `org.rustls`).
- `apps/app/src-tauri/.gitignore`: Zeile `/gen/android` entfernt.

## 4. Bauen

Pfad mit Leerzeichen: Gradle, NDK und Tauri CLI kamen damit zurecht; gebaut wurde trotzdem über den Symlink
`/tmp/pida-android` → Worktree (die CLI löst ihn wieder in den echten Pfad auf).

```sh
source tools/env.sh
tools/fetch-pdfium.sh --jnilibs                       # einmalig: tools/pdfium/android/jniLibs/<abi>/libpdfium.so
cd apps/app
export CARGO_TARGET_DIR="$PWD/../../target-android"   # eigenes Ziel (parallel zu iOS)
export CARGO_PROFILE_DEV_DEBUG=line-tables-only       # Debug-.so 163 MB statt mehrerer hundert
npm run tauri android build -- --debug --target aarch64 --apk
#  → src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk (187 MB)

# JVM-Unit-Tests der Plugin-Logik (ohne Gerät)
cd src-tauri/gen/android && ./gradlew :tauri-plugin-pida-mobile:testDebugUnitTest
```

Emulator und Startprüfung:

```sh
emulator -avd pida_api35 -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect &
adb wait-for-device; until [ "$(adb shell getprop sys.boot_completed | tr -d '\r')" = 1 ]; do sleep 2; done
adb install -r app-universal-debug.apk
adb shell am start -W -n at.pida/.MainActivity --ez pida.smoke true
adb logcat -d | grep pida-smoke                  # Tag RustStdoutStderr (tao leitet stdout/stderr um)
# Dokument per VIEW (die Freigabe klappt nur als Shell-Nutzer, nicht unter `adb root`)
adb push testdata/pdf/generated/simple-xref-table.pdf /sdcard/Download/Vertrag.pdf
adb shell content call --method scan_volume --uri content://media --arg external_primary
adb shell am start -a android.intent.action.VIEW -d content://media/external/file/<id> -t application/pdf \
  --grant-read-uri-permission -n at.pida/.MainActivity
adb emu kill
```

Die WebView ist in Debug-Builds über `adb forward tcp:9222 localabstract:webview_devtools_remote_<pid>`
(Chrome DevTools Protocol) erreichbar; so lassen sich Bedienelemente finden und antippen (`adb shell input`).

## 5. Nachweise (15.10.2026, Emulator `pida_api35`, Android 15, arm64, Sprache de-AT)

- **Start:** `pida-smoke: ok step=open pdfium=true webview_ms=1003 core_ms=1003` (erster Lauf), im
  letzten Build `… pdfium=true webview_ms=1993 core_ms=1995`; Kaltstart `TotalTime` 0,4–1,7 s.
  `pdfium=true` = `libpdfium.so` zur Laufzeit gebunden, die Vorschau rendert (Screenshot 02).
- **Dokument-Eingang:** `VIEW` (Kaltstart): `Dokumente vorgemerkt: 1` → `Dokumente eingegangen count=1` bei zwei
  `takePendingOpenDocuments`-Aufrufen (genau einmal). `SEND` aus der App „Dateien“ (Teilen → PIDA) in die
  laufende App: ebenso genau einmal. Dateiauswahl (`importFile`, `pdf`): Dokument geöffnet. Ohne URI-Freigabe
  (`am start` gibt `EXTRA_STREAM` keine Freigabe) wird das Dokument übersprungen (`unreadable`), ohne Absturz.
- **Abgewiesene Intents:** `VIEW file:///data/user/0/at.pida/config/settings.toml`, `VIEW` auf den eigenen
  Provider (`content://at.pida.pida.files/…`) und `SEND text/plain`: je „Intent ohne zulässiges
  Dokument verworfen“, kein Dokument geöffnet.
- **Positionieren per Touch:** Ziehen mit `adb shell input swipe` verschiebt den Block (Screenshot 02).
- **Signiermittel:** ID Austria mit „FIDO2 nicht verfügbar“ (durchgestrichen) und Hinweis, keine lokale BKU;
  nach dem Import zusätzlich „Keystore“ (Screenshot 03).
- **Keystore-Import:** `testdata/keystores/test-rsa2048.p12` über die SAF-Auswahl, Passwort `test`, „Passwort
  sicher auf diesem Gerät speichern“: `PKCS#12-Import ok=true code=ng-keystore-imported`, Hinweis „Das Passwort
  ist im sicheren Speicher des Geräts abgelegt“ (`keychain_available`). Kopie
  `data/keystores/test-rsa2048.p12` mit `-rw-------`; `shared_prefs/pida_secure_store.xml` enthält nur
  SHA-256-Namen und `v1:`-Chiffrate (kein Klartext `test`). Nach Neuinstallation (neuer Prozess) signiert die App
  ohne Passwortabfrage (`secureStoreGet` liefert das Passwort).
- **Signatur und Export:** Signatur mit dem Keystore in der App („Signatur war erfolgreich“, Screenshot 05),
  Ausgabe (Stand M-Android; seit dem Review `files/Documents` im internen Speicher) `Android/data/at.pida/files/Documents/Vertrag_signed.pdf`; „Sichern unter …“ → SAF →
  `Download/Vertrag_signed.pdf` (bitgleich, SHA-256 `093b844f…d56e`). Auf dem Mac mit `adb pull`:

  ```text
  pyhanko sign validate --pretty-print --no-strict-syntax --trust testdata/keystores/test-rsa2048.crt.pem Vertrag_signed.pdf
  Certificate subject: "Common Name: PDF-Over NG Test RSA (NUR TEST), …"   The signer's certificate is trusted.
  The signature is cryptographically sound.   The signature covers the entire file.
  The signature is judged VALID.
  ```

  Wiederholt mit dem letzten Build (`Download/Vertrag_signed (1).pdf`, `--trust-replace`): VALID, deckt die
  ganze Datei ab; `qpdf --check` ohne Fehler.
- **Teilen:** Auswahldialog mit „Vertrag_signed.pdf“, PIDA selbst nicht mehr als Ziel (Screenshot 06).
- **Öffnen:** `openFile` startet die PDF-Anzeige (Drive-Viewer) mit dem signierten Dokument (Screenshot 08).
- **Lebenszyklus:** Dateiauswahl, Export, Teilen und Anzeige-App pausieren die App (`Lebenszyklus
  suspended=true` … `false`), der Vorgang läuft danach weiter (Export gelang nach der Rückkehr).
- **Drehen** während des Positionierens: gleicher Prozess, gleiche Activity, Schritt bleibt (Screenshot 07).
- **Zurück:** schließt den offenen Konfigurationsdialog; ohne Dialog geht die App in den Hintergrund, Prozess und
  Schritt bleiben (`LaunchState: HOT`).
- **Tests:** `cargo test -p tauri-plugin-pida-mobile -p pida-app` grün (Plugin 9, Shell 41 + 3 + 5);
  `cargo clippy -p pida-app -p tauri-plugin-pida-mobile` (Desktop und `aarch64-linux-android`) ohne
  Warnungen in diesen Crates (verbleibend nur `dead_code` in `pdf-cos`, `sigblock`, `pades`, wie in M0);
  Gradle `testDebugUnitTest` 5/5. Der APK enthält `org.rustls.platformverifier.*`; die Warnung
  „TLS-Prüfung der Plattform nicht initialisiert“ trat nicht auf.

Screenshots (`docs/integration/screenshots/android/`, 432 px breit, JPEG; `07-querformat.png` aus M-Android, übrige aus
dem Review): `01-start`, `02-positionieren`, `03-signiermittel`, `04-keystore-import`, `05-fertig`, `06-teilen`,
`07-querformat`, `08-oeffnen`, `09-sichern-unter`, `10-keystore-dateiauswahl`.

**Review (15.10.2026, mobile.md §10):** neu gebaut, im Flugmodus erneut durchgespielt – Startprüfung `pdfium=true`,
Keystore-Import über SAF (Kopie in `cache/import` danach entfernt), `VIEW`-Intent genau einmal, Ziehen des Blocks,
Hintergrund/Vordergrund ohne Zustandsverlust, Signatur ohne Passwortabfrage, Ausgabe `files/Documents/Vertrag_signed.pdf`
(0600), Teilen und „Öffnen“ über den FileProvider (`content://at.pida.pida.files/…`), „Sichern unter“ bitgleich
nach `Download/`; pyHanko *VALID*. `cmd package query-activities`: `VIEW` mit `BROWSABLE` bzw. `file:` löst nicht mehr
auf PIDA auf.

## 6. Nicht nachgewiesen

- **ID Austria** (Anmeldung, SMS-TAN, App-Wechsel mit Long-Poll nach Rückkehr) und damit **„Signatur
  fortsetzen“ nach App-Kill**: brauchen A-Trust. Mit dem Keystore liegt zwischen `prepare` und Ende keine
  Wartezeit (der Passwortdialog kommt vor `prepare`), ein Kill lässt sich dort nicht platzieren. Abgedeckt nur
  durch die Kern-Tests aus M0 (`crates/core-api/tests/mobile.rs`, `crates/workflow/tests/lifecycle.rs`).
- TLS über den Android-Truststore (kein Netzverkehr in den Nachweisen).
- Release-Build (R8, Signatur), echte Geräte, Android 9–14, x86/armv7.

## 7. Befunde und offene Punkte

1. **Activity zerstört ⇒ Prozess endet.** Wird die Activity nicht wegen einer Konfigurationsänderung zerstört
   (Entwickleroption „Aktivitäten nicht behalten“, Speichermangel im Hintergrund, z. B. während der App „ID
   Austria“), entfernt tauri-runtime-wry das letzte Fenster und beendet die App (`ExitRequested`); wry legt die
   WebView nur nach Konfigurationsänderungen neu an. Nachgewiesen mit „Aktivitäten nicht behalten“: neuer
   Prozess, App startet bei „Dokument öffnen“. Folge: Ein Vorgang **vor** `prepare` ist verloren, danach greift
   „Signatur fortsetzen“. Ein `prevent_exit` hülfe nicht (ohne Fenster bliebe die neue Activity leer). Lösung
   braucht Tauri/wry (WebView für eine neue Activity neu anlegen) – Integration/Upstream.
2. **Abbruch beim Beenden:** Nach `app.exit` (Startprüfung) bzw. beim Ende nach Punkt 1 meldet libc
   `FORTIFY: pthread_mutex_lock called on a destroyed mutex` im HWUI-Thread, teils `SIGABRT` (Prozessende läuft
   über `exit()` mit statischen Destruktoren, während Android-Threads laufen). Die Startprüfung meldet vorher
   `ok`; für Nutzer kann ein Absturzhinweis erscheinen. Integration: Mobile-Ende über `finish()` ohne
   `exit()` oder `_exit` prüfen (M-iOS hat `smoke.rs` für iOS bereits auf `process::exit` umgestellt).
3. **Behoben im Review (mobile.md §10, R-3).** **Ausgabeordner:** `getExternalFilesDir(Documents)` ist unter Android 9/10 für Apps mit Speicher-Berechtigung
   lesbar und in „Dateien“ nicht sichtbar (Nutzer brauchen ohnehin „Teilen“/„Sichern unter“). Empfehlung:
   unter Android `filesDir/Documents` (Shell `mobile_options`, gemeinsame Datei) und die FileProvider-Pfade
   anpassen.
4. **Behoben (R-4).** **Kopien im Cache:** `cache/inbox/<n>/` und `cache/import/<n>/` wurden nie aufgeräumt (Dokumente bleiben,
   bis das System den Cache leert). Aufräumen beim Start oder nach Abschluss (Shell) empfohlen.
5. **Weitgehend behoben (R-8; offen: Warnung „keine Blockvorschau“ beim Laden).** **Oberfläche auf Android**: Tastatur-Kürzel-Unterstreichungen
   („A_b_lehnen“) erscheinen auch auf Touch-Geräten; die Überschreiben-Rückfrage nennt den vollen Pfad
   `/storage/emulated/0/Android/data/…`; der englische Text „Browse for PDF on your computer“ passt nicht zu
   Mobile; beim Laden erscheint einmal `WARN Rendern fehlgeschlagen error=keine Blockvorschau verfügbar`.
6. **Datenschutzhinweis (seit ADR 0003 ohne Dialog):** Ist ID Austria das einzige Signiermittel, startete
   „Zustimmen und fortfahren“ sofort die ID-Austria-Anmeldung (wie PDF-Over). Heute startet ID Austria ohne
   „Signieren“ nie von selbst – die Auswahl mit Kurzhinweis erscheint. Bei den Tests wurde so einmal unbeabsichtigt für etwa 2 s eine
   ID-Austria-Sitzung begonnen (Emulator online, keine Zugangsdaten eingegeben, sofort abgebrochen); danach lief
   der Emulator im Flugmodus.
7. **ABIs:** `tools/fetch-pdfium.sh --jnilibs` liefert arm64-v8a, armeabi-v7a, x86_64; ein Universal-Build ohne
   `--target` enthält auch x86 (i686) – dort ohne PDFium.
8. **Google Play:** `targetSdk 35` wie vorgegeben; die Play-Vorgaben für neue Apps/Updates 2026 (voraussichtlich
   API 36) vor einer Veröffentlichung prüfen. Release-Signatur, R8-Regeln (Tauri-/Plugin-Consumer-Regeln sind
   vorhanden) und App Bundle sind offen.
9. Gradle meldet Abkündigungen der Tauri-Vorlage (AGP 9 `android.newDsl=false`, Java 8 als Quelle/Ziel) –
   kommen mit einer neueren Tauri-Vorlage weg.

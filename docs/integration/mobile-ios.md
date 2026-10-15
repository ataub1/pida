# iOS – Paket M-iOS (Projekt, native Plugin-Seite, Nachweise)

Stand: 15.10.2026, Welle Mobile, Paket **M-iOS** auf dem Fundament M0 (docs/integration/mobile.md).
Gemessen auf macOS 27 (arm64), Xcode 27.0 (27A266a), Swift 6.4, Rust 1.99, Tauri 2.12.4 / CLI 2.12.3,
iOS-Simulator **iOS 27.0** (iPhone 17), PDFium chromium/8100.

Ergebnis: Die App baut für den Simulator, startet (`pdfover-smoke: ok … pdfium=true`), nimmt Dokumente über
„Öffnen in“/Teilen (Kalt- und Warmstart) und die Dateiauswahl an, importiert PKCS#12 in die Sandbox, legt das
Keystore-Passwort in der Keychain ab und signiert damit gültig (pyHanko: *VALID*). Ergebnis teilen, „Sichern
unter …“ und „Öffnen“ (QuickLook) laufen nativ. FIDO2 ist ausgegraut mit Hinweis.

## 1. Projekt `apps/app/src-tauri/gen/apple`

Angelegt mit `npm run tauri ios init -- --ci --skip-targets-install` und **versioniert** (Build-Ausgaben
`build/`, `Externals/`, `.tauri/` ignoriert `gen/apple/.gitignore`). Quelle der Wahrheit ist
`project.yml` (XcodeGen); nach Änderungen `cd apps/app/src-tauri/gen/apple && xcodegen generate`.
**Nicht** erneut `tauri ios init` ausführen – das überschreibt `project.yml`.

| Punkt | Umsetzung |
|---|---|
| Bundle-ID | `at.pdf-over-ng` (`tauri.conf.json` `identifier`, `PRODUCT_BUNDLE_IDENTIFIER`); Anzeigename „PDF-Over NG“ |
| Mindest-iOS | **16.0** (Plan §5.1): `tauri.conf.json` `bundle.iOS.minimumSystemVersion`, `project.yml` `deploymentTarget`, `Podfile`; App-Binary `LC_BUILD_VERSION minos 16.0` (geprüft mit `vtool -show-build`). Abweichung zu M0 (17.0) siehe §3 |
| Dokumenttypen | `CFBundleDocumentTypes`: `com.adobe.pdf`, Rolle `Viewer`, `LSHandlerRank` **`Alternate`** (nie Standard-App für PDF); Tauri ergänzt beim Bauen `CFBundleTypeExtensions: [pdf]` aus `bundle.fileAssociations` |
| Öffnen am Ort | `LSSupportsOpeningDocumentsInPlace = YES` – iOS übergibt das Original als Security-Scoped URL, die App kopiert die **Bytes** in die Sandbox (§2.5). Zusammen mit `UIFileSharingEnabled = YES` erscheint `Documents` (signierte Dokumente) in der Dateien-App unter „Auf meinem iPhone › PDF-Over NG“ (Screenshots 05, 08) |
| Exportkontrolle | `ITSAppUsesNonExemptEncryption = NO`: nur Standardverfahren (TLS, Signatur/Authentisierung) – **vom Auftraggeber zu bestätigen** (Exportkontroll-Einstufung) |
| ATS | keine Ausnahmen (`NSAppTransportSecurity` fehlt bewusst): nur HTTPS, keine lokale BKU. Die WebView lädt die gebündelte Oberfläche (`tauri://`, `pdfover-render://`), nicht `devUrl` |
| Face ID, Kamera, Fotos | keine Zweckangaben (`NSFaceIDUsageDescription` u. a.), weil nicht genutzt |
| Share-Extension | keine (Plan: „Öffnen in“ reicht; die App erscheint im Teilen-Menü über die Dokumenttypen, Screenshot 02) |
| Szenen | kein `UIApplicationSceneManifest`. iOS 27 führt die App trotzdem im Scene-Lebenszyklus aus (`KnownSceneSessions` im Container); tao 0.37 unterstützt beide Wege (App-Delegate bis iOS 26, Szenen ab iOS 27) |
| Orientierung | Vorlage: iPhone Hochformat + Querformat, iPad alle |
| Signierung | Simulator: ad hoc („Sign to Run Locally“). Für Geräte `bundle.iOS.developmentTeam` bzw. `APPLE_DEVELOPMENT_TEAM` setzen (nicht Teil von M-iOS) |

### PDFium

`project.yml` bettet `tools/pdfium/apple/PDFium.xcframework` (aus `tools/fetch-pdfium.sh --xcframework`) mit
**Embed & Sign** ein, **ohne zu linken** (`link: false`): `pdfover-render` lädt
`<App>.app/Frameworks/pdfium.framework/pdfium` zur Laufzeit (`pdfium::mobile_dir`). Xcode wählt den Slice
(`ios-arm64_x86_64-simulator` bzw. `ios-arm64`) und signiert ihn mit (Simulator: ad hoc). Bindung zur
Laufzeit belegt die Startprüfung (`pdfium=true`) und die Seitenvorschau (Screenshot 03). Fehlt das
xcframework, bricht der Xcode-Build ab – vorher `tools/fetch-pdfium.sh --xcframework` ausführen.

## 2. Native Plugin-Seite (`apps/app/plugins/pdfover-mobile/ios`)

Swift-Paket (Swift-Tools 5.3, Paket-Mindestversion iOS 14 für UTType/Dokument-Picker, App iOS 16), Dateien in
`Sources/`. Die Methoden laufen auf der seriellen IPC-Warteschlange von Tauri; UIKit nur über
`DispatchQueue.main`, Dateikopien auf einer eigenen Warteschlange (der sichere Speicher wartet nie hinter
einem iCloud-Download). Fehler nur als kurzer Code (`invoke.reject(_, code:)`), nie mit Pfaden oder Werten.

| Methode | Umsetzung | Codes |
|---|---|---|
| `secureStoreGet/Set/Delete` | `SecureStore.swift`: `kSecClassGenericPassword`, Dienst `<Bundle-ID>.secrets`, Konto = Schlüssel der Rust-Seite, **`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`**, nicht synchronisiert. Setzen = `SecItemUpdate`, sonst `SecItemAdd`; Löschen eines fehlenden Eintrags ist kein Fehler | `keychain-locked`, `keychain-unavailable`, `keychain-error`, `keychain-bad-data` |
| (Start) | Keychain-Einträge überleben das Löschen der App: Fehlt die Markierung in `UserDefaults` (erste Ausführung nach einer Installation), löscht das Plugin alle Einträge seines Dienstes. Außerdem legt es `Library/Application Support/<Bundle-ID>/data/keystores` (0700) an und nimmt den Ordner von iCloud-/Geräte-Sicherungen aus (`isExcludedFromBackup`, Review R-5) | – |
| `shareFile` | `UIActivityViewController` mit der Datei-URL (nur Dateien in der Sandbox); `{}` sobald gezeigt; iPad: Popover mittig | `file-missing`, `no-view` |
| `openFile` | **QuickLook** (`QLPreviewController`, Bearbeiten gesperrt – jede Änderung bräche die Signatur); darin auch Teilen/Drucken. Vertrag von M-Android eingeführt (`PdfoverMobile::open_file`, `ShellHooks::open_path` auf Mobile) | `file-missing`, `no-preview`, `no-view` |
| `exportFile` | `UIDocumentPickerViewController(forExporting:asCopy: true)`; weicht `fileName` ab, zuerst eine benannte Kopie in `tmp/export-…`; Abbruch (auch Wegwischen) → `cancelled` | `file-missing`, `busy`, `no-view`, `io-error` |
| `importFile` | `UIDocumentPickerViewController(forOpeningContentTypes:asCopy: true)`, `pdf` → `UTType.pdf`, `pkcs12` → `com.rsa.pkcs-12`/`.p12`/`.pfx`; Kopie nach `<destination>/<n>/<Name>`, iOS-Kopie in `tmp` danach gelöscht | `bad-args`, `bad-destination`, `busy`, `cancelled` |
| `takePendingOpenDocuments` | `OpenedDocuments.swift` (§2.5) | `bad-destination` |

`destination` muss absolut sein, ohne `..`, innerhalb des App-Containers; Dateien höchstens 512 MiB, nur
reguläre Dateien, Namen ohne Pfadanteile/Steuerzeichen (wie `mobile::safe_name`).

### 2.5 Eingehende Dokumente

Mit „Öffnen am Ort“ liefert iOS Security-Scoped URLs; lesbar sind sie nur über **genau das `URL`-Objekt**.
tao wandelt die URL für `RunEvent::Opened` in einen String (die Sandbox-Erweiterung geht verloren) und meldet
beim Kaltstart im Scene-Lebenszyklus die URLs aus `UIScene.ConnectionOptions` gar nicht. Deshalb hängt sich
das Plugin beim Laden – vor `UIApplicationMain`, Tauri initialisiert die Plugins in `Builder::build` – vor
die Delegate-Methoden von tao (`TaoSceneDelegate scene:willConnectToSession:options:` und
`scene:openURLContexts:`, `AppDelegate application:openURL:options:`), merkt sich die URL-Objekte und ruft
danach die ursprüngliche Methode auf. tao selbst bleibt unverändert.

- **Kaltstart:** URLs aus den Verbindungsoptionen liefert der erste `takePendingOpenDocuments` von selbst.
- **Laufende App:** tao meldet die URL; liest die Rust-Seite sie selbst (Simulator, eigene Dateien), ist sie
  fertig; sonst reicht sie die URL weiter, und das Plugin öffnet sie über das gemerkte Objekt
  (`startAccessingSecurityScopedResource`, `NSFileCoordinator` für File Provider wie iCloud Drive).
  Nicht angefragte Einträge verfallen nach 10 min.
- **`Documents/Inbox`** (AirDrop, Mail): übernommene Kopien entfernt das Plugin; ältere als 10 min räumt es
  auf, damit sie nicht in der Dateien-App stehen bleiben.
- Fehlt eine Klasse/Methode (andere tao-Version), protokolliert das Plugin „hook … nicht installiert“ und es
  bleibt beim Weg ohne Hook (Kopien in der Sandbox funktionieren weiter, Security-Scoped URLs auf Geräten
  nicht). Bei jedem tao-Update prüfen.

### 2.6 Lebenszyklus

Ohne Zutun des Plugins: tao bildet `sceneWillResignActive`/`applicationWillResignActive` auf
`WindowEvent::Suspended` und `sceneWillEnterForeground`/`applicationWillEnterForeground` auf
`WindowEvent::Resumed` ab, die Shell macht daraus `UiEvent::Backgrounded`/`Foregrounded` (kein Abbruch,
Long-Poll-Neustart nach der Rückkehr, M0). Ein zusätzlicher Weg aus Swift würde die Ereignisse doppeln.
Bewusst **keine** Hintergrundzeit (`beginBackgroundTask`): Das System hielte den Vorgang dann bis zu ~30 s
nach dem Wechsel zur App „ID Austria“ am Leben und könnte ihn an beliebiger Stelle (auch nach der
Bestätigung, mitten in der Signaturabholung) einfrieren; ohne Hintergrundzeit endet die Verbindung fast immer
im Long-Poll, den der Kern nach der Rückkehr gezielt neu startet.

## 3. Gemeinsame Dateien (minimal)

- `tauri.conf.json`: nur `bundle.iOS.minimumSystemVersion` 17.0 → **16.0**.
- `tools/fetch-pdfium.sh --xcframework`: neue Variable `PDFIUM_IOS_MIN` (Standard **16.0**, leer = wie die
  Binaries). pdfium-binaries chromium/8100 baut mit `ios_deployment_target = 17.0`, importiert aber nur
  Funktionen aus libSystem, CoreFoundation und CoreGraphics, die es lange vor iOS 16 gibt (`nm -u`: 164
  Symbole, u. a. `CGBitmapContextCreate`, `CFStringCreateWithCStringNoCopy`, `aligned_alloc`), und linkt
  libc++ statisch. Das Skript setzt deshalb `LC_BUILD_VERSION` mit `vtool -set-build-version` auf 16.0
  (nur die Kennzeichnung, nicht der Code) und `MinimumOSVersion` im Framework entsprechend. **Nicht auf
  iOS 16 getestet** (auf diesem Mac gibt es nur die iOS-27-Laufzeit) – vor einer Freigabe auf einem
  iOS-16-Gerät prüfen oder PDFium selbst mit `ios_deployment_target = "16.0"` bauen.
- `apps/app/src-tauri/src/smoke.rs`: Die Startprüfung beendet die App unter iOS mit `process::exit`. tao kennt
  auf iOS kein `ControlFlow::Exit`; `app.exit` endete in einer Panik im Run-Loop-Beobachter und damit in
  `abort` (Absturzbericht nach jedem Smoke-Lauf).
- `apps/app/src-tauri/.gitignore`: `/gen/apple` nicht mehr ignoriert. Wurzel-`.gitignore`: `/target-*/`
  (eigene Target-Ordner der Mobil-Pakete).

## 4. Werkzeuge und Bauen

Einmalig auf dem Entwicklungs-Mac (für M-iOS so eingerichtet):

```sh
xcodebuild -downloadPlatform iOS        # iOS-Simulator-Laufzeit (iOS 27.0, 8 GB) – fehlte trotz Xcode
brew install xcodegen cocoapods          # tauri ios init verlangt beide (CocoaPods ungenutzt, Podfile leer)
rustup component add llvm-tools          # swift-rs 1.0.9: Xcode 27 macht @_cdecl-Symbole lokal, swift-rs
                                         # macht sie mit llvm-objcopy wieder global (sonst Linkfehler)
tools/fetch-pdfium.sh --xcframework      # tools/pdfium/apple/PDFium.xcframework (jetzt auch per xcodebuild)
```

`tauri ios init` installierte zusätzlich `libimobiledevice` (Homebrew). `xcodebuild -create-xcframework`
funktioniert inzwischen (die erste `simctl`-Ausführung hat die Xcode-Erstkomponenten nachinstalliert); der
Rückfall im Skript bleibt für andere Rechner.

Bauen über einen Pfad ohne Leerzeichen (geprüft; mit eigenem Target-Ordner, damit M-Android parallel bauen
kann):

```sh
ln -sfn "<worktree>" /tmp/pdfover-ng-ios
cd /tmp/pdfover-ng-ios/apps/app && source ../../tools/env.sh
export CARGO_TARGET_DIR=/tmp/pdfover-ng-ios/target-ios LANG=en_US.UTF-8
npx tauri ios build --target aarch64-sim --debug
#  → src-tauri/gen/apple/build/arm64-sim/PDF-Over NG.app (Debug 224 MB: Binary 209 MB unoptimiert mit Debug-Infos, PDFium universal 14 MB)

DEV=<UDID>; xcrun simctl boot $DEV; xcrun simctl install $DEV "src-tauri/gen/apple/build/arm64-sim/PDF-Over NG.app"
SIMCTL_CHILD_PDFOVER_SMOKE=1 xcrun simctl launch $DEV at.pdf-over-ng
xcrun simctl spawn $DEV log show --last 1m --predicate 'process == "PDF-Over NG"' | grep pdfover-smoke
```

stdout/stderr leitet Tauri unter iOS in das System-Log um (`log show`), die App-Logdatei liegt im Container
unter `Library/Application Support/at.pdf-over-ng/logs/`. Prüfungen: `cargo clippy --no-deps -p pdfover-app
-p tauri-plugin-pdfover-mobile --target aarch64-apple-ios{,-sim} -- -D warnings` (grün; der Build des
Geräte-Ziels kompiliert dabei auch das Swift-Paket für `arm64-apple-ios`), `cargo test -p pdfover-app -p
tauri-plugin-pdfover-mobile` (58 Tests grün), Swift-Paket ohne Warnungen der eigenen Quellen
(`swift build --triple arm64-apple-ios16.0[-simulator] --sdk …`).

## 5. Nachweise (15.10.2026, iPhone 17, iOS 27.0, Sprache de)

| Nr. | Prüfung | Ergebnis |
|---|---|---|
| I-01 | Simulator-Build | `** BUILD SUCCEEDED **`; `Frameworks/pdfium.framework` (universal x86_64+arm64, ad hoc signiert), App und PDFium `minos 16.0` |
| I-02 | Startprüfung `PDFOVER_SMOKE=1` | `pdfover-smoke: ok step=open pdfium=true webview_ms=913 core_ms=914` (vorher 660 ms warm), Prozessende ohne Absturzbericht (vor dem Fix in `smoke.rs`: SIGABRT) |
| I-03 | Startbildschirm | Screenshot 01 |
| I-04 | „Öffnen in“ **Kaltstart** (App beendet, Vorschau › Teilen › PDF-Over NG) | Log `Dokumente eingegangen count=1`, Positionieren mit gerenderter Seite; Lieferung über den Hook `scene:willConnectToSession:options:` (tao meldet hier nichts) |
| I-05 | „Öffnen in“ **laufende App** | Screenshot 02; genau ein Dokument (`inbox/2/Testdokument.pdf`), kein Duplikat |
| I-06 | Dateiauswahl PDF/PKCS#12 | Dokument-Picker filtert (p12 bzw. PDF ausgegraut), Ordner „PDF-Over NG“ sichtbar (Screenshot 05) |
| I-07 | Positionieren per Touch | Block folgt dem Finger, steht darüber; Felder 38 mm / 249 mm (Screenshot 03) |
| I-08 | Signiermittel-Auswahl | ID Austria mit „FIDO2 nicht verfügbar“ (durchgestrichen) und Hinweis, Keystore wählbar (Screenshot 04) |
| I-09 | PKCS#12-Import `test-rsa2048.p12`, Passwort `test`, „sicher speichern“ | `ng-keystore-imported`, Datei 0600 in `data/keystores/`, Einstellungen `password_storage = "keychain"` (Screenshot 06) |
| I-10 | Keystore-Signatur in der App | ohne Passwortabfrage (Keychain), auch **nach Neustart des Prozesses**; drei Läufe, pyHanko `--trust test-rsa2048.crt.pem`: *trusted, cryptographically sound, covers the entire file, VALID*; `qpdf --check` ohne Fehler |
| I-11 | Ergebnis teilen | Share-Sheet mit `Testdokument_signed.pdf` (Screenshot 07) |
| I-12 | „Sichern unter …“ | Export-Picker mit Ziel „PDF-Over NG“ (Screenshot 08); Abbrechen durch Wegwischen → keine Fehlermeldung |
| I-13 | „Öffnen“ | QuickLook-Vorschau mit Signaturblock (Screenshot 09); vor `openFile`: Hinweis „Konnte signiertes Dokument nicht öffnen“ |
| I-14 | Lebenszyklus | Home-Taste → `Lebenszyklus suspended=true`, zurück → `suspended=false`, gleicher Prozess, Zustand erhalten |
| I-15 | Dateien-App | „Auf meinem iPhone › PDF-Over NG“ zeigt `Testdokument_signed.pdf` |

Screenshots: `docs/integration/screenshots/ios/01-start.jpg` … `11-ueberschreiben-nur-dateiname.jpg` (verkleinert; 01, 03–11
neu aus dem Review, mobile.md §10; 02 aus M-iOS).

**Review (15.10.2026, mobile.md §10):** neu gebaut und erneut durchgespielt – Startprüfung `pdfium=true`,
Keystore-Import (Cache-Kopie entfernt, `data/keystores` mit Sicherungsausschluss), Signatur ohne Passwortabfrage,
Teilen, „Sichern unter“, QuickLook, Überschreiben-Rückfrage nur mit Dateinamen (Screenshot 11), Update-Installation
mit neuer Container-UUID: Keystore und Keychain-Passwörter neu zugeordnet, Signatur weiter ohne Abfrage. Alle
signierten Dokumente laut pyHanko *VALID*. Gestartet mit `SIMCTL_CHILD_HTTPS_PROXY=http://127.0.0.1:9` (u. a.), damit
kein Request A-Trust erreichen kann.

## 6. Nicht nachgewiesen

- **App-Wechsel während der Wartemaske** und **„Signatur fortsetzen“ nach App-Kill**: Beides braucht einen
  Vorgang, der nach `prepare` wartet – das gibt es nur mit ID Austria (A-Trust), Keystore signiert sofort.
  Die Logik ist durch die Kern-Tests von M0 abgedeckt (`core-api` `tests/mobile.rs`, `workflow`
  `tests/lifecycle.rs`); der Simulator belegt nur die Lebenszyklus-Ereignisse (I-14).
- Security-Scoped URLs auf einem **Gerät** (der Simulator erzwingt die Sandbox beim Lesen nicht; dort liest
  schon die Rust-Seite). Ebenso iCloud-Drive-Dateien (Download über `NSFileCoordinator`).
- iOS 16 bis 26 (nur iOS-27-Laufzeit vorhanden): App-Delegate-Weg von tao, PDFium mit umgesetztem
  `minos`.
- Geräte-Build mit Team-Signierung, App-Store-Validierung.

## 7. Befunde und offene Punkte

1. **Behoben im Review (mobile.md §10, R-1).** **Kern, hoch: absolute Pfade in den Einstellungen.** iOS vergibt bei jeder Installation/Aktualisierung
   einen neuen Container-Pfad (`…/Data/Application/<UUID>/`). `[keystore] file` (und
   `general.last_source_directory`) speichern den absoluten Pfad – nach einem App-Update ist der Keystore
   „nicht auf diesem Gerät“, obwohl die Datei mitgewandert ist. Folge im Test: Mit nur einem verbleibenden
   Signiermittel startete die App ohne Auswahl direkt ID Austria (anonymer InfoboxRead-Request an A-Trust,
   sofort abgebrochen). Vorschlag: Keystore-Pfad relativ zu `Paths::data_dir` speichern bzw. beim Laden
   unter `data_dir/keystores/<Name>` neu auflösen (core-api/config, nicht M-iOS).
2. **Behoben (R-2).** **Shell:** `app.exit` panikt unter iOS (tao); betroffen sind außer der Startprüfung `Finished` →
   `app.exit(code)` und `DispatchOutcome::Quit` in `lib.rs` (z. B. „Beenden“ im Fehlerschritt ohne Kern).
   Auf Mobile nicht beenden bzw. `process::exit` nur nach Rückfrage.
3. **Behoben (R-8).** **Oberfläche:** Rückfrage „Datei … existiert bereits“ zeigt den vollen Sandbox-Pfad (unlesbar, läuft aus
   dem Dialog) – auf Mobile nur den Dateinamen. Unterstrichene Tastenkürzel (Mnemonics) sind auf dem
   Telefon ohne Nutzen.
4. **Behoben (R-4).** **Kern/Shell:** Kopien im Cache bleiben liegen: `Caches/…/import/<n>/test-rsa2048.p12` (der importierte
   Keystore liegt danach ein zweites Mal im Cache) und `inbox/<n>/…`. Nach dem Import bzw. nach Abschluss
   löschen.
5. Logs im System-Log enthalten ANSI-Farbcodes (stderr-Ausgabe im Debug-Build).
6. **Behoben (R-7).** `tauri-plugin-dialog` ist auch auf Mobile registriert und zieht `Photos`/`PhotosUI` in das Binary – vor
   der App-Store-Einreichung prüfen, ob Zweckangaben verlangt werden, sonst das Plugin nur auf dem Desktop
   registrieren.
7. `CFBundleShortVersionString`/`CFBundleVersion` setzt die CLI auf 1.0.0, die App meldet 0.1.0 –
   `version` in `tauri.conf.json` bzw. `bundle.iOS.bundleVersion` festlegen.
8. **Behoben (R-10).** Doku-Tabellen in `apps/app/plugins/pdfover-mobile/src/lib.rs` und docs/integration/mobile.md §2 nennen
   für die Keychain noch `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`; umgesetzt ist (Vorgabe M-iOS)
   `…WhenUnlockedThisDeviceOnly` – der Kern liest Secrets nur im Vordergrund. Ebenso ist dort `openFile`
   für iOS noch als fehlend beschrieben.
9. Hooks vor den Delegates von tao (§2.5) hängen an Klassen- und Selektornamen von tao 0.37.1 – bei
   Tauri-Updates prüfen (Log „hook … nicht installiert“).
10. Optional: Pinch-Zoom in der Vorschau, Bildmarke aus der Fotomediathek, Managed App Configuration.

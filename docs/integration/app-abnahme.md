# Abnahme der Desktop-App (PDF-Over NG, Welle 3)

Gegenstand: `apps/app` – Oberfläche (React 19, React Aria Components, Vite) und Tauri-2.12-Shell
(`apps/app/src-tauri`, Crate `pdfover-app`) um den Rust-Kern `pdfover-core-api`. Dieses Dokument hält
fest, was automatisch nachgewiesen ist, und enthält die Checkliste für die **manuelle Prüfung** durch
den Auftraggeber (Bildschirme, Tastatur, Drag & Drop, ID Austria mit Testkonto).

Stand: 15.10.2026, Welle 3 Track T1, gemessen auf macOS 27 (arm64), Xcode 27, Rust 1.99.0, Node 22,
PDFium chromium/8100 (`tools/pdfium.lock`). Windows und Linux baut und prüft die CI (Job `bundle`).

## 1. Automatische Nachweise

| Nr. | Prüfung | Befehl | Ergebnis |
|---|---|---|---|
| N-01 | Oberfläche installieren, bauen, testen | `cd apps/app && npm ci && npm run build && npm test` | grün: tsc strict + Vite-Build, 10 Testdateien, 236 Tests |
| N-02 | Shell-Crate | `cargo test -p pdfover-app` | grün: 32 Tests (Startargumente, Kernstart/Fehlerschritt, Shell-Funktionen, Startprüfung, Custom Protocol, Dateien, Sicherheitskonfiguration, Serde-Vertrag) |
| N-03 | Vertrag Kern ↔ UI | `cargo test -p pdfover-core-api --test ts_bindings` | grün: `generated.ts` entspricht den Rust-Typen (ts-rs 12.0.1) |
| N-04 | Debug-Bundle mit PDFium und Drittlizenzen | `npm run notices && PDFOVER_PDFIUM_DIR=… npm run bundle:mac:debug` | `PDF-Over NG.app` **121,3 MiB** (Binary 112 MiB unoptimiert mit Debug-Infos, `Contents/Frameworks/libpdfium.dylib` 7,0 MiB, `Contents/Resources/THIRD-PARTY-NOTICES.md` + `RUST-STD-COPYRIGHT.html`) |
| N-05 | Release-Bundle | `PDFOVER_PDFIUM_DIR=… CI=true npm run bundle:mac:release` | `PDF-Over NG.app` **28,6 MiB** (Binary 19 MiB), `PDF-Over NG_0.1.0_aarch64.dmg` **12,1 MiB** – unter dem Größen-Gate ≤ 25 MB je Installer (Plan §9.1) |
| N-06 | Startprüfung Debug-Bundle (3 Läufe) | `PDFOVER_SMOKE=1 HOME="$(mktemp -d)" "…/debug/bundle/macos/PDF-Over NG.app/Contents/MacOS/pdf-over-ng"` | `pdfover-smoke: ok step=open pdfium=true`, WebView + Kern bereit nach **471 ms (kalt) bzw. 371–380 ms (warm)**, Prozessende nach 1,7 s bzw. 0,4 s |
| N-07 | Startprüfung Release-Bundle (3 Läufe) | wie N-06 mit `…/release/bundle/…` | `ok step=open pdfium=true`, WebView + Kern bereit nach **447 ms (kalt) bzw. 342 ms (warm)**, Prozessende nach 0,35–0,78 s |
| N-08 | Startargumente im Bundle | `… pdf-over-ng <testdata/pdf/generated/simple-xref-table.pdf> -a` | `ok step=backend` (Dokument geladen, automatische Position, weiter zur Signiermittel-Auswahl) |
| N-09 | Ungültige Startargumente | `… pdf-over-ng /gibt/es/nicht.pdf` bzw. `-b FALSCH` | `ok step=error` (Fehlerschritt `error-FileNotExist` bzw. `error-CmdLineArgs`, Kern startet nicht) |
| N-10 | Hilfe/Version ohne Fenster | `… pdf-over-ng -h`, `--version` | zweisprachige Hilfe (Systemsprache) bzw. `PDF-Over NG 0.1.0` auf stdout, Ende ohne Fenster |
| N-11 | Lizenz-Gate | `cargo deny --locked check`, `node tools/npm-licenses.mjs apps/app/package-lock.json` | grün (begründete Ausnahmen in `deny.toml`/`tools/npm-licenses.mjs`) |

Hinweise zur Messung:

- Die **Startprüfung** (`PDFOVER_SMOKE=1`, `apps/app/src-tauri/src/smoke.rs`) beendet die App, sobald die
  Oberfläche über IPC den ersten ViewState abgefragt hat (WebView, JavaScript-Bundle und IPC laufen) und
  der Kern einen Schritt außer `starting` meldet. `webview_ms`/`core_ms` messen ab Prozessstart; `pdfium`
  sagt, ob die mitgelieferte Bibliothek gebunden wurde. Ohne Antwort binnen 60 s endet die App mit
  Exit-Code 3. `HOME` auf einen leeren Ordner setzen, damit Log und Fensterzustand nicht im echten
  Benutzerprofil landen.
- **DMG ohne Bildschirmsitzung**: Das DMG-Skript von Tauri richtet das Finder-Fenster per AppleScript
  ein und wartet dabei ohne Freigabe „Automation → Finder“ unbegrenzt. In CI und Terminal-Sitzungen
  `CI=true` setzen (Tauri überspringt dann die Finder-Gestaltung); auf einem Arbeitsplatz mit Freigabe
  geht es ohne.
- Ein Release-Build lädt PDFium **nie** aus `PDFOVER_PDFIUM_DIR`, nur aus dem Bundle (ADR 0060 §1);
  N-07 belegt, dass die Bibliothek dort gefunden wird.
- Signieren und Codesignatur/Notarisierung sind nicht Teil dieser Abnahme (Signier-Secrets beim
  Auftraggeber, Plan §9.3). Das ungesignierte Bundle startet lokal; nach einem Download blockiert
  Gatekeeper es.

## 2. Manuelle Prüfung – Checkliste

Vorbereitung: Release-Bundle (N-05) bzw. `npm run tauri dev` im Repo; ein leeres Testprofil
(`HOME=$(mktemp -d) open -n "PDF-Over NG.app"` bzw. neues macOS-Benutzerkonto), Test-PDFs aus
`testdata/` (u. a. `pdf/generated/simple-xref-table.pdf`, QR-Platzhalter
`pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf`), Test-Keystore `testdata/keystores/test-rsa2048.p12`
(Passwort `test`, NUR TEST). Ergebnis je Zeile: ✓ / ✗ mit Notiz und ggf. Log
(Einstellungen › Über › „Log-Ordner anzeigen“).

### 2.1 Bildschirme und Abläufe

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| S-01 | App starten (leeres Profil) | Schritt „Öffnen“, Hauptleiste Öffnen aktiv, Positionieren/Signieren/Ende gesperrt; Sprache = Systemsprache (DE/EN) |
| S-02 | Altkonfiguration `~/.pdf-over/PDF-Over.config` vorhanden, noch keine NG-Einstellungen | Migrationsassistent: Übernehmen / Nicht übernehmen; mit Klartext-Passwörtern zusätzlich die Frage nach dem Schlüsselbund; „Nicht übernehmen“ fragt nicht wieder |
| S-03 | „Durchsuchen“ (⌘O) → PDF wählen | Laden/Analyse mit Abbrechen, danach Positionieren mit gerenderter Seite und Blockvorschau |
| S-04 | PDF mit Nutzungsrechten (`/Perms /UR3`), Hybrid-XFA bzw. unerfüllbaren Seed Values | Schritt „Hinweis zum Dokument“ mit Liste; „Trotzdem signieren“ geht weiter, „Dokument verwerfen“ zurück zu Öffnen |
| S-05 | PDF mit leeren Signaturfeldern bzw. QR-Platzhalter | Frage „Platzhalter verwenden?“ (Ja/Nein/Abbrechen), bei mehreren Feldern Auswahl |
| S-06 | QR-Platzhalter mit `profile=…` | signiert mit dem Profil des Platzhalters; unbekanntes Profil → Hinweis `ng-placeholder-profile-unknown`, eingestelltes Profil |
| S-07 | Positionieren: Ziehen, Seitenwechsel, „Neue Seite“, „Automatisch platzieren“, Zahlenfelder | Block folgt; Live-Region nennt Seite/Position; „Neue Seite“ auf bereits signiertem Dokument bleibt auf der letzten Seite und zeigt `placement.notice.new_page_not_allowed` |
| S-08 | „Unsichtbar signieren“ | nur dieses Dokument ohne sichtbaren Block (Profil-Einstellung unverändert) |
| S-09 | Signiermittel-Auswahl | ID Austria, lokale BKU (nur wenn erreichbar), Keystore (wenn eingerichtet); Datenschutzhinweis einmal je Backend vor dem ersten Upload |
| S-10 | Keystore-Signatur mit Test-Keystore | Passwortabfrage, Signatur, Ergebnis-Schritt; Datei `<name>_signed.pdf` neben der Quelle bzw. im Ausgabeordner |
| S-11 | Ergebnis: Öffnen, Im Ordner anzeigen, weiteres Dokument | Standard-PDF-Anzeige öffnet die Ausgabe; Finder zeigt sie; Warteschlange führt zum nächsten Dokument |
| S-12 | Ausgabe existiert bereits | Rückfrage „Überschreiben?“; Nein → Speichern-Knopf bleibt |
| S-13 | Kein Ausgabeordner eingestellt | Speichern-Dialog mit Startordner der Quelle und Namensvorschlag `<name>_signed.pdf`; Abbrechen → Knopf „Speichern“ |
| S-14 | Fehler (z. B. falsches Keystore-Passwort, defektes PDF) | Fehlerschritt mit verständlichem Text, „Details“ (Code), Wiederholen wo sinnvoll |
| S-15 | Einstellungen (⌘,): alle Tabs, Speichern, Verwerfen, Zurücksetzen | Feldfehler am Feld (z. B. Proxy ohne Host); Blockvorschau; Zurücksetzen mit Bestätigung |
| S-16 | Start mit `-c alt.config` (Altdatei) | Einstellungen mit Hinweis „werden nicht gespeichert“ (schreibgeschützt), Änderungen gelten bis Programmende |
| S-17 | Über: Version, Datenschutz, Log-Ordner, **„Drittlizenzen anzeigen“** | öffnet `THIRD-PARTY-NOTICES.md` aus dem Bundle |
| S-18 | Hinweise (Toasts) | Hinweise des Kerns stapeln, einzeln schließbar; Info-Hinweise verschwinden nach 8 s, Fehler bleiben |
| S-19 | Beenden während einer offenen Maske (⌘Q / Fenster schließen) | Vorgang wird abgebrochen, Einstellungen gesichert, App endet (spätestens nach 5 s) |
| S-20 | Fenstergröße/-position | bleibt nach Neustart erhalten |

### 2.2 Tastatur und Barrierefreiheit

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| K-01 | Nur Tastatur: Tab/Umschalt+Tab durch alle Schritte | sichtbarer Fokus, logische Reihenfolge, „Zum Inhalt springen“ als erstes Element |
| K-02 | Zugriffstasten (macOS ⌃⌥+Taste, Windows/Linux Alt+Taste) | lösen die unterstrichenen Aktionen aus; doppelt belegte Tasten wandern reihum |
| K-03 | ⌘O / ⌘, (Strg unter Windows/Linux) | Dateidialog bzw. Einstellungen |
| K-04 | Positionieren: Pfeiltasten (15 pt), mit Umschalt (1 pt), mit ⌘/Strg (50 pt), Enter | Block bewegt sich, Live-Region kündigt an, Enter bestätigt |
| K-05 | Esc in Dialogen | schließt ohne Änderung |
| K-06 | VoiceOver (macOS), NVDA (Windows), Orca (Linux) | Rollen und Namen aller Bedienelemente, Statusmeldungen werden vorgelesen (Protokoll anlegen; axe-core/Screenreader-Protokolle stehen noch aus) |
| K-07 | Hell/Dunkel, erhöhter Kontrast, reduzierte Bewegung, Fenster schmal (360 px) | lesbar, keine abgeschnittenen Bedienelemente |

### 2.3 Drag & Drop, „Öffnen mit“, Single-Instance, Startargumente

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| D-01 | PDF(s) auf das Fenster ziehen (Schritt Öffnen bzw. Ergebnis) | Dokumente kommen in die Warteschlange; Hervorhebung während des Ziehens |
| D-02 | PDF in einem anderen Schritt ablegen | Hinweis „hier nicht möglich“, nichts passiert |
| D-03 | Bild auf das Bildmarken-Feld in den Einstellungen ziehen | Bildmarke übernommen |
| D-04 | Finder: „Öffnen mit › PDF-Over NG“ bzw. PDF auf das Dock-Symbol (App läuft nicht) | App startet mit dem Dokument |
| D-05 | wie D-04, App läuft bereits | Dokument geht an die laufende Instanz, Fenster kommt nach vorn (keine zweite Instanz) |
| D-06 | Terminal: `pdf-over-ng -b KS -ks test-rsa2048.p12 -kspass test -a -s -o <ordner> a.pdf b.pdf` | beide Dokumente ohne Rückfragen signiert, App endet nach dem letzten Speichern (`-s`) |
| D-07 | `pdf-over-ng -v true a.pdf`, `-e logo.png`, `-n 0664…`, `-proxy host -proxyport 8080` | unsichtbar; Bildmarke; Benutzername vorbelegt (`+43664…`); Proxy nur für diese Sitzung (nicht gespeichert) |
| D-08 | `pdf-over-ng fehlt.pdf`, `pdf-over-ng -b FALSCH`, `pdf-over-ng --headless` | Fehlerschritt ohne Wiederholen (`error-FileNotExist`, `error-CmdLineArgs`, `ng-ui-args-cli-only`), Abbrechen beendet; Passwörter erscheinen nie in der Anzeige oder im Log |
| D-09 | `pdf-over-ng -h` | Hilfe auf stdout, kein Fenster |

### 2.4 ID Austria mit Testkonto (nur durch den Auftraggeber)

Nur mit dem A-Trust-Testkonto bzw. Testtelefon des Auftraggebers; keine echten Zugangsdaten in Tickets
oder Logs. Vorab Datenschutzhinweis bestätigen.

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| A-01 | Anmeldung (Telefonnummer/Benutzername + Passwort, „Merken“) | Maske nativ in der App; falsches Passwort → `ng-atrust-login-rejected`; mit „Merken“ ist der Benutzername beim nächsten Mal vorbelegt (Passwort nie gespeichert) |
| A-02 | Zweiter Faktor: ID-Austria-App (Push/QR) | Warte-Anzeige, nach Bestätigung in der App weiter |
| A-03 | Zweiter Faktor: SMS-TAN | TAN-Maske mit Referenzwert; falsche TAN → Hinweis, erneute Eingabe |
| A-04 | FIDO2-Sicherheitsschlüssel (macOS/Linux CTAP-HID, Windows Hello) | Aufforderung zum Berühren; Schlüssel mit PIN: Hinweis `fido2-pin-unsupported` (CTAP2-clientPIN folgt) |
| A-05 | „Signaturdaten anzeigen“ | öffnet die A-Trust-Seite im Systembrowser (nur `https://`, A-Trust-Host) |
| A-06 | Abbrechen in jeder Maske | zurück zur Signiermittel-Auswahl, nichts gespeichert |
| A-07 | Signatur prüfen | Ausgabe in Adobe Acrobat und mit RTR-Prüfdienst/DSS gültig; `qpdf --check` ohne Fehler |
| A-08 | Mehrere Dokumente (Warteschlange) | je Dokument eine Bestätigung; Anmeldung je Batch |
| A-09 | Zertifikats-Cache (Einstellungen › Opt-in) – zweite Signatur | kein zweites Lesen des Zertifikats (eine Bestätigung weniger); Block zeigt korrekten Namen/Seriennummer |
| A-10 | Zertifikats-Cache – anderes Konto bzw. erneuertes Zertifikat | Signatur wird verworfen, Fehlerschritt `ng-certcache-mismatch` mit Begründung; „Wiederholen“ liest das Zertifikat neu und fragt erneut nach der Bestätigung |
| A-11 | Netzwerk weg / Proxy mit Anmeldung | verständliche Meldung (`error-FailedToConnect`, Proxy-Passwortabfrage) |
| A-12 | Log nach A-01…A-11 | keine Passwörter, TANs, Dokumentinhalte, Benutzernamen im Klartext |

### 2.5 Lokale BKU (falls verfügbar)

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| B-01 | BKU läuft (127.0.0.1:3495) | Option erscheint binnen 5 s, Signatur mit Karte |
| B-02 | BKU beendet | Option verschwindet bzw. ist gesperrt (Polling 30 s) |

## 3. Bekannte Grenzen (Stand dieser Abnahme)

- Windows- und Linux-Bundles sind nur über die CI gebaut (Job `bundle`, Startprüfung dort zunächst
  informativ); die Drittlizenz-Hinweise kommen unter Windows noch nicht ins Debug-Bundle.
- Keine Codesignatur/Notarisierung, kein Updater, kein Release-Workflow (Plan §9.3/§9.4).
- CTAP2-clientPIN (FIDO2 mit PIN) im HID-Provider fehlt (Welle 2, Paket I1).
- Mobile (iOS/Android): Kern-Crates bauen für iOS; Shell, SecretStore und Lebenszyklus folgen in der
  Mobile-Welle.
- Barrierefreiheit: Komponententests prüfen Rollen/Namen/Tastatur; axe-core und Screenreader-Protokolle
  (K-06) stehen aus.

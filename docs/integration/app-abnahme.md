# Abnahme der Desktop-App (PIDA, Welle 3)

Gegenstand: `apps/app` – Oberfläche (React 19, React Aria Components, Vite) und Tauri-2.12-Shell
(`apps/app/src-tauri`, Crate `pida-app`) um den Rust-Kern `pida-core-api`. Dieses Dokument hält
fest, was automatisch nachgewiesen ist, und enthält die Checkliste für die **manuelle Prüfung** durch
den Auftraggeber (Bildschirme, Tastatur, Drag & Drop, ID Austria mit Testkonto).

Stand: 15.10.2026, Welle 3 Track T1; Etappe **E1** der neuen Oberfläche „Arbeitsbereich“ (ADR 0004 §5, Merge
K1 + A1, `workspace_ui` eingeschaltet) ergänzt am 15.10.2026; gemessen auf macOS 27 (arm64), Xcode 27, Rust 1.99.0, Node 22,
PDFium chromium/8100 (`tools/pdfium.lock`). Windows und Linux baut und prüft die CI (Job `bundle`).

## 1. Automatische Nachweise

| Nr. | Prüfung | Befehl | Ergebnis |
|---|---|---|---|
| N-01 | Oberfläche installieren, bauen, testen | `cd apps/app && npm ci && npm run build && npm test` | grün: tsc strict + Vite-Build, 10 Testdateien, 237 Tests (nach dem Review); seit E1 15 Testdateien, 379 Tests (inkl. axe-core, Mock-Projektion, Token-Kontrast, echte Kernzustände) |
| N-02 | Shell-Crate | `cargo test -p pida-app` | grün: 40 Tests nach dem Review (Startargumente, Kernstart/Fehlerschritt, Shell-Funktionen, Startprüfung, Custom Protocol, Dateien, frühe Dokumente/Single-Instance-Socket, PDFium-Auflösung, Sicherheitskonfiguration, Serde-Vertrag) |
| N-03 | Vertrag Kern ↔ UI | `cargo test -p pida-core-api --test ts_bindings` | grün: `generated.ts` entspricht den Rust-Typen (ts-rs 12.0.1) |
| N-04 | Debug-Bundle mit PDFium und Drittlizenzen | `npm run notices && PIDA_PDFIUM_DIR=… npm run bundle:mac:debug` | `PIDA.app` **121,3 MiB** (Binary 112 MiB unoptimiert mit Debug-Infos, `Contents/Frameworks/libpdfium.dylib` 7,0 MiB, `Contents/Resources/THIRD-PARTY-NOTICES.md` + `RUST-STD-COPYRIGHT.html`) |
| N-05 | Release-Bundle | `PIDA_PDFIUM_DIR=… CI=true npm run bundle:mac:release` | `PIDA.app` **28,6 MiB** (Binary 19 MiB), `PIDA_0.1.0_aarch64.dmg` **12,1 MiB** – unter dem Größen-Gate ≤ 25 MB je Installer (Plan §9.1) |
| N-06 | Startprüfung Debug-Bundle (3 Läufe) | `PIDA_SMOKE=1 HOME="$(mktemp -d)" "…/debug/bundle/macos/PIDA.app/Contents/MacOS/pida"` | `pida-smoke: ok step=open pdfium=true`, WebView + Kern bereit nach **471 ms (kalt) bzw. 371–380 ms (warm)**, Prozessende nach 1,7 s bzw. 0,4 s |
| N-07 | Startprüfung Release-Bundle (3 Läufe) | wie N-06 mit `…/release/bundle/…` | `ok step=open pdfium=true`, WebView + Kern bereit nach **447 ms (kalt) bzw. 342 ms (warm)**, Prozessende nach 0,35–0,78 s |
| N-08 | Startargumente im Bundle | `… pida <testdata/pdf/generated/simple-xref-table.pdf> -a` | seit E1 `ok step=position`: Arbeitsbereich, automatisch platziert, **keine** Sitzung ohne „Signieren“ (ADR 0004 A1; bis E0: `step=backend`) |
| N-09 | Ungültige Startargumente | `… pida /gibt/es/nicht.pdf` bzw. `-b FALSCH` | `ok step=error` (Fehlerschritt `error-FileNotExist` bzw. `error-CmdLineArgs`, Kern startet nicht) |
| N-10 | Hilfe/Version ohne Fenster | `… pida -h`, `--version` | zweisprachige Hilfe (Systemsprache) bzw. `PIDA 0.1.0` auf stdout, Ende ohne Fenster |
| N-11 | Lizenz-Gate | `cargo deny --locked check`, `node tools/npm-licenses.mjs apps/app/package-lock.json` | grün (begründete Ausnahmen in `deny.toml`/`tools/npm-licenses.mjs`) |
| N-12 | „Öffnen mit“ beim Kaltstart (Review) | `open -n -W -a "…/PIDA.app" --env PIDA_SMOKE=1 --env HOME=<leer> --stdout <datei> doc.pdf` | `ok step=position` (Dokument geöffnet); der Stand vor dem Review (58e072b) lieferte `step=open` – das Dokument ging verloren, weil `RunEvent::Opened` vor `setup` kam |
| N-13 | Fremder Single-Instance-Socket (Review) | Datei `/tmp/at_pida_si.sock` anlegen, App starten | `ok step=open`, Log: Warnung „Start ohne Single-Instance“; die App sendet nichts an den Pfad |
| N-14 | Zertifikats-Cache mit echter Engine (Review) | `cargo test -p pida-core-api --test real_pades` | Treffer ohne InfoboxRead, qpdf/pyHanko intakt, Signer = Testzertifikat; abweichendes Zertifikat → `ng-certcache-mismatch` (`cms-unexpected-signer`), nichts gespeichert, „Wiederholen“ signiert gültig |
| N-15 | Startargumente (Review) | `… pida "file://…/a%20b.pdf"`, `--log-level warn` | `ok step=position` bzw. nur Warnungen im Log |
| N-16 | E1: Debug-Bundle mit Arbeitsbereich | `PIDA_SMOKE=1 PIDA_HOME=<leer> …/pida` ohne Dokument, mit `a.pdf`, mit `-b MOBILE -a a.pdf` | `ok step=open pdfium=true` (WebView + Kern nach 519–539 ms) bzw. `ok step=position` (beide Male Arbeitsbereich, keine Sitzung) |
| N-17 | E1: echte Signatur im Bundle (D-06) | `PIDA_HOME=<leer> …/pida -b KS -ks testdata/keystores/test-rsa2048.p12 -kspass test -a -s -o <ordner> a.pdf b.pdf` | beide Dokumente signiert, App endet mit 0, keine Log-Warnung (Blockvorschau erst ab Revision 1 abgerufen); `pyhanko sign validate --trust testdata/keystores/test-rsa2048.crt.pem`: `INTACT:TRUSTED,UNTOUCHED`, `qpdf --check` ohne Fehler |
| N-18 | E1: Kern im Arbeitsbereich | `cargo test -p pida-core-api --test workspace` (mit `PIDA_PDFIUM_DIR`) | 16 Tests: Kernpfad Keystore mit echter Engine (Startlage = `plan::build_plan` mit `PositionChoice::Auto`, pyHanko INTACT, Unterzeichner = Test-CN), `-b MOBILE -a` ohne Sitzung, D-06, Konto-Chip, ohne PDFium, Abbruch, Wiederholen ohne zweite Anmeldung und ohne zweites `certificate()`, Ablehnung, „Abbrechen“ in einer Meldung (nur die A-Trust-Fehlerseite gilt als Ablehnung), Felder, QR, Einschränkungen, Layout ohne PDFium im Ereignispfad, Zustände für die Oberfläche (N-20) |
| N-19 | E1: Invariante A1 (F-54) | `cargo test -p pida-workflow --test props --test workspace` | Property-Test `workspace_session_only_after_explicit_action` (50 Ereignisarten, zufällige Einstellungen) und 19 Tabellentests grün |
| N-20 | E1: Oberfläche gegen echte Kernzustände | `cd apps/app && npm test` (`src/app/RealCore.test.tsx`, Fixture `src/core/fixtures/real-core-views.json` aus N-18) | Seitenansicht bleibt vom Bearbeiten bis „Fertig“ eingehängt, Karte je Schritt (Anmeldung, Netzfehler, SMS-TAN), Wortlaut F3, kein Zustand stößt eine Sitzung an |
| N-21 | E1: Mobile-Typprüfung | `cargo check -p pida-app --target aarch64-apple-ios-sim` bzw. `--target aarch64-linux-android` (NDK 29) | grün (nur die bekannte Warnung in `pida-pades`) |

Hinweise zur Messung:

- **Gestaltung E1** neben dem Prototyp (`prototypes/pida-workspace/docs/screenshots/`): Mock-Kern im Browser
  (`npm run dev:mock`, eigener Port), Desktop 1280 × 800 und 375 × 812 @2x, hell/dunkel/EN, 25 Aufnahmen in
  `docs/integration/screenshots/stage1/`. Gewollte Abweichungen: keine „Prototyp“-Plakette und keine
  Demo-Steuerung, „Befehle ⌘K“ erst mit der Palette (E4), „Prüfen“ gesperrt, Startlage nach der PDF-AS-Regel
  (waagrecht mittig unter dem Inhalt), leere Signaturfelder nur angeboten (ADR 0004 §8 F2), Wortlaut F3 auf
  „Fertig“.

- Die **Startprüfung** (`PIDA_SMOKE=1`, `apps/app/src-tauri/src/smoke.rs`) beendet die App, sobald die
  Oberfläche über IPC den ersten ViewState abgefragt hat (WebView, JavaScript-Bundle und IPC laufen) und
  der Kern einen Schritt außer `starting` meldet. `webview_ms`/`core_ms` messen ab Prozessstart; `pdfium`
  sagt, ob die mitgelieferte Bibliothek gebunden wurde. Ohne Antwort binnen 60 s endet die App mit
  Exit-Code 3. `HOME` auf einen leeren Ordner setzen, damit Log und Fensterzustand nicht im echten
  Benutzerprofil landen.
- **DMG ohne Bildschirmsitzung**: Das DMG-Skript von Tauri richtet das Finder-Fenster per AppleScript
  ein und wartet dabei ohne Freigabe „Automation → Finder“ unbegrenzt. In CI und Terminal-Sitzungen
  `CI=true` setzen (Tauri überspringt dann die Finder-Gestaltung); auf einem Arbeitsplatz mit Freigabe
  geht es ohne.
- Ein Release-Build lädt PDFium **nie** aus `PIDA_PDFIUM_DIR` oder Systempfaden, nur aus dem Bundle
  (ADR 0060 §1; fehlt sie dort, nennt die Shell dem Kern den Ordner der Binary – dann ohne Vorschau,
  `apps/app/src-tauri/src/pdfium.rs::resolve`); N-07 belegt, dass die Bibliothek im Bundle gefunden wird.
- Signieren und Codesignatur/Notarisierung sind nicht Teil dieser Abnahme (Signier-Secrets beim
  Auftraggeber, Plan §9.3). Das ungesignierte Bundle startet lokal; nach einem Download blockiert
  Gatekeeper es.

## 2. Manuelle Prüfung – Checkliste

Vorbereitung: Release-Bundle (N-05) bzw. `npm run tauri dev` im Repo; ein leeres Testprofil
(`HOME=$(mktemp -d) open -n "PIDA.app"` bzw. neues macOS-Benutzerkonto), Test-PDFs aus
`testdata/` (u. a. `pdf/generated/simple-xref-table.pdf`, QR-Platzhalter
`pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf`), Test-Keystore `testdata/keystores/test-rsa2048.p12`
(Passwort `test`, NUR TEST). Ergebnis je Zeile: ✓ / ✗ mit Notiz und ggf. Log
(Einstellungen › Über › „Log-Ordner anzeigen“).

### 2.1 Bildschirme und Abläufe

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| S-01 | App starten (leeres Profil) | Startansicht „Was möchten Sie signieren?“ mit Ablagefläche und „PDF auswählen“; Kopfzeile mit Wortmarke (ohne „Prototyp“), „Prüfen“ (gesperrt bis E4), DE/EN, Farbschema, Einstellungen; Sprache = Systemsprache (DE/EN) |
| S-02 | Altkonfiguration `~/.pdf-over/PDF-Over.config` vorhanden, noch keine PIDA-Einstellungen | Migrationsassistent: Übernehmen / Nicht übernehmen; mit Klartext-Passwörtern zusätzlich die Frage nach dem Schlüsselbund; „Nicht übernehmen“ fragt nicht wieder |
| S-03 | „PDF auswählen“ (⌘O) → PDF wählen | Laden/Analyse mit Abbrechen, danach Arbeitsbereich: Miniaturen, Seiten, Zoom, Signaturblock automatisch platziert (Einblendung), Inspektor mit Konto-Chip, Signaturbild, Optionen, Platzierung („Seite n · rechts unten“, keine Zahlenfelder) |
| S-04 | PDF mit Nutzungsrechten (`/Perms /UR3`), Hybrid-XFA bzw. unerfüllbaren Seed Values | Schritt „Hinweis zum Dokument“ mit Liste; „Trotzdem signieren“ geht weiter, „Dokument verwerfen“ zurück zu Öffnen |
| S-05 | Kernpfad Arbeitsbereich: PDF ablegen → automatisch platziert → **Signieren** → Passwort (Keystore) bzw. Anmeldung → Bestätigung in der App (ID Austria) → Fertig | die Seitenansicht bleibt eingehängt (Signierkarte als Overlay, kein Wechsel der Ansicht); Startlage nach der PDF-AS-Regel; „Fertig“ mit „Signatur erstellt und geprüft (Selbstprüfung)“, „… laut Zertifikat“, Unterzeichner, Zeit, Datei, Ordner, Link signaturpruefung.gv.at, nie „gültig“ |
| S-05a | PDF mit leeren Signaturfeldern bzw. QR-Platzhalter (`placeholder.search` an) | Felder als „Hier signieren“ mit Hinweisleiste „Ins Feld setzen“ (nie automatisch, auch ohne Platzhaltersuche); Ziehen ins Feld rastet ein; QR-Platzhalter wird automatisch Ziel; „Frei platzieren“ kehrt zurück; keine Platzhalterfrage |
| S-06 | QR-Platzhalter mit `profile=…` | signiert mit dem Profil des Platzhalters; unbekanntes Profil → Hinweis `ng-placeholder-profile-unknown`, eingestelltes Profil |
| S-07 | Signierkarte: Abbrechen (×, „Abbrechen“, Esc) in Anmeldung bzw. Bestätigung | zurück im Arbeitsbereich mit **unveränderter** Platzierung und Hinweis „Signieren abgebrochen – es wurde nichts signiert.“; kein Fehlerschritt |
| S-07a | Sticker: Ziehen (auch über Seitengrenzen) mit Einrastlinien und Bändern freier Flächen, Doppelklick, Bild↑/↓, „Neu platzieren“, Schalter „Unsichtbar“ und „Auf neuer Seite“ | Block folgt; Live-Region nennt Seite, Lage (mm von unten) und Prüfergebnis; Überdeckung → Warnung direkt über „Signieren“ mit „Freie Fläche suchen“; „Auf neuer Seite“ bei signierten bzw. DocMDP-zertifizierten Dokumenten gesperrt mit Begründung |
| S-08 | „Unsichtbar signieren“ | nur dieses Dokument ohne sichtbaren Block (Profil-Einstellung unverändert) |
| S-09 | ID Austria: Netzfehler **nach** erfolgreicher Anmeldung (z. B. WLAN kurz trennen) → „Erneut versuchen“ | Fehlerschritt in der Karte mit Alltagstext und „Technische Details“ (Code, Zeitpunkt); „Erneut versuchen“ ohne neue Login-Maske und ohne zweites Lesen des Zertifikats |
| S-09a | Konto-Chip: Wechsel ID Austria / Keystore / Bürgerkarte, „Als Standard verwenden“; Standard „Jedes Mal fragen“ | Wahl startet nichts (Sitzung erst mit „Signieren“); ohne Standard öffnet „Signieren“ das Menü, die Wahl dort signiert; bei ID Austria Kurzhinweis „Das Dokument wird zum Signieren an A-Trust übermittelt.“ mit „Details“ (→ Einstellungen › Datenschutz), kein Zustimmungsdialog (ADR 0003); kein Weg startet ID Austria ohne „Signieren“ (F-54) |
| S-10 | Keystore-Signatur mit Test-Keystore | Passwortabfrage, Signatur, Ergebnis-Schritt; Datei `<name>_signed.pdf` neben der Quelle bzw. im Ausgabeordner |
| S-11 | Fertig: Öffnen, Im Ordner zeigen, „Nächstes Dokument“ bzw. „Weiteres Dokument signieren“ | Standard-PDF-Anzeige öffnet die Ausgabe; Finder zeigt sie; bei mehreren Dokumenten „Weiter mit „…“ · n Dokumente warten“, danach das nächste im Arbeitsbereich (kein automatisches Weiterschalten, F-88 nur mit `-b … -a`) |
| S-12 | Ausgabe existiert bereits | Rückfrage „Überschreiben?“; Nein → Speichern-Knopf bleibt |
| S-13 | Kein Ausgabeordner eingestellt | Speichern-Dialog mit Startordner der Quelle und Namensvorschlag `<name>_signed.pdf`; Abbrechen → Knopf „Speichern“ |
| S-14 | Fehler (z. B. falsches Keystore-Passwort, defektes PDF) | Fehlerschritt mit verständlichem Text, „Details“ (Code), Wiederholen wo sinnvoll |
| S-15 | Einstellungen (⌘,): alle Tabs, Speichern, Verwerfen, Zurücksetzen | Feldfehler am Feld (z. B. Proxy ohne Host); Blockvorschau; Zurücksetzen mit Bestätigung |
| S-16 | Start mit `-c alt.config` (Altdatei) | Einstellungen mit Hinweis „werden nicht gespeichert“ (schreibgeschützt), Änderungen gelten bis Programmende |
| S-17 | Über: Version, Verweis auf „Datenschutz“, Log-Ordner, **„Drittlizenzen anzeigen“** | öffnet `THIRD-PARTY-NOTICES.md` aus dem Bundle; Tab „Datenschutz“: Text aus PDF-Over mit Link zu A-Trust, Hinweis auf lokales Signieren, Zertifikats-Cache |
| S-18 | Hinweise (Toasts) | Hinweise des Kerns stapeln, einzeln schließbar; Info-Hinweise verschwinden nach 8 s, Fehler bleiben |
| S-19 | Beenden während einer offenen Maske (⌘Q / Fenster schließen) | Vorgang wird abgebrochen, Einstellungen gesichert, App endet (spätestens nach 5 s) |
| S-20 | Fenstergröße/-position | bleibt nach Neustart erhalten |

### 2.2 Tastatur und Barrierefreiheit

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| K-01 | Nur Tastatur: Tab/Umschalt+Tab durch alle Schritte | sichtbarer Fokus, logische Reihenfolge, „Zum Inhalt springen“ als erstes Element |
| K-02 | Zugriffstasten (macOS ⌃⌥+Taste, Windows/Linux Alt+Taste) | lösen die unterstrichenen Aktionen aus; doppelt belegte Tasten wandern reihum |
| K-03 | ⌘O / ⌘, (Strg unter Windows/Linux) | Dateidialog bzw. Einstellungen |
| K-04 | Sticker mit Fokus: Pfeiltasten (15 pt), mit Umschalt (1 pt), mit ⌘/Strg (50 pt), Bild↑/↓ bzw. Pos1/Ende (Seite); ⌘/Strg+↵ | Block bewegt sich (Schritte rechnet der Kern), Live-Region kündigt Lage und Prüfergebnis an; ⌘/Strg+↵ = „Signieren“ (ausdrückliche Aktion) |
| K-05 | Esc in Dialogen | schließt ohne Änderung |
| K-06 | VoiceOver (macOS), NVDA (Windows), Orca (Linux) | Rollen und Namen aller Bedienelemente, Statusmeldungen werden vorgelesen (Protokoll anlegen; axe-core/Screenreader-Protokolle stehen noch aus) |
| K-07 | Hell/Dunkel, erhöhter Kontrast, reduzierte Bewegung, Fenster schmal (360 px) | lesbar, keine abgeschnittenen Bedienelemente |

### 2.3 Drag & Drop, „Öffnen mit“, Single-Instance, Startargumente

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| D-01 | PDF(s) auf das Fenster ziehen (Schritt Öffnen bzw. Ergebnis) | Dokumente kommen in die Warteschlange; Hervorhebung während des Ziehens |
| D-02 | PDF in einem anderen Schritt ablegen | Hinweis „hier nicht möglich“, nichts passiert |
| D-03 | Bild auf das Bildmarken-Feld in den Einstellungen ziehen | Bildmarke übernommen |
| D-04 | Finder: „Öffnen mit › PIDA“ bzw. PDF auf das Dock-Symbol (App läuft nicht) | App startet mit dem Dokument |
| D-05 | wie D-04, App läuft bereits | Dokument geht an die laufende Instanz, Fenster kommt nach vorn (keine zweite Instanz) |
| D-05a | macOS, zweites Benutzerkonto: dort läuft PIDA bzw. `/tmp/at_pida_si.sock` gehört dem anderen Konto; App im eigenen Konto starten | App startet normal (ohne Single-Instance, Warnung im Log); keine Startargumente an das andere Konto |
| D-05b | Linux: Start über den Desktop-Eintrag mit Datei-URL (`pida file:///…/a%20b.pdf`) | Dokument wird geöffnet (Datei-URL = Pfad) |
| D-06 | Terminal: `pida -b KS -ks test-rsa2048.p12 -kspass test -a -s -o <ordner> a.pdf b.pdf` | beide Dokumente ohne Rückfragen signiert, App endet nach dem letzten Speichern (`-s`) |
| D-07 | `pida -v true a.pdf`, `-e logo.png`, `-n 0664…`, `-proxy host -proxyport 8080` | unsichtbar; Bildmarke; Benutzername vorbelegt (`+43664…`); Proxy nur für diese Sitzung (nicht gespeichert) |
| D-08 | `pida fehlt.pdf`, `pida -b FALSCH`, `pida --headless` | Fehlerschritt ohne Wiederholen (`error-FileNotExist`, `error-CmdLineArgs`, `ng-ui-args-cli-only`), Abbrechen beendet; Passwörter erscheinen nie in der Anzeige oder im Log |
| D-09 | `pida -h` | Hilfe auf stdout, kein Fenster |
| D-10 | `pida --log-level debug` | ausführlicheres Log (Vorrang vor `PIDA_LOG`), weiterhin ohne Passwörter/Dokumentinhalte |

### 2.4 ID Austria mit Testkonto (nur durch den Auftraggeber)

Nur mit dem A-Trust-Testkonto bzw. Testtelefon des Auftraggebers; keine echten Zugangsdaten in Tickets
oder Logs. Einen Zustimmungsdialog gibt es nicht mehr (ADR 0003); der Kurzhinweis steht bei ID Austria.

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| A-01 | Anmeldung (Telefonnummer/Benutzername + Passwort, „Merken“) | Maske nativ in der App; falsches Passwort → `ng-atrust-login-rejected`; mit „Merken“ ist der Benutzername beim nächsten Mal vorbelegt (Passwort nie gespeichert) |
| A-02 | Zweiter Faktor: ID-Austria-App (Push/QR) | Warte-Anzeige, nach Bestätigung in der App weiter |
| A-03 | Zweiter Faktor: SMS-TAN | TAN-Maske mit Referenzwert; falsche TAN → Hinweis, erneute Eingabe |
| A-04 | FIDO2-Sicherheitsschlüssel (macOS/Linux CTAP-HID, Windows Hello) | Aufforderung zum Berühren; Schlüssel mit PIN: Hinweis `fido2-pin-unsupported` (CTAP2-clientPIN folgt) |
| A-05 | „Signaturdaten anzeigen“ | öffnet die A-Trust-Seite im Systembrowser (nur `https://`, A-Trust-Host) |
| A-06 | Abbrechen in jeder Maske | zurück zum Arbeitsbereich mit unveränderter Platzierung und Hinweis „es wurde nichts signiert“, nichts gespeichert, Passwort der Sitzung gelöscht (F-72); Ablehnung in der App → Fehlerschritt „In der App abgelehnt oder abgelaufen“ mit „Erneut versuchen“ ohne neue Anmeldung |
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
- Mobile (iOS/Android): Fundament M0 steht (Kern-Lebenszyklus mit „Signatur fortsetzen“, Shell
  `cfg(mobile)`, Plugin `pida-mobile` mit leeren nativen Seiten, Oberfläche); native Seiten, Projekte
  und Start im Simulator/Emulator folgen mit M-iOS/M-Android (docs/integration/mobile.md).
- Positionsanzeige (Befund aus der Abnahme): Live-Region und Felder nennen jetzt dieselben Kanten
  („linke Kante … von links, Oberkante … von unten“); der Link im Datenschutztext (seit ADR 0003 in
  Einstellungen › Datenschutz) ist ein Inline-Link, der Satzpunkt bleibt in derselben Zeile.
- Barrierefreiheit: Komponententests prüfen Rollen/Namen/Tastatur, seit E1 auch axe-core (Start,
  Arbeitsbereich, alle Kartenschritte, Fertig, Zustandskarten, schmale Ansicht) und den Token-Kontrast
  Hell/Dunkel; Screenreader-Protokolle (K-06), `forced-colors` und die Leistungsziele (ADR 0004 §3.3) auf dem
  Referenzrechner stehen aus.
- E1: Stapel (E2), Einstellungen „sofort wirksam“/Live-Editor (E3), Prüfmodus, Befehlspalette und
  Ersteinrichtung (E4) folgen; bis dahin bisheriger Einstellungsdialog, „Prüfen“ gesperrt, kein „Befehle ⌘K“.

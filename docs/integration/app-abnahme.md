# Abnahme der Desktop-App (PIDA, Welle 3)

Gegenstand: `apps/app` – Oberfläche (React 19, React Aria Components, Vite) und Tauri-2.12-Shell
(`apps/app/src-tauri`, Crate `pida-app`) um den Rust-Kern `pida-core-api`. Dieses Dokument hält
fest, was automatisch nachgewiesen ist, und enthält die Checkliste für die **manuelle Prüfung** durch
den Auftraggeber (Bildschirme, Tastatur, Drag & Drop, ID Austria mit Testkonto).

Stand: 15.10.2026, Welle 3 Track T1; Etappe **E1** der neuen Oberfläche „Arbeitsbereich“ (ADR 0004 §5, Merge
K1 + A1, `workspace_ui` eingeschaltet) ergänzt am 15.10.2026, Etappe **E2** (Stapel, Merge K2 + A2,
`batch_overview` eingeschaltet) und Etappe **E3** (Einstellungen „sofort wirksam“, Live-Editor, Bereich „Dokument“,
Datenschutz, „Zuletzt signiert“, Merge K3 + B3) und Etappe **E4** (Prüfmodus, Befehlspalette und Kürzelübersicht,
Ersteinrichtung, Merge K4 + B4) am 16.10.2026; gemessen auf macOS 27 (arm64), Xcode 27, Rust 1.99.0, Node 22,
PDFium chromium/8100 (`tools/pdfium.lock`). Windows und Linux baut und prüft die CI (Job `bundle`).

> **17.10.2026 – Prüfmodus entfernt (ADR 0004 Nachtrag 15, Entscheidung des Auftraggebers).** Die Nachweise und
> Prüfschritte zum Prüfmodus (N-37 Teil `core-api --test verify`, N-39/N-40/N-44 soweit Prüfmodus, S-31, S-32, der
> Prüfmodus-Teil von S-34, die Aufnahmen `stage4/*pruefmodus*`, `*verification-mode*`, `ios-ng/44–47, 70`) sind
> Geschichte. Neu: N-45 (automatisch) und S-35 (manuell) für „Zuletzt signiert“ mit den Aktionen von „Fertig“.

## 1. Automatische Nachweise

| Nr. | Prüfung | Befehl | Ergebnis |
|---|---|---|---|
| N-01 | Oberfläche installieren, bauen, testen | `cd apps/app && npm ci && npm run build && npm test` | grün: tsc strict + Vite-Build, 10 Testdateien, 237 Tests (nach dem Review); seit E1 15 Testdateien, 379 Tests (inkl. axe-core, Mock-Projektion, Token-Kontrast, echte Kernzustände); seit E2 17 Testdateien, 429 Tests; seit E3 20 Testdateien, 464 Tests; seit E4 24 Testdateien, 568 Tests |
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
| N-22 | E2: Debug-Bundle mit Stapelübersicht | `PIDA_SMOKE=1 PIDA_HOME=<leer> HOME=<leer> …/pida a.pdf b.pdf geheim.pdf` bzw. `-b MOBILE -a a.pdf b.pdf` | `ok step=batch pdfium=true` (Übersicht ab zwei Dokumenten, `batch_overview` eingeschaltet; WebView + Kern nach 377–434 ms) bzw. `ok step=loading` (Startargument-Automatik: nacheinander, keine Übersicht, keine Sitzung); ohne Dokument `ok step=open` (450–518 ms); je zwei Läufe |
| N-23 | E2: D-06 mit eingeschalteter Stapelübersicht (Abnahme E2.9) | `PIDA_HOME=<leer> …/pida -b KS -ks testdata/keystores/test-rsa2048.p12 -kspass test -a -s -o <ordner> a.pdf b.pdf` | unverändert ohne Übersicht: beide signiert, Ende mit 0 nach 2 s, Log ohne Pfade/Namen; `pyhanko sign validate --trust …/test-rsa2048.crt.pem`: `INTACT:TRUSTED,UNTOUCHED`, `qpdf --check` ohne Fehler |
| N-24 | E2: Stapel im Kern | `cargo test -p pida-workflow --test batch --test props`, `cargo test -p pida-core-api --test batch --test mobile` (mit `PIDA_PDFIUM_DIR`) | 28 Tabellentests (Übersicht, Voranalyse, öffnen/zurück mit Plan, eine Sitzung, Überspringen ≠ Abbrechen, Pausen je Fehlerart, vorhandene Ziele, Abbruch während des Schreibens); A1-Property-Test mit Stapelereignissen; core-api: E2.1–E2.7 und E2.9 mit ID-Austria-artigem Fake-Backend (eine Anmeldung, höchstens ein `certificate()`, eigener Vergleichswert je Dokument), Keystore mit echter Engine (pyHanko), E2.8 (`mobile.rs`: App-Kill im Lauf → nur das unterbrochene Dokument, danach Übersicht) |
| N-25 | E2: Oberfläche gegen echte Stapelzustände | `cd apps/app && npm test` (`src/app/RealCoreBatch.test.tsx`, Fixture `src/core/fixtures/real-core-batch-views.json` aus N-24) | jeder Zustand in der Stapelübersicht, Schätzung, `start_batch` mit genau den signierbaren Dokumenten, eine Anmeldung, „Bestätigung k von n · Name“ mit eigenem Vergleichswert, Pause (Verbindung) mit Erneut versuchen/Überspringen/Stapel abbrechen, Zusammenfassung mit Speicherort und „Im Ordner zeigen“; kein Zustand stößt eine Sitzung an (F-54); axe ohne Verstöße |
| N-26 | E2: Stapel gegen den Mock | `cd apps/app && npm test` (`features/batch/batch.test.tsx`, `core/mock/mock.test.ts`) | alle [M]-Kriterien E2.1–E2.7, Mobile (App-Dokumente, schmal), Englisch, ⌘↵, Keystore mit einmaligem Passwort; axe (auch schmal, Startdialog, Pausen, Abbruch-Rückfrage) |
| N-27 | E2: Mobile-Typprüfung | wie N-21 | grün (nur die bekannte Warnung in `pida-pades`) |
| N-28 | E3: Einstellungen „sofort wirksam“ und „Zuletzt signiert“ im Kern (Abnahme E3.1, E3.2, E3.5–E3.7) | `cargo test -p pida-core-api --test settings` (mit `PIDA_PDFIUM_DIR`), `cargo test -p pida-config recent`, `-p pida-core-api settings_apply` | 16 Tests: Teilübernahme (ungültiger Port + gültiges Profil), unbekannter Pfad, veraltete Revision, `last_source_directory` zwischen zwei Übernahmen bleibt, Neustart zeigt die Werte; genau ein Neubau der Vorschau je Änderung am Signaturbild mit `preview_revision` in derselben Projektion wie `applied_revision`; Block im offenen Dokument (automatisch neu gesetzt, manuell Lage behalten); PDF/A erkannt/erzwungen (Amtssignatur ohne PDF/A-Variante); Feld- und QR-Suche sofort wirksam (echte Engine: ins erste leere Feld, OutputIntent, pyHanko intakt); schreibgeschützt; Mobile; `recent.json` feldgenau, 0600, höchstens 10, Abschalten/Richtlinie löscht, ohne Oberfläche nichts; Signierbeginn schließt die Einstellungen; alle Wertformen der Einstellungs-Seitenleiste werden ohne Ablehnung übernommen |
| N-29 | E3: Bereich „Dokument“ im Workflow | `cargo test -p pida-workflow --test workspace --test batch --test props` | Tabellentests: erstes leeres Feld mit „Leere Signaturfelder“, QR nachträglich, Automatik endet mit jeder Platzierung der Nutzerin, Stapelpläne folgen den Einstellungen (Feld, QR), „Unsichtbar signieren“ im unverändert geöffneten Eintrag bleibt Plan der Nutzerin (Befund im Merge); A1-Property-Test mit wechselnder Platzhaltersuche und `PlaceholderSearched` |
| N-30 | E3: Oberfläche gegen echte Kernzustände (Nachtrag 5) | `cd apps/app && npm test` (`src/app/RealCore.test.tsx`, Zustände `doc_pdfa_detected`, `doc_pdfa_forced`, `doc_field_auto`, `doc_qr_auto`, `settings_open`, `settings_partial` aus N-18) | Inspektor-Hinweise „PDF/A-Dokument erkannt …“, „… (erzwungen)“, „Signaturfeld gefunden …“, „QR-Platzhalter gefunden …“, Sprechblase nennt das Ziel; Einstellungen mit Feldfehler am Port, übernommenen Werten und Namensbeispiel des Kerns; axe ohne Verstöße; keine Sitzung (F-54) |
| N-31 | E3: Einstellungen, „Zuletzt signiert“ und Hinweise gegen den Mock | `cd apps/app && npm test` (`features/settings/{settings,a11y,units,mock}.test.ts(x)`, `features/recent/recent.test.tsx`, `features/workspace/workspace.test.tsx`) | alle [M]-Kriterien E3.1–E3.8 (sofort wirksam ohne „Speichern“, Feldfehler, Suche „Logo“/„Handy“/„Proxy“ mit Vorschau, Datenschutz, „Zuletzt signiert“, schreibgeschützt, Mobile, axe je Bereich und im Suchmodus); Hinweise im Arbeitsbereich mit sofortiger Wirkung der Platzhaltersuche, von Hand Platziertes bleibt |
| N-32 | E3: Debug-Bundle | `PIDA_SMOKE=1 PIDA_HOME=<leer> HOME=<leer> …/pida` ohne Dokument, mit `a.pdf`, mit `a.pdf b.pdf`, `-b MOBILE -a a.pdf`, `-c alt.config a.pdf` | `ok step=open pdfium=true` (WebView + Kern nach 413–464 ms) bzw. `ok step=loading` (Dokument, Startargument-Automatik, `-c`) bzw. `ok step=batch`; mit `-c` keine `settings.toml` geschrieben |
| N-33 | E3: D-06 und „Zuletzt signiert“ im Bundle | `PIDA_HOME=<leer> …/pida -b KS -ks testdata/keystores/test-rsa2048.p12 -kspass test -a -s -o <ordner> a.pdf b.pdf` | beide signiert, Ende mit 0 nach 2 s; `pyhanko sign validate --trust …/test-rsa2048.crt.pem`: `INTACT:TRUSTED,UNTOUCHED`, `qpdf --check` ohne Fehler; `<PIDA_HOME>/data/recent.json` (0600) enthält genau `id`, `file_name`, `path`, `signed_at`, neueste zuerst; mit `general.recent_documents = false` wird eine vorhandene Liste beim Start gelöscht und nichts geschrieben; Log ohne Pfade, Namen, Passwörter |
| N-34 | E3: Einstellungen › Dokument im Bundle | `settings.toml` mit `[placeholder] search = true, kind = "fields"` und `[signature] pdfa_force = true`, dann wie N-33 mit `manySignFields.pdf` | signiert in das erste leere Feld `Signature_0` (8 Felder vorher und nachher, kein neues Feld), inkrementell, `/OutputIntents` ergänzt (PDF/A-kompatibel); pyHanko `INTACT:TRUSTED,UNTOUCHED`, qpdf ohne Fehler |
| N-35 | E3: Vorschau nach einer Änderung (Abnahme E3.1 [H], Näherung) | Messung gegen den echten Kern mit PDFium (Debug-Build, unoptimiert) | `ApplySettings` → bestätigt mit neuer Vorschau nach 160–411 ms, PNG der Vorschau in 27–202 ms, gesamt ≤ 0,62 s (Profil, Blocksprache, Hinweis, PDF/A); Messung auf dem Referenzrechner steht aus |
| N-36 | E3: Mobile-Typprüfung | wie N-21 | grün (nur die bekannte Warnung in `pida-pades`) |
| N-37 | E4: Prüfmodus im Kern (Abnahme E4.1–E4.3 [K]) | `cargo test -p pida-pades --test verify_testdata`, `cargo test -p pida-core-api --test verify`, `-p pida-cades` (qc, build_verify) | Tabelle je Datei aus `testdata/verify/*` (PDF-AS-signiert, manipuliert → verändert, `adbe.pkcs7.sha1` → nicht prüfbar, CAdES mit SHA-1 → intakt + Hinweis, zwei Signaturen, später ergänzt, unsichtbar, QES-Testzertifikat „qualifiziert“, gedreht 90°, ohne Signatur); Bytes in der ByteRange-Lücke → verändert, im Merge auch im Inspektor (Analysewarnung) und in `pida verify`; `AppearanceView` auf der gedrehten Seite; eigene `DocId`, freigegeben beim Schließen; Zustand bleibt; `ng-verify-busy`; „Dieses Dokument signieren“ mit und ohne Stapelübersicht; Prüfmodus über dem Arbeitsbereich sperrt Signieren und Bearbeiten des verdeckten Dokuments (Merge); „Fertig“ QES laut Zertifikat; F-54 |
| N-38 | E4: Ersteinrichtung im Kern (Abnahme E4.5 [K]) | `cargo test -p pida-core-api --test first_run`, `cargo test -p pida-config --test migration` | fehlende Datei → Ersteinrichtung und `output.folder = "."` (echte Engine: Ausgabe neben dem Original); Neustart vor „Fertig“ → erneut; Bestandsdatei ohne Schlüssel → nie, `output.folder` bleibt `None`; headless und klassisch ohne; Übernahmeliste mit Details; Übernehmen/Nicht übernehmen/Später/`OfferMigration` samt Sicherung; eigene Bildmarke übernommen, PDF-AS-Standard-Bildmarke (Pfad bzw. Inhalt) nicht (`ng-migrate-emblem-default`, PIDA-Siegel bleibt); Mobile |
| N-39 | E4: Oberfläche gegen echte Kernzustände (Nachtrag 6) | `cd apps/app && npm test` (`src/app/RealCoreTools.test.tsx`, Fixture `src/core/fixtures/real-core-tools-views.json` aus N-37) | Gesamtergebnis und Status je Signatur, Zeit „(Angabe des Signierenden)“, „Qualifizierte elektronische Signatur laut Zertifikat“, SHA-1-Hinweis, spätere Revisionen, Rechteck des Signaturbilds auf der Querseite, nie „gültig“; Ersteinrichtung mit Übernahmeliste des Kerns (maskierte Telefonnummer, Keystore-Datei, `host:port`, „Deutsch“), Übernehmen → Schritte 2/3 → „Los geht’s“; keine Sitzung (F-54); axe |
| N-40 | E4: Prüfmodus, Palette, Kürzel, Ersteinrichtung gegen den Mock (Abnahme E4.1–E4.6 [M]) | `cd apps/app && npm test` (`features/verify/verify.test.tsx`, `features/palette/palette.test.tsx`, `features/onboarding/onboarding.test.tsx`, `features/settings/{mock,units}.test.ts`, `features/recent/recent.test.tsx`) | alle Zustände des Prüfmodus, Details, Hervorhebung samt Geometrie, unsichtbar, „Dieses Dokument signieren“ (Start bzw. Warteschlange), „Zuletzt signiert“ → Prüfmodus, Katalogtest (nie „gültig“/„rechtsgültig“/„valid“); Palette = `COMMANDS` mit Freigaben, Tastatur, Kürzelübersicht = `buildShortcuts` + Palette; Ersteinrichtung in allen Schritten, Einstellungen › Erweitert; im Merge: Kopfzeile „Prüfmodus“, Ablegen prüft, ⌘O öffnet, ⌘K nicht über anderen Dialogen, ein Hinweis bei „keine PDF-Over-Einstellungen“; axe auch schmal |
| N-41 | E4: Debug-Bundle, Ersteinrichtung | `PIDA_SMOKE=1 PIDA_HOME=<leer> HOME=<leer> …/pida` ohne Einstellungen, mit `<PIDA_HOME>/.pdf-over/PDF-Over.config` (EMBLEM = PDF-AS-Standardlogo), mit vorhandener `settings.toml` | `ok step=open pdfium=true` (WebView + Kern nach 427–516 ms); Log „Neuinstallation – Ersteinrichtung offer=false“ bzw. „offer=true“ (keine `settings.toml` geschrieben, das Angebot bleibt bis zur Entscheidung); Bestandsdatei: keine Ersteinrichtung, Datei unverändert |
| N-42 | E4: D-06 im Bundle bei Neuinstallation, Prüfung der Ausgabe | `PIDA_HOME=<leer> …/pida -b KS -ks testdata/keystores/test-rsa2048.p12 -kspass test -a -s -o <ordner> a.pdf b.pdf`; danach `pida-cli verify` | beide signiert, Ende mit 0 nach 1,9 s (die offene Ersteinrichtung stört die Startargument-Automatik nicht, keine Sitzung außer der ausdrücklich verlangten); `pyhanko sign validate --trust …/test-rsa2048.crt.pem`: `INTACT:TRUSTED,UNTOUCHED`, `qpdf --check` ohne Fehler; `recent.json` 0600 mit genau `id`, `file_name`, `path`, `signed_at`; Log ohne Pfade, Namen, Passwörter; `pida-cli verify` intakt, `testdata/verify/tampered.pdf` → `cms-digest-mismatch` |
| N-43 | E4: Mobile-Typprüfung | wie N-21 | grün (nur die bekannte Warnung in `pida-pades`) |
| N-44 | E4: axe-core im Browser (Chrome headless, echte Darstellung, Mock-Kern) | axe 4.14 im Prüfmodus (auch über einem Dokument, Details, dunkel, 375 px), Ersteinrichtung (Schritte 1/2, dunkel), Palette, Kürzelübersicht | ohne Befunde außer: `label-content-name-mismatch` an Wortmarke und „Befehle ⌘K“ (Name ohne Tagline bzw. Tastenkappen, seit E1/E4) und an Palette-Einträgen mit Tastenkappen, `scrollable-region-focusable` an der Befehlsliste (virtueller Fokus, Pfeiltasten scrollen); im Merge behoben: fokussierbare Seitenspalte des Prüfmodus; 360 px ohne waagrechtes Scrollen (Prüfmodus, Details, Ersteinrichtung, Palette, Kürzel) |
| N-45 | Nachtrag 15: Prüfmodus entfernt, „Zuletzt signiert“ wie „Fertig“ | `cargo test -p pida-core-api --test settings recent`, `--test batch sign_recent`, `--test first_run recent_and_migration`, `--test flow classic_mode`, `--lib verify_mode_is_gone`, `cargo test -p pida-cli`, `cargo test -p pida-i18n`, `cargo test -p pida-app --test serde_contract`, `cd apps/app && npm test` | grün: „Öffnen“/„Im Ordner zeigen“ (Desktop) bzw. „Öffnen“/„Teilen“ (iOS) gehen an die Shell, „Erneut signieren“ öffnet wie `open_files` (auch bei geparkter Arbeit mit `mode`), fehlende Datei → `ng-recent-missing`, keine Sitzung; „Im Ordner zeigen“ im Hintergrund (ein hängender Dateimanager hält den Kern nicht auf); beim Signieren bleibt „Erneut signieren“ ohne Wirkung; eine Liste im Format von K3 wird gelesen; kein `verify` in `ViewState`/`actions`, keine Prüfmodus-Texte im Katalog; `pida verify` sagt „Intakt (unverändert)“ bzw. „Nicht intakt“ und „Vertrauenskette und Sperrstatus nicht geprüft“ (DE/EN, nie „gültig“); Oberfläche 31 Testdateien, 639 Tests |

Hinweise zur Messung:

- **Gestaltung E1** neben dem Prototyp (`prototypes/pida-workspace/docs/screenshots/`): Mock-Kern im Browser
  (`npm run dev:mock`, eigener Port), Desktop 1280 × 800 und 375 × 812 @2x, hell/dunkel/EN, 25 Aufnahmen in
  `docs/integration/screenshots/stage1/`. Gewollte Abweichungen: keine „Prototyp“-Plakette und keine
  Demo-Steuerung, „Befehle ⌘K“ erst mit der Palette (E4), „Prüfen“ gesperrt, Startlage nach der PDF-AS-Regel
  (waagrecht mittig unter dem Inhalt), leere Signaturfelder nur angeboten (ADR 0004 §8 F2), Wortlaut F3 auf
  „Fertig“.
- **Gestaltung E2** (Stapel) neben dem Prototyp (`?scenario=batch`): Mock-Kern im Browser auf eigenem Port,
  Desktop 1280 × 800 hell/dunkel und 375 × 812 @2x hell/dunkel, 23 Aufnahmen in
  `docs/integration/screenshots/stage2/`; 360 px ohne waagrechtes Scrollen. Gewollte Abweichungen (ADR 0004
  §9 Nachtrag 4): abgelehnte Dateien „Abgelehnt“ + „Entfernen“ statt „Kennwort eingeben“ (§8 F1), Zeile mit
  Schätzung und Speicherort, Startdialog nur bei Ordnerwahl bzw. vorhandenen Zieldateien, k/n im Kopf der
  Stapelkarte statt im Stepper.

- **Gestaltung E3** neben dem Prototyp (Commit `fd7def3`, eigener Port): Mock-Kern im Browser auf eigenem Port,
  Desktop 1280 × 800 hell/dunkel/EN und 375 × 812 @2x hell/dunkel, 33 Aufnahmen in
  `docs/integration/screenshots/stage3/` (Einstellungen von B3; Hinweise im Arbeitsbereich, Bereich „Dokument“ mit
  „Leere Signaturfelder“ und schmale Ansicht im Merge: `desktop-15`–`19`, `dunkel-04`/`05`, `mobil-07`–`09`); 360 px
  und 375 px ohne waagrechtes Scrollen der Seite (die Bereichsleiste der Einstellungen scrollt schmal waagrecht).
  Gewollte Abweichungen (ADR 0004 §9 Nachtrag 5): keine Demo-Steuerung, keine Zugriffstasten in der Bereichsleiste,
  „Prüfen“ gesperrt bis E4, Hinweistexte aus `document.detection` des Kerns.
- **Gestaltung E4** neben dem Prototyp (`?scenario=verify`, Ersteinrichtung, Palette, Kürzel; eigene Ports): Mock-Kern
  im Browser, Desktop 1280 × 800 hell/dunkel/EN und 375 × 812 @2x hell/dunkel, 44 Aufnahmen in
  `docs/integration/screenshots/stage4/` (B4: `desktop-01`–`20`, `dunkel-01`–`04`, `englisch-01`–`03`, `mobil-01`–`06`;
  im Merge neu aufgenommen mit der Kopfzeile „Prüfmodus“: `desktop-02`–`09`, `desktop-20`, `dunkel-01`, `englisch-01`,
  `mobil-01`/`02`/`06`; neu `desktop-21`–`25`, `dunkel-05`/`06`, `englisch-04`, `mobil-07`–`09`). Gewollte Abweichungen
  (ADR 0004 §9 Nachtrag 6): „Intakt“/„Verändert“/„Nicht prüfbar“ statt „Gültig“/„Ungültig“, Zeit „(Angabe des
  Signierenden)“, keine Rolle/Methode, kein „Beispiel prüfen“ und keine eigene Ablagefläche, gesperrte Befehle nicht in
  der Palette, Übernahmeliste aus dem Kern, „Es bleibt das PIDA-Siegel“ ohne eigene Bildmarke.
- **Laufende Instanz:** Läuft bereits ein PIDA desselben Kontos, übergibt eine aus dem Terminal gestartete Binary
  ihre Startargumente über `/tmp/at_pida_si.sock` der laufenden Instanz und endet (Single-Instance, D-05). Für die
  Prüfungen N-32–N-34 und N-41/N-42 lief eine andere Instanz; die Läufe liefen deshalb unter
  `sandbox-exec -p '(version 1)(allow default)(deny file-read* file-write* network-outbound (literal "/private/tmp/at_pida_si.sock"))'`
  – die App sieht den Socket als nicht prüfbar und startet ohne Single-Instance (Pfad aus N-13, Warnung im Log).
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
- Signieren mit Identität und Notarisierung sind nicht Teil dieser Abnahme (Signier-Secrets beim
  Auftraggeber, Plan §9.3). Seit 17.10.2026 ist das Bundle ad hoc signiert (versiegelte Ressourcen, ohne
  Hardened Runtime; `tools/verify-macos-bundle.sh`, docs/entwicklung.md §10 „Release bauen“): Es startet
  lokal; nach einem Download lehnt Gatekeeper es ab (`spctl` → `rejected`), bietet aber „Dennoch öffnen“
  statt „beschädigt“ (0.2.0, nur vom Linker signiert).

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
| S-01 | App starten (leeres Profil) | seit E4 zuerst die Ersteinrichtung (S-02); danach Startansicht „Was möchten Sie signieren?“ mit Ablagefläche und „PDF auswählen“; Kopfzeile mit Wortmarke (ohne „Prototyp“), „Prüfen“, „Befehle ⌘K“, DE/EN, Farbschema, Einstellungen; Sprache = Systemsprache (DE/EN) |
| S-02 | Erster Start ohne PIDA-Einstellungen (seit E4), mit bzw. ohne `~/.pdf-over/PDF-Over.config` | Ersteinrichtung „Willkommen bei PIDA“; mit Altkonfiguration Schritt 1 „PDF-Over-Einstellungen gefunden“ mit Liste der Übernahmen (Bildmarke nur bei eigener Bildmarke, sonst „Es bleibt das PIDA-Siegel“; die PDF-AS-Standard-Bildmarke zählt nicht als eigene), Klartext-Passwörter nur mit Zustimmung; ohne Altkonfiguration direkt das Standard-Signiermittel; Signaturbild mit Vorschau; „Später einrichten“ (auch Esc); ohne Übernahme speichert PIDA „neben dem Original“; Bestandsnutzer sehen sie nie; App vor „Los geht’s“ beendet → beim nächsten Start erneut; erneut über Einstellungen › Erweitert › „Suchen …“ (ohne Fund Hinweis „keine PDF-Over-Einstellungen gefunden“); nichts startet eine Signatur (F-54) |
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
| S-11 | Fertig: Öffnen, Im Ordner zeigen, „Nächstes Dokument“ bzw. „Weiteres Dokument signieren“ | Standard-PDF-Anzeige öffnet die Ausgabe; Finder zeigt sie; bei mehreren Dokumenten „Weiter mit „…“ · n Dokumente warten“, danach das nächste im Arbeitsbereich (kein automatisches Weiterschalten, F-88 nur mit `-b … -a`); seit E2 (Dokumente während eines Einzeldokuments hinzugefügt) zusätzlich „Zum Stapel“ – dieses Dokument zählt dort als signiert |
| S-12 | Ausgabe existiert bereits | Rückfrage „Überschreiben?“; Nein → Speichern-Knopf bleibt |
| S-13 | Kein Ausgabeordner eingestellt | Speichern-Dialog mit Startordner der Quelle und Namensvorschlag `<name>_signed.pdf`; Abbrechen → Knopf „Speichern“ |
| S-14 | Fehler (z. B. falsches Keystore-Passwort, defektes PDF) | Fehlerschritt mit verständlichem Text, „Details“ (Code), Wiederholen wo sinnvoll |
| S-15 | Einstellungen (⌘,) seit E3: Seitenleiste Signaturbild, Dokument, Signiermittel, Speichern, Darstellung, Datenschutz, Erweitert, Über PIDA; Suche („Logo“, „Handy“, „Proxy“); Zurücksetzen | jede Änderung wirkt sofort (kein „Speichern“/„Abbrechen“); Feldfehler am Feld (z. B. Proxy-Port 99999), andere Änderungen bleiben übernommen; Treffer im Signaturbild zeigen die Vorschau; Zurücksetzen mit Bestätigung; Neustart zeigt die Werte |
| S-16 | Start mit `-c alt.config` (Altdatei) | Einstellungen mit Banner „werden nicht gespeichert“ (schreibgeschützt), Änderungen gelten bis Programmende |
| S-17 | Über: Version, Verweis auf den Bereich Datenschutz, Protokollordner, **„Drittlizenzen anzeigen“** | öffnet `THIRD-PARTY-NOTICES.md` aus dem Bundle; Bereich „Datenschutz“: Text bzw. Richtlinientext, Link zu A-Trust, lokales Signieren, Zertifikats-Cache, „„Zuletzt signiert“ merken“, „Liste leeren“ |
| S-18 | Hinweise (Toasts) | Hinweise des Kerns stapeln, einzeln schließbar; Info-Hinweise verschwinden nach 8 s, Fehler bleiben |
| S-19 | Beenden während einer offenen Maske (⌘Q / Fenster schließen) | Vorgang wird abgebrochen, Einstellungen gesichert, App endet (spätestens nach 5 s) |
| S-20 | Fenstergröße/-position | bleibt nach Neustart erhalten |
| S-21 | E2: mehrere PDFs ablegen bzw. „Öffnen mit“ (≥ 2) | Stapelübersicht „Stapel · n Dokumente“: je Datei Miniatur, Seiten, Größe, Status (Wird geprüft → Bereit/Positioniert „im QR-Platzhalter“/Abgelehnt „Mit Kennwort geschützt“ mit „Entfernen“/Bitte prüfen); Schätzung „3 Dokumente · 1 Anmeldung · 3 Bestätigungen (ggf. +1 für das Zertifikat)“ (Keystore: ohne Anmeldung), Speicherort, Kurzhinweis Datenschutz bei ID Austria; **nichts startet von selbst** (F-54) |
| S-22 | E2: Eintrag öffnen, Block verschieben, „Zurück zum Stapel“ (Kopfzeile bzw. Inspektor); einzeln „Signieren“ | Eintrag „Positioniert“ mit dieser Lage (erneut öffnen zeigt sie); einzeln signiert → Hinweis „… signiert“, zurück zur Übersicht, zählt in der Zusammenfassung; „Start“/„Weiteres Dokument“ führt zurück zur Übersicht |
| S-23 | E2: „Alle signieren (n)“ (⌘↵) mit vorhandener Zieldatei bzw. „Jedes Mal fragen“ als Ausgabeordner | Startdialog **vor** dem Lauf: „Beide behalten“ (Standard, `… (2).pdf`), „Überschreiben“, „Überspringen“ (ohne Bestätigung für dieses Dokument) bzw. Ordnerwahl; sonst startet der Lauf direkt; ohne Standard-Signiermittel öffnet sich das Menü des Konto-Chips |
| S-24 | E2: Stapellauf mit Keystore (Test-Keystore) | Passwort **einmal**, Stapelkarte „n Dokumente“ mit „Dokument k von n · Name“, Fortschritt in Karte und Liste, danach Zusammenfassung „n von m signiert …“ mit Speicherort, „Im Ordner zeigen“, „Neuer Stapel“; Ausgaben gültig (pyHanko) |
| S-25 | E2: Esc bzw. × im Lauf, „Stapel abbrechen“ | Rückfrage „Stapel abbrechen?“ („Weiter signieren“); Abbruch: Gespeichertes bleibt, aktuelles „Abgebrochen“, Rest „Bereit“, Zusammenfassung „Stapel abgebrochen“, Passwort vergessen |
| S-26 | E3: Signaturbild live – Profil, Blocksprache, Bildmarke (Dialog bzw. Ablegen), Logo-Größe, Hinweistext mit offenem Dokument | die Vorschau oben in den Einstellungen und der Block im Dokument ändern sich binnen einer Sekunde; automatisch platziert → neu gesetzt, von Hand platziert → Lage bleibt; ohne eigene Bildmarke „Standard-Bildmarke (PIDA-Siegel)“, eine abgelegte Datei ändert das Profil nicht |
| S-27 | E3: Einstellungen › Dokument – „PDF/A-kompatibel signieren“, „Signaturplatzhalter automatisch suchen“ mit „QR-Code-Platzhalter“ bzw. „Leere Signaturfelder“ (Auswahl gesperrt bei ausgeschalteter Suche) | wirkt sofort im offenen Dokument: Inspektor „PDF/A-Dokument erkannt – wird PDF/A-konform signiert“ (PDF/A-Datei) bzw. „PDF/A-kompatible Signatur (erzwungen)“; „Signaturfeld gefunden – Signatur im Feld platziert“ bzw. „QR-Platzhalter gefunden – Signatur dort platziert“ nur bei automatischer Platzierung, Sprechblase nennt das Ziel; Suche aus → Felder nur angeboten; von Hand Platziertes bleibt |
| S-28 | E3: „Zuletzt signiert“ auf der Startansicht nach einigen Signaturen; Eintrag anklicken, Datei vorher verschieben, „Entfernen“, Einstellungen › Datenschutz „Liste leeren“ bzw. Schalter aus | höchstens 10 Einträge mit Name und Zeitpunkt (Ordner als Tooltip); Eintrag öffnet seit Nachtrag 15 die Datei (bis dahin den Prüfmodus); fehlende Datei → Hinweis, Eintrag bleibt; Abschalten löscht sofort; `recent.json` enthält nur Name, Pfad, Zeit |
| S-29 | E3: Startargument-Automatik `-b KS -ks … -a a.pdf` und während des Ladens die Einstellungen öffnen | mit Beginn des Signierens schließen sich die Einstellungen; die Signierkarte (Passwort) liegt oben |
| S-30 | E3 Mobile (Simulator/Emulator): Einstellungen | Vollbild mit waagrechter Bereichsleiste, ohne Proxy, Ordner, Update-Prüfung, Protokolle, Chipkarte und Bildmarke; Signiermittel nur ID Austria – kein Keystore (seit 16.10.2026, ADR 0004 Nachtrag 11; vorher nur über den PKCS#12-Import-Assistenten) |
| S-31 | **Entfällt (ADR 0004 Nachtrag 15, Prüfmodus entfernt).** E4: Prüfmodus – „Prüfen“ (Kopfzeile, Startansicht, Palette) bzw. „Zuletzt signiert“; Dateien aus `testdata/verify/` | Seiten links, rechts Ergebnis je Signatur: „Intakt“/„Verändert“/„Nicht prüfbar“, SHA-1-Hinweis, Unterzeichner, Zeit „(Angabe des Signierenden)“, QES „laut Zertifikat“ (`qes.pdf`), spätere Revisionen benannt, Zertifikatsdetails; „Auf der Seite zeigen“ hebt das Signaturbild hervor (auch `rotated-90.pdf`), unsichtbare Signaturen ohne Knopf; Hinweis und Link signaturpruefung.gv.at; nie „gültig“; „Dieses Dokument signieren“ öffnet die Datei (bei offenem Dokument: Warteschlange + Hinweis); nicht lesbare Datei → eigener Zustand |
| S-32 | **Entfällt (ADR 0004 Nachtrag 15).** E4: Prüfmodus über einem offenen Dokument bzw. der Stapelübersicht | Kopfzeile „Prüfmodus“ (ohne „Prüfen“), Fenstertitel „PIDA – Prüfmodus“; ⌘↵ und die Palette signieren das verdeckte Dokument nicht; „Prüfmodus verlassen“ → Arbeitsbereich unverändert; PDF ablegen → wird geprüft; ⌘O → Datei im Arbeitsbereich (Prüfmodus schließt); Wortmarke → Startansicht; während des Signierens ist „Prüfen“ gesperrt |
| S-33 | E4: Befehlspalette ⌘/Strg+K und Kürzelübersicht „?“ | Palette mit genau den freigegebenen Befehlen (Aktionen/Ansicht/Hilfe), Filter mit Synonymen („logo“, „einst“), Pfeile/Eingabe/Esc, ⌘K schaltet um; öffnet nicht über der Ersteinrichtung oder Rückfragen; Kürzelübersicht vollständig (Palette, Öffnen, Signieren, Einstellungen, Schließen, Zugriffstasten, 15/1/50 pt, Seite, Doppelklick, Zoom) |
| S-34 | E4 Mobile (Simulator/Emulator): Prüfmodus (entfällt, Nachtrag 15) und Ersteinrichtung | Seiten über dem Ergebnis, Sheet-Dialoge mit Safe Area, Ersteinrichtung ohne Chipkarte, Palette ohne Tastenkappen; App-Kill während der Ersteinrichtung → beim nächsten Start erneut |
| S-35 | Nachtrag 15: „Zuletzt signiert“ ohne Prüfmodus – Eintrag anklicken, Menü „Weitere Aktionen“ (Desktop: „Öffnen“, „Im Ordner zeigen“, „Erneut signieren“; Mobile: „Öffnen“, „Teilen“, „Erneut signieren“), Datei vorher verschieben, „Erneut signieren“ bei Karte „In Bearbeitung“ | Eintrag bzw. „Öffnen“ öffnet die Datei mit der Standardanwendung (Mobile: Vorschau), „Im Ordner zeigen“ zeigt sie im Finder/Explorer, „Teilen“ öffnet das Share-Sheet; „Erneut signieren“ öffnet sie im Arbeitsbereich (keine Sitzung bis „Signieren“), bei geparkter Arbeit erst „Zur offenen Arbeit hinzufügen?“; fehlende Datei → Hinweis, Eintrag bleibt; nirgends „Prüfen“/„Prüfmodus“ (Kopfzeile, Startseite, Palette, Kürzel) |

### 2.2 Tastatur und Barrierefreiheit

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| K-01 | Nur Tastatur: Tab/Umschalt+Tab durch alle Schritte | sichtbarer Fokus, logische Reihenfolge, „Zum Inhalt springen“ als erstes Element |
| K-02 | Zugriffstasten (macOS ⌃⌥+Taste, Windows/Linux Alt+Taste) | lösen die unterstrichenen Aktionen aus; doppelt belegte Tasten wandern reihum |
| K-03 | ⌘O / ⌘, / ⌘K / „?“ (Strg unter Windows/Linux) | Dateidialog, Einstellungen, Befehlspalette (seit E4) bzw. Kürzelübersicht (seit E4) |
| K-04 | Sticker mit Fokus: Pfeiltasten (15 pt), mit Umschalt (1 pt), mit ⌘/Strg (50 pt), Bild↑/↓ bzw. Pos1/Ende (Seite); ⌘/Strg+↵ | Block bewegt sich (Schritte rechnet der Kern), Live-Region kündigt Lage und Prüfergebnis an; ⌘/Strg+↵ = „Signieren“ (ausdrückliche Aktion) |
| K-05 | Esc in Dialogen | schließt ohne Änderung |
| K-06 | VoiceOver (macOS), NVDA (Windows), Orca (Linux) | Rollen und Namen aller Bedienelemente, Statusmeldungen werden vorgelesen (Protokoll anlegen; axe-core/Screenreader-Protokolle stehen noch aus) |
| K-07 | Hell/Dunkel, erhöhter Kontrast, reduzierte Bewegung, Fenster schmal (360 px) | lesbar, keine abgeschnittenen Bedienelemente |

### 2.3 Drag & Drop, „Öffnen mit“, Single-Instance, Startargumente

| Nr. | Prüfschritt | Erwartung |
|---|---|---|
| D-01 | PDF(s) auf das Fenster ziehen (Schritt Öffnen bzw. Ergebnis) | Dokumente kommen in die Warteschlange; Hervorhebung während des Ziehens |
| D-02 | PDF in einem anderen Schritt ablegen | Hinweis „hier nicht möglich“, nichts passiert |
| D-03 | Bild auf das Bildmarken-Feld in den Einstellungen ziehen | Bildmarke übernommen (seit E3 sofort, ohne Profilwechsel) |
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
| A-08 | Stapel (≥ 2 Dokumente, Stapelübersicht) – „Alle signieren (n)“ | eine Anmeldung je Stapel, je Dokument eine eigene Bestätigung mit eigenem Vergleichswert („Bestätigung k von n · Name“); weitere Anmeldung nur nach A-Trust-Fehlerseite bzw. Sitzungsfehler; „Dokument überspringen“ ohne neue Anmeldung (eine offene Anfrage in der App bitte ablehnen); Netzfehler → Pause „Erneut versuchen“ in derselben Sitzung; Zusammenfassung über alle Einträge mit Speicherort |
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
- E2: Der Stapel signiert mit ID Austria je Dokument einzeln (eine Anmeldung, n Bestätigungen); „Eine Anmeldung“
  ist eine Erwartung – nach einer A-Trust-Fehlerseite fragt die Karte erneut. Überspringen bzw. Abbrechen
  verwerfen die laufende Anfrage nur in PIDA; in der App kann sie bis zum Ablauf sichtbar bleiben (Schutz: eigener
  Vergleichswert). Wieder signierbar werden übersprungene/abgebrochene Einträge nur über „öffnen → Zurück zum
  Stapel“. Offen [H]: A-08 mit Testkonto, E2.8 im Simulator/Emulator.
- E3: Einstellungen wirken sofort (ADR 0004 §3.11, Nachtrag 5). „Zuletzt signiert“ öffnet bis E4 noch nicht den
  Prüfmodus (Hinweis); „PDF-Over-Einstellungen übernehmen …“ folgt mit E4; die Bereichsleiste der Einstellungen hat
  keine Zugriffstasten; Abschalten des Zertifikats-Caches leert vorhandene Einträge nicht. Offen [H]: Vorschau binnen
  einer Sekunde auf dem Referenzrechner (N-35 ist eine Näherung im Debug-Build), S-30 im Simulator/Emulator.
- Nachtrag 15 (17.10.2026): Der Prüfmodus ist entfernt; PIDA prüft wie PDF-Over keine fremden Signaturen und verweist
  auf signaturpruefung.gv.at. Offen [H]: S-35 auf dem Referenzrechner und am Telefon.
- E4: Prüfmodus, Befehlspalette und Kürzelübersicht, Ersteinrichtung (ADR 0004 §3.9, §3.12, Nachtrag 6). Der
  Prüfmodus prüft Integrität und Zertifikatsangaben, **nicht** Vertrauenskette und Sperrstatus (Hinweis auf
  signaturpruefung.gv.at); QES nur „laut Zertifikat“; PAdES-B-B anderer Werkzeuge ohne `signing-time` zeigt keine
  Zeit; Dokument-Zeitstempel sind nur im Code abgebildet. „Prüfbericht speichern“ fehlt (§8 F8). „Einstellungen
  zurücksetzen“ startet die Ersteinrichtung nicht erneut. Kleine Befunde aus N-44 (Name von Wortmarke und „Befehle
  ⌘K“, Befehlsliste mit virtuellem Fokus) offen. Offen [H]: S-31–S-34 auf dem Referenzrechner und im
  Simulator/Emulator.

# PIDA – App (Tauri 2)

Oberfläche und Shell von PIDA: **React 19 + TypeScript (strict) + React Aria Components**
(Vite) im System-WebView, **Tauri 2.12** als dünne Shell um den Rust-Kern `pida-core-api`
(Plan §2, §3.1; ADR 0001). Fachlogik liegt ausschließlich im Kern: Die Oberfläche projiziert den
`ViewState` und sendet `UiEvent`s.

| Teil | Ort |
|---|---|
| Bridge-Typen: **erzeugt** aus `crates/core-api/src/view.rs` (ts-rs; einzige Quelle des Vertrags) | `src/core/generated.ts` (über `src/core/types.ts`) |
| Bridge zu Tauri (`core_view`, `core_dispatch`, Ereignis `pida:view`, `pida-render://`) | `src/core/bridge.ts` |
| TS-Mock-Kern (Ablauf wie PDF-Over, für `VITE_MOCK=1`, Browser und Tests) | `src/core/mock.ts` |
| Fluent-Kataloge (zur Build-Zeit aus `crates/i18n/locales/{de,en}/*.ftl`, auch die UI-Texte `ng-ui-*`) | `src/i18n/` |
| Schritte, Masken, Einstellungen | `src/screens/`, `src/settings/` |
| Shell (PDF-Over-Startargumente, Single-Instance, „Öffnen mit“, Drag & Drop, Fensterzustand, Dialoge, Shell-Funktionen des Kerns, Custom Protocol, Startprüfung) | `src-tauri/` |

## Befehle

```sh
npm ci                 # Abhängigkeiten exakt nach package-lock.json
npm run dev:mock       # Oberfläche mit Mock-Kern im Browser (http://localhost:1420)
                       # Mobile-Ansicht: http://localhost:1420/?platform=ios (bzw. android), &resume=1
                       # zeigt „Signatur fortsetzen“ (docs/integration/mobile.md)
npm run build          # tsc (strict) + Vite → dist/
npm test               # Vitest + Testing Library (jsdom)
npm run tauri dev      # App mit Rust-Kern (Vite-Dev-Server + cargo)
npm run notices        # THIRD-PARTY-NOTICES nach ../../target/notices (cargo-about, PDFium, npm)
PIDA_PDFIUM_DIR=… npm run bundle:mac:debug    # macOS-.app (Debug) mit libpdfium.dylib + Hinweisen
PIDA_PDFIUM_DIR=… npm run bundle:mac:release  # .app + .dmg (Release), verlangt PDFium und Hinweise
PIDA_SMOKE=1 HOME="$(mktemp -d)" "../../target/debug/bundle/macos/PIDA.app/Contents/MacOS/pida"
```

Startargumente wie PDF-Over (`-i -o -b -n -p -ks -kstype -kspass -ksalias -kskeypass -proxy -proxyport
-proxyuser -proxypass -e -a -s -c -v`, freie Tokens = Dokumente), ausgewertet mit dem Parser von
`pida-cli` (`src-tauri/src/args.rs`); `-h` gibt die Hilfe aus, ungültige Argumente zeigen den
Fehlerschritt. Abnahme-Checkliste: `docs/integration/app-abnahme.md`.

Rust-Seite: `cargo check -p pida-app`, `cargo test -p pida-app` (Vertragstest
`tests/serde_contract.rs`). Typen und Vertragsdateien neu schreiben:

```sh
PIDA_BLESS=1 cargo test -p pida-core-api --test ts_bindings     # Rust-Typen → src/core/generated.ts
PIDA_BLESS=1 cargo test -p pida-app --test serde_contract   # Rust → src/core/fixtures/view-states.json
PIDA_BLESS=1 npx vitest run src/core/contract.test.ts          # TS   → src/core/fixtures/ui-events.json
```

## Laufzeit

- **PDFium** wird gesucht in `PIDA_PDFIUM_DIR` (**nur Debug-Builds**), neben der Binary, in
  `../Frameworks` (macOS-Bundle) und im Ressourcenordner (`pdfium/`). Ohne Treffer entscheidet in
  Debug-Builds der Kern; ein Release-Build lädt nie aus der Umgebung oder Systempfaden (dann ohne
  Vorschau, automatische Position; `src/pdfium.rs`).
- **Kern nicht verfügbar** (Startfehler, Panik) bzw. Konfiguration nicht ermittelbar: Die Shell zeigt
  den Fehlerschritt `ng-ui-core-unavailable`, ein unerwartet beendeter Kern-Task `ng-core-stopped` –
  jeweils mit „Wiederholen“ (Abbrechen = Beenden) statt abzustürzen; „Öffnen mit“-Dokumente aus dieser
  Zeit werden für den nächsten Start vorgemerkt.
- **„Öffnen mit“ beim Kaltstart (macOS)**: `RunEvent::Opened` kommt vor `setup`; die Dokumente merkt
  `instance::EarlyDocuments` vor und übergibt sie mit dem Kernstart.
- **Single-Instance**: Plugin `single-instance`; unter macOS nur, wenn dessen Socket im gemeinsamen
  `/tmp` fehlt oder dem eigenen Konto gehört (sonst erhielte ein fremdes Konto die Startargumente samt
  Passwörtern) – „Öffnen mit“/Dock übergibt dort ohnehin LaunchServices (`src/instance.rs`).
- **Beenden** (Fenster schließen, ⌘Q) läuft über den Kern (`UiEvent::Quit` → `Finished`): laufende
  Dialoge werden abgebrochen (F-73), Einstellungen gesichert (F-105); spätestens nach 5 s endet die App.
- **Logs**: Log-Ordner der Plattform (`pida_config::Paths`), rotierend (`pida_core_api::logging`);
  Filter über `--log-level`, sonst `PIDA_LOG`.
  Keine Passwörter, TANs oder Dokumentinhalte (ADR 0001 Regel 9).
- **Sicherheit**: CSP `default-src 'self'`, minimale Capabilities (`capabilities/default.json`: nur
  Ereignisse empfangen, eigene Commands, Datei-/Speichern-Dialog, Links; kein `core:default`),
  externe Links nur `https:`/`mailto:`, „Im Ordner anzeigen“ nur für die Ergebnisdatei des Kerns.
  `tests/security_config.rs` hält Capabilities und CSP fest.

## Barrierefreiheit (WCAG 2.2 AA)

React Aria Components für Rollen, Fokus und Tastatur; Zugriffstasten aus den `&`-Mnemonics der Kataloge
(Alt+Taste, macOS ⌃⌥+Taste); Positionierung vollständig per Tastatur (Pfeile, Bild↑/↓, Pos1/Ende) mit
Live-Region; Fokus springt beim Schrittwechsel auf die Überschrift; Hell/Dunkel, Kontrastdesigns
(`forced-colors`), reduzierte Bewegung, Zielgrößen ≥ 44 px, responsiv bis Smartphone-Breite.

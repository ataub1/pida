# PDF-Over NG – App (Tauri 2)

Oberfläche und Shell von PDF-Over NG: **React 19 + TypeScript (strict) + React Aria Components**
(Vite) im System-WebView, **Tauri 2.12** als dünne Shell um den Rust-Kern `pdfover-core-api`
(Plan §2, §3.1; ADR 0001). Fachlogik liegt ausschließlich im Kern: Die Oberfläche projiziert den
`ViewState` und sendet `UiEvent`s.

| Teil | Ort |
|---|---|
| Bridge-Typen (exakt serde-Format von `crates/core-api/src/view.rs`) | `src/core/types.ts` |
| vorgeschlagene additive Erweiterungen (Ausgabe-Rückfragen, Hinweise, „Unsichtbar“) | `src/core/extensions.ts` |
| Bridge zu Tauri (`core_view`, `core_dispatch`, Ereignis `pdfover:view`, `pdfover-render://`) | `src/core/bridge.ts` |
| TS-Mock-Kern (Ablauf wie PDF-Over, für `VITE_MOCK=1`, Browser und Tests) | `src/core/mock.ts` |
| Fluent-Kataloge (zur Build-Zeit aus `crates/i18n/locales/{de,en}/*.ftl`) + UI-Texte `ng-ui-*` | `src/i18n/` |
| Schritte, Masken, Einstellungen | `src/screens/`, `src/settings/` |
| Shell (Single-Instance, „Öffnen mit“, Drag & Drop, Fensterzustand, Dialoge, Custom Protocol) | `src-tauri/` |

## Befehle

```sh
npm ci                 # Abhängigkeiten exakt nach package-lock.json
npm run dev:mock       # Oberfläche mit Mock-Kern im Browser (http://localhost:1420)
npm run build          # tsc (strict) + Vite → dist/
npm test               # Vitest + Testing Library (jsdom)
npm run tauri dev      # App mit Rust-Kern (Vite-Dev-Server + cargo)
npm run tauri build -- --debug --bundles app   # macOS-.app (Debug), ohne PDFium
PDFOVER_PDFIUM_DIR=… npm run bundle:mac:debug  # macOS-.app mit libpdfium.dylib in Contents/Frameworks
```

Rust-Seite: `cargo check -p pdfover-app`, `cargo test -p pdfover-app` (Vertragstest
`tests/serde_contract.rs`). Beide Vertragsdateien neu schreiben:

```sh
PDFOVER_BLESS=1 cargo test -p pdfover-app --test serde_contract   # Rust → src/core/fixtures/view-states.json
PDFOVER_BLESS=1 npx vitest run src/core/contract.test.ts          # TS   → src/core/fixtures/ui-events.json
```

## Laufzeit

- **PDFium** wird gesucht in `PDFOVER_PDFIUM_DIR` (**nur Debug-Builds**; ein Release-Build lädt keine
  Bibliothek aus einem per Umgebung genannten Ordner), neben der Binary, in `../Frameworks`
  (macOS-Bundle) und im Ressourcenordner (`pdfium/`); sonst entscheidet der Kern.
- **Kern nicht verfügbar** (Fehler oder Stub in Welle 2) bzw. Konfiguration nicht ermittelbar: Die
  Shell zeigt den Fehlerschritt `ng-ui-core-unavailable` mit „Wiederholen“ (Abbrechen = Beenden) statt
  abzustürzen; „Öffnen mit“-Dokumente aus dieser Zeit werden für den nächsten Start vorgemerkt.
- **Beenden** (Fenster schließen, ⌘Q) läuft über den Kern (`UiEvent::Quit` → `Finished`): laufende
  Dialoge werden abgebrochen (F-73), Einstellungen gesichert (F-105); spätestens nach 5 s endet die App.
- **Logs**: Log-Ordner der Plattform (`pdfover_config::Paths`), rotierend; Filter über `PDFOVER_LOG`.
  Keine Passwörter, TANs oder Dokumentinhalte (ADR 0001 Regel 9).
- **Sicherheit**: CSP `default-src 'self'`, minimale Capabilities (`capabilities/default.json`: nur
  Ereignisse empfangen, eigene Commands, Datei-/Speichern-Dialog, Links; kein `core:default`),
  externe Links nur `https:`/`mailto:`, „Im Ordner anzeigen“ nur für die Ergebnisdatei des Kerns.
  `tests/security_config.rs` hält Capabilities und CSP fest.

## Barrierefreiheit (WCAG 2.2 AA)

React Aria Components für Rollen, Fokus und Tastatur; Zugriffstasten aus den `&`-Mnemonics der Kataloge
(Alt+Taste, macOS ⌃⌥+Taste); Positionierung vollständig per Tastatur und Zahlenfeldern mit
Live-Region; Fokus springt beim Schrittwechsel auf die Überschrift; Hell/Dunkel, Kontrastdesigns
(`forced-colors`), reduzierte Bewegung, Zielgrößen ≥ 44 px, responsiv bis Smartphone-Breite.

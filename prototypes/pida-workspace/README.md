# PIDA – klickbarer Prototyp „Arbeitsbereich“

**PIDA** (PDF · ID · Austria) ist der neue Name des PDF-Over-Klons. Dieser Prototyp zeigt die vorgeschlagene,
moderne Oberfläche: **ein Arbeitsbereich statt vier Schritten**, Signieren als **Statuskarte**, Einstellungen mit
**sofortiger Wirkung**, Stapelbetrieb, Prüfmodus, Befehlspalette – alles mit realistischen Mock-Daten.

> Eigenständig, ohne Rust-Kern und ohne Tauri. **Keine Netzwerkzugriffe zur Laufzeit**, keine externen Fonts
> oder CDNs, keine echten Zugangsdaten. Signieren, Prüfen und Speichern sind reine Simulation.
> Gestaltungsentscheidungen und Übertragung auf `apps/app`: [`docs/ux-konzept.md`](docs/ux-konzept.md).

## Starten

Voraussetzung: Node.js ≥ 22.12 und npm.

```sh
cd prototypes/pida-workspace
npm ci
npm run dev        # http://localhost:1430 (fester Port, strictPort)
```

| Skript | Zweck |
| --- | --- |
| `npm run dev` | Entwicklungsserver auf Port **1430** |
| `npm run build` | Typprüfung (`tsc`) und Produktions-Build nach `dist/` |
| `npm run preview` | gebauten Stand auf Port 1430 ausliefern |
| `npm test` | alle Tests (Vitest + Testing Library + axe-core) |
| `npm run typecheck` | nur Typprüfung |

Abnahme: `npm ci && npm run build && npm test` (grün).

**Direkt in ein Szenario springen:** `http://localhost:1430/?scenario=batch` (Werte siehe Tabelle unten).
`&onboarding=0` überspringt die Ersteinrichtung, `&lang=en` startet englisch.
Farbschema, Sprache und „Ersteinrichtung erledigt“ merkt sich der Browser (localStorage); die Demo-Steuerung
bietet „Ersteinrichtung zeigen“ und „Datenschutzhinweis zurücksetzen“.

## Demo-Steuerung

Unten links liegt dezent der Knopf **„Demo“**. Er klappt ein Panel auf:

- **Szenario** – lädt einen Ausgangszustand (Einstellungen bleiben erhalten).
- **Antwort des Signiermittels** – bestimmt, wie die simulierte ID Austria nach der Anmeldung reagiert.
- **Signiermittel** – ID Austria, Keystore oder Bürgerkarte (auch über den Konto-Chip im Arbeitsbereich).
- **Szenario neu laden**, **Ersteinrichtung zeigen**, **Datenschutzhinweis zurücksetzen**.

### Szenarien

| Szenario (`?scenario=`) | Inhalt | Was man zeigen kann |
| --- | --- | --- |
| `start` | Leerzustand | Ablagefläche, „Zuletzt signiert“ (öffnet Prüfmodus), Hinweis auf Prüfmodus, Beispielvertrag |
| `blank` | Leeres Dokument, 1 Seite | Sticker rechts unten, Einrasten an Rändern/Mitte |
| `contract` | Dienstleistungsvertrag, 3 Seiten (Text, Liste, Tabelle, Unterschriftszeilen), PDF/A | Kernablauf: automatisch platziert → ziehen → Signieren → Fertig; Hinweis „PDF/A-Dokument erkannt“ |
| `field` | Förderantrag, 2 Seiten, mit leerem Signaturfeld | „Hier signieren“-Markierung, Hinweisleiste „Ins Feld setzen“; mit Einstellungen › Dokument › „Leere Signaturfelder“ automatisch im Feld |
| `signed` | Mietvertrag, bereits signiert (Dr. Anna Gruber) | „Vorhandene Signaturen“, „Auf neuer Seite“ gesperrt |
| `batch` | Stapel mit 4 Dateien (eine kennwortgeschützt, eine mit QR-Platzhalter) | „Alle signieren“ mit einer Anmeldung, Fortschritt, Zusammenfassung, Toast |
| `verify` | Kaufvertrag mit 2 Signaturen | Prüfmodus: Status, Zertifikatsdetails aufklappbar, „Auf der Seite zeigen“ |
| `error-password` | Gehaltszettel, kennwortgeschützt | Fehler in Alltagssprache mit Aktion – Demo-Kennwort **`demo`** |
| `error-atrust` | Vertrag; A-Trust beim ersten Verbindungsaufbau „nicht erreichbar“ | Fehlerkarte, Details aufklappbar, „Erneut versuchen“ |

### Antworten des Signiermittels

| Antwort | Verhalten nach „Anmelden“ (beliebiges Passwort ab 5 Zeichen) |
| --- | --- |
| App bestätigt (Standard) | App-Bestätigung mit Vergleichswert und Countdown; bestätigt sich nach ~9 s selbst oder per „In App bestätigt (Demo)“ |
| FIDO2 | direkt FIDO2: PIN (ab 4 Zeichen) → „Schlüssel berühren“ (automatisch nach ~6 s oder per Demo-Knopf) |
| SMS-TAN | TAN-Eingabe; der Vergleichswert wird als TAN abgelehnt, jede andere TAN (≤ 6 Zeichen) gilt |
| Abbruch in der App | nach ~3,5 s „In der App abgelehnt“ |
| Zeitablauf | Countdown von 0:12, danach „Die Zeit ist abgelaufen“ → „Neue Bestätigung anfordern“ |

Aus der App-Bestätigung kann man jederzeit über **„Stattdessen: FIDO2-Schlüssel · SMS-TAN“** wechseln; der
Datenschutzhinweis erscheint nur beim ersten Signieren mit ID Austria.

## Tastatur

| Kürzel | Wirkung |
| --- | --- |
| ⌘/Strg + K | Befehlspalette (Öffnen, Signieren, Einstellungen, Prüfen, Farbschema, Sprache …) |
| ⌘/Strg + O | Dokument öffnen |
| ⌘/Strg + Eingabe | Signieren |
| ⌘/Strg + , | Einstellungen |
| ? | Tastenkürzel-Übersicht |
| Pfeiltasten (Sticker fokussiert) | Signaturblock um 10 pt verschieben; **Umschalt**: 1 pt, **Alt**: 50 pt |
| Bild ↑ / Bild ↓ (Sticker fokussiert) | Block auf vorige/nächste Seite |
| ⌘/Strg + Plus / Minus / 0 | Zoom / An Breite anpassen |
| Doppelklick auf eine Seite | Block an diese Stelle setzen |

## Aufbau

```
src/
  app/        App-Rahmen, Zustand (Reducer), Zeitverhalten der Simulation
  features/   start, onboarding, workspace (Seiten, Sticker, Inspektor), signing (Statuskarte),
              done, batch, settings, verify, palette, demo, sigblock (Signaturblock)
  model/      Typen, Mock-Dokumente (in pt gesetzt), Geometrie (Einrasten, freie Flächen), Formatierung
  i18n/       Texte Deutsch (Standard) und Englisch
  ui/         Design-System-Bausteine auf React Aria Components, Icons, Wortmarke
  styles/     tokens.css (Design-Tokens, Hell/Dunkel), base, components, document, views
  test/       Test-Setup, Render-/axe-Helfer, Katalog- und Kontrasttests
```

Technik wie `apps/app`: React 19.3.1, React Aria Components 1.22.1, TypeScript 6.0.3, Vite 8.3.4,
Vitest 5.0.4, jsdom 30.1.2, Testing Library; zusätzlich **axe-core 4.14.0** für die Barrierefreiheitstests.
Keine weiteren UI-Frameworks; eigenes Design-System über CSS-Variablen.

## Tests

- **Durchläufe:** Öffnen → Signieren mit App-Bestätigung → Fertig; FIDO2-Zweig; SMS-TAN; Stapel „Alle signieren“;
  Einstellungen ändern die Signaturbild-Vorschau; Prüfmodus; Fehlerwege (Kennwort, A-Trust, Zeitablauf, Ablehnung,
  Abbrechen); Keystore; Befehlspalette, Kürzel, Ersteinrichtung, Demo-Steuerung.
- **Sticker:** Tastatur (Pfeile, Umschalt fein, Bild auf/ab) mit Live-Region, Ziehen mit Einrasten und
  Freiflächen, Doppelklick, Überdeckungswarnung, Signaturfeld, „Unsichtbar“/„Auf neuer Seite“, Zoom.
- **Barrierefreiheit:** axe-core ohne Verstöße für alle Hauptansichten und Overlays (Start, Ersteinrichtung,
  Arbeitsbereich, Signierkarte in allen Schritten, Fertig, Stapel, Einstellungen je Bereich, Prüfmodus,
  Palette, Kürzel, Demo, Englisch, schmale Ansicht mit Bottom-Sheet) plus Gegenprobe.
  `color-contrast` ist in jsdom nicht messbar; stattdessen prüft `src/test/catalog.test.ts` alle Text- und
  Bedienelement-Farbpaare der Tokens in Hell und Dunkel gegen WCAG 2.2 AA.
- **Regeln:** automatische Position, Einrasten, freie Flächen, Dateinamenmuster; DE/EN-Kataloge vollständig.
- **Review-Befunde** (`src/test/review.test.tsx`): Fehler im Stepper mit Symbol und Screenreader-Text,
  Einzahl/Mehrzahl und Speicherort im Stapel, Überdeckungswarnung direkt über „Signieren“, Fokus nach Overlays
  und Demo-Steuerung, Toolbar mit Pfeiltasten, Einstellungssuche mit Vorschau, vollständige Kürzelübersicht,
  Vorwarnung vor Ablauf der App-Bestätigung, unterscheidbare Statusfarben, stimmige „Zuletzt signiert“-Daten;
  dazu Token-Tests (Fokusfarbe deutlich anders als Fehlerrot, beide Dunkel-Blöcke identisch).

Screenshots des Reviews (Desktop 1280 × 800, Smartphone 375 × 812, Dunkelmodus) liegen unter
[`docs/screenshots/`](docs/screenshots/).

## Grenzen des Prototyps

- Dokumente werden **nicht gelesen**: Jede geöffnete Datei erscheint als Beispielvertrag mit ihrem Dateinamen
  (mehrere Dateien als Stapel). Logos für den Signaturblock werden lokal als Data-URL gelesen.
- Links nach außen (signaturpruefung.gv.at, A-Trust-Datenschutzerklärung) öffnen nichts, sondern zeigen einen
  Hinweis („würde im Browser geöffnet“).
- Texte und Personen sind frei erfunden; Logos sind eigene Entwürfe (kein Bundesadler, keine ID-Austria- oder
  A-Trust-Logos).

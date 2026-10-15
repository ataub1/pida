# PIDA – UX-Konzept „Arbeitsbereich“

> Begleitdokument zum klickbaren Prototyp unter `prototypes/pida-workspace`. Beschreibt die
> Gestaltungsentscheidungen, das Design-System und wie sich beides schrittweise auf `apps/app`
> (Tauri 2 + React, Rust-Kern) übertragen lässt. Bezüge auf Funktionen von PDF-Over (F-xx) siehe
> `docs/analyse/01-funktionsumfang-und-ablauf.md`.

## 1. Ausgangslage und Ziel

PDF-Over – und damit `apps/app` – folgt einer **Hauptleiste mit vier Schritten** (Öffnen → Positionierung →
Signatur → Fertig), je ein Bildschirm (§4.0). Das ist nachvollziehbar, kostet aber bei jedem Dokument vier
Kontextwechsel, versteckt den Signaturstatus hinter Bildschirmwechseln und bietet für Stapel, Prüfen und
Einstellungen keine passenden Orte.

**Ziel:** In möglichst wenigen, vorhersehbaren Handgriffen rechtsgültig signieren – mit der Sicherheit,
_was_ wo signiert wird – und dabei Tastatur, Screenreader, Hell/Dunkel und schmale Geräte gleichwertig behandeln.

**Messbarer Kernpfad (Einzeldokument, ID Austria, gemerkter Benutzer):** Datei ablegen → **Signieren** →
Passwort → in der App bestätigen → fertig. Kein Positionieren nötig, kein Bildschirmwechsel bis „Fertig“.

## 2. Leitprinzipien

1. **Ein Arbeitsbereich statt Schritte.** Dokument, Platzierung und Signiermittel sind gleichzeitig sichtbar;
   „Signieren“ ist immer derselbe primäre Knopf (⌘/Strg + Eingabe).
2. **Automatik zuerst, Korrektur jederzeit.** Der Signaturblock liegt beim Öffnen bereits auf freier Fläche
   der letzten Seite (rechtsbündig unter dem Text); QR-Platzhalter werden automatisch genutzt; leere
   Signaturfelder werden angeboten. Wer will, zieht oder tippt nach.
3. **Status statt Seitenwechsel.** Signieren läuft in einer **Statuskarte** über dem Dokument
   (Anmelden → Bestätigen → Signiert). Das Dokument bleibt im Blick, Abbrechen ist immer möglich.
4. **Alltagssprache mit Ausweg.** Jeder Fehler sagt, _was_ passiert ist, _was jetzt hilft_ (eine Aktion) und
   hält Technik in aufklappbaren Details bereit (Fehlercode, Zeitpunkt).
5. **Sofort wirksam.** Einstellungen haben kein „Speichern/Abbrechen“ – jede Änderung gilt sofort und ist
   in der Live-Vorschau sichtbar (vgl. F-101/F-102: dort Validierungsdialoge beim Speichern).
6. **Gleichwertige Bedienung.** Alles per Tastatur; Ziehen hat eine Tastatur-Entsprechung mit Live-Region;
   Fokus ist immer sichtbar; Bedienelemente kommen aus React Aria.
7. **Ruhiges, eigenständiges Erscheinungsbild.** Neutrale Flächen, eine klare Typografie, Rot-Weiß-Rot nur
   als Akzent – keine Hoheitszeichen, keine Fremdlogos.

## 3. Informationsarchitektur

| Ebene | Elemente |
| --- | --- |
| **Kopfzeile** | Wortmarke (→ Start), Dokumentkontext (Name, Seiten, Größe; im Stapel „Zurück“), „Prüfen“, „Befehle ⌘K“, DE/EN, Farbschema, Einstellungen |
| **Ansichten** | Start/Leerzustand · Arbeitsbereich · Stapel · Fertig · Prüfmodus |
| **Overlays** | Signierkarte · Einstellungen (Seitenleiste) · Befehlspalette · Tastenkürzel · Ersteinrichtung |
| **Rückmeldungen** | Toasts (Erfolg/Info verschwinden nach 6 s, pausieren bei Fokus/Hover; Fehler bleiben; im Arbeitsbereich links neben der Seitenleiste, damit sie „Signieren“ nicht verdecken), Live-Region im Arbeitsbereich |
| **Nur Prototyp** | Demo-Steuerung unten links (Szenario, Antwort des Signiermittels) |

### Abbildung der PDF-Over-Zustände

| PDF-Over (docs/analyse/01 §3) | PIDA |
| --- | --- |
| Open (F-10…F-16) | Start: Ablagefläche, Dateiauswahl, ⌘O; mehrere Dateien → Stapel |
| Platzhalter-Dialog Ja/Nein/Abbrechen (F-20/F-21) | kein Dialog: QR-Platzhalter automatisch, Signaturfeld als „Hier signieren“-Markierung + Hinweisleiste „Ins Feld setzen“ |
| Positioning (F-30…F-43) | Arbeitsbereich mit Sticker; automatische Position; „Auf neuer Seite“, „Unsichtbar“ als Schalter |
| BKUSelection (F-50…F-56) | Konto-Chip „ID Austria · +43 660 ···123“ im Inspektor; Standard-Signiermittel in den Einstellungen |
| MobileBKU-Masken (F-63…F-68) | Signierkarte: Anmelden, App-Bestätigung (Vergleichswert, Countdown), FIDO2, SMS-TAN |
| Keystore/LocalBKU (F-62, F-70) | Signierkarte mit eigenem, kurzem Weg (Passwort bzw. „PIN in der Bürgerkartenumgebung“) |
| Output (F-80…F-91) | Fertig: Prüfsiegel, Dateiname nach Muster, Öffnen/Ordner/Teilen/Nächstes; im Stapel Toast |
| Batch (F-88/F-89) | Stapelansicht mit Status je Datei, „Alle signieren“ mit einer Anmeldung |
| ConfigurationUI (F-101…F-103) | Einstellungen als Seitenleiste mit Suche, sofort wirksam |
| — (neu) | Prüfmodus, Befehlspalette, Ersteinrichtung mit Übernahme der PDF-Over-Einstellungen |

## 4. Abläufe

### 4.1 Einzeldokument

1. **Start:** große Ablagefläche („PDF hierher ziehen oder auswählen“), daneben „Zuletzt signiert“ (öffnet
   im Prüfmodus) und der Hinweis auf den Prüfmodus. Ohne PDF: „Beispielvertrag öffnen“.
2. **Arbeitsbereich:** Der Block steht auf der letzten Seite, die Ansicht scrollt dorthin, eine kurze
   Einblendung erklärt „Signaturblock automatisch auf freier Fläche platziert“ (verschwindet bei der ersten
   Interaktion).
3. **Signieren:** Statuskarte. Beim ersten Mal ein knapper Datenschutzhinweis (Dokument geht an A-Trust).
   Danach Anmeldung (Benutzer vorbelegt, Fokus im Passwortfeld), App-Bestätigung mit großem
   **Vergleichswert** in Einzelzeichen und **Countdown-Ring**; Alternativen „FIDO2-Schlüssel · SMS-TAN“
   immer sichtbar.
4. **Fertig:** grünes Prüfsiegel „Signatur gültig – geprüft“ (PIDA prüft nach dem Speichern, vgl. die
   Selbstprüfung der PAdES-Engine), Vorschau der signierten Seite, Dateiname `{name}_signiert.pdf`,
   Aktionen Öffnen / Im Ordner zeigen / Teilen, primär **Nächstes Dokument**, Link zu signaturpruefung.gv.at.

### 4.2 Signaturfelder und QR-Platzhalter

- **QR-Platzhalter** (PDF-AS-Marker) sind ausdrücklich für die automatische Platzierung gedacht → der Block
  wird direkt hineingesetzt (Status „Positioniert · im QR-Platzhalter“).
- **Leere Signaturfelder (AcroForm)** werden als pulsierende **„Hier signieren“**-Markierung gezeigt; eine
  Hinweisleiste über der Seite bietet „Ins Feld setzen“. Beim Ziehen über ein Feld rastet der Block ein
  („Im Signaturfeld“, grüner Rahmen). „Frei platzieren“ macht es rückgängig.
- Die feste Platzierung im Feld ersetzt den Ja/Nein/Abbrechen-Dialog von PDF-Over: keine modale Frage, die
  Entscheidung ist sichtbar und umkehrbar.
- Wonach automatisch gesucht wird, steht in Einstellungen › Dokument (wie PDF-Over `USE_MARKER`/`USE_FIELDS`):
  „QR-Code-Platzhalter“ (Standard) oder „Leere Signaturfelder“ (dann wird das erste leere Feld automatisch
  genutzt); ausgeschaltet wird nichts automatisch genutzt, Markierungen bleiben als Angebot. Der Inspektor nennt
  dezent, was erkannt wurde („QR-Platzhalter gefunden – Signatur dort platziert“, „PDF/A-Dokument erkannt“).

### 4.3 Stapel

Mehrere Dateien → Liste mit Miniatur, Seitenzahl, Größe und Status (**Bereit · Positioniert · Wird signiert ·
Fertig · Fehler**). Statusfarben: grau = wartet (Bereit, Positioniert), blau = läuft (Wird signiert), grün =
fertig, rot = Fehler – „Wird signiert“ bewusst nicht im roten Markenakzent. „Alle signieren (n)“ startet
**eine** Anmeldung; danach Fortschrittsbalken, Zeile für Zeile, und am Ende eine Zusammenfassung über den
ganzen Stapel („3 von 4 signiert, 1 übersprungen“, Speicherort „jeweils neben dem Original“ bzw. der feste
Ordner) plus Toast. Vorher einzeln signierte Dokumente zählen als signiert, nicht als übersprungen. Dokumente
mit Fehler (z. B. Kennwort) blockieren den Stapel nicht; sie lassen sich einzeln öffnen und beheben.

### 4.4 Fehler

| Fall | Darstellung | Aktion |
| --- | --- | --- |
| Kennwortgeschütztes PDF | Zustandskarte statt Seiten: „Dieses PDF ist mit einem Kennwort geschützt“ | Kennwort eingeben (Demo: `demo`), „Anderes Dokument öffnen“; Details: `pdf-user-password`, Verschlüsselung |
| A-Trust nicht erreichbar | Fehlerschritt in der Signierkarte | „Erneut versuchen“; Details: `backend-network`, Host, Zeitpunkt |
| Zeitablauf App-Bestätigung | Fehlerschritt | „Neue Bestätigung anfordern“ |
| Ablehnung in der App | Fehlerschritt | „Erneut versuchen“ / Schließen |
| Abbruch durch Nutzer | Karte schließt, Toast „es wurde nichts signiert“ | – |

Fehlerkarten nennen nie Stacktraces im Fließtext; Codes stehen in aufklappbaren „Technischen Details“ und
entsprechen den stabilen Fehlercodes der Crates (`crates/i18n/locales/*/pida.ftl`).

### 4.5 Prüfmodus

Signiertes PDF ablegen → links die Seiten (Signaturbilder werden beim Überfahren/„Auf der Seite zeigen“
hervorgehoben), rechts eine Zusammenfassung („Alle 2 Signaturen sind gültig“ bzw. „Achtung: Eine Signatur
ist ungültig“ mit Handlungsempfehlung) und je Signatur eine Karte: Unterzeichner, Rolle, Zeitpunkt,
QES/AdES, Methode, Integrität („Dokument seit dieser Signatur unverändert“) und aufklappbare
Zertifikatsdetails. Von hier aus: „Dieses Dokument signieren“ (Gegenzeichnen), „Prüfbericht speichern“.

## 5. Arbeitsbereich im Detail

- **Raster:** Miniaturen (148 px) | Seitenansicht | Inspektor (328 px). Unter 1100 px schmaler, unter 760 px
  Miniaturen als Überlagerung und Inspektor als **Bottom-Sheet** (eingeklappt: Konto-Chip + „Signieren“).
- **Seitenansicht:** alle Seiten durchgehend scrollbar, „An Breite anpassen“ als Standard, Zoom in Stufen
  (50–200 %), Seitenanzeige folgt dem Scrollen, Miniatur springt zur Seite.
- **Sticker:** der echte Signaturblock (Profil, Sprache, Logo, Hinweis aus den Einstellungen), mit dezentem
  Schatten, Griff-Lasche beim Überfahren und Fokusrahmen. Ziehen über Seitengrenzen hinweg; beim Ziehen:
  - **Einrastlinien** (rot, 1 px) an Satzspiegelrändern links/rechts/oben/unten, horizontaler und vertikaler
    Mitte und „unter dem Text“; rechts + unten ergibt „rechts unten“.
  - **Freie Flächen** als zart getönte, gestrichelte Bänder.
  - **Überdeckung** von Inhalt: Rahmen und Plakette in Bernstein („Überdeckt Inhalt“), dazu eine Warnung mit
    „Freie Fläche suchen“ **direkt über „Signieren“** (auch im eingeklappten Bottom-Sheet), damit sie vor dem
    Signieren nicht unten in der Seitenleiste verschwindet.
- **Doppelklick** auf eine Seite setzt den Block dorthin (PDF-Over: Einfachklick, F-34 – im scrollbaren
  Mehrseiten-Layout würde ein Einfachklick beim Lesen versehentlich verschieben).
- **Tastatur:** Pfeile 10 pt, Umschalt 1 pt (feiner), Alt 50 pt, Bild auf/ab wechselt die Seite. Der Sticker
  ist `role="application"` mit `aria-roledescription="verschiebbarer Signaturblock"` und Hilfetext; eine
  **Live-Region** sagt nach jeder Bewegung z. B. „Seite 3 von 3, 112 mm von links, 38 mm von unten. Rastet ein:
  rechts unten. Freie Fläche.“
- **Keine Zahlenfelder für die Position** (vom Auftraggeber verworfen). Der Inspektor zeigt stattdessen eine
  sprechende Lage („Seite 3 · rechts unten“) und „Neu platzieren“.
- **Inspektor:** Signieren mit (Konto-Chip mit Umschalter), Signaturbild (Vorschau + Bearbeiten → springt in
  die Einstellungen), Optionen (Unsichtbar signieren, Auf neuer Seite – bei bereits signierten Dokumenten
  gesperrt mit Begründung), Platzierung, vorhandene Signaturen, primärer Knopf „Signieren ⌘↵“.

## 6. Signierkarte

- Modal (Fokusfalle, Esc = Abbrechen), aber **ohne** Schließen durch Klick daneben – ein versehentlicher Klick
  soll keinen laufenden Vorgang verwerfen.
- **Fortschritt** als dreiteiliger Stepper; der mittlere Schritt benennt den gewählten Weg (App / FIDO2 / SMS).
  Beim Schrittwechsel geht der Fokus auf das erste Feld bzw. die Schrittüberschrift.
- **Vergleichswert** groß und in Einzelzeichen (Screenreader: buchstabiert), Countdown als `role="timer"`
  (nicht laufend angesagt), Warnfarbe in den letzten 30 s.
- **FIDO2:** PIN des Schlüssels → „Berühren Sie jetzt Ihren Sicherheitsschlüssel“ mit Pulsanimation.
- **SMS-TAN:** Eingabe mit `autocomplete="one-time-code"`; der Vergleichswert wird als TAN erkannt und
  freundlich abgewiesen (PDF-Over F-66).
- **Datenschutzhinweis** einmalig vor der ersten ID-Austria-Signatur, erneut auslösbar in Einstellungen ›
  Erweitert.
- Im Stapel: „3 Dokumente – eine Anmeldung“, danach schließt die Karte und die Liste zeigt den Fortschritt.

## 7. Einstellungen

Seitenleiste von rechts (auf schmalen Geräten Vollbild), **Suche** über alle Bereiche (Treffer auch über
Synonyme wie „Logo“, „Handy“, „Proxy“), Bereichsnavigation links:

- **Signaturbild:** Live-Vorschau (bleibt beim Bearbeiten oben sichtbar), Profil Standard (klein) /
  Amtssignatur, Blocksprache DE/EN, Bildmarke per Drag & Drop oder Dateiauswahl (bleibt lokal), Hinweistext
  mit „Standardtext wiederherstellen“.
- **Dokument:** „PDF/A-kompatibel signieren“ (PDF/A wird ohnehin erkannt; der Schalter erzwingt es auch ohne
  Kennung), „Signaturplatzhalter automatisch suchen“ mit Auswahl QR-Code-Platzhalter / Leere Signaturfelder.
- **Signiermittel:** Standard (ID Austria / Keystore / Bürgerkarte / Jedes Mal fragen), Konten (ID-Austria-
  Benutzer + „merken“, Keystore-Datei + Alias, Bürgerkartenumgebung erkannt), FIDO2 bevorzugen.
- **Speichern:** Neben dem Original / fester Ordner / jedes Mal fragen; Namensmuster `{name}_signiert`
  (Platzhalter `{name}`, `{datum}`) mit Live-Beispiel „Vertrag.pdf → Vertrag_signiert.pdf“.
- **Darstellung:** Farbschema (System/Hell/Dunkel), Oberflächensprache.
- **Erweitert:** Proxy (System/kein/manuell mit Port-Prüfung), Updates, Protokolle,
  Datenschutzhinweis erneut zeigen, PDF-Over-Einstellungen übernehmen.

**Ersteinrichtung** (einmalig, überspringbar, 3 Schritte): gefundene PDF-Over-Einstellungen übernehmen
(Liste dessen, was übernommen wird; alte Datei bleibt unverändert), Standard-Signiermittel, Signaturbild.

## 8. Design-System

Alle Werte als CSS-Variablen in `src/styles/tokens.css`; Komponenten nutzen ausschließlich Tokens.

| Bereich | Festlegung |
| --- | --- |
| **Typografie** | System-Schriften (kein Laden aus dem Netz): `system-ui`, Segoe UI Variable, Roboto …; Monospace für Vergleichswert, Seriennummern, Muster. Skala 12 · 13 · 14 (Basis) · 15 · 16 · 18 · 20 · 24 · 30 · 36 px; Gewichte 400/520/620/700/800; Zeilenhöhe 1,5 (Text), 1,2 (Titel) |
| **Abstände** | 4/8-pt-Raster: 4 · 8 · 12 · 16 · 20 · 24 · 32 · 40 · 48 · 64 px; Seitenränder (Gutter) 16 px |
| **Radien** | 4 · 6 · 8 · 12 · 16 · 20 px, Pill 999 px – Bedienelemente 8, Karten 16–20 |
| **Schatten** | sparsam, drei Stufen (Hebung, Popover, Modal) plus Papierschatten der Seiten |
| **Farben** | neutrale Graustufen für Flächen; **Primär = Tinte** (dunkel auf hell, hell auf dunkel); Status Erfolg/Warnung/Fehler/Info je mit „soft“-Fläche und Text darauf; **Fokus Blau** (nach dem Review: ein roter Fokusrahmen sah auf Eingabefeldern wie ein Fehlerzustand aus; ein Test sichert ≥ 90° Farbtonabstand zu `--c-danger`) |
| **Akzent Rot-Weiß-Rot** | dezent: 3-px-Linie Rot/Weiß/Rot an der Oberkante, Wortmarke, Einrastlinien, aktive Navigation, aktueller Schritt im Stepper. Kein Rot für primäre Knöpfe, Fokus oder „läuft gerade“ (Verwechslung mit Fehler); ein Fehler im Stepper zeigt zusätzlich „!“ und den Screenreader-Text „fehlgeschlagen“ |
| **Hell/Dunkel** | `prefers-color-scheme` + Umschalter (`data-theme`); Dokumentseiten bleiben „Papier“ (im Dunkelmodus leicht gedimmt) |
| **Bewegung** | 120/200/320/520 ms, `ease-out`, Feder nur für Bestätigungen (Sticker, Häkchen); `prefers-reduced-motion` reduziert alle Animationen auf ≈ 0 |
| **Wortmarke** | eigenes Zeichen: Blatt mit Eselsohr, zwei rote „Zeilen“ ergeben Rot-Weiß-Rot; Schriftzug „PIDA“ gesperrt, Unterzeile „PDF · ID · Austria“. **Kein** Bundesadler, keine ID-Austria-/A-Trust-Logos; Standard-Bildmarke im Signaturblock ist ein neutrales Siegel |

## 9. Barrierefreiheit (WCAG 2.2 AA)

- **Kontraste:** alle Text-Token-Paare ≥ 4,5:1, Fokus und Ränder von Eingabefeldern/Kästchen/Schaltern ≥ 3:1
  (eigenes Token `--c-control-border`) – automatisch geprüft in Hell und Dunkel (`catalog.test.ts`).
- **Fokus:** 2 px Ring mit 2 px Abstand an allen Bedienelementen; Fokus wird bei Ansichts- und Schrittwechseln
  gezielt gesetzt (Überschrift „Signiert“, erstes Feld), Overlays geben ihn zurück.
- **Tastatur:** komplette Bedienung inkl. Sticker, Palette, Menüs; Sprunglink „Zum Inhalt springen“.
- **Semantik:** Landmarken (Kopf, Hauptbereich, Inspektor, Benachrichtigungen, Demo), Überschriftenhierarchie,
  React-Aria-Komponenten für Dialoge, Menüs, Schalter, Radiogruppen, Aufklappbereiche, Ablageflächen.
- **Ansagen:** Live-Region für die Position, Toast-Region `aria-live`, Fehler als `alert`; Feldfehler nach
  „Absenden“ über eine dauerhafte `alert`-Region am Feld (4.1.3), zusätzlich per `aria-describedby`;
  Countdown ohne Dauer-Ansage, aber mit einmaliger Vorwarnung eine Minute vor Ablauf; Toasts mit Info
  verschwinden nach 6 s, pausieren bei Fokus/Hover, Fehler bleiben (2.2.1).
- **Fokus-Rückfall:** Verschwindet der Auslöser eines Overlays (z. B. Einstellungen aus der Palette) oder die
  Demo-Steuerung wird auf-/zugeklappt, landet der Fokus gezielt im Hauptbereich bzw. im Panel/auf „Demo“.
- **Zielgrößen:** Bedienelemente ≥ 30 px, Textlinks ≥ 24 px hoch, primäre Aktionen 44–48 px (2.5.8).
- **Werkzeugleiste** der Seitenansicht als React-Aria-`Toolbar` (Pfeiltasten zwischen den Knöpfen).
- **Ziehen hat Alternativen** (2.5.7): Pfeiltasten, Doppelklick, „Neu platzieren“, „Ins Feld setzen“.
- **Tests:** axe-core ohne Verstöße für alle Hauptansichten und Overlays (inkl. schmaler Ansicht).

## 10. Responsiv

| Breite | Verhalten |
| --- | --- |
| ≥ 1100 px | drei Spalten, Kopfzeile mit Befehlsfeld „Befehle ⌘K“ |
| 760–1100 px | schmalere Spalten, Befehlsknopf ohne Text |
| < 760 px | Inspektor als Bottom-Sheet, Miniaturen als Überlagerung, Signierkarte und Dialoge als Sheet von unten, Einstellungen im Vollbild mit waagrechter Bereichsnavigation, Farbschema/Sprache in Einstellungen und Palette |
| 360 px | geprüft ohne waagrechtes Scrollen der Seite; 16 px Gutter |

## 11. Übertragung auf `apps/app`

Der Prototyp verwendet bewusst dieselbe Technik (React 19, React Aria Components, Vite) und lässt sich
**schrittweise** übernehmen. Die fachliche Logik bleibt im Rust-Kern; die Oberfläche ändert, _wie_ der
`ViewState` dargestellt wird, nicht _was_ der Kern entscheidet.

### 11.1 Zuordnung zu Kern-Ereignissen (`UiEvent`) und Ansichten (`StepView`)

| Prototyp | `apps/app` / Kern |
| --- | --- |
| Datei ablegen/auswählen, Stapel | `open_files` (mehrere Pfade → Warteschlange, F-14) |
| Sticker ziehen / Tastatur / Doppelklick | `move_block` (absolut) bzw. `nudge_block` (Schritte), Seitenwechsel `change_page` |
| „Neu platzieren“, „Freie Fläche suchen“ | `auto_place` (Kern ermittelt freie Fläche, `placement.notice.*`) |
| Überdeckungswarnung | `placement.notice.overlaps_content` / `overlaps_widget` aus dem Kern statt eigener Geometrie |
| „Unsichtbar signieren“ / „Auf neuer Seite“ | `choose_invisible` / `toggle_new_page` (bei signierten Dokumenten `new_page_not_allowed`) |
| „Hier signieren“ / „Ins Feld setzen“ | `placeholder_answer` mit Feld bzw. QR-Platzhalter (ersetzt den modalen Dialog) |
| „Signieren“ | `confirm_position` + `choose_backend` (Standard-Signiermittel aus den Einstellungen) in einem Klick |
| Datenschutzhinweis | `acknowledge_privacy` (`PrivacyNoticeView`) |
| Anmelden / SMS-TAN / FIDO2 / App-Bestätigung | `submit_form` zu `NativeForm::Login`, `SmsTan`, `Fido2`, `WaitForApp` (Vergleichswert, `timeout_secs`) |
| Abbrechen / Erneut versuchen | `cancel` / `retry` |
| Fertig: Öffnen / Teilen / Speichern | `open_result` / `share_result` / `export_result`, `save` |
| Einstellungen sofort wirksam | `save_settings` je Änderung (entprellt) statt Speichern-Knopf; `close_settings` nur noch Schließen |
| Ersteinrichtung „übernehmen“ | `accept_migration` / `decline_migration` (vorhandener Migrationsdialog) |

Die **Hauptleiste** (`MainBarView`) entfällt optisch; ihre Freigaben (welcher Schritt erreichbar ist) steuern
weiterhin, welche Aktionen der Inspektor und die Palette anbieten. Die Steps `position`, `backend` und
`signing` werden in **einer** Ansicht „Arbeitsbereich“ dargestellt; `signing` und `backend` erscheinen als
Statuskarte über dem Arbeitsbereich, `output` als „Fertig“ (bzw. Toast bei nicht leerer Warteschlange).

### 11.2 Vorgehen in Etappen

1. **Design-Tokens übernehmen:** `tokens.css` nach `apps/app/src/styles.css` (Variablen ersetzen
   Einzelwerte), Wortmarke/Favicon tauschen (Umbenennung läuft ohnehin). Geringes Risiko, sofort sichtbar.
2. **Bausteine:** `ui/controls.tsx`, `ui/Dialog.tsx`, Toasts und Icons neben die bestehenden Bausteine legen
   (gleiche React-Aria-Basis) und Bildschirm für Bildschirm verwenden.
3. **Signierkarte:** `SigningScreen` als Overlay über `PositionScreen` rendern (Stepper, Vergleichswert,
   Countdown, Alternativen) – keine Kernänderung nötig.
4. **Arbeitsbereich:** `PositionScreen` → durchgehende Seitenansicht mit Miniaturen (PDFium-Render des
   Kerns), Sticker mit Einrastlinien; Positions-Zahlenfelder entfernen; Inspektor statt `BackendScreen`
   (Konto-Chip, Standard-Signiermittel).
5. **Einstellungen:** Dialog → Seitenleiste mit Suche und sofortigem `save_settings`; Live-Vorschau nutzt die
   vorhandene Signaturblock-Vorschau des Kerns.
6. **Neu:** Stapelansicht über `QueueItemView`, Prüfmodus (braucht eine Prüf-API im Kern), Befehlspalette.
7. **Mobil:** Bottom-Sheet und Sheet-Dialoge sind bereits angelegt; auf iOS/Android ersetzen
   Share-Sheet/Dokumentauswahl die Ablagefläche.

### 11.3 Was im Prototyp vereinfacht ist

- Seiten sind HTML-Mock-Seiten; in `apps/app` liefert PDFium Bitmaps (Miniaturen in niedriger Auflösung,
  Seiten im sichtbaren Bereich nachladen).
- Freie Flächen/Überdeckung rechnet der Prototyp selbst; in der App kommen sie aus `pida_placement`.
- Der Stapel bestätigt einmal in der App; ob A-Trust je Dokument eine eigene Bestätigung verlangt
  (`max_batch: 1`, `login_once_per_batch`), muss die Statuskarte dann pro Dokument zeigen („2 von 3 bestätigen“).
- Keystore- und Bürgerkartenweg sind bewusst knapp.

## 12. Offene Fragen an den Auftraggeber

1. Signaturfeld vorhanden: **automatisch ins Feld setzen** (wie QR-Platzhalter) oder – wie jetzt –
   anbieten? Der Prototyp bietet standardmäßig an, um die Markierung erlebbar zu machen; mit Einstellungen ›
   Dokument › „Leere Signaturfelder“ setzt er automatisch ins Feld.
2. Standard-Signiermittel „Jedes Mal fragen“: Auswahl als erster Schritt der Signierkarte oder über den
   Konto-Chip? (Prototyp: Konto-Chip.)
3. Soll „Fertig“ bei Einzeldokumenten automatisch das Prüfsiegel zeigen, auch wenn die Prüfung offline
   (ohne Sperrlistenabfrage) erfolgt? Formulierung „geprüft“ ggf. präzisieren.
4. Name des Ausgabemusters: „_signiert“ (Prototyp) statt „_signed“ (PDF-Over-Standard, F-81)?
5. Rot als Akzent: passt die dezente Variante, oder ist eine neutrale (blaue) Akzentfarbe gewünscht?

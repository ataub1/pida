# 06 – Barrierefreie Positionierung und Signatur: Anforderungsanalyse

Stand: 30.10.2026 · Status: **Stufe 1 umgesetzt** (siehe „Umsetzungsstand“ am Ende, ADR 0004 Nachtrag 23) · Bezug: PDF-AS-WAI
(<https://pdf.egiz.gv.at/pdf-as-wai/>), Plan §6.7/§7.6, ADR 0004, Abnahme K-01…K-07

**Kennzeichnung:**
- **[V]** im Code bzw. auf der Seite verifiziert
- **[I]** Interpretation bzw. Schlussfolgerung
- **Muss / Soll / Kann** Priorität für eine barrierefreie PIDA-Variante

---

## 1. Ziel und Abgrenzung

Untersucht werden folgende Fragen:
- Welche Bedürfnisse deckt PDF-AS-WAI ab?
- Was davon kann PIDA heute schon?
- Was bräuchte eine PIDA-Variante, die auf barrierefreie Positionierung und Signatur ausgelegt ist?

Die Analyse betrifft die Oberfläche (Desktop, Mobile, Web) und das erzeugte Dokument. Ausgeklammert ist die Barrierefreiheit der ID-Austria-App und der A-Trust-Seiten. Dafür ist A-Trust verantwortlich.

**Kernaussage [I]:** PDF-AS-WAI löst ein Problem, das jede visuelle Signatur-App hat. Den Ort des Signaturblocks wählt man normalerweise durch Hinsehen und Ziehen. Blinde und stark sehbehinderte Menschen können das nicht. WAI ersetzt das Hinsehen durch **drei nicht-visuelle Methoden**:
- automatisch,
- per Platzhalter-ID,
- per Millimeter-Koordinaten.

Dazu kommen eine Klartext-Anleitung und ein schlichtes Formular. PIDA hat für zwei der drei Methoden schon den Kern (Auto-Platzierung, QR-Platzhalter) und eine gute Tastatur- und Live-Region-Basis. Es fehlen vor allem:
1. die **explizite, nicht-visuelle Methodenwahl**,
2. die **Millimeter-Eingabe**,
3. die **Auswahl eines Platzhalters per ID**,
4. eine **textuelle Prüfung vor dem Signieren** („wo landet die Signatur, was überdeckt sie?“).

---

## 2. Ist-Analyse PDF-AS-WAI [V]

### 2.1 Aufbau

Zwei Seiten, serverseitig gerendert (jQuery 3.5.1, Bootstrap 4.4.1, `lang="de"`), mit Header-Navigation „PDF-Signatur“ und „QR Platzhalter“.

**Seite „PDF-Signatur mit barrierefreier Positionierung“ (`/pdf-as-wai/`):**
- Einleitung in einem Satz und ein aufklappbarer Datenschutz-Hinweis (`<details>`).
- Anleitung „Unterschreiben in 3 Schritten“ als Fließtext unter h2, die Schritte als h4.
- Formular `POST /uploadPos` (multipart):

| Feld | Typ | Bemerkung |
|---|---|---|
| Zu unterschreibendes Dokument | `input type=file`, `aria-required` | – |
| Art der Positionierung | `select` | `toSelect` (Bitte wählen) / `AUTO` / `PLACEHOLDER` / `MANUELL` |
| Auf Seite | Text, `pattern=\d+$` | nur bei MANUELL sichtbar |
| Position von links [mm] | Text, `pattern=\d+$` | nur ganze Zahlen |
| Position von unten [mm] | Text, `pattern=\d+$` | – |
| Identifikator eines Platzhalters | Text, `pattern=\d+$` | nur bei PLACEHOLDER sichtbar |
| „ID Austria / Handy-Signatur“, „Signaturkarte“ | zwei Submit-Buttons, `connector=mobilebku/bku` | danach A-Trust-Seiten bzw. lokale BKU |

**Seite „QR Platzhalter“ (`/qrCodeGeneration`), für die Person, die das PDF erstellt:**
- Identifikation (Zahl, Standard `0001`, max. 10 Stellen).
- Signaturprofil: Standard, Amtssignatur, „PDF Over“ oder manuelle Profil-ID.
- Sprache des Signaturblocks: de/en.
- Ergebnis: ein PNG von `apps.egiz.gv.at/pdf-as-web/placeholder?id=…&profile=…&lang=…` als Vorschau und zum Herunterladen. Der Inhalt des QR-Codes ist `PDF-AS-POS;id=…;profile=…`.

### 2.2 Die drei Methoden

| Methode | Semantik (laut Anleitung) | PDF-AS-Abbildung [I, analyse/04 §4–5] |
|---|---|---|
| Automatisch | Block direkt unter der letzten Zeile, sonst auf einer neuen Seite | `p:auto;x:auto;y:auto` (Freiraumsuche auf der letzten Seite) |
| QR-Code-Platzhalter | Block ersetzt den QR-Code mit der eingegebenen ID; mehrere IDs für mehrere Unterzeichnende | Platzhaltersuche mit `placeholder_id` |
| Manuell | **linke obere Ecke** des Blocks, gemessen von der **linken unteren Seitenecke**, in mm; Block 80 × 29 mm | `p:<n>;x:<mm→pt>;y:<mm→pt>` |

### 2.3 Was WAI gut macht

- **Positionierung ohne Sicht.** Keine der drei Methoden braucht eine Vorschau.
- **Arbeitsteilung Ersteller:in ↔ Unterzeichner:in.** Die Ersteller:in legt den Ort fest (QR-ID oder mm-Werte) und teilt ihn mit. Die Unterzeichner:in muss nur eine Zahl eingeben. Für mehrere Unterzeichnende gibt es je eine ID.
- **Klartext-Anleitung** in einfacher Sprache, erarbeitet mit Expert:innen für Barrierefreiheit (Fußzeile: D. Klein; S. Buchner-Sabathy, D. Ossberger/wortklaviatur).
- **Native HTML-Steuerelemente.** Sie funktionieren mit jedem Screenreader und jeder Vergrößerung.

### 2.4 Schwächen und Lücken in WAI

Diese Punkte sollte PIDA besser machen.

| # | Beobachtung | Auswirkung |
|---|---|---|
| W-1 | **Keine Rückmeldung vor dem Signieren**, wo der Block landet oder ob er Text überdeckt | Blinde Nutzer:innen signieren „blind“ und können das Ergebnis nicht prüfen |
| W-2 | mm-Felder nur ganzzahlig (`\d+$`), keine Bereichsprüfung gegen die Seitengröße, kein Hinweis auf die Seitenzahl des Dokuments | Fehleingaben fallen erst serverseitig oder gar nicht auf |
| W-3 | Bezugspunkt gemischt: obere Ecke des Blocks, aber von **unten** gemessen | Fehleranfällig. Zum Kompatibilitätsproblem siehe §4, A-P4 |
| W-4 | Blockgröße fest „80 × 29 mm“ angegeben, hängt aber von Profil und Emblem ab (Amtssignatur: Breite = Seitenbreite − 2·x, analyse/04 §4.3) | Die Angabe stimmt nicht für alle Profile |
| W-5 | Bei der Platzhalter-Methode keine Liste der im Dokument gefundenen IDs | Die ID muss bekannt sein, Tippfehler bleiben unbemerkt |
| W-6 | Überschriften springen von h2 auf h4; redundante `aria-label` gleich Label-Text; Fehler nur über `setCustomValidity` | Kleinere WCAG-Mängel (1.3.1, 3.3.1) |
| W-7 | QR-Generator: Vorschau-`alt="QR Placeholder"` (englisch, ohne ID), Einblendung per `fadeToggle` ohne Statusmeldung | Das Ergebnis wird vom Screenreader nicht angekündigt (4.1.3) |
| W-8 | Nach dem Signieren nur eine Download-Seite; keine Prüfung, ob der Platzhalter gefunden wurde | Eine fehlende oder falsche ID fällt nicht auf (Rückfall von PDF-AS unklar) |
| W-9 | Die Erstellung des Platzhalters setzt voraus, dass die Ersteller:in ein PNG in ihr Dokument einfügt (Word o. ä.) | Für die Ersteller:in selbst ist das nicht barrierefrei geführt |

---

## 3. Abgleich mit PIDA (Stand 0.6.5) [V]

| Fähigkeit | WAI | PIDA heute | Lücke |
|---|---|---|---|
| Auto-Platzierung „unter dem Text“ | ✔ | ✔ PDF-AS-Semantik, Einstellung `auto_position`, Knopf „Freie Fläche suchen“; Ansage `ng-ui-ws-placed` | Nicht als bewusste **Methode** vor der Vorschau wählbar |
| QR-Platzhalter erkennen | ✔ per ID | ✔ automatisch (`find_qr_placeholders`, Modus wie PDF-Over), Ansage „Im QR-Code-Platzhalter“ | **Keine Auswahl per ID** bei mehreren Platzhaltern, keine Liste |
| Signaturfelder (AcroForm) | ✘ | ✔ erkannt, „Hier signieren“-Marker, Ansage mit Feldname | Liste und Tastaturwahl prüfen |
| Manuelle Eingabe in mm | ✔ | ✘ Im Plan §6.7 als „Zahlenfelder in mm/pt als vollwertige Alternative“ vorgesehen, **nicht umgesetzt** | Muss |
| Tastatur-Positionierung | ✘ | ✔ Pfeile 15 pt, Umschalt 1 pt, Strg/⌘ 50 pt, Bild↑/↓, Pos1/Ende | – |
| Zeiger-Alternative zum Ziehen (WCAG 2.5.7) | n/a | ✔ Doppelklick bzw. Doppeltipp platziert | – |
| Live-Region nach jeder Änderung | ✘ | ✔ „Seite x von y, … mm von links, … mm von unten“, Einrasten, Überdeckung, Feld/QR | Bezugspunkt vereinheitlichen (siehe A-P4) |
| Überdeckungswarnung | ✘ | ✔ Inhalt bzw. Formularfeld, mit Abhilfe-Knopf | Sagt nicht, **was** überdeckt wird |
| Textliche Zusammenfassung vor dem Signieren | ✘ | teilweise (Inspector) | Soll |
| QR-Platzhalter erzeugen | ✔ | ✘ | Soll/Kann |
| Mobile Screenreader (VoiceOver „anpassbar“, TalkBack-Aktionen) | n/a | ✘ (Plan §6.7, nicht umgesetzt) | Muss für Mobile |
| ID-Austria-Anmeldung | A-Trust-Webseiten | **eigene native Formulare** (React Aria) | Vorteil PIDA, Zeitlimits prüfen |
| PDF/UA des Ergebnisses | PDF-AS | ✔ `/StructParent` und `ParentTree` fortgeschrieben, `/TU` = Text des Signaturblocks | Gegen veraPDF/PAC prüfen |
| Forced Colors, reduzierte Bewegung, Kontrast | Bootstrap | ✔ CSS vorhanden, axe-core in Komponententests | Screenreader-Protokolle (K-06) ausständig |

**Zwischenfazit [I]:** PIDA ist bei Interaktion und Rückmeldung schon weiter als WAI. Es fehlen aber die **nicht-visuellen Eingabewege** (mm, ID) und die **geführte Methodenwahl**. Ohne sie bleibt PIDA im Kern eine „Hinseh-App mit guter Tastaturbedienung“.

---

## 4. Zielgruppen und Bedürfnisse [I]

| Gruppe | Typische Hilfsmittel | Bedürfnis bei Positionierung und Signatur |
|---|---|---|
| Blind | Screenreader (NVDA, JAWS, VoiceOver, TalkBack, Orca), Braillezeile | Ort ohne Vorschau festlegen (Methode, ID, mm, Textanker); **verlässliche Rückmeldung**, wo der Block steht und ob er etwas überdeckt; Seiteninhalt als Text |
| Stark sehbehindert | Vergrößerung 200–400 %, hoher Kontrast, Farbinvertierung | Reflow ohne horizontales Scrollen, gut sichtbarer Blockrahmen (Kontrast 3:1), große Vorschau-Zoomstufen, Fokus immer sichtbar |
| Motorisch eingeschränkt | Tastatur, Schaltersteuerung, Sprachsteuerung | Kein Ziehen nötig, große Ziele, keine Zeitlimits, Sprachbefehle über sichtbare Beschriftungen („Klicke Signieren“) |
| Kognitiv / wenig Erfahrung | – | Einfache Sprache, wenige Entscheidungen, sichere Voreinstellung (automatisch), klare Bestätigung |
| Ersteller:innen von Dokumenten | wie oben | Ort für andere festlegen: Platzhalter erzeugen oder mm-Werte bestimmen und weitergeben |

---

## 5. Anforderungen

### 5.1 Positionierungsmethoden (P)

| ID | Anforderung | Prio |
|---|---|---|
| P-1 | **Methodenwahl als eigener, nicht-visueller Schritt**, erreichbar ohne Vorschau: automatisch · Platzhalter/Signaturfeld · Koordinaten · (Textanker, P-5) · frei mit Vorschau. Voreinstellung „automatisch“; ein gefundener Platzhalter bzw. ein gefundenes Feld wird vorgeschlagen. Die Wahl ist eine Radiogruppe mit Beschreibungstext je Option, nicht nur ein Select. | Muss |
| P-2 | **Automatisch** wie heute (PDF-AS-Semantik). Danach eine Ansage mit Seite, Lage und Bezug zum Text, z. B. „Seite 3 von 3, unter dem letzten Absatz, rechts“, außerdem ob eine neue Seite angefügt wurde. Auf bereits signierten Dokumenten deutlich sagen, dass keine neue Seite möglich ist (Hinweis existiert: `ng-ui-position-signed-hint`). | Muss |
| P-3 | **Platzhalter per ID**: Liste aller gefundenen QR-Platzhalter und leeren Signaturfelder mit ID bzw. Feldname, Seite und Lage („Platzhalter 0002, Seite 4, unten links“). Auswahl per Tastatur. Zusätzlich direkte ID-Eingabe wie in WAI, mit Prüfung „ID nicht im Dokument“ **vor** dem Signieren. Kein stilles Zurückfallen auf einen anderen Platzhalter. | Muss |
| P-4 | **Koordinaten-Eingabe**: Seite, Abstand von links, Abstand von unten in mm, mit Dezimalstellen und Gebietsschema-Format. Bezugspunkt **wie PDF-AS-WAI** (linke obere Blockecke, gemessen von der linken unteren Seitenecke), damit Werte, die Ersteller:innen für WAI angeben, in PIDA dasselbe Ergebnis liefern. Bereichsprüfung gegen die Seitengröße (inkl. CropBox und Rotation, analyse/04 §4.6). Die aktuelle Blockgröße wird für das gewählte Profil **berechnet angezeigt** statt fest „80 × 29 mm“. Fehlertexte am Feld (`role=alert`, `aria-invalid`, `aria-describedby`). Die Felder sind mit der Vorschau synchronisiert: Tastatur bzw. Ziehen aktualisiert die Felder und umgekehrt. | Muss |
| P-5 | **Textanker** (über WAI hinaus): „Signatur unter bzw. rechts neben dem Text …“, z. B. nach „Unterschrift“ oder „Ort, Datum“ suchen. Treffer werden als Liste mit Seite und Kontext angeboten. Braucht Textextraktion mit Positionen aus PDFium (`FPDFText_*`; heute in `crates/render` nicht angebunden). | Soll |
| P-6 | **Freie Flächen als Liste**: Die vorhandenen `PageLayout.free`-Rechtecke werden als „Freie Fläche 1 von 4: Seite 2, rechts unten, 60 × 40 mm“ wählbar, ebenso die Einrastlinien als Sprungziele. Heute nur intern bzw. über „Freie Fläche suchen“. | Soll |
| P-7 | Methode und Werte bleiben je Dokument erhalten, wenn man zurückspringt (wie im Original für die manuelle Position, analyse/04 §4.7). Optional „letzte Methode merken“ in den Einstellungen. | Soll |
| P-8 | Stapelsignatur: Die Methode gilt für alle Dokumente. Pro Dokument wird gemeldet, ob die ID bzw. der Anker gefunden wurde (`ng-ui-batch-detail-*` erweitern). | Soll |
| P-9 | Übergabe von außen: Methode und Werte per Startargument bzw. Deep Link, z. B. `--position "p:2;x:20mm;y:40mm"` oder `--placeholder 0002`. So können Behörden-Workflows die Lage vorgeben. | Kann |

### 5.2 Rückmeldung und Prüfung (R)

| ID | Anforderung | Prio |
|---|---|---|
| R-1 | **Zusammenfassung vor dem Signieren** als Text, im Fokus vor dem Knopf „Signieren“. Inhalt: Methode, Seite, Lage in mm und in Worten, Blockgröße, Überdeckung ja/nein, Profil und Sprache. Muss ohne Vorschau verständlich sein. | Muss |
| R-2 | **Überdeckung konkret benennen**: welcher Text überdeckt wird (Auszug aus P-5-Textextraktion, z. B. „überdeckt ‚Ich bestätige, dass…‘“) bzw. welches Formularfeld (Name). Heute nur „überdeckt Inhalt“. | Soll |
| R-3 | **Seite als Text lesen**: Für die aktuelle Seite ist der Text in Lesereihenfolge abrufbar (eigener Bereich bzw. Dialog), damit die Wahl von Anker oder Platzhalter begründet erfolgen kann. Die Seitenvorschau bekommt einen sinnvollen Alternativtext (heute: „Seite x von y – Name“). | Soll |
| R-4 | Nach dem Signieren: Ansage mit Ort und Ergebnis („Signiert, Signatur auf Seite 3, unten rechts, gespeichert als …“). Die Datei-Aktionen sind per Tastatur erreichbar. | Muss |
| R-5 | Die Live-Region ist **gedrosselt** (keine Ansage pro Pfeiltastendruck bei gedrückter Taste, sondern nach kurzer Ruhe). Ausführlichkeit einstellbar: knapp (nur mm) oder ausführlich (mit Bezug zum Text). | Soll |
| R-6 | Einheitlicher Bezugspunkt in **allen** Ansagen. Heute nennt `ng-ui-position-announce` die Oberkante, `ng-ui-ws-announce` „mm von unten“ ohne Kante. Festlegen: linke obere Ecke (wie WAI und P-4). | Muss |

### 5.3 Platzhalter für Ersteller:innen (E)

| ID | Anforderung | Prio |
|---|---|---|
| E-1 | **QR-Platzhalter erzeugen** in PIDA, offline aus dem Kern: ID, Profil, Sprache → PNG/SVG zum Speichern bzw. Kopieren, Inhalt `PDF-AS-POS;id=…;profile=…`. Mit Ansage des Ergebnisses und Alternativtext mit ID. Kompatibel mit PDF-AS-WAI und PDF-Over. | Soll |
| E-2 | **Platzhalter direkt ins PDF setzen**: Die Ersteller:in wählt in PIDA per Methode P-4/P-5/P-6 einen Ort und speichert ein PDF mit **leerem Signaturfeld** (AcroForm, Name bzw. TU „Unterschrift Antragsteller:in“) oder mit eingebettetem QR-Code. Signaturfelder sind semantisch, für Screenreader in PDF-Readern ansagbar und brauchen keine Bilderkennung. Achtung: erzeugt eine neue Revision bzw. ein verändertes Dokument. | Kann |
| E-3 | Anleitung „Dokument für andere vorbereiten“ in der Offline-Hilfe (Platzhalter, mm-Werte weitergeben, mehrere Unterzeichnende) in einfacher Sprache. | Soll |

### 5.4 Bedienung allgemein (B), WCAG 2.2 AA / EN 301 549 Kap. 9–11

| ID | Anforderung | WCAG | Prio |
|---|---|---|---|
| B-1 | Der gesamte Ablauf (öffnen → Methode → signieren → speichern) geht **nur mit Tastatur** und **nur mit Screenreader** und ist auf allen Plattformen protokolliert (Abnahme K-06: NVDA, JAWS optional, VoiceOver macOS/iOS, TalkBack, Orca) | 2.1.1, 4.1.2 | Muss |
| B-2 | Mobile: der Signaturblock als **anpassbares Element** (VoiceOver `adjustable`, Wischen hoch/runter = Schritt) bzw. TalkBack-Aktionen „nach links/rechts/oben/unten, nächste Seite“. Heute nur Ziehen bzw. Doppeltipp. | 2.5.7, 4.1.2 | Muss (Mobile) |
| B-3 | Zeiger-Alternative zum Ziehen bleibt (Doppelklick bzw. Doppeltipp, mm-Felder) | 2.5.7 | Muss ✔ |
| B-4 | Zielgröße mind. 24 × 24 CSS-px (Plan: 44 pt / 48 dp) | 2.5.8 | Muss ✔ prüfen |
| B-5 | Reflow bei 320 CSS-px bzw. 400 % ohne zweidimensionales Scrollen außerhalb der Seitenvorschau; Methodenwahl und Felder vollständig nutzbar | 1.4.10 | Muss |
| B-6 | Blockrahmen, Fokus und Einrastlinien mit Kontrast ≥ 3:1 zur Seite, auch in `forced-colors` | 1.4.11, 2.4.7, 2.4.13 | Muss |
| B-7 | Fokus wird nicht verdeckt, auch nicht durch die mobile Fußleiste oder Tastatur (vgl. Fix 0.6.5) | 2.4.11 | Muss |
| B-8 | Keine Zeitlimits in PIDA. Zeitlimits von A-Trust (Sitzung, Wartezeit auf die Bestätigung in der App) werden vorher angekündigt, Restzeit bei Bedarf angesagt, Wiederholen ohne Datenverlust | 2.2.1 | Muss |
| B-9 | Fehlermeldungen benennen Feld und Abhilfe (Basis: „Verständliche Fehlermeldungen“, „Was bedeutet das?“) | 3.3.1, 3.3.3 | Muss ✔ ausbauen |
| B-10 | Sprachsteuerung: sichtbare Beschriftung ist im zugänglichen Namen enthalten (WAI-Mangel W-6 vermeiden) | 2.5.3 | Muss |
| B-11 | Einfache Sprache für Methodenwahl und Anleitung, mit Fachleuten erarbeitet (wie bei WAI) | – (3.1.5 AAA) | Soll |
| B-12 | Überschriftenhierarchie ohne Sprünge, Landmarken, `lang` je Sprache | 1.3.1, 2.4.6, 3.1.1 | Muss |

### 5.5 Signaturvorgang (S)

| ID | Anforderung | Prio |
|---|---|---|
| S-1 | Native ID-Austria-Formulare (Nummer bzw. Benutzername, Passwort mit Augen-Symbol, Push-Wartezustand) vollständig ansagbar; Wartezustand als `role=status` mit Hinweis „Bestätigen Sie in der App ID Austria“ | Muss |
| S-2 | FIDO2/Passkey-Weg (`fido2_default`) als oft besser zugänglicher Faktor ankündigen; Plattform-Dialoge sind betriebssystemseitig barrierefrei | Soll |
| S-3 | Chipkarte (lokale BKU): Hinweis, dass die Chipkarten-Software eigene Dialoge zeigt, deren Barrierefreiheit PIDA nicht beeinflusst | Soll |
| S-4 | Abhängigkeit dokumentieren: Barrierefreiheit der ID-Austria-App bzw. A-Trust liegt außerhalb von PIDA (Erklärung zur Barrierefreiheit verweist darauf) | Muss |

### 5.6 Ergebnisdokument (D)

| ID | Anforderung | Prio |
|---|---|---|
| D-1 | Die Signatur macht ein getaggtes bzw. PDF/UA-Dokument nicht unzugänglich: Struktur fortschreiben (✔ `/StructParent`, `ParentTree`). Abnahme mit veraPDF PDF/UA-1 bzw. PDF/UA-2 und PAC vor und nach dem Signieren. | Muss |
| D-2 | Das Signatur-Widget hat einen aussagekräftigen Alternativtext (`/TU` = Blocktext ✔). Zusätzlich prüfen: `Form`-Strukturelement, Bildmarke als Artefakt bzw. mit Alt-Text, Lesereihenfolge. | Muss |
| D-3 | Der Text des Signaturblocks bleibt als echter Text extrahierbar (keine reine Rastergrafik), auch beim Signaturbild bzw. Emblem | Muss ✔ prüfen |
| D-4 | Unsichtbare Signatur (`INVISIBLE`) als bewusste Option für Nutzer:innen, die keine visuelle Platzierung wollen. Mit Hinweis, dass Empfänger:innen dann kein Signaturbild sehen. | Soll ✔ vorhanden |

### 5.7 Qualitätssicherung und Nachweis (Q)

| ID | Anforderung | Prio |
|---|---|---|
| Q-1 | axe-core in CI für alle neuen Ansichten (Methodenwahl, mm-Felder, Platzhalterliste, Zusammenfassung) | Muss |
| Q-2 | Screenreader-Protokolle je Release (Abnahme K-06, bisher ausständig) | Muss |
| Q-3 | **Tests mit Betroffenen**: mind. 5 Personen (blind, sehbehindert, motorisch), idealerweise mit den WAI-Expert:innen | Muss für „barrierefreie Variante“ |
| Q-4 | Externes Audit (WCAG 2.2 AA / EN 301 549) und **Erklärung zur Barrierefreiheit** je Plattform; rechtliche Anwendbarkeit klären (WZG / RL 2016/2102 bei Bereitstellung durch öffentliche Stellen, BaFG/EAA) | Soll |
| Q-5 | Golden-Tests: Gleiche WAI-Eingaben (Seite, x, y, ID) ergeben in PIDA dieselbe Blockposition wie PDF-AS (`golden_pdfas442`-Muster) | Muss |

---

## 6. Varianten: Wie könnte eine „barrierefreie PIDA“ aussehen?

| Variante | Beschreibung | Für | Gegen |
|---|---|---|---|
| **A – Inklusiv im Produkt** (Empfehlung) | Methodenwahl (P-1) und alle Anforderungen in **jeder** PIDA-Variante; „Positionierung ohne Vorschau“ ist ein gleichwertiger Weg neben dem Ziehen | Eine Codebasis, keine Sonderversion, alle profitieren (Stapel, Behörden-Workflows, Mobile); entspricht WCAG-Gedanken | Etwas mehr Oberfläche im Hauptablauf |
| B – Barrierefreier Modus | Schalter bzw. Ersteinrichtung „Ich nutze einen Screenreader“ → Ablauf startet mit Methodenwahl statt Vorschau, ausführliche Ansagen | Wenig Ablenkung für die Zielgruppe | Modus muss gefunden werden; Gefahr, dass er veraltet |
| C – Eigene Web-Seite „PIDA WAI“ | Schlichtes Formular wie WAI auf der PIDA-Web-Variante, mit Rust-Kern | Schnell, sehr robust mit allen Screenreadern | Doppelte Oberfläche; die WAI-Lücken W-1/W-5 bleiben, solange es keine Zwischenseite zur Prüfung gibt |

**Empfehlung [I]:** Variante **A**, ergänzt um einen Einstieg aus **B**. Die Methodenwahl erscheint immer, wenn kein Platzhalter bzw. Feld gefunden wurde oder wenn Bildschirmleser bzw. Einstellung das nahelegen. Eine separate App würde Pflege und Abnahme verdoppeln. WAI braucht nur deshalb eine eigene Seite, weil die Ziehen-Oberfläche von pdf-as-web nicht zugänglich ist. Diesen Grund hat PIDA nicht.

---

## 7. Grobe Aufwandsschätzung [I]

| Paket | Inhalt | Umfang |
|---|---|---|
| 1 | P-1, P-2, P-4, R-1, R-4, R-6 (Methodenwahl, mm-Eingabe, Zusammenfassung, einheitliche Ansagen) | M |
| 2 | P-3, P-8 (Platzhalter- und Feldliste mit ID, Prüfung vor dem Signieren, Stapel) | S–M |
| 3 | B-2 (VoiceOver adjustable / TalkBack-Aktionen in den Mobile-Plugins) | M |
| 4 | P-5, R-2, R-3 (PDFium-Textextraktion, Textanker, Seitentext) | M–L |
| 5 | E-1 (QR-Generator im Kern, QR-Encoder ergänzen) / E-2 (leeres Signaturfeld schreiben) | S / M |
| 6 | Q-2…Q-5, D-1 (Protokolle, Nutzertests, Audit, veraPDF/PAC) | M + extern |

Reihenfolge: 1 → 2 → 6 (erste Tests mit Betroffenen) → 3 → 4 → 5.

---

## 8. Offene Fragen und Entscheidungen

1. **Bezugspunkt:** WAI-kompatibel festlegen (linke obere Ecke, von unten in mm) oder intuitiver (linke untere Ecke)? Empfehlung: WAI-kompatibel, wegen weitergegebener Werte.
2. **Zielgruppe und Partner:** Zusammenarbeit mit den Expert:innen hinter WAI (Buchner-Sabathy, Ossberger) für einfache Sprache und Tests?
3. **QR-Generator vs. leeres Signaturfeld** (E-1/E-2): Welcher Weg soll für Ersteller:innen der empfohlene sein?
4. **Textanker** (P-5): gewünscht? Er geht über WAI hinaus und braucht Textextraktion.
5. **Rechtsrahmen:** Muss PIDA als A-SIT-Angebot eine Erklärung zur Barrierefreiheit haben? Für welche Plattformen?
6. **Verhalten bei falscher ID:** Abbrechen mit Hinweis (Empfehlung) oder auf automatisch zurückfallen?
7. **PDF-AS-WAI selbst:** Soll PIDA-Web WAI langfristig ersetzen oder parallel bleiben?

---

## Anhang: Quellen

- PDF-AS-WAI: `/pdf-as-wai/`, `/pdf-as-wai/qrCodeGeneration`, `js/pdf-as.js`, `js/qr-code.js` (abgerufen 30.10.2026)
- PIDA:
  - `crates/placement` (`placeholder.rs`, `layout.rs`, `coords.rs::KeyStep`)
  - `apps/app/src/features/workspace/Pages.tsx` (Tastatur, Doppelklick, `role=application`)
  - `Workspace.tsx` (Live-Region)
  - `crates/pades/src/build.rs` (`/TU`)
  - `crates/pades/src/check.rs` (StructTreeRoot)
  - Plan §6.7/§7.6, `docs/integration/app-abnahme.md` §2.2
- Platzhalter- und Positionssemantik: [`04-signaturblock-layout-positionierung.md`](04-signaturblock-layout-positionierung.md) §4–5

---

## Umsetzungsstand (30.10.2026)

**Stufe 1 – umgesetzt** (ADR 0004 Nachtrag 23):
- Einstellungen › Barrierefreiheit und Schalter „Barrierefreie Bedienung“ in der Kopfzeile, Direktlink
  `?barrierefrei=1` (Variante A mit Einstieg aus B, §6).
- P-1 Methodenwahl, P-2 automatisch, P-3 Platzhalter mit Liste und Nummer (nur der vom Kern gefundene
  QR-Platzhalter, siehe unten), P-4 Millimeter (WAI-Bezugspunkt), P-6 freie Flächen als Liste.
- R-1 „Vor dem Signieren prüfen“, R-5 ausführliche bzw. verzögerte Ansagen, R-6 einheitlicher Bezugspunkt.
- B-1/B-3/B-12: Tastatur, Zeiger-Alternative, Struktur (axe-geprüft in den Komponententests).
- Schrittweite der Pfeiltasten einstellbar.

**Stufe 2 – offen:**
- P-5 Textanker, R-2 überdeckten Text benennen, R-3 Seitentext (PDFium-Textextraktion).
- P-3 vollständig: alle QR-Platzhalter mit Nummer zur Auswahl. Heute sucht der Kern nach PDF-Over-Regel und
  meldet nur einen. Dafür braucht es eine Kernänderung an Suche, Zustand und Fortsetzen.
- B-2 VoiceOver „anpassbar“ bzw. TalkBack-Aktionen (Mobile-Plugins).
- P-8 Stapel, P-9 Übergabe von außen.
- E-1/E-2 Platzhalter erzeugen.
- Q-2…Q-5: Screenreader-Protokolle, Tests mit Betroffenen, Audit.

# 04 – Signaturblock: Profile, Layout, Positionierung, Platzhalter, Vorschau, Emblem

> Analyse-Grundlage für einen plattformunabhängigen Klon (Desktop + iOS/Android) von PDF-Over.
> Referenzstand: PDF-Over `main`, Commit `c07c5ec` (2026-09-03), Version 4.4.10-SNAPSHOT; PDF-AS 4.4.2 (PDFBox-2-Backend);
> PDFBox-Version zur Laufzeit von PDF-Over: 2.0.34 (`PDF-Over/pom.xml:115`), PDF-AS baut gegen 2.0.32 (`pdf-as-pdfbox-2/build.gradle:36`).
> Ziel: Ein Nachbau soll **optisch identische** Signaturblöcke an **identischen Positionen** erzeugen und die Vorschau/Positionierung
> von PDF-Over nachbilden. Dieses Dokument ist die Spezifikation dafür.

## 0. Konventionen

**Pfad-Abkürzungen**

| Kürzel | steht für |
|---|---|
| `gui/` | `PDF-Over/pdf-over-gui/src/main/java/at/asit/pdfover/gui/` |
| `res/` | `PDF-Over/pdf-over-gui/src/main/resources/` |
| `commons/` | `PDF-Over/pdf-over-commons/src/main/java/at/asit/pdfover/commons/` |
| `signer/` | `PDF-Over/pdf-over-signer/src/main/java/at/asit/pdfover/signer/` |
| `pb2/` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/gv/egiz/pdfas/lib/impl/` |
| `pb2k/` | `pdf-as-4/pdf-as-pdfbox-2/src/main/java/at/knowcenter/wag/egov/egiz/pdfbox2/` |
| `lib/` | `pdf-as-4/pdf-as-lib/src/main/java/at/gv/egiz/pdfas/lib/` |
| `libk/` | `pdf-as-4/pdf-as-lib/src/main/java/at/knowcenter/wag/egov/egiz/` |
| `com/` | `pdf-as-4/pdf-as-common/src/main/java/at/gv/egiz/pdfas/common/` |
| `web/` | `pdf-as-4/pdf-as-web/src/main/java/at/gv/egiz/pdfas/web/` |
| `cfg/` | von PDF-Over mitgelieferte PDF-AS-Konfiguration (`PDFASConfig.zip` → `~/.pdf-over/cfg/`), hier `ref/cfgzip/cfg/` |
| `cfgzip/` | Wurzel des Zip-Inhalts (`cfg/`, `fonts/`, `images/`) = PDF-AS-Arbeitsverzeichnis `~/.pdf-over/` |

**Kennzeichnung der Aussagen**

- **[V]** im Repo-Code verifiziert (Fundstelle angegeben).
- **[P]** im PDFBox-2.0.34-Bytecode verifiziert (`javap -c` auf das gecachte JAR, keine Ausführung). Betrifft PDFBox-Interna, die nicht im Repo liegen.
- **[B]** berechnet mit einem eigenen Referenzmodell (Python-Port der unten spezifizierten Algorithmen inkl. float32-Emulation, gelesen wurden nur Profil-, AFM- und TTF-Daten; PDF-AS selbst wurde **nicht** ausgeführt). Hilfsskript: `scratchpad/tools/sigblock_model.py` (temporär, nicht normativ).
- **[I]** Interpretation/Vermutung (aus Code abgeleitet, aber nicht laufzeit-verifiziert, oder abhängig von Java2D/PDF-Viewer-Verhalten).

Einheiten: Alle Maße in **PDF-User-Space-Einheiten (pt, 1/72 Zoll)**, außer explizit „px“. `PDF_UNITS_PER_MM = 595/210 ≈ 2.8333` [V] `commons/Constants.java:99`.

Keine Geheimnisse/personenbezogenen Daten übernommen. Im Repo liegt ein Beispielzertifikat `res/example.cer` bzw. `PDF-Over/pdf-over-signer/src/main/resources/example.cer` (für die Vorschau, s. §5); dessen Inhalt ist hier nicht dokumentiert. Beispielwerte („Max Mustermann“) sind fiktiv.

---

## 1. Überblick: Wer macht was?

| Schritt | Komponente | Fundstelle |
|---|---|---|
| Profil-ID aus GUI-Einstellungen ableiten (Sprache, Notiz, PDF/A) | PDF-Over `PdfAs4SignatureParameter.getPdfAsSignatureProfileId()` | [V] `signer/pdfas/PdfAs4SignatureParameter.java:156-193` |
| Laufzeit-Overrides (Emblem, Notiz, Logo-only-Größe) in eine **Request-Kopie** der PDF-AS-Konfiguration schreiben | `PdfAs4Signer.prepare()` / `getPlaceholder()` | [V] `signer/pdfas/PdfAs4Signer.java:60-86`, `signer/pdfas/PdfAs4SignatureParameter.java:93-116`; Kopie: [V] `lib/impl/PdfAsImpl.java:312-314`, `lib/impl/configuration/ConfigurationImpl.java:48-59` |
| Positions-String erzeugen (`p:..;x:..;y:..` oder auto) | `getPdfAsSignaturePosition()` | [V] `signer/pdfas/PdfAs4SignatureParameter.java:133-149` |
| Profil → abstraktes Tabellenmodell (`Table`/`Entry`/`Style`) | PDF-AS `TableFactory.createSigTable()` | [V] `lib/impl/stamping/TableFactory.java:107-272` |
| Position + Breite bestimmen, Tabelle mit fixer Breite layouten | `Positioning.determineTablePositioning()` → `PdfBoxVisualObject.fixWidth()` → `PDFBoxTable` | [V] `pb2/pdfbox2/positioning/Positioning.java:95-215`, `pb2/stamping/pdfbox2/PdfBoxVisualObject.java:63-71` |
| Erscheinungsbild (Form-XObjects, Widget-Rect) bauen, zeichnen | `PDFAsVisualSignatureProperties/Designer/Builder`, `TableDrawUtils` | [V] `pb2/stamping/pdfbox2/*` |
| Einfügen in das Dokument (PDFBox `addSignature`) | `PADESPDFBOXSigner.signPDF()` | [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:117-387` |
| Vorschau-Bild des Blocks (288 dpi) | `PADESPDFBOXSigner.generateVisibleSignaturePreview()` | [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:752-848` |
| Positionierungs-UI | `PositioningComposite` + `SignaturePanel` | [V] `gui/composites/PositioningComposite.java`, `gui/composites/SignaturePanel.java` |

Konsequenz für den Nachbau: Der gesamte sichtbare Block ist **reine Layout-Logik + PDF-Erzeugung**, die vollständig in der Zielsprache nachgebaut werden muss (keine Netzwerk-/Kryptoabhängigkeit). Sie muss byte-unabhängig, aber **maß-identisch** sein.

---

## 2. Profil-Sprache

### 2.1 Laden, Include, Vererbung

1. **Einstieg** ist `<workdir>/cfg/config.properties` (`workdir` = `~/.pdf-over`) [V] `lib/settings/Settings.java:307-313`, `signer/pdfas/PdfAs4Helper.java:64-66`.
2. Jede Datei wird mit `java.util.Properties.load(InputStream)` gelesen (⇒ **ISO-8859-1**, Escapes `\uXXXX`, `\n`, `\"` etc.) und per `putAll` in **eine** flache Map gemerged (spätere Dateien überschreiben gleiche Schlüssel) [V] `lib/settings/Settings.java:65-83`.
3. **Include:** Alle Schlüssel mit Präfix `include` (z. B. `include.01`, `include.advancedconfig`) werden als Dateimuster relativ zu `<workdir>/cfg` interpretiert; Wildcards über `WildcardFileFilter`, nicht rekursiv; jede gefundene Datei wird rekursiv geladen [V] `lib/settings/Settings.java:76-77, 85-108`. PDF-Over-Konfiguration: `include.advancedconfig = advancedconfig.properties`, `include.01 = profiles/*.properties` [V] `cfg/config.properties:53,73`.
   - [I] Die Kommentar-Behauptung „Include-Keys werden lexikografisch sortiert“ (`cfg/config.properties:66-67`) stimmt mit dem Code nicht überein: die Include-Werte kommen aus einer `HashMap` (`Collectors.toMap`) [V] `lib/settings/Settings.java:346-351`; Reihenfolge der Dateien im Verzeichnis-Listing ist ebenfalls nicht sortiert. Für die mitgelieferten Profile irrelevant (keine Schlüsselkollisionen zwischen Dateien).
4. **Aktive Profile** = alle `sig_obj.types.<P>=on` [V] `lib/settings/Settings.java:199-206`. (`off`, `verify_only`, `sign_only` nehmen an der Vererbungsauflösung nicht teil.)
5. **Vererbung (`parent`)**: `sig_obj.<P>.parent=<Q>`. Auflösung iterativ, bis alle Eltern initialisiert sind; dann werden **alle Schlüssel** `sig_obj.<Q>.*` nach `sig_obj.<P>.*` **kopiert, sofern das Kind den Schlüssel nicht selbst hat** (Schlüssel-für-Schlüssel, nicht tabellen- oder stilweise!) [V] `lib/settings/Settings.java:166-194, 226-293`, `com/settings/Profiles.java:27-43`.
   - Folge: Ein Kind, das nur `table.main.Style.font` setzt, erbt alle anderen `table.main.Style.*`, alle Zeilen `table.main.<n>` usw.
   - Mehrstufig (BASE_NORMAL → BASE_SMALL → BASE_SMALL_NOTE → SIGNATURBLOCK_SMALL_DE_NOTE) funktioniert, weil Eltern zuerst vollständig aufgelöst werden.
   - Zyklen/fehlende Eltern: Abbruch mit Log-Fehler, betroffene Profile bleiben unaufgelöst [V] `lib/settings/Settings.java:281-288`.
6. **`augments`** (`sig_obj.<P>.augments.*=<Q>`): wie `parent`, aber mehrere Quellen; Augments haben **Vorrang** vor dem Parent (werden zuerst kopiert) [V] `lib/settings/Settings.java:137-164, 262-275`. Von PDF-Over-Profilen nicht genutzt.
7. **Request-Overrides**: `PdfAs.getConfiguration()` liefert pro Aufruf eine neue `ConfigurationImpl` mit eigener Override-Map über den (geteilten) Settings [V] `lib/impl/PdfAsImpl.java:312-314`, `lib/impl/configuration/ConfigurationImpl.java:39-59`. PDF-Over schreibt dort `sig_obj.<ID>.value.SIG_LABEL`, `...value.SIG_NOTE`, bei BASE_LOGO zusätzlich `...table.main.Style.padding`, `...pos`, `...table.main.Style.imagescaletofit` (§2.8). Overrides wirken **nach** der Vererbungsauflösung, d. h. nur auf genau die adressierte Profil-ID (keine Weitervererbung). Overrides sind **nicht persistent** (pro Request).

### 2.2 Schlüsselreferenz (alles unter `sig_obj.<P>.` sofern nicht anders angegeben)

| Schlüssel | Bedeutung / Auswertung | Fundstelle |
|---|---|---|
| `sig_obj.types.<P>` | `on` aktiviert (nur dann Vererbung) | [V] `lib/settings/Settings.java:199-206` |
| `parent`, `augments.*` | Vererbung (§2.1) | [V] `com/settings/Profiles.java:12-43` |
| `description` | Text, nicht im Block | – |
| `key.<K>` | **Caption** (Bezeichner) für Feld `K`. Fehlt sie, Default aus `DefaultSignatureProfileSettings` | [V] `com/settings/SignatureProfileSettings.java:71-96`, `com/settings/DefaultSignatureProfileSettings.java:33-82` |
| `value.<K>` | **Wert** für Feld `K` (Text mit `${…}`-Ausdrücken, bei `SIG_DATE` ein Datumsformat, bei Bildern Pfad/Base64) | [V] `com/settings/SignatureProfileSettings.java:98-119, 166-183` |
| `key.SIG_PDFA1B_VALID`, `key.SIG_PDFA_VALID`, `key.SIG_PDFUA_FORCE`, `key.latin1_encoding` | **Schalter, die als Caption abgelegt werden** (Wert `true`) | [V] `com/settings/SignatureProfileSettings.java:205-254` |
| `pos` | Default-Positions-String (§4.1) | [V] `lib/impl/configuration/SignatureProfileConfiguration.java:82-85` |
| `table.<T>.<n>` | Zeile `n` (1-basiert, **lückenlos**) der Tabelle `T` | [V] `lib/impl/stamping/TableFactory.java:157-268`, `libk/table/Table.java:198-207` |
| `table.<T>.ColsWidth` | relative Spaltenbreiten, Leerzeichen-getrennt (`1 4.5`) | [V] `lib/impl/stamping/TableFactory.java:139-148` |
| `table.<T>.Style.<attr>` | Stil der Tabelle (§2.4) | [V] `lib/impl/stamping/TableFactory.java:149-154, 348-362` |
| `isvisible` | `false` ⇒ unsichtbar (zusätzlich muss `table.main` existieren, sonst ebenfalls unsichtbar) | [V] `lib/impl/configuration/SignatureProfileConfiguration.java:64-80` |
| `enable_placeholder_search`, `placeholder_id` | Platzhaltersuche pro Profil (§5) | [V] `lib/impl/configuration/PlaceholderConfiguration.java:54-80` |
| `minWidth` | Mindestbreite bei Platzhalter-Positionierung (Default `Float.MIN_VALUE`) | [V] `lib/impl/configuration/SignatureProfileConfiguration.java:45-62`, `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:603-611` |
| `adobeSignReasonValue` | `/Reason` des Signaturdictionaries (Fallback „PAdES Signature“) | [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:192-194` |
| `timezone` | Zeitzone für `SIG_DATE` (fehlt ⇒ JVM-Default-Zeitzone) | [V] `lib/impl/stamping/ValueResolver.java:88-93` |
| `signatureSize` | reservierte Signaturgröße (Default 0x1000) | [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:582-597` |
| `SIGNED_NEWPAGE_FORCE` | neue Seite auch auf bereits signierten Dokumenten erlauben | [V] `pb2/pdfbox2/positioning/Positioning.java:235-239` |
| `adobeSignFieldValue`, `verifyURL`, `sigLogoAltText` | werden im PDFBox-2-Pfad **nicht ausgewertet** (nur gelesen/ungenutzt) | [V] (Suche ohne Treffer außer Getter `com/settings/SignatureProfileSettings.java:193-195`) |
| global `default.<key>` | Fallback, wenn Profil keinen Wert hat (`getValue` → `default.<key>`) | [V] `com/settings/SignatureProfileSettings.java:155-183` |
| global `debug.table=true` | zeichnet farbige Debug-Linien in den Block | [V] `pb2/stamping/pdfbox2/TableDrawUtils.java:52, 621-681` |
| global `sigblock.placement.bgcolor.detection.enabled`, `sigblock.placement.debug.file`, `sigblock.placement.less.space.failing` | Auto-Positionierung (§4.4) | [V] `lib/api/IConfigurationConstants.java:120-122` |
| global `signature_field_name` | Name eines vorhandenen leeren Signaturfelds + Basisname neuer Felder (§5.6) | [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:572-580, 642-644` |

### 2.3 Tabellen-Definition

Zeilensyntax: `ELEM ( '|' ELEM )*`, `ELEM = KEY '-' TYP` (Split an `-`; mindestens 2 Teile, sonst wird die **ganze Tabelle verworfen** → `null`) [V] `lib/impl/stamping/TableFactory.java:160-169`.

| `KEY-TYP` | erzeugte Zelle(n) | Default-Zellstil | Fundstelle |
|---|---|---|---|
| `TABLE-<name>` | eine Zelle mit verschachtelter Tabelle `table.<name>` (gleiches Profil) | keiner (`null`) | [V] `TableFactory.java:172-181` |
| `<K>-i` | Bildzelle, Wert = `value.<K>`; fehlt der Wert: Textzelle „IMG MISSING“ | Bild: padding 3, halign center, valign middle | [V] `TableFactory.java:182-196, 83-85` |
| `<K>-v` | Wertzelle (`value.<K>`, aufgelöst) | halign left, valign middle | [V] `TableFactory.java:197-210, 90-91` |
| `<K>-c` | Caption-Zelle (`key.<K>`, **ebenfalls durch den Resolver**) | halign center, valign middle | [V] `TableFactory.java:211-224, 87-88` |
| `<K>-cv` / `-vc` / `-req` | zwei Zellen: Caption (roh, **nicht** aufgelöst) + Wert (aufgelöst) | wie oben | [V] `TableFactory.java:226-265` |

- Zeilen werden unter dem Schlüssel `n` gespeichert und als `1..rows.size()` gelesen ⇒ Nummern müssen lückenlos ab 1 sein, sonst `null`-Zeile [V] `libk/table/Table.java:198-207, 219-223`.
- **Normalisierung**: `maxCols` = größte Summe der Colspans einer Zeile; kürzere Zeilen: **letzte Zelle** bekommt Colspan += Fehlbetrag [V] `libk/table/Table.java:225-289`.
- `noWrap` wird bei Captions gesetzt, aber vom PDFBox-2-Renderer **nie gelesen** ⇒ Captions brechen um [V] `TableFactory.java:238,253` (keine Verwendung von `isNoWrap` in `pdf-as-pdfbox-2`).
- [I] Colspan > 1 in einer **nicht letzten** Zelle wird im Layout/Zeichnen fehlerhaft behandelt (Eintragsindex wird als Spaltenindex benutzt) [V] `pb2/stamping/pdfbox2/PDFBoxTable.java:225-253`, `TableDrawUtils.java:90-155`. Für PDF-Over-Profile irrelevant.

### 2.4 Style-Attribute und Vererbung

`table.<T>.Style.<attr>` [V] `libk/table/Style.java:275-335`:

| attr | Werte | Hinweis |
|---|---|---|
| `bgcolor` | `R G B` (0–255, Leerzeichen) | sonst ignoriert; `null` ⇒ **kein** Hintergrund (transparent) |
| `padding` | float | Default 1 |
| `border` | float (Linienbreite) | Default 1; `0` ⇒ keine Linien |
| `halign` | left/center/right | für **Captions** |
| `valign` | top/middle/bottom | für **Captions** |
| `valuehalign` | left/center/right/**lineCenter** | für Werte |
| `valuevalign` | top/middle/bottom | für Werte |
| `imagehalign`, `imagevalign` | left/center/right, top/middle/bottom | für Bilder |
| `font`, `valuefont` | `FACE,SIZE,STYLE` bzw. `TTF:<datei>,SIZE[,…]` | §3.6 |
| `imagescaletofit` | `W;H` (float) | fixe Bildbox, §3.4 |

**`Style.doInherit(base, inherit)`** [V] `libk/table/Style.java:540-576`: Kopie von `base`; jedes Attribut, das in `base` `null` ist, kommt aus `inherit`. **Sonderfall**: `padding == 1` bzw. `border == 1` gelten als „nicht gesetzt“ und werden ebenfalls aus `inherit` übernommen (ein explizit gesetzter Wert 1 ist nicht von „Default“ unterscheidbar).

Wo `doInherit` greift:
1. Verschachtelte Tabelle ohne eigenen Stil: `style = doInherit(subtable.style /*null*/, parent.style)` ⇒ volle Kopie des Elternstils (inkl. Padding, Rahmen, Hintergrund, Fonts) [V] `pb2/stamping/pdfbox2/PDFBoxTable.java:116-120`. Die PDF-Over-Profile definieren **keinen** `table.info.Style` ⇒ `info` erbt alles von `main`.
2. Beim Zeichnen jeder Zelle: `cellStyle = doInherit(tableStyle, cellDefaultStyle)` – **Tabellenstil hat Vorrang**, die Zell-Defaults (§2.3) füllen nur Lücken [V] `TableDrawUtils.java:93-95`.
3. Das **Padding der Zelle wird nie benutzt**; Layout und Zeichnen verwenden immer `table.padding` [V] `PDFBoxTable.java:156`, `TableDrawUtils.java:85`.

Konsequenz für PDF-Over-Profile [V]/[B]: Captions = `halign` der Tabelle (SIGNATURBLOCK: `right`, AMTS: `center`), `valign=top`; Werte = `valuehalign=left`, `valuevalign` **nicht gesetzt** ⇒ `null` ⇒ wie `top` (Default-Zellstil setzt nur `valign`, nicht `valuevalign`); Bilder = `imagevalign=middle`, `imagehalign` `null` ⇒ zentriert.

### 2.5 Werte, Variablen, Ausdrücke

**Auflösung** (`ValueResolver.resolve`) [V] `lib/impl/stamping/ValueResolver.java:70-126`:
1. Optional `latin1_encoding`: `new String(value.getBytes(ISO-8859-1), UTF-8)` (Reparatur doppelt kodierter Strings) [V] `:77-79`.
2. **`SIG_DATE`**: Der Wert ist ein `java.text.SimpleDateFormat`-Muster (Default `yyyy-MM-dd'T'HH:mm:ssXXX`), formatiert wird `Calendar.getInstance()` (Zeitpunkt des Tabellenaufbaus), Zeitzone `timezone` aus Profil/`default.timezone`, sonst **lokale JVM-Zeitzone** [V] `:81-96`, `com/settings/DefaultSignatureProfileSettings.java:66`. Die PDF-Over-Profile setzen weder Format noch Zeitzone ⇒ Ausgabe z. B. `2026-10-14T12:34:56+02:00`, obwohl die Caption „Datum/Zeit-UTC“ lautet [I].
3. Sonst: Regex `\$(\{[^\$]*\})` sucht `${…}`; Gruppe 1 **inklusive geschweifter Klammern** wird an OGNL übergeben ⇒ OGNL wertet `{expr}` als **Listen-Literal** aus, das Ergebnis `[wert]` wird durch Abschneiden eines führenden `[` und schließenden `]` entpackt [V] `ValueResolver.java:54,98-123`, `com/utils/OgnlUtils.java:46-62`. Fehler ⇒ Ausdruck bleibt roh stehen.
   - Nebenwirkungen für den Nachbau [I]: Ein Wert, der selbst mit `[` beginnt/mit `]` endet, verliert diese Zeichen; ein Ausdruck mit Komma auf oberster Ebene ergibt mehrere Listenelemente („a, b“).
4. **Kontext-Variablen** [V] `lib/impl/stamping/CertificateAndRequestParameterResolver.java:78-108`:
   - `subject`, `issuer`: Map RDN-Typ → Wert, aus `LdapName(cert.getSubjectDN().getName())`; zusätzlich Alias `T` für `title`; `DN` = gesamter DN-String [V] `com/utils/DNUtils.java:36-66`. Bei mehrfach vorkommendem Typ gewinnt der zuletzt iterierte (LdapName iteriert von rechts nach links ⇒ der **linkeste**) [I].
   - `sn` = Seriennummer **dezimal** (`BigInteger.toString()`).
   - `sbp` = dynamische Signaturblock-Parameter des Requests (PDF-Over setzt keine).
   - `iui.pdfVersion` = PDF-Version des Dokuments.
5. In PDF-Over-Profilen genutzt: `SIG_SUBJECT = ${subject.title != null ? (subject.title + " ") : ""}${subject.CN}` ⇒ „[Titel ]CN“ [V] `cfg/profiles/BASE_NORMAL.properties:43`, `cfg/profiles/AMTSSIGNATURBLOCK_BASE.properties:18`. `SIG_ISSUER`, `SIG_NUMBER` sind definiert, aber in keiner PDF-Over-Tabelle verwendet (auskommentiert) [V] `cfg/profiles/BASE_NORMAL.properties:120-121`.
   - [I] Ob der IAIK-DN-String den Titel als `title=` oder `T=` ausgibt, ist offen; im zweiten Fall wäre `subject.title` immer `null` (nur `subject.T` gesetzt). Siehe Offene Fragen.
6. **Zeichensatz-Fallback** (`normalizeContent`): Für jede Caption- **und** Wertzelle wird `valueFont.getStringWidth(text)` probiert; wirft das (Zeichen nicht kodierbar), wird `convertStringToPDFFormat` angewendet: falls nicht verlustfrei in CP1252 darstellbar ⇒ **`URLEncoder.encode(text, "UTF-8")`** (!), sonst Tab→Leerzeichen [V] `pb2/stamping/pdfbox2/PDFBoxTable.java:76-110`, `com/utils/StringUtils.java:64-87`. Mit `DejaVuSansMono` als Wertefont (alle PDF-Over-Profile) praktisch nur bei Zeichen ohne Glyph relevant; `\n` im Text löst den Fallback aus, ist aber CP1252-darstellbar ⇒ Text bleibt unverändert [I].

### 2.6 Mehrsprachigkeit

Es gibt keine Laufzeit-Lokalisierung im Block: Sprache = **getrenntes Profil** (`…_DE` / `…_EN`) mit eigenen `key.*`, `value.SIG_META`, `value.SIG_NOTE`, Logo (`signatur-logo_de.png` / `_en.png`; Amtssignatur: dasselbe Demo-Logo) [V] `cfg/profiles/BASE_NORMAL_EN.properties`, `BASE_SMALL_EN.properties`, `AMTSSIGNATURBLOCK_BASE_EN.properties`. PDF-Over wählt `_EN` genau dann, wenn die Signatursprache `"en"` ist, sonst `_DE` [V] `signer/pdfas/PdfAs4SignatureParameter.java:191-193`. Der vom Nutzer bearbeitbare Hinweistext (Default aus `messages*.properties`, `simple_config.Note_Default_Standard` / `…_OfficialSignature`) ersetzt `value.SIG_NOTE` [V] `commons/Profile.java:31-40`, `res/at/asit/pdfover/gui/messages_de.properties:236-238`.

### 2.7 PDF/A-Varianten

- Profilschalter `key.SIG_PDFA1B_VALID=true` [V] z. B. `cfg/profiles/SIGNATURBLOCK_SMALL_DE_PDFA.properties:9`.
- Sichtbarer Unterschied: Caption-Font **`TTF:DejaVuSansCondensed-Bold.ttf,5`** statt Standard-14 `HELVETICA,5,BOLD` (PDF/A verlangt eingebettete Fonts); Wertefont bleibt `TTF:DejaVuSansMono.ttf,5` [V] `SIGNATURBLOCK_SMALL_*_PDFA.properties:14-15`.
- **Abweichende Spaltenbreiten** nur bei `SIGNATURBLOCK_SMALL_EN_PDFA`: `main 1 5.5`, `info 1 4.5` (bei allen anderen SMALL-Varianten auskommentiert) [V] `cfg/profiles/SIGNATURBLOCK_SMALL_EN_PDFA.properties:13,18`.
- Laufzeit-Effekte von `isPDFA()` [V]:
  - Bilder: Alphakanal wird entfernt (`removeAlphaChannel`, Ziel `TYPE_INT_RGB`) [V] `pb2/stamping/pdfbox2/PDFAsVisualSignatureBuilder.java:226-234`, `com/utils/ImageUtils.java:58-73`. [I] Vollständig transparente Pixel werden dabei voraussichtlich **schwarz** (Java2D `AlphaComposite.Src` auf opakes Ziel).
  - OutputIntent sRGB (`/icm/sRGB Color Space Profile.icm`) wird gesetzt und **ersetzt** vorhandene OutputIntents [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:260-288`.
  - Nach dem Signieren: Preflight-Prüfung (nur Logging, außer Syntaxfehler) [V] `PADESPDFBOXSigner.java:398-401, 669-714`.
  - PDF/A-Version: vorab aus XMP gelesen (`getPDFAVersion`); `isPDFA()` ist dann nur bei Teil `1` wahr, `isPDFA3()` bei Teil `3`; ohne XMP-Kennung greift der Profilschalter [V] `PADESPDFBOXSigner.java:200-203, 850-875`, `com/settings/SignatureProfileSettings.java:205-254`. [I] Folge: Bei einem PDF/A-2-Dokument wirkt das PDF/A-Profil nur über die Fonts; bei Nicht-PDF/A-Dokumenten wird trotzdem ein OutputIntent ergänzt.
  - Nur PDF/A-3: `CIDSet` aus den Type0-Fontdeskriptoren der Vorlage entfernt [V] `pb2/stamping/pdfbox2/PDFAsTemplateCreator.java:165-186`, `PDFAsVisualSignatureBuilder.java:666-692`.

### 2.8 Welche Profil-ID PDF-Over verwendet

`PdfAs4SignatureParameter.getPdfAsSignatureProfileId()` [V] `signer/pdfas/PdfAs4SignatureParameter.java:156-189`, Enum [V] `commons/Profile.java:5-53`:

| GUI-Profil (`Profile`) | erzeugte PDF-AS-Profil-ID |
|---|---|
| `SIGNATURBLOCK_SMALL` (Default) | `SIGNATURBLOCK_SMALL` + `_DE`/`_EN` + (`_NOTE` wenn Hinweistext ≠ leer) + (`_PDFA` wenn PDF/A-Option) |
| `AMTSSIGNATURBLOCK` | `AMTSSIGNATURBLOCK_DE_RECOMMENDED` / `AMTSSIGNATURBLOCK_EN_RECOMMENDED` (Hinweiszeile **immer** vorhanden, kein PDF/A, kein `_NOTE`) |
| `BASE_LOGO` | `BASE_LOGO` (sprachunabhängig) |
| `INVISIBLE` | `INVISIBLE`; ebenso immer, wenn CLI `InvisibleProfile` gesetzt (`PROFILE_VISIBILITY=false`) [V] `PdfAs4SignatureParameter.java:48, 161-164` |

`signatureNote` wird nur gesetzt, wenn der konfigurierte Hinweis nicht leer ist [V] `gui/workflow/states/PositioningState.java:111-112`, `gui/workflow/states/PrepareSigningState.java:141-143`.

**Laufzeit-Overrides** (Request-Kopie) [V] `signer/pdfas/PdfAs4Signer.java:60-86` (identisch für die Vorschau in `PdfAs4SignatureParameter.java:93-116`):

```text
if emblem.getCachedFileName() nicht leer:  sig_obj.<ID>.value.SIG_LABEL = <absoluter Pfad ~/.pdf-over/.emblem.png>
if signatureNote != null:                 sig_obj.<ID>.value.SIG_NOTE  = signatureNote
if Profil == BASE_LOGO:
    ew, eh  = emblem.getWidth()/getHeight() (Pixel des gecachten Emblems; ohne Emblem 1/1)
    ar      = ew / eh                                  (double)
    tw = th = targetLogoSize[mm] * 595/210             (double)
    if ar < 1: tw *= ar  else: th /= ar
    sig_obj.BASE_LOGO.table.main.Style.padding         = "0"
    sig_obj.BASE_LOGO.pos                              = "w:" + tw + ";f:0"        (Java Double.toString)
    sig_obj.BASE_LOGO.table.main.Style.imagescaletofit = tw + ";" + th
```

`targetLogoSize`: Default 23 mm [V] `commons/Constants.java:102`; GUI-Schieberegler 10–200 mm [V] `gui/composites/configuration/SimpleConfigurationComposite.java:253-263`. In der **Positionierungsvorschau** auf 120 mm begrenzt (`Math.min(120.0, …)`, „WORKAROUND FOR #117“), beim **Signieren nicht** [V] `gui/workflow/states/PositioningState.java:116` vs. `gui/workflow/states/PrepareSigningState.java:150` ⇒ bei > 120 mm weichen Vorschau und Ergebnis ab [I].

### 2.9 Effektiver Inhalt der von PDF-Over genutzten Profile [V]+[B]

(Nach Vererbungsauflösung; berechnet durch Nachbau von §2.1 über `cfg/profiles/*.properties`.)

**Gemeinsam für alle `SIGNATURBLOCK_SMALL_*`** (aus BASE_NORMAL → BASE_SMALL[_EN] …):

```properties
pos = f:80;w:230
table.main.1 = SIG_LABEL-i|TABLE-info
table.main.Style.bgcolor = 255 255 255
table.main.Style.padding = 4
table.main.Style.border = 0.1
table.main.Style.halign = right
table.main.Style.valign = top
table.main.Style.valuehalign = left
table.main.Style.imagevalign = middle
table.main.Style.font = HELVETICA,5,BOLD            # PDFA: TTF:DejaVuSansCondensed-Bold.ttf,5
table.main.Style.valuefont = TTF:DejaVuSansMono.ttf,5[,NORMAL]
table.info.1 = SIG_SUBJECT-cv
table.info.2 = SIG_DATE-cv
table.info.3 = SIG_META-cv
# nur *_NOTE*:
table.main.2 = SIG_NOTE-cv
```

| Profil-ID | `main.ColsWidth` | `info.ColsWidth` | Captions (SUBJECT / DATE / META / NOTE) | Logo |
|---|---|---|---|---|
| `SIGNATURBLOCK_SMALL_DE` | 1 4.5 | 1 2.7 | Unterzeichner / Datum/Zeit-UTC / Prüfinformation / (Hinweis) | `./images/signatur-logo_de.png` (326×342 px, RGB) |
| `SIGNATURBLOCK_SMALL_DE_NOTE` | 1 4.5 | 1 2.7 | dto. + Hinweis | dto. |
| `SIGNATURBLOCK_SMALL_DE_PDFA` | 1 4.5 | 1 2.7 | dto. | dto. |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA` | 1 4.5 | 1 2.7 | dto. + Hinweis | dto. |
| `SIGNATURBLOCK_SMALL_EN` | 1 4.5 | 1 2.7 | Signatory / Date/Time-UTC / Verification / (Note) | `./images/signatur-logo_en.png` (326×342 px, RGB) |
| `SIGNATURBLOCK_SMALL_EN_NOTE` | 1 4.5 | **1 3.2** | dto. + Note | dto. |
| `SIGNATURBLOCK_SMALL_EN_PDFA` | **1 5.5** | **1 4.5** | dto. | dto. |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA` | 1 4.5 | 1 2.7 | dto. + Note | dto. |

Fundstellen: [V] `cfg/profiles/BASE_NORMAL.properties:16-122`, `BASE_SMALL.properties:8-16`, `BASE_SMALL_EN.properties:8-17`, `BASE_SMALL_NOTE.properties:10`, `BASE_SMALL_EN_NOTE.properties:10`, `SIGNATURBLOCK_SMALL_EN_NOTE.properties:9`, `SIGNATURBLOCK_SMALL_*_PDFA.properties`.

Werte:
- `SIG_META` DE: „Informationen zur Prüfung der elektronischen Signatur finden Sie unter: https://www.signaturpruefung.gv.at“; EN: „Information about the verification of the electronic signature can be found at: https://www.signaturpruefung.gv.at“ [V] `BASE_NORMAL.properties:46`, `BASE_NORMAL_EN.properties:20`.
- `SIG_NOTE` Default DE/EN: eIDAS-Hinweistext [V] `BASE_NORMAL.properties:47`, `BASE_NORMAL_EN.properties:21` (wird von PDF-Over durch den Nutzertext ersetzt).

**`AMTSSIGNATURBLOCK_DE_RECOMMENDED` / `_EN_RECOMMENDED`** [V] `cfg/profiles/AMTSSIGNATURBLOCK_BASE*.properties`, `AMTSSIGNATURBLOCK_*_RECOMMENDED.properties`:

```properties
pos = f:80                     # KEINE Breite ⇒ Breite = Seitenbreite − 2·x (§4.3)
table.main.1 = SIG_LABEL-i|TABLE-info
table.main.2 = SIG_NOTE-cv
table.main.ColsWidth = 1 6
table.main.Style.bgcolor = 255 255 255 ; padding = 4 ; border = 0.1
table.main.Style.halign = center ; valign = top ; valuehalign = left ; imagevalign = middle
table.main.Style.font = HELVETICA,8,BOLD
table.main.Style.valuefont = TTF:DejaVuSansMono.ttf,8,NORMAL
table.info.ColsWidth = 1 4
table.info.1..3 = SIG_SUBJECT-cv | SIG_DATE-cv | SIG_META-cv
key.SIG_SUBJECT = "Unterzeichner/\nSiegelersteller"   (EN: "Signatory/\nCreator of seal")  ← echter Zeilenumbruch
value.SIG_LABEL = ./images/amtsignatur-logo_demo.png   (236×235 px, RGBA)
value.SIG_META  = "...signaturpruefung.gv.at\n\nInformationen zur Prüfung des Ausdrucks sind von der ausstellenden Behörde bereitzustellen."
value.SIG_NOTE  = "Dieses Dokument wurde amtssigniert. ... Beweiskraft einer öffentlichen Urkunde."
```

**`BASE_LOGO`** (kein Parent!) [V] `cfg/profiles/BASE_LOGO.properties:4-27`:

```properties
pos = w:230.0;f:80                         # von PDF-Over überschrieben: w:<tw>;f:0
table.main.1 = SIG_LABEL-i                 # nur das Bild, keine ColsWidth ⇒ 1 Spalte = volle Breite
table.main.Style.border = 0                # kein Rahmen
# kein bgcolor ⇒ transparenter Hintergrund
table.main.Style.halign = center ; valign = middle   # imagevalign NICHT gesetzt ⇒ Bild oben bündig
table.main.Style.imagescaletofit = 65.0;65.0         # von PDF-Over überschrieben: <tw>;<th>
table.main.Style.font = HELVETICA,5,BOLD ; valuefont = TTF:DejaVuSansMono.ttf,5   # Pflicht, obwohl ungenutzt
```

**`INVISIBLE`**: `isvisible=false`, keine Tabelle, `enable_placeholder_search=false` [V] `cfg/profiles/INVISIBLE.properties:6-15`.

Default-Profil von PDF-AS laut Config: `SIGNATURBLOCK_SMALL_DE_NOTE` [V] `cfg/config.properties:103` (PDF-Over setzt die ID immer explizit).

---

## 3. Layout-Algorithmus (exakt)

### 3.1 Modell und Arithmetik

- Alle Layoutgrößen sind Java-**`float`** (IEEE-754 binary32). Für bit-genaue Übereinstimmung muss der Nachbau float32 rechnen (z. B. `Math.fround` in JS/TS, `Float` in Dart/Swift/Kotlin). Beispiel, wo es zählt: Amtssignatur-Logo `floor(f32(62/236)·236) = 61`, nicht 62 [B].
- `PDFBoxTable` hat zwei Konstruktoren: „natürliche“ Größe (nur zur ersten Instanziierung, Ergebnis wird verworfen) und **fixe Breite**; maßgeblich ist ausschließlich die fixe Breite nach `Positioning` (`setWidth` → `fixWidth`) [V] `pb2/stamping/pdfbox2/PdfBoxVisualObject.java:51-71`, `pb2/pdfbox2/positioning/Positioning.java:156-157`.

### 3.2 Breite und Spaltenbreiten

```text
W = Blockbreite aus Positionierung (§4.3)
if ColsWidth gesetzt:  total = Σ rel_i (float);  unit = W / total;  col_i = unit * rel_i
else:                  col_i = W / maxCols
tableWidth = Σ col_i
```
[V] `PDFBoxTable.java:167-202, 323-327`. Eine Zelle mit Colspan `s` hat Breite `Σ col_{j..j+s-1}` [V] `PDFBoxTable.java:229-240`. Eine verschachtelte Tabelle bekommt als fixe Breite **die volle Zellbreite** (ohne Padding-Abzug) [V] `PDFBoxTable.java:585-596`.

### 3.3 Zeilenumbruch `breakString(text, maxwidth, font, size)` [V] `PDFBoxTable.java:399-447`

`maxwidth = cellWidth − 2·padding`. Exakter Algorithmus (Java-`split`-Semantik: Regex-Split, **nachfolgende** Leerstrings werden entfernt, führende bleiben):

```text
words = split(text, " ")
lines = []; cur = ""
for word in words:
    parts = split(word, "\n")
    if len(parts) > 1:
        for j, sub in enumerate(parts):
            if (j == 0 and word.startsWith("\n")) or j != 0:
                lines.append(trim(cur)); cur = ""
            w = stringWidth(replace(cur + sub, NBSP, " ")) / 1000 * size
            if w > maxwidth and cur != "": lines.append(trim(cur)); cur = ""
            cur += sub + " "
    else:
        w = stringWidth(replace(cur + word, NBSP, " ")) / 1000 * size
        if w > maxwidth and cur != "": lines.append(trim(cur)); cur = ""
        cur += word + " "
lines.append(trim(cur))
lines = [replace(l, NBSP, " ") for l in lines]
```

Eigenschaften (wichtig für Identität):
- Greedy-Wortumbruch; gemessen wird **ohne** abschließendes Leerzeichen (`cur` endet mit Leerzeichen, das neue Wort wird ohne angehängt); Vergleich **strikt `>`**.
- **Keine** Silbentrennung, **kein** Umbruch innerhalb überlanger Wörter (Wort steht allein in der Zeile und **ragt über die Zelle hinaus**).
- Mehrfache Leerzeichen bleiben innerhalb der Zeile erhalten (leere Wörter ⇒ zusätzliche Leerzeichen); nur Zeilenanfang/-ende werden getrimmt (`String.trim()` trimmt alle Zeichen ≤ U+0020).
- `\n\n` erzeugt eine **Leerzeile** (Teil „“ in der Mitte); `\n` am Wortende (z. B. „abc\n“ gefolgt von Leerzeichen) wird wegen Java-Split **nicht** als Umbruch erkannt [I] (Randfall).
- Ergebnis wird mit `\n` verbunden in der Zelle gespeichert und beim Zeichnen wieder an `\n` gesplittet [V] `PDFBoxTable.java:388-397, 566`, `TableDrawUtils.java:375, 409`.

`stringWidth` = Summe der Glyphbreiten in 1/1000 em, **ohne Kerning**; Details je Fontart §3.6.

### 3.4 Zellhöhen, Zeilenhöhen, Tabellenhöhe [V] `PDFBoxTable.java:214-328, 546-605, 676-684`

```text
p = table.padding
Text (caption/value):   h = size * nLines + 2p            # Zeilenabstand = Schriftgröße, kein Leading/Ascent!
Bild ohne scaletofit:    wf = (cellW - 2p) / imgPxW       # float, Division mit double imgPxW, dann (float)
                         h  = floor(wf * imgPxH) + 2p
Bild mit scaletofit:     h  = stf.H + 2p
Tabelle:                 sub = layout(subtable, width = cellW);  h = sub.height
rowHeight_i = max(h über Zellen der Zeile)
tableHeight = Σ rowHeight_i
danach: jede verschachtelte Tabelle, deren Höhe ≠ rowHeight ist, wird per setHeight(rowHeight) gestreckt:
        diff > 0 ⇒ letzte Zeile der Untertabelle += diff;  diff < 0 ⇒ nur Warnung, keine Änderung
```

Bildpixelmaße stammen aus `ImageReader.getWidth/Height(0)` (ohne EXIF-Rotation) [V] `com/utils/ImageUtils.java:115-135`. `getStringHeights` wird berechnet, aber **nicht** verwendet [V] `PDFBoxTable.java:517-544, 567-568`.

### 3.5 Zeichnen [V] `pb2/stamping/pdfbox2/TableDrawUtils.java`

Koordinaten im Formular-Raum (y nach oben). Die Haupttabelle wird mit `drawTable(x=0, y=1, width=tableWidth, height=tableHeight)` gezeichnet [V] `pb2/stamping/pdfbox2/PDFAsVisualSignatureBuilder.java:271-273` ⇒ sie belegt `[0, W] × [1, H+1]`.

**Reihenfolge je Tabelle** (`drawTable`): ① Hintergrund, ② Rahmen, ③ Inhalt Zeile für Zeile, Zelle für Zelle; eine verschachtelte Tabelle wird rekursiv an Ort und Stelle (inkl. eigenem Hintergrund **und** eigenem Rahmen) gezeichnet [V] `TableDrawUtils.java:54-74, 132-144`.

① **Hintergrund** (nur wenn `bgcolor` ≠ null): `setNonStrokingColor(bg)`, `addRect(x, y, tableWidth, tableHeight)` + `fill`, danach `setNonStrokingColor(BLACK)` [V] `TableDrawUtils.java:604-619`. Die Untertabelle füllt ihre Zelle erneut weiß und überdeckt dabei die halbe Linienbreite der zuvor gezeichneten Hauptrahmenlinien; sie zeichnet aber anschließend ihre eigenen Linien an denselben Koordinaten [I] (visuell identisch).

② **Rahmen** (nur wenn `border > 0`): `setLineWidth(border)`, jede Linie einzeln `moveTo/lineTo/stroke` (PDFBox `drawLine`), Strichfarbe = Default **Schwarz**, Cap/Join Default (butt/miter) [V] `TableDrawUtils.java:524-602`:
- Horizontale: oben bei `y+H`, dann nach jeder Zeile `y+H−Σrow` (also auch unten) über die volle Breite.
- Vertikale je Zeile: links bei `x`, dann nach **jeder Zelle** (rechte Zellkante, also auch ganz rechts), jeweils von Zeilenober- bis Zeilenunterkante.

③ **Text** (`drawString`) [V] `TableDrawUtils.java:164-355`:

```text
lines   = split(cellText, "\n");  textH = size * nLines
ty = cellTop − p;   tx = cellLeft + p
innerH = rowH − 2p; innerW = cellW − 2p
valign: bottom ⇒ ty −= innerH − textH ; middle ⇒ ty −= (innerH − textH)/2 ; sonst (top/null) unverändert
halign (Block-Ausrichtung!): maxW = max(stringWidth(line))/1000*size
        center/lineCenter(1 Zeile) ⇒ tx += (innerW − maxW)/2  (nur wenn > 0)
        right                      ⇒ tx += innerW − maxW      (nur wenn > 0)
descent = fontDescriptor.descent/1000*size        (negativ)
Baseline Zeile 1:  (tx, ty − size − descent)       # = ty − size + |descent|
Folgezeilen:       Td (0, −size)
PDF: BT /Fx size Tf  tx y Td (l1) Tj  0 −size Td (l2) Tj ... ET
```
- **Alle Zeilen beginnen am selben `tx`** (Block wird als Ganzes ausgerichtet, Zeilen sind innerhalb des Blocks linksbündig) – das gilt auch für rechts/zentriert ausgerichtete mehrzeilige Captions wie „Unterzeichner/↵Siegelersteller“.
- `lineCenter` mit mehreren Zeilen: jede Zeile einzeln zentriert (eigener Pfad) [V] `TableDrawUtils.java:191-266` (von PDF-Over nicht genutzt).
- Captions: Font/Größe = `table.font`, Ausrichtung `halign`/`valign`; Werte: `table.valueFont`, `valuehalign`/`valuevalign` [V] `TableDrawUtils.java:357-423`.
- Textfarbe: Schwarz (durch das `BLACK` nach dem Hintergrund bzw. PDF-Default).
- Descent-Werte (pro Font) siehe §3.6; Caption und Wert einer Zeile haben daher leicht unterschiedliche Baselines (z. B. 5 pt: Helvetica-Bold −1.035, DejaVuSansMono −1.1792) [B].

③ **Bild** (`drawImage`) [V] `TableDrawUtils.java:425-481` – **ohne Padding**, Bezug ist die volle Zelle:

```text
(iw, ih) = vorab berechnete Bildgröße (s. u.)
hoff = cellW − iw
imagehalign: right ⇒ x = cellLeft + hoff ;  left ⇒ x = cellLeft + hoff/2 (BUG: wie center) ; sonst ⇒ x = cellLeft + hoff/2
voff = rowH − ih
imagevalign: middle ⇒ yTop = cellTop − voff/2 ; bottom ⇒ yTop = cellTop − voff ; sonst (top/null) ⇒ yTop = cellTop
PDF: q iw 0 0 ih x (yTop − ih) cm /Imx Do Q
```

**Bildgröße** (`readTableResources`, einmal pro Bildpfad) [V] `PDFAsVisualSignatureBuilder.java:138-251`:

```text
width  = colSizes[j]            # j = Eintragsindex der Zelle in der Zeile (nicht Colspan-Summe)
height = rowHeight_i + 2p       # (sic: +2p)
iw = (int)floor(width)  − 2p
ih = (int)floor(height) − 2p    # ⇒ = floor(rowH + 2p) − 2p
if table.style.imagescaletofit: iw, ih = stf.W, stf.H
sf = min(iw / imgPxW, ih / imgPxH)        # float
iw = floor(sf * imgPxW);  ih = floor(sf * imgPxH)        # float-Produkt, dann floor ⇒ ganze pt
```
- Seitenverhältnis bleibt erhalten; Ergebnis sind **ganzzahlige pt**; das Bild wird in Originalauflösung eingebettet und nur per `cm` skaliert.
- Einbettung: `LosslessFactory.createFromImage` (verlustfrei/Flate; Alpha als SMask) [V] `:237`; [I] JPEG-Embleme werden dadurch **nicht** als DCT übernommen, sondern dekodiert und verlustfrei neu kodiert.
- Alpha: PDF/A ⇒ immer entfernt; sonst nur entfernt, wenn das Bild `hasAlpha()` ohne Alpha-Raster hat (indizierte Bilder mit transparenter Farbe) [V] `:226-234`.

### 3.6 Fonts [V] `pb2/stamping/pdfbox2/PDFBoxFont.java:159-247`, `PDFAsFontCache.java:25-41`

Beschreibungssyntax `FACE,SIZE,STYLE` (3 Teile) oder `TTF:<datei>,SIZE` (2 Teile); bei 3 Teilen mit `TTF:` wird STYLE ignoriert; alles andere ⇒ **Fallback Helvetica 8** [V] `PDFBoxFont.java:222-239`.

| Art | unterstützte Werte | PDF-Font | Encoding | Einbettung |
|---|---|---|---|---|
| Standard-14 | `HELVETICA` NORMAL/BOLD, `COURIER` NORMAL/BOLD, `TIMES_ROMAN` NORMAL/BOLD/ITALIC (sonst Exception „Invalid font descriptor“) | Type1 Helvetica[-Bold] usw. | WinAnsi (CP1252), 1 Byte | **nicht eingebettet** (Viewer substituiert) |
| TTF | Datei `<workdir>/fonts/<name>` (`DejaVuSansMono.ttf`, `DejaVuSansCondensed[-Bold].ttf`, `arial.ttf`) | Type0 / CIDFontType2 | Identity-H, 2 Byte (GID) | eingebettet, `PDType0Font.load(doc, stream)` ⇒ `embedSubset=true` [P] |

Metriken für den Nachbau:
- **Standard-14**: Breiten und Descender aus den AFM-Dateien in PDFBox (`org/apache/pdfbox/resources/afm/*.afm`), keine Kerning-Paare. Helvetica-Bold: Ascender 718, **Descender −207**; Helvetica: −207; Courier: −157, Breite 600 [P] (AFM aus `pdfbox-2.0.34.jar`). Zeichen außerhalb WinAnsi ⇒ `IllegalArgumentException` beim Messen/Schreiben.
- **TTF (Type0)**: Glyphbreite = `Math.round(advanceWidth · 1000/unitsPerEm)` (gerundet, so steht es im `/W`-Array und so misst `getStringWidth`) [P] `PDCIDFontType2Embedder.getWidths`, `PDCIDFont.getWidth → getWidthForCID`. Descent = `hhea.descender · 1000/unitsPerEm` (ungerundet float) [P] `TrueTypeEmbedder.createFontDescriptor`.
  - `DejaVuSansMono.ttf`: upem 2048, hhea −483 ⇒ descent −235.84; Advance 1233 ⇒ **602** je Zeichen (monospace) [B].
  - `DejaVuSansCondensed-Bold.ttf`: upem 2048, hhea −483 ⇒ descent −235.84 [B].
- Fonts werden pro Dokument gecacht (Schlüssel = Fontpfad bzw. `FACE:STYLE`) [V] `PDFBoxFont.java:165-193`.
- [I] Für Nicht-PDF/A-Profile hängt das **Aussehen der Captions vom PDF-Viewer** ab (Helvetica-Bold wird ersetzt, z. B. durch Arial Bold/Liberation Sans Bold). Der Nachbau sollte ebenfalls nicht einbetten und nur die AFM-Metriken verwenden, um layout-identisch zu bleiben.

### 3.7 PDF-Struktur des Erscheinungsbildes

Aufbau über PDFBox' „Visible Signature Template“ [V] `pb2/stamping/pdfbox2/PDFAsTemplateCreator.java:58-198`, `PDFAsVisualSignatureBuilder.java`:

| Element | Wert | Fundstelle |
|---|---|---|
| Widget-`/Rect` (unrotierte Seite) | `[x, y−H, x+W, y]`, `x`,`y` = linke obere Ecke aus Positionierung (§4) | [V] `PDFAsVisualSignatureBuilder.java:404-487`, `PDFAsVisualSignatureProperties.java:88-106` |
| Holder-Form `/BBox` | `[W+1, H+1, 0, 0]` (Ecken vertauscht geschrieben; normalisiert `[0 0 W+1 H+1]`) | [V] `PDFAsVisualSignatureProperties.java:107-109`, `PDFAsVisualSignatureBuilder.java:507-522, 537-547` |
| Holder-Form `/Matrix` | Rotation um `(r + Seitenrotation)` Grad (bei 0 ⇒ Identität) | [V] `PDFAsVisualSignatureBuilder.java:549-567` |
| Holder-Form-Inhalt | `q 1.0 0.0 0.0 1.0 0.0 0.0 cm /FRMn Do Q` | [V] `PDFAsVisualSignatureBuilder.java:319-346` |
| Inner-Form `/FRMn` | gleiche BBox; Inhalt = Tabellenzeichnung (§3.5), Ressourcen: Fonts, Bilder, `/ProcSet [/PDF /Text /ImageC /ImageB /ImageI]` | [V] `PDFAsVisualSignatureBuilder.java:96-104, 253-286, 574-590` |
| AcroForm | `SigFlags 3`, `DA "/sylfaen 0 Tf 0 g"` (Vorlage) | [V] `PDFAsVisualSignatureBuilder.java:390-402` |
| Feldname `/T` | `signature_field_name` oder `"PDF-AS Signatur"` + laufende Nummer (max. vorhandene + 1) | [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:572-580`, `pb2/pdfbox2/utils/PdfBoxUtils.java:41-79` |
| `/TU` (Alternativname) | Text aller Caption-/Wertzellen der **äußersten** Tabelle, je mit `\n` (s. Quirk) | [V] `TableDrawUtils.java:87, 118-125, 161, 696-699`, `PADESPDFBOXSigner.java:331-335` |
| Signatur-Dict | `/Name` = aufgelöster `SIG_SUBJECT`, `/Reason` = `adobeSignReasonValue`, `/Location` = Platzhaltername (nur bei QR-Platzhalter) | [V] `PADESPDFBOXSigner.java:154-197` |

**Sichtbare Folge der `+1`-BBox** [I] (PDF 32000-1 §12.5.5: die transformierte BBox wird auf das `/Rect` **skaliert**): Die Tabelle erscheint um `W/(W+1)` horizontal und `H/(H+1)` vertikal gestaucht; die linke obere Ecke liegt exakt bei `(x, y)`, rechts und unten bleibt ein Rand von knapp 1 pt innerhalb des Rect (z. B. 230×49-Block: Tabelle ≈ 229.00 × 48.02 pt sichtbar). Für optische Identität: dieselbe BBox/Rect-Kombination erzeugen.

**Seitenrotation** [V] `PDFAsVisualSignatureBuilder.java:425-481`, `PDFAsVisualSignatureDesigner.java:87-123`: `x`,`y` sind Koordinaten der **sichtbaren (gedrehten)** Seite (Seitenmaße aus der MediaBox, bei 90/270° vertauscht). Das Rect wird um die linke untere Ecke rotiert und je nach `/Rotate` verschoben; nachgerechnet [I] ergibt sich mit `W0×H0` = unrotierte MediaBox:
- 90°: Rect `x∈[W0−y, W0−y+H]`, `y∈[x, x+W]`
- 180°: Rect `x∈[W0−x−W, W0−x]`, `y∈[H0−y, H0−y+H]`
- 270°: Rect `x∈[y−H, y]`, `y∈[H0−x−W, H0−x]`

und das AP-`/Matrix` dreht den Inhalt so, dass er für den Betrachter aufrecht steht. Annahme dabei: MediaBox-Ursprung (0,0); ein CropBox-/MediaBox-Offset wird **nicht** berücksichtigt [V] (keine Verwendung von `getLowerLeftX/Y` im Rect-Pfad).

### 3.8 Rechenbeispiele [B]

Annahmen: Unterzeichner „Max Mustermann“ (kurz genug für eine Zeile), Datum `2026-10-14T12:34:56+02:00` (25 Zeichen), Standardlogos, Hinweistext = Profil-Default, Breite gemäß Profil (SMALL: 230; AMTS: Auto-Breite auf A4 = 595 − 2·50 = 495).

| Profil-ID | W × H | `main` Spalten | `info` Spalten | Zeilenhöhen (main; info) | Logo gezeichnet (pt) |
|---|---|---|---|---|---|
| SIGNATURBLOCK_SMALL_DE | 230 × **49** | 41.8182 / 188.1818 | 50.8599 / 137.3219 | 49; 13/13/23 | 33×34 bei (4.409, 8.5) |
| SIGNATURBLOCK_SMALL_DE_NOTE | 230 × **82** | dto. | dto. | 49/33; 13/13/23 | 33×34 |
| SIGNATURBLOCK_SMALL_EN | 230 × 49 | dto. | 50.8599 / 137.3219 | 49; 13/13/23 | 33×34 |
| SIGNATURBLOCK_SMALL_EN_NOTE | 230 × 82 | dto. | 44.8052 / 143.3766 | 49/33; 13/13/23 | 33×34 |
| SIGNATURBLOCK_SMALL_DE_PDFA / _DE_NOTE_PDFA | 230 × 49 / 82 | dto. | 50.8599 / 137.3219 | wie DE | 33×34 |
| SIGNATURBLOCK_SMALL_EN_PDFA | 230 × 49 | 35.3846 / 194.6154 | 35.3846 / 159.2308 | 49; 13/13/23 | 27×28 |
| SIGNATURBLOCK_SMALL_EN_NOTE_PDFA | 230 × 82 | 41.8182 / 188.1818 | 50.8599 / 137.3219 | 49/33 | 33×34 |
| AMTSSIGNATURBLOCK_DE/EN_RECOMMENDED (A4, auto) | 495 × **120** | 70.7143 / 424.2857 | 84.8571 / 339.4286 | 96/24; 24/16/56 | 61×61 |
| BASE_LOGO, 23 mm, Standardlogo DE | 62.118 × 65.167 | 1 Spalte | – | 65.167 | 62×65 (oben, horizontal zentriert) |
| BASE_LOGO, 23 mm, Amts-Demo-Logo | 65.167 × 64.891 | 1 Spalte | – | 64.891 | 65×64 |

Detail `SIGNATURBLOCK_SMALL_DE` (Formularraum, Tabelle `[0,230]×[1,50]`; Baseline = Startpunkt des `Td`):

```text
image   SIG_LABEL   Zelle x=0 top=50 w=41.818 h=49        → 33×34 pt, gezeichnet bei (4.409, 8.5)
table   info        Zelle x=41.818 top=50 w=188.182 h=49  colw 50.8599/137.3219, rowh 13/13/23
  caption "Unterzeichner"   Helvetica-Bold 5, right/top   → Baseline (55.338, 42.035)
  value   "Max Mustermann"  DejaVuSansMono 5, left/top    → Baseline (96.678, 42.179)
  caption "Datum/Zeit-UTC"                                → (50.903, 29.035)
  value   "2026-10-14T12:34:56+02:00"                     → (96.678, 29.179)
  caption "Prüfinformation"                               → (51.178, 16.035)
  value   3 Zeilen: "Informationen zur Prüfung der" | "elektronischen Signatur finden Sie unter:" |
          "https://www.signaturpruefung.gv.at"            → (96.678, 16.179), Zeilenabstand 5
```

Zeichen pro Wertzeile (DejaVuSansMono, 602/1000·size je Zeichen): SMALL-Info 42 Zeichen (Grenze 129.32 pt), SMALL-Hinweis 59 Zeichen (180.18 pt), AMTS-Info 68 Zeichen (331.43 pt) [B]. Der Unterzeichnername bricht im SMALL-Block also erst ab 43 Zeichen um (dann +5 pt Blockhöhe je Zusatzzeile).

Hinweis: Ein **benutzerdefiniertes Emblem** ändert über das Seitenverhältnis die Logo-Zellhöhe (`floor((41.818−8)/pxW·pxH)+8`) und kann so den ganzen Block höher machen; die Info-Tabelle wird dann über ihre **letzte Zeile** (SIG_META) gestreckt.

### 3.9 Quirks, die ein identischer Nachbau übernehmen muss

| # | Verhalten | Fundstelle |
|---|---|---|
| Q1 | Zeilenhöhe = `size·n + 2p` (kein Leading), Baseline `top − p − size + |descent|` | [V] `PDFBoxTable.java:568`, `TableDrawUtils.java:300-319` |
| Q2 | Mehrzeilige Captions: Block-, nicht Zeilenausrichtung | [V] `TableDrawUtils.java:269-299` |
| Q3 | Werte ohne `valuevalign` sind oben bündig (nicht „middle“) | [V] `TableDrawUtils.java:412-414, 176-183` |
| Q4 | Bild ohne Padding positioniert; `imagehalign=left` wirkt wie `center` | [V] `TableDrawUtils.java:432-456` |
| Q5 | Bildbox `floor(colW)−2p` × `floor(rowH+2p)−2p`, float32, Ergebnis ganzzahlig | [V] `PDFAsVisualSignatureBuilder.java:190-221` |
| Q6 | BBox `W+1 × H+1`, Tabelle bei y=1 ⇒ Stauchung beim Mapping auf Rect | [V] `PDFAsVisualSignatureProperties.java:107`, `PDFAsVisualSignatureBuilder.java:271` |
| Q7 | Untertabelle erbt den kompletten Stil (inkl. Rahmen/Hintergrund) und zeichnet beides erneut | [V] `PDFBoxTable.java:116-120`, `TableDrawUtils.java:132-144` |
| Q8 | Padding/Border = 1 sind „nicht gesetzt“ (werden überschrieben) | [V] `Style.java:562,567` |
| Q9 | `SIG_DATE` in lokaler Zeitzone trotz „UTC“-Caption | [V] `ValueResolver.java:81-96` |
| Q10 | `/TU` enthält nur Texte der äußersten Tabelle (Untertabelle überschreibt zuerst, äußere Tabelle danach) ⇒ SMALL ohne Hinweis: `/TU` = leer | [V] `TableDrawUtils.java:87,161` (Rekursion `:140`) |
| Q11 | Nicht-CP1252-Text im Fallback wird URL-kodiert | [V] `com/utils/StringUtils.java:72-78` |
| Q12 | Überlange Wörter ragen aus der Zelle | [V] `PDFBoxTable.java:424-439` |

---

## 4. Positionierung

### 4.1 Positions-String (`TablePos`) [V] `libk/pdf/TablePos.java:168-330`

Format: `;`-getrennte `k:v`-Paare, `k` genau ein Zeichen (sonst Fehler).

| k | Werte | Bedeutung |
|---|---|---|
| `p` | `auto` / `new` / `last` / Ganzzahl ≥ 1 | Seitenmodus AUTO (Default) / NEW / LAST / EXACT |
| `x`, `y` | `auto` / float ≥ 0 | linke obere Ecke; y von **unten** gemessen |
| `w` | `auto` / float ≥ 1 | Blockbreite |
| `f` | float ≥ 0 | Fußzeilenhöhe (nur bei `y:auto` und nicht `p:new` wirksam, `getFooterLine()` liefert sonst 0) [V] `:143-147` |
| `r` | Vielfaches von 90 | Blockrotation (gegen Uhrzeigersinn um linke untere Ecke) |

Merge: `new TablePos(request, profileDefault)` = Profilwerte als Basis, Request-Werte überschreiben **nur die im Request genannten Schlüssel** [V] `:192-199, 317-330`, `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:599-640`. Ohne Request-Position: Profil-`pos`; ohne beides: alles auto.

### 4.2 Was PDF-Over übergibt [V] `signer/pdfas/PdfAs4SignatureParameter.java:133-149`

| Situation | String | Ergebnis nach Merge mit `f:80;w:230` (SMALL) |
|---|---|---|
| Auto-Positionierung (Einstellung oder Profil INVISIBLE) [V] `gui/workflow/states/OpenState.java:93` | `p:auto;x:auto;y:auto` | auto, w=230, f=80 |
| Manuell auf Seite n | `p:%d;x:%f;y:%f` (Locale `null` ⇒ Punkt, 6 Nachkommastellen) | EXACT, w=230 |
| „Neue Seite“ in der UI | `p:<numPages+1>;x;y` (die `p:new`-Variante nur bei Seite < 1) | EXACT mit Seite > Seitenzahl ⇒ neue Seite (§4.5) |
| QR-Platzhalter gewählt | `p;x;y` aus Platzhalter, die Platzhalter-`TablePos` (inkl. `w`) ersetzt beim Signieren die Position komplett | §5 |

### 4.3 Algorithmus `adjustSignatureTableandCalculatePosition` [V] `pb2/pdfbox2/positioning/Positioning.java:95-289`

```text
MARGIN_H = 50, MARGIN_V = 20
page = numPages;  newPage = (mode == NEW)
if mode == EXACT: page = pos.page; if page > numPages: newPage = true; page = numPages
newPage = checkIfNewPageIsAllowed(newPage)          # s. 4.5
box = rotateBox(page.CropBox, page.Rotate)          # sichtbare Seitenmaße (Breite/Höhe ggf. vertauscht)
pw, ph = box.width, box.height
x = pos.xAuto ? 50 : pos.x
w = pw − 2·x                                        # !!! auch bei manuellem x, wenn w auto
if !pos.wAuto: w = pos.w; if pos.xAuto: x = (pw − w)/2      # Zentrierung
table.setWidth(w); table.fixWidth(); h = table.height
if mode == LAST:   return (newPage, page, x, pos.yAuto ? ph − 20 : pos.y)
if !pos.yAuto or h == 0: if newPage: page++ ; return (newPage, page, x, pos.y)
if newPage:        page++; return (true, page, x, ph − 20)
lowest = getMaxYPosition(page, footer = pos.footerLine)      # Raster-Scan, §4.4 (IOException ⇒ wie leere Seite)
y = ph − lowest − 20
if y − footer <= h:                                  # passt nicht über die Fußzeile
    newPage = checkIfNewPageIsAllowed(mode == AUTO); if newPage: page++
    if mode != AUTO: page = numPages
    y = ph − 20
return (newPage, page, x, y)
```

Konsequenzen [V]/[I]:
- **SMALL** (w=230) auto auf A4 (595×842): `x = (595−230)/2 = 182.5`; leere Seite ⇒ `y = 822`.
- **AMTSSIGNATURBLOCK** (keine Breite im Profil): auto ⇒ `x=50`, `w = pw − 100` (A4: 495). **Manuell** platziert ⇒ `w = pw − 2·x` – die Breite (und damit die Höhe) hängt von der x-Position ab und weicht von der Vorschau (immer 495 auf A4) ab; bei `x > pw/2` wird `w` negativ [I].
- Auto-Positionierung arbeitet immer auf der **letzten** Seite (`p:auto` ⇒ `page = numPages`).
- Es gibt keine Prüfung, ob ein manuell gesetzter Block innerhalb der Seite liegt (UI klemmt aber, §4.7).

### 4.4 Freiraumsuche (Raster-Scan) [V] `pb2k/pdf/PDFUtilities.java:67-113`, `pb2/pdfbox2/positioning/PositioningPageDrawer.java:19-24`

```text
img = ARGB-Bild (int)CropBox.width × (int)CropBox.height Pixel (1 px = 1 pt), Hintergrund MAGIC = RGB(152,254,52)
render page mit PositioningPageDrawer: JEDE Füll-/Strich-/Textfarbe wird durch (234,14,184,α=211) ersetzt; Bilder unverändert
bg = MAGIC  (optional: häufigste Farbe der 4 Pixel 5 px innerhalb der Ecken, wenn sigblock.placement.bgcolor.detection.enabled=true)
row = img.height − 1 − (int)footer
for row abwärts bis 1: wenn irgendein Pixel der Zeile ≠ bg (exakter ARGB-Vergleich) ⇒ lowest = row; stop
falls nichts gefunden: lowest = 0
```
- `lowest` ist der Abstand der **untersten belegten Pixelzeile oberhalb der Fußzeile** von der Oberkante (in pt) ⇒ Blockoberkante 20 pt darunter.
- Inhalte in den unteren `f` pt (Fußzeile) werden ignoriert; **jede** gezeichnete Fläche zählt als Inhalt – auch weiße Rechtecke (Farbe wird ersetzt) und Bilder.
- `PDFRenderer.renderPageToGraphics` füllt den Hintergrund per `clearRect` (Hintergrundfarbe) und wendet die Seitenrotation an [P]. [I] Bei gedrehten Seiten ist das Bild in unrotierten CropBox-Maßen angelegt, der Inhalt aber gedreht ⇒ Scan potenziell ungenau.
- Debug: `sigblock.placement.debug.file` schreibt das Rasterbild als PNG [V] `PDFUtilities.java:108-111`.
- Die alte vektorbasierte Analyse `pb2k/pdf/PDFPage.java` wird nicht mehr verwendet [V] (kein Aufrufer).

Für den Nachbau: Ein Renderer mit „Farb-Override“ ist nicht nötig, wenn man stattdessen die Belegung als „irgendein Pixel ≠ Hintergrund“ eines normal gerenderten Bildes mit einer garantiert nicht vorkommenden Hintergrundfarbe auswertet und dabei weiße Füllungen als belegt zählt – exakt identische Ergebnisse erfordern aber denselben Effekt (Anti-Aliasing-Ränder zählen mit) [I].

### 4.5 Neue Seite

- Erzeugt **vor** dem Platzieren: `new PDPage(lastPage.MediaBox)`, leere Ressourcen, gleiche `/Rotate` wie die letzte Seite, angehängt [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:227-237`.
- **Nicht erlaubt auf bereits signierten Dokumenten** (≥ 1 Signaturfeld mit Wert): dann ⇒ ohne Fehler auf die letzte vorhandene Seite (manuell: an die gewählten Koordinaten; auto: `y = ph − 20`), außer `SIGNED_NEWPAGE_FORCE=true` (erzwingen) oder `sigblock.placement.less.space.failing=true` (Fehler `error.pdf.stamp.12`) [V] `Positioning.java:241-262, 291-302`. Das betrifft auch die UI-Option „Neue Seite“ [I].
- Bei Verwendung eines bestehenden Signaturfeldes wird nie eine Seite angelegt [V] `PADESPDFBOXSigner.java:227`.

### 4.6 Rotation, CropBox, MediaBox – Inkonsistenzen

| Stelle | verwendete Box | Fundstelle |
|---|---|---|
| PDF-Over-UI (Seitenmaße, Rendering) | `getBBox()` = **CropBox** [P], rotiert | [V] `gui/composites/SignaturePanel.java:262-266` |
| Positioning (Breite, Freiraum) | **CropBox**, rotiert | [V] `Positioning.java:119-130`, `PDFUtilities.java:75-76` |
| Designer/Rect | **MediaBox**, rotiert | [V] `PDFAsVisualSignatureDesigner.java:94-117` |
| Platzhalter-Extraktion | CropBox-Maße bei Rotation | [V] `pb2/pdfbox2/placeholder/SignaturePlaceholderExtractor.java:224-234` |

[I] Solange CropBox = MediaBox mit Ursprung (0,0) gilt, sind alle Systeme konsistent. Bei verschobener CropBox landet der Block um den CropBox-Ursprung versetzt gegenüber der UI-Vorschau. Der Nachbau sollte das Originalverhalten (Koordinaten relativ zum User-Space-Ursprung) übernehmen, um identische Ergebnisse zu liefern, oder dies bewusst als Fix dokumentieren.

### 4.7 UI-Koordinaten ↔ PDF [V] `gui/composites/SignaturePanel.java`

Koordinatenräume (`enum U`) [V] `:63-99`:

| Raum | Ursprung | Einheit |
|---|---|---|
| `PAGE_ABS` | links **unten** der sichtbaren Seite, y nach oben | pt |
| `PAGE_REL` | links oben, y nach unten | pt |
| `IMAGE` | links oben im gerenderten Seitenbild | px = pt · `pageToImageScale` |
| `SCREEN_REL` | links oben des gezeichneten Bildes | px = IMAGE · `imageToScreenScale` |
| `SCREEN_ABS` | links oben des Panels | SCREEN_REL + (`offX`,`offY`) |

- `pageWidth/Height` = CropBox, bei `Rotate % 180 == 90` vertauscht [V] `:262-265`.
- `pageToImageScale = Bildschirmhöhe(px) / pageHeight` (Seite wird in Bildschirmhöhen-Auflösung gerendert, mit Rotation) [V] `:266, 283`.
- `imageToScreenScale = min(panelW/imgW, panelH/imgH)`, Bild zentriert (`offX/offY`) [V] `:311-323`.
- **Signaturposition `sigPagePos` = linke obere Ecke in `PAGE_ABS`** = exakt das `x`/`y` für PDF-AS [V] `:107-109`, `gui/composites/PositioningComposite.java:317-330`.
- Blockgröße in Seiteneinheiten: `sigPageWidth = previewImg.width / 4`, `sigPageHeight = previewImg.height / 4` (Ganzzahldivision; Faktor 4 = 288 dpi / 72) [V] `:174-183`.
- Klemmung: `x ∈ [0, pageWidth − sigPageWidth]`, `y ∈ [sigPageHeight, pageHeight]` [V] `:218-225`. Startposition, falls keine vorhanden: `(0.5·pageWidth, 0.25·pageHeight)` (geklemmt) [V] `:290-296`.
- Maus: Drücken außerhalb des Blocks setzt die linke obere Ecke auf den Cursor; auf dem Block bleibt der Greif-Offset erhalten; Ziehen aktualisiert [V] `:358-417`. Cursor: Hand über Block, Move beim Ziehen [V] `:371-407`.
- Tastatur: Pfeile verschieben um **15 pt** (`SIGNATURE_KEYBOARD_POSITIONING_OFFSET`), Bild↑/↓/Pos1/Ende blättern, Enter = Signieren; Mausrad blättert Seiten (Shift: 5) [V] `gui/composites/PositioningComposite.java:184-266`, `commons/Constants.java:112`.
- Darstellung: Vorschau-Bild skaliert auf `sigPageWidth·pageToImageScale·imageToScreenScale`, dazu 1-px-Rahmen in `MAINBAR_ACTIVE_BACK_DARK` (Default `#6BA5D9`); ohne Vorschau-Bild rotes 100×40-px-Rechteck [V] `:327-339`, `gui/composites/PositioningComposite.java:97-100`, `commons/Constants.java:275-285`.
- „Neue Seite“: virtuelle Seite `numPages+1`, weiß, mit Maßen der letzten Seite [V] `:246-277`, `gui/composites/PositioningComposite.java:103-109, 270-295`. Startseite der UI = **letzte Seite** [V] `PositioningComposite.java:133-138`.
- Die Position bleibt beim Blättern erhalten (keine Neu-Klemmung bei anderer Seitengröße) [I] `:197-202, 290`.
- Beim **Zurückspringen** in die Positionierung (vorheriger State weder Positioning noch Open, z. B. aus der BKU-Auswahl) wird die zuvor gewählte manuelle Position wiederhergestellt [V] `gui/workflow/states/PositioningState.java:137-142, 123-129`. Die von `PdfAs4Signer.sign` im `SignResult` zurückgemeldete Position (bei Amtssignatur immer „auto“) wird von der GUI nicht ausgewertet [V] `signer/pdfas/PdfAs4Signer.java:145-157` (keine Verwendung in `gui/`).
- Scheitert der Aufbau der Positionierungs-UI (Exception), wird mit Fehlermeldung auf **Auto-Positionierung** zurückgefallen [V] `gui/workflow/states/PositioningState.java:169-180`.

---

## 5. Platzhalter

PDF-Over unterstützt zwei Arten, gesteuert durch Einstellungen `USE_PLACEHOLDER_SEARCH`, `USE_FIELDS`, `USE_MARKER` [V] `commons/Constants.java:142-148`, `gui/workflow/states/OpenState.java:127-230`. **Signaturfelder haben Vorrang**: ist „Felder verwenden“ aktiv, werden QR-Marker gar nicht gesucht (`else if`) [V] `OpenState.java:136, 184`.

### 5.1 QR-Code-Platzhalter – Erkennung [V] `pb2/pdfbox2/placeholder/SignaturePlaceholderExtractor.java`

1. Seiten werden der Reihe nach mit einem `PDFStreamEngine` verarbeitet, der nur Grafikzustands- und Textoperatoren kennt (`cm, q, Q, gs, Tm, w, BT…`; `Do` ist eigens behandelt) [V] `:107-125`, Ressource `placeholder/pdfbox-reader-2.properties`.
2. Bei `Do` mit **Bild-XObject**: Prüfung als QR; **Form-XObjects werden nicht betreten**, Inline-Bilder (`BI…EI`) und Annotationen werden nicht geprüft [V] `:186-257`.
3. `checkImage`: `PDImageXObject.getImage()` (dekodiertes Bild); Bilder < 10 px in einer Dimension übersprungen; ZXing `MultiFormatReader` mit `POSSIBLE_FORMATS={QR_CODE}`, `HybridBinarizer`, ohne `TRY_HARDER`; der QR-Code darf Teil eines größeren Bildes sein [V] `:423-513`.
4. **Inhaltsformat**: Text muss mit `PDF-AS-POS` **beginnen** (`startsWith`), danach `;key=value`-Paare (Split an `;` und `=`, Paare ≠ 2 Teile ignoriert, Schlüssel case-insensitiv): `id`, `profile`, `type`, `key` [V] `:456-490`, `lib/impl/placeholder/SignaturePlaceholderData.java:59-65`, `lib/impl/placeholder/PlaceholderExtractorConstants.java:4`. Beispiele: `PDF-AS-POS`, `PDF-AS-POS;id=1`, `PDF-AS-POS;id=2;profile=SIGNATURBLOCK_SMALL_DE`.
5. **Position** aus der CTM beim `Do` (Bild = Einheitsquadrat): `x = e`, `yTop = f + d`, `w = a` (nach Herausrechnen der Seitenrotation; Sonderformeln für 90/180/270°) ⇒ `TablePos("p:<seite>;x:<floor(x)>;y:<ceil(yTop)>;w:<ceil(w)>")` (Werte als Java-`double`-String, z. B. `x:72.0`) [V] `:198-243`. **Die Höhe des Platzhalters wird nicht verwendet**; die Breite wird zur Blockbreite.
6. `placeholderName` = Ressourcenname des XObjects (z. B. `Im3`) [V] `:242`.

### 5.2 Auswahl [V]

**Modi** [V] `lib/impl/placeholder/PlaceholderExtractorConstants.java:5-8`, `SignaturePlaceholderExtractor.java:138-183, 259-337`, Doku `pdf-as-4/doc/PDFAS4_Dokumentation.docx` §3.7.4:

```text
für jede Seite nacheinander (Liste wächst kumulativ):
    Kandidaten = gefundene Platzhalter ohne bereits benutzte (Name ∈ /Location vorhandener Signaturen)
    Seiten-Match: erster Kandidat mit id == gesuchte id   (Vergleich: als Integer, sonst equalsIgnoreCase)
                  oder (Modus ≠ SORTED und keine id gesucht und Kandidat ohne id)
nach allen Seiten (falls kein Seiten-Match):
    STRICT(0)  ⇒ Fehler error.pdf.stamp.09
    SORTED(3)  ⇒ Kandidat mit kleinster id (numerisch, sonst lexikografisch, ignore case)
    dann       ⇒ erster Kandidat ohne id
    LENIENT(2) ⇒ sonst erster Kandidat überhaupt
    MODERATE(1)⇒ sonst keiner (Signatur ohne Platzhalter)
```
- [V] Bug: `matchPlaceHolderId` vergleicht `Integer`-Objekte mit `==` ⇒ numerische IDs > 127 matchen nie (Java-Integer-Cache) `SignaturePlaceholderExtractor.java:339-351`. Der Nachbau sollte bewusst entscheiden (Fix empfohlen, Abweichung dokumentieren).
- [I] Ressourcennamen sind pro Seite/Ressourcen-Dictionary vergeben; gleiche Namen auf verschiedenen Seiten können fälschlich als „benutzt“ gelten.

**PDF-Over-Ablauf (QR)** [V] `gui/workflow/states/OpenState.java:184-219`, `pb2/pdfbox2/placeholder/SignatureFieldsAndPlaceHolderExtractor.java:46-57`, `signer/pdfas/PdfAs4Signer.java:99-105`:
1. Beim Öffnen: Suche mit `id="1"`, Modus SORTED ⇒ ein Kandidat.
2. Dialog „Im vorliegenden Dokument wurde ein Signaturplatzhalter gefunden. Soll dieser verwendet werden?“ (Ja/Nein/Abbrechen) [V] `res/at/asit/pdfover/gui/messages_de.properties:129-130`.
3. Ja ⇒ `status.signaturePosition = (x, y, page)` des Platzhalters (UI-Positionierung wird übersprungen), `placeholderId = id` (kann `null` sein).
4. Beim Signieren: globale Suche aktiv, Modus **MODERATE (1)**, gesuchte ID = gespeicherte ID ⇒ erneute Extraktion; die **Platzhalter-`TablePos` (inkl. `w`) ersetzt die Position** [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:161-179, 599-614`, `lib/impl/placeholder/PlaceholderFilter.java:42-67`.
5. Enthält der QR-Code `profile=…`, wird das Profil **überschrieben** (Default `placeholder_profile_overwrite=true`) – die PDF-Over-Overrides (Emblem, Notiz) gelten dann nicht, da sie an die ursprüngliche Profil-ID gebunden sind [V] `PADESPDFBOXSigner.java:169-178` + [I].

### 5.3 Leere Signaturfelder und `PlaceholderSelectionGui`

- Suche: alle AcroForm-Felder vom Typ Signatur mit `partialName ≠ null` und ohne Wert [V] `pb2/pdfbox2/placeholder/SignatureFieldsAndPlaceHolderExtractor.java:18-35`.
- Dialog „Ja/Nein/Abbrechen“; bei genau einem Feld wird es direkt genommen, bei mehreren `PlaceholderSelectionGui` [V] `gui/workflow/states/OpenState.java:140-181`.
- `PlaceholderSelectionGui`: modales SWT-Fenster 290×195 px, Titel „Info“, Label „Bitte wählen Sie den gewünschten Platzhalter aus der Liste aus:“ (`positioning.placeholder`), `Combo` mit Feldnamen (vorausgewählt: erster), Buttons „Ok“ (liefert Index) / „Cancel“ (liefert −1, Dialogschleife beginnt erneut) – Button-Texte hart kodiert englisch [V] `gui/PlaceholderSelectionGui.java:95-142`.
- Gewählter Feldname wird **persistent** in `~/.pdf-over/cfg/advancedconfig.properties` als `signature_field_name=<name>` gespeichert [V] `OpenState.java:66, 239-252`, `commons/Constants.java:69`.
- PDF-AS: existiert ein Feld dieses Namens ohne Signatur, wird es benutzt (`isAdobeSigForm`); das **vorhandene Widget-Rect bleibt erhalten**, die Erscheinung (für die profilgemäße Breite gebaut) wird per BBox→Rect-Mapping **in das Feld skaliert (ggf. verzerrt)** [V] `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:144-152, 878-897` + [P] `PDDocument.assignSignatureRectangle` (Rect nur gesetzt, wenn keines mit 4 Werten vorhanden).
- [I] Nebenwirkungen: (a) `signature_field_name` bleibt gesetzt und dient danach auch als Basisname neuer Felder (`<name>1`, …); (b) ob die laufende PDF-AS-Instanz den neu geschriebenen Wert sieht, hängt davon ab, ob `PdfAs4Helper.getPdfAs()` vorher schon initialisiert war (Settings werden nur beim Erzeugen geladen) [V] `signer/pdfas/PdfAs4Helper.java:40-58`, `lib/settings/Settings.java:56-63`; (c) im Mehrfeld-Zweig wird nach `setNextState(BKUSelectionState)` am Ende von `run()` noch `setNextState(PositioningState)` aufgerufen `OpenState.java:166-168, 232`.

### 5.4 Ersetzen/Überdecken

Der Platzhalter wird **nicht entfernt oder verändert**. Der Signaturblock wird als Widget-Annotation **darüber** gelegt: linke obere Ecke = Platzhalter-Oberkante links, Breite = Platzhalterbreite, Höhe = Layout-Höhe [V] §5.1/§3.7. Der weiße Tabellenhintergrund verdeckt den QR-Code nur, soweit der Block reicht; ist der Platzhalter höher als der Block, bleibt sein unterer Teil sichtbar; bei BASE_LOGO (transparent) bleibt der QR-Code sichtbar [I]. `/Location` der Signatur = Platzhaltername (markiert ihn als benutzt) [V] `PADESPDFBOXSigner.java:167`.

### 5.5 Platzhalter-Generator (Referenz)

PDF-Over verweist auf `https://pdf.egiz.gv.at/pdf-as-wai/qrCodeGeneration` [V] `PDF-Over/Readme.md:13`. Der PDF-AS-Web-Generator [V] `web/servlets/PlaceholderGeneratorServlet.java:62-210`, `web/helper/QRCodeGenerator.java:20-25`: Inhalt `PDF-AS-POS[;id=<nur Ziffern, max. 5, ohne führende Nullen>][;profile=<p>]`; Bild 300×60 px (Parameter `w`,`h`,`b`), QR (ECC **L**) links mit Kantenlänge `h−b`, rechts Text „Platzhalter für die / elektronische Signatur [/ NR: <id>]“ (EN mit `lang=EN`), Ausgabe **JPEG** (wegen Word-PDF-Export). [I] Ob der öffentliche Dienst genau diesen Servlet nutzt, ist nicht verifiziert.

---

## 6. Vorschau des Signaturblocks

**Erzeugung** [V] `signer/pdfas/PdfAs4SignatureParameter.java:84-126`, `pb2/signing/pdfbox2/PADESPDFBOXSigner.java:752-848`:
1. Gleiche Profil-ID und Overrides wie beim Signieren (§2.8), Zertifikat = Beispielzertifikat `/example.cer` (Name/Seriennummer daraus), aktuelles Datum.
2. Leeres A4-Dokument; Positionierung mit dem **Profil-`pos`** auf diesem A4-Dokument ⇒ Breite: SMALL 230, BASE_LOGO `tw`, **AMTS 495** (A4-Auto).
3. Erscheinung wie beim Signieren gebaut, aber `buildPreviewOnly=true` ⇒ Holder-Stream bleibt leer; die Tabelle steht als **Seiteninhalt** der Vorlagenseite bei (0,1) [V] `PDFAsTemplateCreator.java:147-151`, `PDFAsVisualSignatureBuilder.java:268-278`.
4. Rendern mit `PDFRenderer.renderImageWithDPI(0, 288, ARGB)` (Faktor 4), dann Ausschnitt: Quelle `x∈[0,(W+2)·4]`, `y∈[imgH−(H+1)·4, imgH]` ⇒ Ziel `(int)(W·4) × (int)(H·4)` px, `TYPE_4BYTE_ABGR` [V] `PADESPDFBOXSigner.java:814-839`. ⇒ Das Vorschaubild ist leicht gestaucht (Faktor W/(W+2), H/(H+1)) und hat rechts ~2 pt / unten ~1 pt transparenten Rand [I]. Beispiel SMALL_DE: 920×196 px [B].
5. Fehler ⇒ schwarzes 229×77-Bild [V] `PdfAs4SignatureParameter.java:122-125`. INVISIBLE ⇒ kein Bild [V] `PdfAs4SignaturePlaceholder.java:63-69`.

**Caching/Threading** [V] `signer/pdfas/PdfAs4SignaturePlaceholder.java:15-77`: statischer `TreeMap`-Cache (nie geleert), Schlüssel = (Profil-ID, MD5 des Original-Emblems, Hinweistext, `targetLogoSize`); Erzeugung in eigenem Thread; Callbacks nach Fertigstellung. Erzeugung ist synchron über `PdfAs4Helper.class` serialisiert.

**Verwendung**:
- Positionierung: AWT-Bild an `SignaturePanel` (Größe /4 ⇒ Seiten-pt) [V] `gui/workflow/states/PositioningState.java:118-121`, `gui/composites/SignaturePanel.java:174-183`.
- Einstellungen: SWT-Canvas, Bild seitenverhältnistreu eingepasst und zentriert, Neuberechnung bei Änderung von Profil/Notiz/Sprache/PDF-A/Logo/Logo-Größe (Fokusverlust des Schiebereglers) [V] `gui/composites/configuration/SimpleConfigurationComposite.java:265-267, 396-467`. AWT→SWT-Konvertierung [V] `commons/utils/ImageUtil.java:208-288`.
- Konstante `SIGNATURE_PLACEHOLDER_TRANSPARENCY` existiert, wird aber nirgends ausgewertet [V] `commons/Constants.java:150-151`.

Empfehlung für den Nachbau [I]: Die Vorschau mit **derselben Layout-Engine** wie das Signieren erzeugen (ideal: Erscheinungs-XObject einmal bauen, für Vorschau rendern bzw. als Vektor zeichnen). Die Original-Abweichungen (Stauchung, AMTS-Breite, 120-mm-Klemmung) sind Artefakte und sollten nicht übernommen werden – das reale Ergebnis zählt.

---

## 7. Emblem / Logo

| Aspekt | Verhalten | Fundstelle |
|---|---|---|
| Auswahl | Dateidialog-Filter `*.jpg;*.png;*.gif` / einzeln / `*`; Drag&Drop auf die Vorschau (schaltet zusätzlich auf Profil BASE_LOGO!) | [V] `gui/composites/configuration/SimpleConfigurationComposite.java:296-317, 362-380` |
| Validierung | Datei muss existieren und von SWT `ImageData` ladbar sein | [V] `gui/workflow/config/ConfigurationDataInMemory.java:48-69` |
| CLI | `-e <datei>` (Overlay, nicht persistent) | [V] `gui/cliarguments/EmblemArgument.java:34-57` |
| Laden | `ImageIO` (erster passender Reader); **EXIF-Orientierung nur bei JPEG** | [V] `commons/utils/ImageUtil.java:105-167` |
| EXIF-Mapping | 1: –, 2: Spiegeln, 3: 180°, 4: 180°+Spiegeln, 5: 90° im UZS + Spiegeln, 6: 90° im UZS, 7: 270° im UZS + Spiegeln, 8: 270° im UZS (Spiegeln **nach** Rotation) | [V] `ImageUtil.java:61-71, 138-165` |
| Verkleinerung | max. **480×600 px**, erst Höhe, dann Breite begrenzen, Seitenverhältnis via `(int)`-Trunkierung; Skalierung per `Graphics2D.drawImage` (Default-Interpolation [I] nearest neighbour) in Bildtyp des Originals (Typ 0 ⇒ ARGB) | [V] `signer/Emblem.java:53-54, 89-115` |
| Cache | **eine** Datei `~/.pdf-over/.emblem.png` (immer PNG) + `.emblem.properties` (`IMG`=Originalpfad, `HSH`=MD5-Hex des Originals); Treffer, wenn Pfad und Hash gleich; Fehler beim Schreiben ⇒ Originalpfad wird verwendet | [V] `Emblem.java:46-52, 120-169` |
| Übergabe an PDF-AS | absoluter Pfad des Caches als `value.SIG_LABEL` | [V] `signer/pdfas/PdfAs4Signer.java:53, 63-65` |
| PDF-AS-Laden | relativer Pfad ⇒ `<workdir>/<pfad>`; existiert die Datei nicht ⇒ Versuch, den Wert als **Base64** zu dekodieren | [V] `com/utils/ImageUtils.java:137-213` |
| Einbettung | §3.5 (LosslessFactory, Alpha, Größe) | [V] |
| Ohne Emblem | Profil-Default-Logo (`./images/signatur-logo_de|en.png`, Amtssignatur `amtsignatur-logo_demo.png`) | [V] §2.9 |

[I] Randfälle: Formate, die SWT, aber nicht `ImageIO` lesen kann, scheitern erst beim Cache-Aufbau (NPE, nicht abgefangen); große GIFs werden beim Verkleinern in `TYPE_BYTE_INDEXED` mit Standardpalette umgerechnet (Farbverlust); transparente indizierte Bilder verlieren in PDF-AS die Transparenz (§3.5).

---

## 8. Hinweise für die Neuimplementierung (nicht-Java)

1. **Profil-Engine**: Java-`.properties`-Parser (ISO-8859-1, Escapes, Fortsetzungszeilen), Include mit Wildcards, schlüsselweise Parent-Kopie (§2.1). Die mitgelieferten Profile können 1:1 weiterverwendet werden (EUPL-Lizenz beachten).
2. **Layout-Engine** als reine Funktion `(Profil, Werte, Breite, Fontmetriken, Bildmaße) → Zellgeometrie + Zeichenoperationen` in float32 (§3). Testbar gegen die Werte aus §3.8.
3. **Fontmetriken**: AFM-Breiten der Standard-14-Fonts (Helvetica-Bold, ggf. Courier/Times) einbetten; DejaVu-TTF-Breiten gerundet wie PDFBox; Descent aus `hhea`.
4. **PDF-Writer** muss können: Type1-Standard-14 (WinAnsi), Type0/CIDFontType2 Identity-H mit Subset + ToUnicode, Flate-Bilder mit SMask, Form-XObjects mit BBox/Matrix, Widget-Annotation im AcroForm, inkrementelles Update.
5. **Renderer** für (a) Positionierungs-UI, (b) Freiraum-Raster-Scan (§4.4) und (c) optional Vorschau – auf allen Zielplattformen nötig.
6. **Bildverarbeitung**: PNG/JPEG/GIF dekodieren, EXIF-Orientierung, Verkleinerung, PNG-Cache.
7. **QR-Decoder** (ZXing-kompatibel) für Platzhalter; Bild-XObjects aus Content-Streams extrahieren inkl. CTM.

---

## 9. Offene Fragen

1. Gibt IAIK `Name.getName()` die Titel-RDN als `title=` oder `T=` aus (⇒ wirkt `subject.title` in den Profilen überhaupt)? Enthalten ID-Austria/A-Trust-Zertifikate einen Titel?
2. Werden die DejaVu-Fonts in der Ausgabe als Subset eingebettet (Fonts werden auf dem Originaldokument geladen, die Vorlage aber separat gespeichert)? Visuell irrelevant, für PDF/A und Dateigröße relevant – Laufzeittest empfohlen.
3. Genaues Ergebnis von `removeAlphaChannel` (Java2D `AlphaComposite.Src` auf `TYPE_INT_RGB`) für transparente Pixel: schwarz oder gespeicherte RGB-Werte?
4. Rendern alle relevanten Viewer (Acrobat, pdf.js, Apple PDFKit, PDFium) die `W+1/H+1`-BBox wie spezifiziert gestaucht? (Nach PDF-Spezifikation ja.)
5. Soll der Klon die bekannten Fehler übernehmen (AMTS-Breite bei manueller Position, Integer-`==`, 120-mm-Klemmung nur in der Vorschau, `/TU` leer, CropBox-Offset) oder bewusst korrigieren? Für „optisch identisch“ betreffen nur AMTS-Breite und CropBox-Offset das Ergebnis.
6. Auto-Positionierung auf gedrehten Seiten: liefert der Raster-Scan dort korrekte Werte? (Test mit 90°-Seiten nötig.)
7. Wird `signature_field_name` beim Feld-Workflow in derselben Sitzung wirksam (Zeitpunkt der PDF-AS-Initialisierung)?

---

## 10. Quellen (wichtigste Dateien)

- PDF-AS Layout/Stamping: `pb2/stamping/pdfbox2/PDFBoxTable.java`, `TableDrawUtils.java`, `PDFBoxFont.java`, `PDFAsFontCache.java`, `PDFAsVisualSignatureBuilder.java`, `PDFAsVisualSignatureDesigner.java`, `PDFAsVisualSignatureProperties.java`, `PDFAsTemplateCreator.java`, `PdfBoxVisualObject.java`
- Modell/Profile: `libk/table/Table.java`, `Entry.java`, `Style.java`, `lib/impl/stamping/TableFactory.java`, `ValueResolver.java`, `CertificateAndRequestParameterResolver.java`, `lib/settings/Settings.java`, `com/settings/SignatureProfileSettings.java`, `DefaultSignatureProfileSettings.java`, `Profiles.java`, `com/utils/OgnlUtils.java`, `DNUtils.java`, `StringUtils.java`, `ImageUtils.java`
- Positionierung: `pb2/pdfbox2/positioning/Positioning.java`, `PositioningPageDrawer.java`, `PositioningRenderer.java`, `pb2k/pdf/PDFUtilities.java`, `libk/pdf/TablePos.java`, `PositioningInstruction.java`
- Signer/Vorschau: `pb2/signing/pdfbox2/PADESPDFBOXSigner.java`, `lib/impl/PdfAsImpl.java`
- Platzhalter: `pb2/pdfbox2/placeholder/SignaturePlaceholderExtractor.java`, `SignatureFieldsAndPlaceHolderExtractor.java`, `lib/impl/placeholder/*`, `web/servlets/PlaceholderGeneratorServlet.java`
- PDF-Over: `signer/pdfas/PdfAs4SignatureParameter.java`, `PdfAs4Signer.java`, `PdfAs4SignaturePlaceholder.java`, `signer/Emblem.java`, `SignaturePosition.java`, `commons/utils/ImageUtil.java`, `commons/Profile.java`, `commons/Constants.java`, `gui/composites/PositioningComposite.java`, `SignaturePanel.java`, `gui/workflow/states/PositioningState.java`, `OpenState.java`, `PrepareSigningState.java`, `gui/PlaceholderSelectionGui.java`, `gui/composites/configuration/SimpleConfigurationComposite.java`
- Konfiguration: `cfg/config.properties`, `cfg/advancedconfig.properties`, `cfg/profiles/*.properties`, `cfgzip/fonts/*.ttf`, `cfgzip/images/*.png`
- Doku: `pdf-as-4/doc/PDFAS4_Profile_Dokumentation.docx` (Text aus DOCX extrahiert, inhaltsgleich zur PDF), `PDFAS4_Dokumentation.docx` §3.6–3.7, `PDFAS4_WebDokumentation.docx` (QR-Generator)
- PDFBox 2.0.34 (Bytecode/AFM, nur gelesen): `TrueTypeEmbedder`, `PDCIDFontType2Embedder`, `PDCIDFont`, `PDType0Font.load`, `PDPage.getBBox`, `PDFRenderer.renderPageToGraphics`, `PDDocument.assignSignatureRectangle`, `resources/afm/*.afm`

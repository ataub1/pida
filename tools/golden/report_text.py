"""Fester Text von docs/integration/golden-report.md (Tabellen ergänzt report.py)."""

TEMPLATE = r"""# Golden-Bericht: Signaturblock und Positionierung gegen PDF-AS 4.4.2

Paket I5 (Welle 2), Plan §8.2 „Golden Files gegen PDF-AS“. Erzeugt von `tools/golden/report.py`
aus dem Golden-Lauf und den Ergebnissen der Rust-Golden-Tests; die Tabellen nicht von Hand ändern
(fester Text: `tools/golden/report_text.py`).

## 1. Aufbau des Orakels

| Baustein | Inhalt |
|---|---|
| `tools/golden/run-pdfas.sh` | Lädt die **offizielle PDF-AS-4.4.2-CLI** (`apps.egiz.gv.at/releases/pdf-as/release/4.4.2/pdf-as-cli-4.4.2.zip`, SHA-256 fest im Skript, Cache `target/golden/`), erzeugt die Eingaben, signiert alle Fälle und schreibt die Manifeste. Optionen `--only <regex>`, `--jobs N`, `--keep-work`, `--reextract`. |
| Signieren | CLI `pdf-as-cli -m sign -c ks -ksf testdata/keystores/test-rsa2048.p12 -ksa test-rsa2048 …` (Keystore-Option, RSA-Test-PKCS#12, feste Zertifikatsdaten), Konfiguration = Kopie von `$PDFOVER_REFERENCE/pdfas-config` je Variante, JVM mit `-Duser.timezone=Europe/Vienna`. |
| PDF-Over-Overrides | wie `PdfAs4Signer.prepare`: `value.SIG_LABEL` (Bildmarke), `value.SIG_NOTE` (Hinweis), bei `BASE_LOGO` `table.main.Style.padding=0`, `pos=w:<tw>;f:0`, `table.main.Style.imagescaletofit=<tw>;<th>` (aus Emblemgröße und `targetLogoSize`, Default 23 mm) – als Zeilen am Ende der Profildatei der Konfigurationskopie. |
| QR-Platzhalter | `tools/golden/PlaceholderSign.java`: dieselbe Signatur wie `Main.perform_sign`, zusätzlich wie PDF-Over `setPlaceHolderSearchEnabled(true)`, `placeholder_mode=1`, Platzhalter-ID 1 bzw. 200. Die CLI selbst kann die Platzhaltersuche nicht einschalten (`SignParameter.isPlaceHolderSearchEnabled()` ist dort immer `false`). QR-Module mit ZXing aus der CLI-Distribution (`MakeQr.java`). |
| Eingaben (`make_inputs.py`, deterministisch) | `a4` (595,32 × 841,92), `a4-landscape`, `a4-rot90/180/270` (`/Rotate`), `a4-cropbox` (CropBox ≠ MediaBox, Ursprung 40/60), `content-half`/`content-full` (viel Inhalt: Auto-Position darunter bzw. neue Seite), `two-pages` (letzte Seite A5 quer), `presigned` (von PDF-AS vorsigniert), `qr`/`qr-id200` (Platzhalter 300 × 60 px auf 225 × 45 pt). Seiteninhalt nur aus Rechtecken, damit der Rasterscan nicht von der Schriftrasterung abhängt. Bildmarken: breit (RGB), hoch (Graustufen), RGBA mit Transparenz, JPEG, indiziert mit `tRNS`. |
| Positionen | `p:auto;x:auto;y:auto` (PDF-Over Auto), `p:1;x:100;y:700`, `p:1;x:50.500000;y:300.250000` (PDF-Over-Format `p:%d;x:%f;y:%f`), `p:new`, `p:2;x:100;y:700` (PDF-Over „Neue Seite“), `p:last`, `f:0`, `f:300`, `w:300`, `r:90`, ohne Position (nur Profil-`pos`), Platzhalter. |
| `tools/golden/extract.py` | Manifest je Fall aus `qpdf --json=2` (dekodierte Streams): Seite, Widget-`/Rect`, `/TU`, Holder-`/BBox`/`/Matrix`, Operationen des Inner-Forms (Rechtecke, Linien, Linienbreiten, Bildmatrizen, Textblöcke mit Font, Größe, Zeilenstart, kodierten Bytes und Unicode), Fonts (Typ, Einbettung, Descriptor, `/W`), Bilder (Maße, Farbraum, SHA-256 der Samples, SMask), `SIG_DATE`; dazu `qpdf --check`, pyHanko-Validierung (Testzertifikat als Vertrauensanker) und Werte aus dem PDF-AS-DEBUG-Log (`PositioningInstruction`, feste Tabellenbreite, Platzhalterposition). |
| Ablage | `testdata/golden/pdfas-4.4.2/`: `index.json`, `inputs/`, `cases/<Fall>/manifest.json`; für 84 Fälle (alle Profile auf A4 manuell, alle Hinweis- und Bildmarken-Varianten, Rotation/CropBox/QR) zusätzlich `signed.pdf` für den Pixelvergleich (gesamt rund 11 MB). |
| Zeitstempel | Der Lauf hat keine feste Uhr; normalisiert wird beim Vergleich: der Nachbau verwendet genau den `SIG_DATE`-Text aus dem PDF-AS-Block (`yyyy-MM-dd'T'HH:mm:ssXXX`, Europe/Vienna). |

<!-- TABELLE:KENNZAHLEN -->

Tests (lesen nur `testdata/golden/`, kein Java, kein Netz; PDFium-Teile nur mit `PDFOVER_PDFIUM_DIR`):

- `crates/sigblock/tests/golden_pdfas442.rs`: `index_covers_all_profiles_and_oracles_pass`,
  `blocks_match_pdfas_442` (Struktur ±0,01 pt), `content_bytes_identical_with_quirks`,
  `pixels_match_pdfas_442` (PDFium, 288 dpi), `own_blocks_pass_qpdf_check` (`qpdf --check` auf den
  eigenen Mini-PDFs aller A4-Varianten, Standard und Quirks).
- `crates/placement/tests/golden_pdfas442.rs`: `pdfas_quirks_mode_matches_pdfas_442`,
  `corrected_mode_deviates_only_where_documented`, `invisible_profile_has_empty_rect`.

Neu erzeugen: `bash tools/golden/run-pdfas.sh` (etwa 5 Minuten bei 8 parallelen JVMs), danach die
Golden-Tests mit `PDFOVER_GOLDEN_REPORT_DIR=target/golden/report` und
`python3 tools/golden/report.py --golden testdata/golden/pdfas-4.4.2 --reports target/golden/report
--out docs/integration/golden-report.md`.

## 2. Signaturblock

Nachbau je Fall mit `pdfover-sigblock`: eingebaute Profile, PDF-Over-Overrides
(`ProfileOverrides`, bei BASE_LOGO `ProfileOverrides::base_logo`), Testzertifikat, `SIG_DATE` aus
dem Block, Breite = von PDF-AS berechnete feste Tabellenbreite, Rotation aus der Holder-`/Matrix`,
PDF/A-Profile mit `PdfaMode::Part1`. Verglichen werden Blockmaße (`/BBox` von Holder und
Inner-Form), alle Zeichenoperationen (Rechtecke, Linien, Linienbreite, Bildmatrix, Textblöcke:
Font ohne Subset-Präfix, Größe, Startpunkt jeder Zeile ±0,01 pt, kodierte Bytes exakt), Fonts
(Subtype, Encoding, Einbettung, Descent, `CIDSet`, `/W` aller verwendeten Glyphen) und Bilder
(Pixelmaße, Farbraum, SHA-256 der dekodierten Samples, SMask), im Standardmodus und mit
`pdfas_quirks`.

<!-- TABELLE:SIGBLOCK -->

**Ergebnis:** Geometrie, Zeilenumbruch, Texte, Fonts, Linien und Bildboxen stimmen in allen Fällen
überein (größte Abweichung 2·10⁻⁶ pt, Rundung der 5-Nachkommastellen-Ausgabe). Mit `pdfas_quirks`
ist der Content-Stream des Inner-Forms in allen Fällen außer SB-G1 **byte-gleich** (inklusive
`/DeviceRGB cs … sc`, Zahlenformat, Literal-/Hex-Strings mit CIDs), ebenso `/TU` und der
Holder-Inhalt. Alle verbleibenden Unterschiede betreffen Bildmarken und sind dokumentiert:

| ID | Thema | PDF-AS 4.4.2 (Golden-Befund) | PDF-Over NG | sichtbar? |
|---|---|---|---|---|
| SB-G1 | Logo > 120 mm (BASE_LOGO, Register „120-mm-Grenze“) | PDF-Over klemmt `targetLogoSize` nur in der Vorschau; beim Signieren mit 150 mm ⇒ Block 425 pt | Klemmung auch beim Signieren (120 mm ⇒ 340 pt), Vorschau = Ergebnis | ja (gewollt) |
| SB-G2 | voll deckender Alphakanal (z. B. `amtsignatur-logo_demo.png`, RGBA mit Alpha 255) | `LosslessFactory` schreibt immer eine SMask (lauter 255) | keine SMask (`Emblem::has_alpha` = „hat Transparenz“) | nein (pixelgleich) |
| SB-G3 | indizierte Transparenz (PNG-Palette mit `tRNS`, GIF) | Transparenz verworfen (`hasAlpha` ohne Alpha-Raster), Palettenfarbe sichtbar | als SMask erhalten | an Kanten und auf farbigem Grund |
| SB-G4 | Graustufen-Bildmarke | DeviceGray mit Java-Umrechnung `CS_GRAY` (linear) → sRGB: Wert = sRGB-OETF(Original), z. B. 20 → 79, 45 → 117, 120 → 182, 220 → 239 (Bild wird heller); bei PDF/A-Profilen RGB mit Originalwerten | DeviceRGB mit Originalwerten (wie PDF-AS bei PDF/A) | ja (PDF-AS hellt auf) |
| SB-G5 | JPEG-Bildmarke | ImageIO (libjpeg; Ergebnis identisch mit libjpeg-turbo) | `zune-jpeg`: Samples weichen um wenige Stufen ab | nein (≤ 1 Stufe nach dem Rendern) |
| SB-G6 | PDF/A-1 ohne Alpha (transparente Bildmarke in PDF/A-Profil) | `removeAlphaChannel` behält die gespeicherten RGB-Werte: voll transparente Pixel zeigen ihre gespeicherte Farbe (meist Schwarz) – bisher offene Frage (analyse/04 §9 Nr. 3, Welle-1-Bericht) | Standard: Komposition auf Weiß; **mit `pdfas_quirks` jetzt exakt wie PDF-AS** | ja (PDF-AS: schwarze Flächen) |

SB-G2 bis SB-G4 ließen sich exakt nachbilden, wenn `Emblem` den Farbtyp der Quelle trüge (Gray,
Indexed, Alphakanal vorhanden); das wäre eine Änderung der öffentlichen Felder von `Emblem`
(Strukturliterale in bestehenden Tests) und ist deshalb hier nicht umgesetzt, sondern als offener
Punkt vermerkt. Für SB-G4 liefert der Golden-Lauf die exakte Abbildung (sRGB-OETF, alle Stufen
getroffen).

**Pixelvergleich** (PDFium, 288 dpi): Block der PDF-AS-Ausgabe (Holder-`/BBox` mit `/Matrix` wie im
Viewer auf das Rect abgebildet) gegen den eigenen Block auf demselben Weg sowie gegen
`Appearance::to_standalone_pdf`; Diff-Quote = Anteil abweichender Pixel.

<!-- TABELLE:PIXEL -->

Ohne dokumentierte Abweichung ist der Block in allen Referenz-PDFs **pixelgleich** (Diff-Quote 0),
auch über `to_standalone_pdf`. Diff-Quoten der dokumentierten Fälle: SB-G4 (Aufhellung, bis 8,2 %
der Blockpixel, max. 73 Stufen; bei BASE_LOGO, wo die Bildmarke den ganzen Block füllt, bis 59 %),
SB-G3 (Kanten der Transparenz), SB-G6 im Standardmodus (weiße statt schwarzer Flächen; mit Quirks 0
bis auf SB-G5), SB-G5 (≤ 1 Stufe), SB-G1 (andere Blockgröße, Quote 100 %).

**Behoben in `crates/sigblock/src`:**

1. `Appearance::to_standalone_pdf` zeichnet jetzt wie ein Viewer über das Holder-Form (Seite → Holder
   mit `/Matrix` → Inner-Form) und schreibt die Abbildung mit voller f32-Genauigkeit. Vorher
   (Seite → Inner-Form, 5 Nachkommastellen) rasterte PDFium die Bildmarke mit anderer Rundung:
   bis 17,6 % abweichende Pixel (BASE_LOGO, max. 227 Stufen), 0,2–2 % bei den übrigen Profilen;
   jetzt 0. Bei `preview_only` (Holder ohne Inhalt) bleibt der direkte Weg.
2. `pdfas_quirks` + PDF/A-1: Alphakanal wird wie `removeAlphaChannel` entfernt (gespeicherte
   RGB-Werte statt Komposition auf Weiß); SB-G6 mit Quirks exakt (Unit-Test
   `pdfa1_alpha_removal_default_and_quirk`).

## 3. Positionierung

Nachgerechnet mit `pdfover-placement` (Seiten, Felder und Bildaufrufe über `pdfover-pdf-cos` –
Gegentest zum lopdf-Mini-COS der übrigen Placement-Tests): `TablePos::merge` aus Request-Position
und Profil-`pos` (aus dem PDF-AS-Log), `resolve_width`, `resolve_position` (Rasterscan mit PDFium),
`widget_rect_with`, `appearance_rotation`, `new_page_geometry`, QR-Platzhalter mit
`find_qr_placeholders` + `select_placeholder` (id 1 bzw. 200, MODERATE). Blockhöhe aus der
PDF-AS-BBox. Toleranz: ±0,01 pt (manuell, `p:new`, `p:last`, Platzhalter), ±1 pt bei Rasterscan
(Plan §8.2).

**`CompatMode::PdfasQuirks`:**

<!-- TABELLE:PLACEMENT_QUIRKS -->

**`CompatMode::Corrected` (Standard):**

<!-- TABELLE:PLACEMENT_CORRECTED -->

Breite, Seite, x, y, Rect, neue Seite (Index, MediaBox, `/Rotate`) und Rotation der Erscheinung
stimmen im Quirk-Modus in allen Fällen; Ausnahme KR-F03 (keine Quirk-Nachbildung der
Platzhalterauswahl). Im korrigierten Modus weichen nur die Register-Fälle ab, und zwar genau in der
dokumentierten Form (der Test prüft die korrigierten Werte):

| ID | Thema | PDF-AS 4.4.2 | PDF-Over NG (Corrected) | Fälle |
|---|---|---|---|---|
| KR-F01 | AMTS-Breite bei manuellem `x` ohne `w` (Register „AMTS-Breite“) | `w = pw − 2x` (A4, x = 100 ⇒ 395,32) | `w = pw − 100` wie bei Auto-Position und in der Vorschau | AMTS × manuell |
| KR-F02 | CropBox ≠ MediaBox (Register „CropBox-Offset“) | Rect ohne CropBox-Ursprung; neue Seite mit CropBox-Maßen der alten Seite | Rect relativ zur CropBox (+40/+60); neue Seite mit MediaBox-Geometrie (700 × 1000 ⇒ y = 980) | alle `a4-cropbox` |
| KR-F03 | Platzhalter-ID > 127 (Register „IDs > 127“) | id = 200 wird nie gefunden (`Integer ==`), MODERATE ⇒ Auto-Position | Platzhalter gefunden, Block am Platzhalter (300/195, w = 225) | alle `qr-id200` |
| KR-F04 | Rasterscan gedrehter Seiten | Rasterbild in ungedrehten Maßen, transparente Zeilen zählen als Inhalt ⇒ A4 mit `/Rotate 90/270` und `f:80` **immer neue Seite** | Scan im sichtbaren Raum, Block unter dem Inhalt | `a4-rot90/270` × auto |
| KR-F10 | Rasterscan-Rundung | Java2D, Seite 1:1 auf `(int) w × (int) h` | PDFium, Seite auf `round(w) × round(h)` abgebildet: bei Seitenhöhen mit Nachkommaanteil ≥ 0,5 (A4 841,92) liegt die Auto-Position genau 1 pt tiefer, sonst exakt | innerhalb ±1 pt |

**Behoben in `crates/placement/src`:** KR-F04 im Modus `PdfasQuirks` nachgebildet
(`pdfas_rotated_scan`: liegt die Startzeile `(int) h − 1 − (int) f` in einer transparenten Zeile –
bzw. bei 90° mit transparenter Spalte 0 immer –, ist sie das Ergebnis ohne Scan; aus der
PDFBox-2.0.32-Logik `renderPageToGraphics` + `clearRect` abgeleitet und am Golden bestätigt,
Unit-Test `pdfas_rotated_scan_like_pdfas`). Vorher 22 Fälle abweichend, jetzt 0. Register-Einträge
KR-F04/KR-F10 in der Crate-Doku präzisiert: Die 1-pt-Abweichung aus Welle 1 kommt von der
Rasterabbildung in `pdfover-render`, nicht von der Glyphenrasterung (gemessen: ein 7 pt hoher Balken
mit Kanten auf Pixelgrenzen belegt bei A4 hoch 8 Zeilen, bei A4 quer – 595,32 ⇒ 595 px, gestaucht –
7 Zeilen).

## 4. Weitere Befunde

- Alle PDF-AS-Ausgaben: pyHanko `intact`/`valid`/`trusted`, Abdeckung `ENTIRE_FILE`,
  `qpdf --check` ohne Befund.
- `/TU`: SMALL ohne Hinweis leer, mit Hinweis bzw. AMTS nur die Texte der äußeren Tabelle (Q10
  bestätigt); `INVISIBLE` setzt `/TU` = Feldname und `/Rect [0 0 0 0]`.
- Neue Seite: PDF-AS setzt `make_new_page` auch bei `p:2` auf einseitigen Dokumenten; auf dem
  vorsignierten Dokument wird nie eine Seite angehängt (Auto ⇒ letzte Seite, `y = ph − 20`).
- `p:last` ignoriert die Freiraumsuche (`y = ph − 20`); `f:0` und `f:300` wirken wie in
  analyse/04 §4.3 beschrieben (AMTS auf `content-half` mit `f:300` ⇒ neue Seite).
- Rotierte Seiten: Holder-`/Matrix` = Blockrotation + Seitenrotation; PDF-AS schreibt bei 90/180/270
  ein nicht normalisiertes Rect (gleiche Kanten).

## 5. Offene Punkte und Schnittstellenwünsche

- **pdfover-render (Integration/Paket F):** `render_forced_color` bildet die Seite auf
  `round(w) × round(h)` ab (`FPDF_RenderPageBitmapWithColorScheme_Start` mit gerundeter Fläche) statt
  1:1 wie Java2D; dadurch liegt die Auto-Position bei A4 (841,92 pt) genau 1 pt tiefer als bei
  PDF-AS (KR-F10, innerhalb Plan §8.2). Exakt würde es mit einer Abbildung ohne Streckung (Bitmap
  `(int) w × (int) h`, Seitenfläche `w × h` über eine Matrix statt einer gerundeten Fläche).
- **pdfover-sigblock `Emblem`:** Farbtyp der Quelle (Graustufen, indiziert, Alphakanal vorhanden)
  fehlt; damit wären SB-G2 bis SB-G4 im Quirk-Modus exakt nachbildbar (SB-G4: sRGB-OETF).
- **Platzhalterauswahl:** kein Quirk-Modus für KR-F03 (`PlaceholderQuery` hat keinen `CompatMode`);
  bewusst korrigiert.
- Der Golden-Lauf hat keine feste Uhr (dafür wäre ein Java-Agent nötig); `SIG_DATE` wird beim
  Vergleich übernommen. Die PDF-AS-Ausgaben ändern sich daher bei jedem Neulauf (Manifeste stabil
  bis auf Datum, Signatur und Subset-Präfixe).
- Neue Meldungs-IDs: keine.

## 6. Tabelle je Fall

Spalten: Seite (1-basiert, „neu“ = angehängte Seite) und Rect wie von PDF-AS geschrieben
(normalisiert), Block W × H aus der Inner-BBox, Signaturblock = Strukturvergleich im Standardmodus,
Bytes = Content-Stream mit `pdfas_quirks`, Pixel = Diff-Quote (nur Fälle mit Referenz-PDF), Position
= Ergebnis der Placement-Tests je Modus (Scan: Δy gegen PDF-AS).

<!-- TABELLE:FAELLE -->
"""

# Golden-Bericht: Signaturblock und Positionierung gegen PDF-AS 4.4.2

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

| Größe | Wert |
|---|---|
| PDF-AS | 4.4.2 (offizielle CLI-Distribution, SHA-256 `09c37cc004f14537e5d0d3d83e44687a833119a5903dd03b6de5d9d907e18eee`) |
| Java (nur Orakel) | openjdk version "21.0.12.1" 2026-08-18 |
| qpdf / pyHanko / Python | qpdf version 12.4.2 / 0.37.0 / 3.13.16 |
| Lauf | 2026-10-15T05:16:28Z, Zeitzone Europe/Vienna |
| Fälle | 537 (12 Profil-IDs, 12 Eingaben, 12 Positionsarten) |
| Orakel je Ausgabe | pyHanko intact + valid + trusted und `qpdf --check` ohne Befund: 537 von 537 |
| signierte Referenz-PDFs im Repository | 84 (übrige Fälle nur als Manifest) |

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

| Profil | Fälle | Struktur gleich | dokumentiert abweichend (Register) | undokumentiert | max. Abweichung Koordinaten | Content-Stream byte-gleich (Quirks) | Pixel: PDFs / pixelgleich |
|---|---|---|---|---|---|---|---|
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED` | 46 | 2 | 44 (SB-G2, SB-G3, SB-G4, SB-G5) | 0 | 0,000002 pt | 46 | 8 / 5 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED` | 46 | 2 | 44 (SB-G2, SB-G3, SB-G4, SB-G5) | 0 | 0,000002 pt | 46 | 8 / 5 |
| `BASE_LOGO` | 46 | 42 | 4 (SB-G1, SB-G3, SB-G4, SB-G5) | 0 | 0 pt | 45 | 8 / 4 |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA` | 46 | 44 | 2 (SB-G5, SB-G6) | 0 | 0,000001 pt | 46 | 8 / 6 |
| `SIGNATURBLOCK_SMALL_DE_NOTE` | 46 | 43 | 3 (SB-G3, SB-G4, SB-G5) | 0 | 0,000001 pt | 46 | 8 / 5 |
| `SIGNATURBLOCK_SMALL_DE_PDFA` | 44 | 42 | 2 (SB-G5, SB-G6) | 0 | 0,000001 pt | 44 | 6 / 4 |
| `SIGNATURBLOCK_SMALL_DE` | 44 | 41 | 3 (SB-G3, SB-G4, SB-G5) | 0 | 0,000001 pt | 44 | 9 / 6 |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA` | 46 | 44 | 2 (SB-G5, SB-G6) | 0 | 0,000001 pt | 46 | 8 / 6 |
| `SIGNATURBLOCK_SMALL_EN_NOTE` | 46 | 43 | 3 (SB-G3, SB-G4, SB-G5) | 0 | 0,000001 pt | 46 | 8 / 5 |
| `SIGNATURBLOCK_SMALL_EN_PDFA` | 44 | 42 | 2 (SB-G5, SB-G6) | 0 | 0,000001 pt | 44 | 6 / 4 |
| `SIGNATURBLOCK_SMALL_EN` | 44 | 41 | 3 (SB-G3, SB-G4, SB-G5) | 0 | 0,000001 pt | 44 | 6 / 3 |

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

| Profil | PDFs | Diff-Quote ohne dokumentierte Abweichung (Standard / Quirks / `to_standalone_pdf`) | Diff-Quote Fälle mit dokumentierter Abweichung (Standard / Quirks / `to_standalone_pdf`) |
|---|---|---|---|
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED` | 8 | 0,0000 % / 0,0000 % / 0,0000 % | 5,9907 % / 5,9907 % / 5,9907 % |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED` | 8 | 0,0000 % / 0,0000 % / 0,0000 % | 5,9907 % / 5,9907 % / 5,9907 % |
| `BASE_LOGO` | 8 | 0,0000 % / 0,0000 % / 0,0000 % | 100,0000 % / 100,0000 % / 100,0000 % |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA` | 8 | 0,0000 % / 0,0000 % / 0,0000 % | 2,0493 % / 0,0073 % / 2,0493 % |
| `SIGNATURBLOCK_SMALL_DE_NOTE` | 8 | 0,0000 % / 0,0000 % / 0,0000 % | 6,3200 % / 6,3200 % / 6,3200 % |
| `SIGNATURBLOCK_SMALL_DE_PDFA` | 6 | 0,0000 % / 0,0000 % / 0,0000 % | 3,3784 % / 0,0122 % / 3,3784 % |
| `SIGNATURBLOCK_SMALL_DE` | 9 | 0,0000 % / 0,0000 % / 0,0000 % | 8,2339 % / 8,2339 % / 8,2339 % |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA` | 8 | 0,0000 % / 0,0000 % / 0,0000 % | 2,0493 % / 0,0073 % / 2,0493 % |
| `SIGNATURBLOCK_SMALL_EN_NOTE` | 8 | 0,0000 % / 0,0000 % / 0,0000 % | 6,3200 % / 6,3200 % / 6,3200 % |
| `SIGNATURBLOCK_SMALL_EN_PDFA` | 6 | 0,0000 % / 0,0000 % / 0,0000 % | 2,2859 % / 0,0089 % / 2,2859 % |
| `SIGNATURBLOCK_SMALL_EN` | 6 | 0,0000 % / 0,0000 % / 0,0000 % | 8,2339 % / 8,2339 % / 8,2339 % |

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

| Eingabe | Fälle | gleich | davon Rasterscan (exakt / 1 pt) | dokumentiert abweichend (Register) | undokumentiert | übersprungen | max. Abweichung (pt) |
|---|---|---|---|---|---|---|---|
| `a4-cropbox` | 33 | 33 | 11 (0 / 11) | 0 (–) | 0 | 0 | 1 |
| `a4-landscape` | 33 | 33 | 11 (11 / 0) | 0 (–) | 0 | 0 | 0 |
| `a4-rot180` | 33 | 33 | 11 (0 / 11) | 0 (–) | 0 | 0 | 1 |
| `a4-rot270` | 33 | 33 | 0 (0 / 0) | 0 (–) | 0 | 0 | 0 |
| `a4-rot90` | 33 | 33 | 0 (0 / 0) | 0 (–) | 0 | 0 | 0 |
| `a4` | 179 | 179 | 33 (0 / 33) | 0 (–) | 0 | 0 | 1 |
| `content-full` | 33 | 33 | 22 (12 / 10) | 0 (–) | 0 | 0 | 1 |
| `content-half` | 33 | 33 | 33 (2 / 31) | 0 (–) | 0 | 0 | 1 |
| `presigned` | 33 | 33 | 22 (6 / 16) | 0 (–) | 0 | 0 | 1 |
| `qr-id200` | 11 | 0 | 0 (0 / 0) | 11 (KR-F03) | 0 | 0 | 0 |
| `qr` | 11 | 11 | 0 (0 / 0) | 0 (–) | 0 | 0 | 0 |
| `two-pages` | 33 | 33 | 11 (0 / 11) | 0 (–) | 0 | 0 | 1 |

**`CompatMode::Corrected` (Standard):**

| Eingabe | Fälle | gleich | davon Rasterscan (exakt / 1 pt) | dokumentiert abweichend (Register) | undokumentiert | übersprungen | max. Abweichung (pt) |
|---|---|---|---|---|---|---|---|
| `a4-cropbox` | 33 | 0 | 0 (0 / 0) | 33 (KR-F01, KR-F02) | 0 | 0 | 0 |
| `a4-landscape` | 33 | 31 | 11 (11 / 0) | 2 (KR-F01) | 0 | 0 | 0 |
| `a4-rot180` | 33 | 31 | 11 (0 / 11) | 2 (KR-F01) | 0 | 0 | 1 |
| `a4-rot270` | 33 | 20 | 0 (0 / 0) | 13 (KR-F01, KR-F04) | 0 | 0 | 0 |
| `a4-rot90` | 33 | 20 | 0 (0 / 0) | 13 (KR-F01, KR-F04) | 0 | 0 | 0 |
| `a4` | 179 | 157 | 33 (0 / 33) | 22 (KR-F01) | 0 | 0 | 1 |
| `content-full` | 33 | 33 | 22 (12 / 10) | 0 (–) | 0 | 0 | 1 |
| `content-half` | 33 | 33 | 33 (2 / 31) | 0 (–) | 0 | 0 | 1 |
| `presigned` | 33 | 31 | 22 (6 / 16) | 2 (KR-F01) | 0 | 0 | 1 |
| `qr-id200` | 11 | 0 | 0 (0 / 0) | 11 (KR-F03) | 0 | 0 | 0 |
| `qr` | 11 | 11 | 0 (0 / 0) | 0 (–) | 0 | 0 | 0 |
| `two-pages` | 33 | 31 | 11 (0 / 11) | 2 (KR-F01) | 0 | 0 | 1 |

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

| Fall | Seite (PDF-AS) | Rect (PDF-AS) | Block W × H | Signaturblock | Bytes (Quirks) | Pixel (Standard / Quirks) | Position Quirks | Position korrigiert |
|---|---|---|---|---|---|---|---|---|
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__auto` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__beyond` | 2 neu | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__footer0` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__last` | 1 | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual__emblem-alpha` | 1 | [100 572 495,32 700] | 395,32 × 128 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual__emblem-indexed` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G3 | gleich | 0,0524 % / 0,0524 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual__emblem-photo` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G5 | gleich | 0,0062 % / 0,0062 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual__emblem-tall` | 1 | [100 515 495,32 700] | 395,32 × 185 | dok. SB-G4 | gleich | 5,9907 % / 5,9907 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual__emblem-wide` | 1 | [100 572 495,32 700] | 395,32 × 128 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual__note-custom` | 1 | [100 588 495,32 700] | 395,32 × 112 | dok. SB-G2 | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__manual__note-long` | 1 | [100 548 495,32 700] | 395,32 × 152 | dok. SB-G2 | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__new` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__none` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__pdfover` | 1 | [50,5 180,25 544,82 300,25] | 494,32 × 120 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__rot90` | 1 | [-28 372 100 767,32] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4__width` | 1 | [100 548 400 700] | 300 × 152 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-cropbox__auto` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-cropbox__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-cropbox__new` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | dok. KR-F02 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-landscape__auto` | 1 | [50 285,32 791,92 414,32] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-landscape__manual` | 1 | [100 585 741,92 700] | 641,92 × 115 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-landscape__new` | 2 neu | [50 446,32 791,92 575,32] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot180__auto` | 1 | [50 279 545,32 399] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot180__manual` | 1 | [100 141,92 495,32 269,92] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot180__new` | 2 neu | [50 20 545,32 140] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot270__auto` | 2 neu | [446,32 50 575,32 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | dok. KR-F04 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot270__manual` | 1 | [585 100 700 741,92] | 641,92 × 115 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot270__new` | 2 neu | [446,32 50 575,32 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot90__auto` | 2 neu | [20 50 149 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | dok. KR-F04 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot90__manual` | 1 | [-104,68 100 10,32 741,92] | 641,92 × 115 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__a4-rot90__new` | 2 neu | [20 50 149 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__content-full__auto` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__content-full__footer0` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__content-full__last` | 1 | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__content-half__auto` | 1 | [50 294,92 545,32 414,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__content-half__footer0` | 1 | [50 294,92 545,32 414,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__content-half__footer300` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__presigned__auto` | 1 | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__presigned__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__presigned__new` | 1 | [50 12,92 545,32 132,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__qr__placeholder` | 1 | [300 19 525 195] | 225 × 176 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__qr-id200__placeholder` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__two-pages__auto` | 2 | [50 170,53 545,32 290,53] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__two-pages__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_DE_RECOMMENDED__two-pages__new` | 3 neu | [50 279,53 545,32 399,53] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__auto` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__beyond` | 2 neu | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__footer0` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__last` | 1 | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual__emblem-alpha` | 1 | [100 572 495,32 700] | 395,32 × 128 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual__emblem-indexed` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G3 | gleich | 0,0524 % / 0,0524 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual__emblem-photo` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G5 | gleich | 0,0062 % / 0,0062 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual__emblem-tall` | 1 | [100 515 495,32 700] | 395,32 × 185 | dok. SB-G4 | gleich | 5,9907 % / 5,9907 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual__emblem-wide` | 1 | [100 572 495,32 700] | 395,32 × 128 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual__note-custom` | 1 | [100 588 495,32 700] | 395,32 × 112 | dok. SB-G2 | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__manual__note-long` | 1 | [100 548 495,32 700] | 395,32 × 152 | dok. SB-G2 | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__new` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__none` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__pdfover` | 1 | [50,5 180,25 544,82 300,25] | 494,32 × 120 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__rot90` | 1 | [-28 372 100 767,32] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4__width` | 1 | [100 540 400 700] | 300 × 160 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-cropbox__auto` | 1 | [50 428,92 545,32 548,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-cropbox__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-cropbox__new` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | dok. KR-F02 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-landscape__auto` | 1 | [50 285,32 791,92 414,32] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-landscape__manual` | 1 | [100 585 741,92 700] | 641,92 × 115 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-landscape__new` | 2 neu | [50 446,32 791,92 575,32] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot180__auto` | 1 | [50 279 545,32 399] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot180__manual` | 1 | [100 141,92 495,32 269,92] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot180__new` | 2 neu | [50 20 545,32 140] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot270__auto` | 2 neu | [446,32 50 575,32 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | dok. KR-F04 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot270__manual` | 1 | [585 100 700 741,92] | 641,92 × 115 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot270__new` | 2 neu | [446,32 50 575,32 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot90__auto` | 2 neu | [20 50 149 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | dok. KR-F04 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot90__manual` | 1 | [-104,68 100 10,32 741,92] | 641,92 × 115 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__a4-rot90__new` | 2 neu | [20 50 149 791,92] | 741,92 × 129 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__content-full__auto` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__content-full__footer0` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__content-full__last` | 1 | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__content-half__auto` | 1 | [50 294,92 545,32 414,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__content-half__footer0` | 1 | [50 294,92 545,32 414,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__content-half__footer300` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__presigned__auto` | 1 | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__presigned__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__presigned__new` | 1 | [50 12,92 545,32 132,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__qr__placeholder` | 1 | [300 3 525 195] | 225 × 192 | dok. SB-G2 | gleich | – | gleich | gleich |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__qr-id200__placeholder` | 2 neu | [50 701,92 545,32 821,92] | 495,32 × 120 | dok. SB-G2 | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__two-pages__auto` | 2 | [50 170,53 545,32 290,53] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__two-pages__manual` | 1 | [100 572 495,32 700] | 395,32 × 128 | dok. SB-G2 | gleich | – | gleich | dok. KR-F01 |
| `AMTSSIGNATURBLOCK_EN_RECOMMENDED__two-pages__new` | 3 neu | [50 279,53 545,32 399,53] | 495,32 × 120 | dok. SB-G2 | gleich | – | gleich | gleich |
| `BASE_LOGO__a4__auto` | 1 | [265,08 483,75 330,24 548,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__a4__beyond` | 2 neu | [100 634,83 165,17 700] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4__footer0` | 1 | [265,08 483,75 330,24 548,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__a4__last` | 1 | [265,08 756,75 330,24 821,92] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4__manual` | 1 | [100 634,83 165,17 700] | 65,167 × 65,167 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `BASE_LOGO__a4__manual__emblem-alpha` | 1 | [100 634,83 165,17 700] | 65,167 × 65,167 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `BASE_LOGO__a4__manual__emblem-indexed` | 1 | [100 667,42 165,17 700] | 65,167 × 32,583 | dok. SB-G3 | gleich | 1,6564 % / 1,6564 % | gleich | gleich |
| `BASE_LOGO__a4__manual__emblem-photo` | 1 | [100 651,12 165,17 700] | 65,167 × 48,875 | dok. SB-G5 | gleich | 0,1544 % / 0,1544 % | gleich | gleich |
| `BASE_LOGO__a4__manual__emblem-tall` | 1 | [100 634,83 121,72 700] | 21,722 × 65,167 | dok. SB-G4 | gleich | 58,9510 % / 58,9510 % | gleich | gleich |
| `BASE_LOGO__a4__manual__emblem-wide` | 1 | [100 680,45 165,17 700] | 65,167 × 19,55 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `BASE_LOGO__a4__manual__logo150mm` | 1 | [100 275 525 700] | 425 × 425 | dok. SB-G1 | dok. SB-G1 | – / – | gleich | gleich |
| `BASE_LOGO__a4__manual__logo50mm` | 1 | [100 558,33 241,67 700] | 141,667 × 141,667 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `BASE_LOGO__a4__new` | 2 neu | [265,08 756,75 330,24 821,92] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4__none` | 1 | [265,08 483,75 330,24 548,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__a4__pdfover` | 1 | [50,5 235,08 115,67 300,25] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4__rot90` | 1 | [34,83 434,83 100 500] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4__width` | 1 | [100 634,83 400 700] | 300 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-cropbox__auto` | 1 | [265,08 483,75 330,24 548,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `BASE_LOGO__a4-cropbox__manual` | 1 | [100 634,83 165,17 700] | 65,167 × 65,167 | gleich | gleich | – | gleich | dok. KR-F02 |
| `BASE_LOGO__a4-cropbox__new` | 2 neu | [265,08 756,75 330,24 821,92] | 65,167 × 65,167 | gleich | gleich | – | gleich | dok. KR-F02 |
| `BASE_LOGO__a4-landscape__auto` | 1 | [388,38 349,15 453,54 414,32] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `BASE_LOGO__a4-landscape__manual` | 1 | [100 634,83 165,17 700] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-landscape__new` | 2 neu | [388,38 510,15 453,54 575,32] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-rot180__auto` | 1 | [265,08 279 330,24 344,17] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__a4-rot180__manual` | 1 | [430,15 141,92 495,32 207,09] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-rot180__new` | 2 neu | [265,08 20 330,24 85,17] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-rot270__auto` | 2 neu | [510,15 388,38 575,32 453,54] | 65,167 × 65,167 | gleich | gleich | – | gleich | dok. KR-F04 |
| `BASE_LOGO__a4-rot270__manual` | 1 | [634,83 676,75 700 741,92] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-rot270__new` | 2 neu | [510,15 388,38 575,32 453,54] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-rot90__auto` | 2 neu | [20 388,38 85,17 453,54] | 65,167 × 65,167 | gleich | gleich | – | gleich | dok. KR-F04 |
| `BASE_LOGO__a4-rot90__manual` | 1 | [-104,68 100 -39,51 165,17] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__a4-rot90__new` | 2 neu | [20 388,38 85,17 453,54] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__content-full__auto` | 1 | [265,08 19,75 330,24 84,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__content-full__footer0` | 1 | [265,08 19,75 330,24 84,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__content-full__last` | 1 | [265,08 756,75 330,24 821,92] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__content-half__auto` | 1 | [265,08 349,75 330,24 414,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__content-half__footer0` | 1 | [265,08 349,75 330,24 414,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__content-half__footer300` | 1 | [265,08 349,75 330,24 414,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__presigned__auto` | 1 | [265,08 67,75 330,24 132,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__presigned__manual` | 1 | [100 634,83 165,17 700] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__presigned__new` | 1 | [265,08 67,75 330,24 132,92] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__qr__placeholder` | 1 | [300 129,83 525 195] | 225 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__qr-id200__placeholder` | 1 | [265,08 65,75 330,24 130,92] | 65,167 × 65,167 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `BASE_LOGO__two-pages__auto` | 2 | [265,08 225,36 330,24 290,53] | 65,167 × 65,167 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `BASE_LOGO__two-pages__manual` | 1 | [100 634,83 165,17 700] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `BASE_LOGO__two-pages__new` | 3 neu | [265,08 334,36 330,24 399,53] | 65,167 × 65,167 | gleich | gleich | – | gleich | gleich |
| `INVISIBLE__a4__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__beyond` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__footer0` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__last` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__none` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__pdfover` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__rot90` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4__width` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-cropbox__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-cropbox__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-cropbox__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-landscape__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-landscape__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-landscape__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot180__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot180__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot180__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot270__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot270__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot270__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot90__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot90__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__a4-rot90__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__content-full__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__content-full__footer0` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__content-full__last` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__content-half__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__content-half__footer0` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__content-half__footer300` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__presigned__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__presigned__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__presigned__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__qr__placeholder` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__qr-id200__placeholder` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__two-pages__auto` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__two-pages__manual` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `INVISIBLE__two-pages__new` | 1 | [0 0 0 0] | – | unsichtbar | – | – | – | – |
| `SIGNATURBLOCK_SMALL_DE__a4__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__a4__beyond` | 2 neu | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__footer0` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__a4__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__manual__emblem-alpha` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__manual__emblem-indexed` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G3 | gleich | 0,1597 % / 0,1597 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__manual__emblem-photo` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G5 | gleich | 0,0122 % / 0,0122 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__manual__emblem-tall` | 1 | [100 591 330 700] | 230 × 109 | dok. SB-G4 | gleich | 8,2339 % / 8,2339 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__manual__emblem-wide` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__none` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__a4__pdfover` | 1 | [50,5 251,25 280,5 300,25] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__rot90` | 1 | [51 451 100 681] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4__width` | 1 | [100 644 400 700] | 300 × 56 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-cropbox__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE__a4-cropbox__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE__a4-cropbox__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE__a4-landscape__auto` | 1 | [305,96 365,32 535,96 414,32] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE__a4-landscape__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-landscape__new` | 2 neu | [305,96 526,32 535,96 575,32] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-rot180__auto` | 1 | [182,66 279 412,66 328] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__a4-rot180__manual` | 1 | [265,32 141,92 495,32 190,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-rot180__new` | 2 neu | [182,66 20 412,66 69] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-rot270__auto` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE__a4-rot270__manual` | 1 | [651 511,92 700 741,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-rot270__new` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-rot90__auto` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE__a4-rot90__manual` | 1 | [-104,68 100 -55,68 330] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__a4-rot90__new` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__content-full__auto` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE__content-full__footer0` | 1 | [182,66 35,92 412,66 84,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__content-full__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__content-half__auto` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__content-half__footer0` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__content-half__footer300` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__presigned__auto` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__presigned__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__presigned__new` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__qr__placeholder` | 1 | [300 146 525 195] | 225 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__qr-id200__placeholder` | 1 | [182,66 81,92 412,66 130,92] | 230 × 49 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_DE__two-pages__auto` | 2 | [182,66 241,53 412,66 290,53] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE__two-pages__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE__two-pages__new` | 3 neu | [182,66 350,53 412,66 399,53] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__beyond` | 2 neu | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__footer0` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual__emblem-alpha` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual__emblem-indexed` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G3 | gleich | 0,0954 % / 0,0954 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual__emblem-photo` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G5 | gleich | 0,0073 % / 0,0073 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual__emblem-tall` | 1 | [100 558 330 700] | 230 × 142 | dok. SB-G4 | gleich | 6,3200 % / 6,3200 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual__emblem-wide` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual__note-custom` | 1 | [100 633 330 700] | 230 × 67 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__manual__note-long` | 1 | [100 608 330 700] | 230 × 92 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__none` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__pdfover` | 1 | [50,5 218,25 280,5 300,25] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__rot90` | 1 | [18 418 100 648] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4__width` | 1 | [100 616 400 700] | 300 × 84 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-cropbox__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-cropbox__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-cropbox__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-landscape__auto` | 1 | [305,96 332,32 535,96 414,32] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-landscape__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-landscape__new` | 2 neu | [305,96 493,32 535,96 575,32] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot180__auto` | 1 | [182,66 279 412,66 361] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot180__manual` | 1 | [265,32 141,92 495,32 223,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot180__new` | 2 neu | [182,66 20 412,66 102] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot270__auto` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot270__manual` | 1 | [618 511,92 700 741,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot270__new` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot90__auto` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot90__manual` | 1 | [-104,68 100 -22,68 330] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__a4-rot90__new` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__content-full__auto` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__content-full__footer0` | 1 | [182,66 2,92 412,66 84,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__content-full__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__content-half__auto` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__content-half__footer0` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__content-half__footer300` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__presigned__auto` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__presigned__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__presigned__new` | 1 | [182,66 50,92 412,66 132,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__qr__placeholder` | 1 | [300 113 525 195] | 225 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__qr-id200__placeholder` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_DE_NOTE__two-pages__auto` | 2 | [182,66 208,53 412,66 290,53] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE__two-pages__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE__two-pages__new` | 3 neu | [182,66 317,53 412,66 399,53] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__beyond` | 2 neu | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__footer0` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual__emblem-alpha` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G6 | gleich | 2,0493 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual__emblem-indexed` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual__emblem-photo` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G5 | gleich | 0,0242 % / 0,0073 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual__emblem-tall` | 1 | [100 558 330 700] | 230 × 142 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual__emblem-wide` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual__note-custom` | 1 | [100 633 330 700] | 230 × 67 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__manual__note-long` | 1 | [100 608 330 700] | 230 × 92 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__none` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__pdfover` | 1 | [50,5 218,25 280,5 300,25] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__rot90` | 1 | [18 418 100 648] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4__width` | 1 | [100 616 400 700] | 300 × 84 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-cropbox__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-cropbox__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-cropbox__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-landscape__auto` | 1 | [305,96 332,32 535,96 414,32] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-landscape__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-landscape__new` | 2 neu | [305,96 493,32 535,96 575,32] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot180__auto` | 1 | [182,66 279 412,66 361] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot180__manual` | 1 | [265,32 141,92 495,32 223,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot180__new` | 2 neu | [182,66 20 412,66 102] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot270__auto` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot270__manual` | 1 | [618 511,92 700 741,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot270__new` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot90__auto` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot90__manual` | 1 | [-104,68 100 -22,68 330] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__a4-rot90__new` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__content-full__auto` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__content-full__footer0` | 1 | [182,66 2,92 412,66 84,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__content-full__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__content-half__auto` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__content-half__footer0` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__content-half__footer300` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__presigned__auto` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__presigned__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__presigned__new` | 1 | [182,66 50,92 412,66 132,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__qr__placeholder` | 1 | [300 113 525 195] | 225 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__qr-id200__placeholder` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__two-pages__auto` | 2 | [182,66 208,53 412,66 290,53] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__two-pages__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_NOTE_PDFA__two-pages__new` | 3 neu | [182,66 317,53 412,66 399,53] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__beyond` | 2 neu | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__footer0` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__manual__emblem-alpha` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G6 | gleich | 3,3784 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__manual__emblem-indexed` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__manual__emblem-photo` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G5 | gleich | 0,0405 % / 0,0122 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__manual__emblem-tall` | 1 | [100 591 330 700] | 230 × 109 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__manual__emblem-wide` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__none` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__pdfover` | 1 | [50,5 251,25 280,5 300,25] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__rot90` | 1 | [51 451 100 681] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4__width` | 1 | [100 644 400 700] | 300 × 56 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-cropbox__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-cropbox__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-cropbox__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-landscape__auto` | 1 | [305,96 365,32 535,96 414,32] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-landscape__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-landscape__new` | 2 neu | [305,96 526,32 535,96 575,32] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot180__auto` | 1 | [182,66 279 412,66 328] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot180__manual` | 1 | [265,32 141,92 495,32 190,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot180__new` | 2 neu | [182,66 20 412,66 69] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot270__auto` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot270__manual` | 1 | [651 511,92 700 741,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot270__new` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot90__auto` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot90__manual` | 1 | [-104,68 100 -55,68 330] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__a4-rot90__new` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__content-full__auto` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__content-full__footer0` | 1 | [182,66 35,92 412,66 84,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__content-full__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__content-half__auto` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__content-half__footer0` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__content-half__footer300` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__presigned__auto` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__presigned__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__presigned__new` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__qr__placeholder` | 1 | [300 146 525 195] | 225 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__qr-id200__placeholder` | 1 | [182,66 81,92 412,66 130,92] | 230 × 49 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_DE_PDFA__two-pages__auto` | 2 | [182,66 241,53 412,66 290,53] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_DE_PDFA__two-pages__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_DE_PDFA__two-pages__new` | 3 neu | [182,66 350,53 412,66 399,53] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__a4__beyond` | 2 neu | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__footer0` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__a4__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__manual__emblem-alpha` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__manual__emblem-indexed` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G3 | gleich | 0,1597 % / 0,1597 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__manual__emblem-photo` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G5 | gleich | 0,0122 % / 0,0122 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__manual__emblem-tall` | 1 | [100 591 330 700] | 230 × 109 | dok. SB-G4 | gleich | 8,2339 % / 8,2339 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__manual__emblem-wide` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__none` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__a4__pdfover` | 1 | [50,5 251,25 280,5 300,25] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__rot90` | 1 | [51 451 100 681] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4__width` | 1 | [100 644 400 700] | 300 × 56 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-cropbox__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN__a4-cropbox__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN__a4-cropbox__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN__a4-landscape__auto` | 1 | [305,96 365,32 535,96 414,32] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN__a4-landscape__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-landscape__new` | 2 neu | [305,96 526,32 535,96 575,32] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-rot180__auto` | 1 | [182,66 279 412,66 328] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__a4-rot180__manual` | 1 | [265,32 141,92 495,32 190,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-rot180__new` | 2 neu | [182,66 20 412,66 69] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-rot270__auto` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN__a4-rot270__manual` | 1 | [651 511,92 700 741,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-rot270__new` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-rot90__auto` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN__a4-rot90__manual` | 1 | [-104,68 100 -55,68 330] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__a4-rot90__new` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__content-full__auto` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN__content-full__footer0` | 1 | [182,66 35,92 412,66 84,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__content-full__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__content-half__auto` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__content-half__footer0` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__content-half__footer300` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__presigned__auto` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__presigned__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__presigned__new` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__qr__placeholder` | 1 | [300 146 525 195] | 225 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__qr-id200__placeholder` | 1 | [182,66 81,92 412,66 130,92] | 230 × 49 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_EN__two-pages__auto` | 2 | [182,66 241,53 412,66 290,53] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN__two-pages__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN__two-pages__new` | 3 neu | [182,66 350,53 412,66 399,53] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__beyond` | 2 neu | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__footer0` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual__emblem-alpha` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual__emblem-indexed` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G3 | gleich | 0,0954 % / 0,0954 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual__emblem-photo` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G5 | gleich | 0,0073 % / 0,0073 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual__emblem-tall` | 1 | [100 558 330 700] | 230 × 142 | dok. SB-G4 | gleich | 6,3200 % / 6,3200 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual__emblem-wide` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual__note-custom` | 1 | [100 633 330 700] | 230 × 67 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__manual__note-long` | 1 | [100 608 330 700] | 230 × 92 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__none` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__pdfover` | 1 | [50,5 218,25 280,5 300,25] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__rot90` | 1 | [18 418 100 648] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4__width` | 1 | [100 616 400 700] | 300 × 84 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-cropbox__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-cropbox__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-cropbox__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-landscape__auto` | 1 | [305,96 332,32 535,96 414,32] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-landscape__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-landscape__new` | 2 neu | [305,96 493,32 535,96 575,32] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot180__auto` | 1 | [182,66 279 412,66 361] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot180__manual` | 1 | [265,32 141,92 495,32 223,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot180__new` | 2 neu | [182,66 20 412,66 102] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot270__auto` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot270__manual` | 1 | [618 511,92 700 741,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot270__new` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot90__auto` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot90__manual` | 1 | [-104,68 100 -22,68 330] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__a4-rot90__new` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__content-full__auto` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__content-full__footer0` | 1 | [182,66 2,92 412,66 84,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__content-full__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__content-half__auto` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__content-half__footer0` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__content-half__footer300` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__presigned__auto` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__presigned__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__presigned__new` | 1 | [182,66 50,92 412,66 132,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__qr__placeholder` | 1 | [300 113 525 195] | 225 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__qr-id200__placeholder` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_EN_NOTE__two-pages__auto` | 2 | [182,66 208,53 412,66 290,53] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE__two-pages__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE__two-pages__new` | 3 neu | [182,66 317,53 412,66 399,53] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__beyond` | 2 neu | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__footer0` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual__emblem-alpha` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G6 | gleich | 2,0493 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual__emblem-indexed` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual__emblem-photo` | 1 | [100 618 330 700] | 230 × 82 | dok. SB-G5 | gleich | 0,0242 % / 0,0073 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual__emblem-tall` | 1 | [100 558 330 700] | 230 × 142 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual__emblem-wide` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual__note-custom` | 1 | [100 633 330 700] | 230 × 67 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__manual__note-long` | 1 | [100 608 330 700] | 230 × 92 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__none` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__pdfover` | 1 | [50,5 218,25 280,5 300,25] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__rot90` | 1 | [18 418 100 648] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4__width` | 1 | [100 616 400 700] | 300 × 84 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-cropbox__auto` | 1 | [182,66 466,92 412,66 548,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-cropbox__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-cropbox__new` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-landscape__auto` | 1 | [305,96 332,32 535,96 414,32] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-landscape__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-landscape__new` | 2 neu | [305,96 493,32 535,96 575,32] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot180__auto` | 1 | [182,66 279 412,66 361] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot180__manual` | 1 | [265,32 141,92 495,32 223,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot180__new` | 2 neu | [182,66 20 412,66 102] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot270__auto` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot270__manual` | 1 | [618 511,92 700 741,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot270__new` | 2 neu | [493,32 305,96 575,32 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot90__auto` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot90__manual` | 1 | [-104,68 100 -22,68 330] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__a4-rot90__new` | 2 neu | [20 305,96 102 535,96] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__content-full__auto` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__content-full__footer0` | 1 | [182,66 2,92 412,66 84,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__content-full__last` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__content-half__auto` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__content-half__footer0` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__content-half__footer300` | 1 | [182,66 332,92 412,66 414,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__presigned__auto` | 1 | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__presigned__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__presigned__new` | 1 | [182,66 50,92 412,66 132,92] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__qr__placeholder` | 1 | [300 113 525 195] | 225 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__qr-id200__placeholder` | 2 neu | [182,66 739,92 412,66 821,92] | 230 × 82 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__two-pages__auto` | 2 | [182,66 208,53 412,66 290,53] | 230 × 82 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__two-pages__manual` | 1 | [100 618 330 700] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA__two-pages__new` | 3 neu | [182,66 317,53 412,66 399,53] | 230 × 82 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__beyond` | 2 neu | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__footer0` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__manual__emblem-alpha` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G6 | gleich | 2,2859 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__manual__emblem-indexed` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__manual__emblem-photo` | 1 | [100 651 330 700] | 230 × 49 | dok. SB-G5 | gleich | 0,0288 % / 0,0089 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__manual__emblem-tall` | 1 | [100 610 330 700] | 230 × 90 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__manual__emblem-wide` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | 0,0000 % / 0,0000 % | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__none` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__pdfover` | 1 | [50,5 251,25 280,5 300,25] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__rot90` | 1 | [51 451 100 681] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4__width` | 1 | [100 652 400 700] | 300 × 48 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-cropbox__auto` | 1 | [182,66 499,92 412,66 548,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-cropbox__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-cropbox__new` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F02 |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-landscape__auto` | 1 | [305,96 365,32 535,96 414,32] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-landscape__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-landscape__new` | 2 neu | [305,96 526,32 535,96 575,32] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot180__auto` | 1 | [182,66 279 412,66 328] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot180__manual` | 1 | [265,32 141,92 495,32 190,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot180__new` | 2 neu | [182,66 20 412,66 69] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot270__auto` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot270__manual` | 1 | [651 511,92 700 741,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot270__new` | 2 neu | [526,32 305,96 575,32 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot90__auto` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | dok. KR-F04 |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot90__manual` | 1 | [-104,68 100 -55,68 330] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__a4-rot90__new` | 2 neu | [20 305,96 69 535,96] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__content-full__auto` | 2 neu | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy 0) | gleich (Scan, Δy 0) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__content-full__footer0` | 1 | [182,66 35,92 412,66 84,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__content-full__last` | 1 | [182,66 772,92 412,66 821,92] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__content-half__auto` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__content-half__footer0` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__content-half__footer300` | 1 | [182,66 365,92 412,66 414,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__presigned__auto` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__presigned__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__presigned__new` | 1 | [182,66 83,92 412,66 132,92] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__qr__placeholder` | 1 | [300 146 525 195] | 225 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__qr-id200__placeholder` | 1 | [182,66 81,92 412,66 130,92] | 230 × 49 | gleich | gleich | – | dok. KR-F03 | dok. KR-F03 |
| `SIGNATURBLOCK_SMALL_EN_PDFA__two-pages__auto` | 2 | [182,66 241,53 412,66 290,53] | 230 × 49 | gleich | gleich | – | gleich (Scan, Δy -1) | gleich (Scan, Δy -1) |
| `SIGNATURBLOCK_SMALL_EN_PDFA__two-pages__manual` | 1 | [100 651 330 700] | 230 × 49 | gleich | gleich | – | gleich | gleich |
| `SIGNATURBLOCK_SMALL_EN_PDFA__two-pages__new` | 3 neu | [182,66 350,53 412,66 399,53] | 230 × 49 | gleich | gleich | – | gleich | gleich |

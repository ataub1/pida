# Signaturblock: Golden-Vergleich gegen PDF-AS (Paket C)

Stand: 14.10.2026, `pdfover-sigblock` (Worktree Paket C). Tests: `cargo test -p pdfover-sigblock`
(`tests/golden.rs`, `tests/layout.rs`, `tests/render.rs` – letzterer nur mit `PDFOVER_PDFIUM_DIR`).

## Referenzen

Die Referenz-PDFs liegen bereits als Kopien der pdf-as-4-Testressourcen unter `testdata/pdf/`
(gemeinsame Testdaten, EUPL); sie werden nicht ein zweites Mal kopiert. Unter `testdata/sigblock/`
liegen nur die zusätzlich nötigen Eingaben:

| Pfad | Zweck |
|---|---|
| `pdfas-default/cfg/` | Auszug der PDF-AS-4-Standardkonfiguration (`BASE_NORMAL`, `SIGNATURBLOCK_DE`, EUPL) für G5 |
| `pdfas-default/images/signatur-logo_{de,en}.png` | 200×210-px-Logos der PDF-AS-Standardkonfiguration, mit denen alle Referenzen erzeugt wurden |
| `images/` | synthetische Bild-Fixtures für die Emblem-Tests (`crates/sigblock/tools/make_image_fixtures.py`) |

| Fall | Datei | Formular (BBox) | Profil | Breite | Eingaben |
|---|---|---|---|---|---|
| G1 | `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf` | 231.99998 × 64 | `SIGNATURBLOCK_SMALL_DE_NOTE` | 230 | CN „Stephan Keller“, `2021-07-20T01:42:56-07:00`, Hinweis `!simple_config.Note_Default!` |
| G2 | `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf` | 227 × 51 | `SIGNATURBLOCK_SMALL_DE` | 225 (QR-Platzhalter) | CN „Stephan Keller“, `2021-07-20T01:39:53-07:00` |
| G3 | `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf` | 227 × 51 („Signatory“) | `SIGNATURBLOCK_SMALL_EN` | 225 (QR-Platzhalter) | CN „Stephan Keller“, `2021-07-20T01:42:29-07:00` |
| G4 | `pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf` | 231.99998 × 84 | `SIGNATURBLOCK_SMALL_EN_NOTE_PDFA` | 230 | CN „Alexander Marsalek“, `2021-07-22T15:53:55+02:00`, Profil-Hinweis |
| G5 | `pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf` | 496.30396 × 74 | `SIGNATURBLOCK_DE` (PDF-AS-Standard, TTF 8 pt) | 595.30396 − 2·50 | CN „Andreas Fitzek ECC“, `2024-03-08T16:18:06+01:00` |

(Die Namen sind die öffentlichen Testdaten der PDF-AS-Testsuite.)

## Ergebnisse

| Prüfung | Ergebnis |
|---|---|
| Tokenstrom des Inner-Forms (Rechtecke, Linien, Linienbreite, Bildmatrix, Textblöcke mit Font, Größe, Startpunkt, Zeilenvorschub, kodierten Strings), ±0,01 pt | **G1–G5 gleich**: 35/29/29/35/29 Operationen, 8/6/6/8/6 Textblöcke, maximale Abweichung < 0,00001 pt (gleiche 5-Nachkommastellen-Ausgabe) |
| G5 (aktuelles PDF-AS-Format) mit `pdfas_quirks` | **Content-Stream byte-gleich** (inklusive `/DeviceRGB cs … sc`, Zahlenformat, Literal-/Hex-Strings mit CIDs) |
| G1–G4 mit `pdfas_quirks` | Operatorfolge identisch |
| Pixelvergleich PDFium (Faktor 4 = 288 dpi, eigenes vs. Referenz-Inner-Form auf gleicher Seite) | **0,0000 % abweichende Pixel**, mittlere Abweichung 0,000 (SMALL_DE_NOTE 920×252 px, SMALL_EN 900×200, SMALL_EN_NOTE_PDFA 920×332, SIGNATURBLOCK_DE 1981×296) – im Standardmodus, also mit DeviceGray-Farben und eigenen Font-Subsets |
| Mini-PDF (`to_standalone_pdf`) | `qpdf --check` ohne Befund (SMALL_DE_NOTE, SMALL_EN_PDFA 270°, AMTS_EN 90°); PDFium-Render 920×196 px für SMALL_DE |

### Rechenbeispiele docs/analyse/04 §3.8 (`tests/layout.rs`)

Alle Werte getroffen (±0,01 pt): SMALL_DE/EN/PDFA 230×49 (Spalten 41,8182/188,1818; Info
50,8599/137,3219; Zeilen 49; 13/13/23; Logo 33×34 bei (4,409; 8,5); Baselines (55,338; 42,035),
(96,678; 42,179), (50,903; 29,035), (51,178; 16,035)), _NOTE 230×82 (49/33), SMALL_EN_NOTE Info
44,8052/143,3766, SMALL_EN_PDFA 35,3846/194,6154 und 35,3846/159,2308 mit Logo 27×28, AMTS DE/EN
auf A4 495×120 (96/24; 24/16/56, Demo-Logo 61×61), BASE_LOGO 23 mm 62,118×65,167 (Logo 62×65) bzw.
mit Amts-Demo-Logo 65,167×64,891 (65×64). Hinweis: Die Blockbreite ist wie bei PDF-AS die
`f32`-Summe der Spaltenbreiten, z. B. 229,99998 statt 230.

## Abweichungen und Befunde

1. **Älteres PDF-AS-Format in G1–G4 (2021):** Die Tabelle steht dort bei x = 1 mit
   `/BBox [W+2 H+2 0 0]`. Der aktuelle Code (PDF-AS 4.4.2, `PDFAsVisualSignatureBuilder`:
   `drawTable(…, 0, 1, …)`, `PDFAsVisualSignatureProperties`: `{0, 0, W+1, H+1}`) und G5 (2024)
   zeichnen bei x = 0 mit `[W+1 H+1 0 0]`. Implementiert ist das aktuelle Format; der Test erkennt
   das alte an der BBox und rechnet dx = −1 heraus. Alle übrigen Werte sind identisch.
2. **G4 mit älterer Profilversion:** Die Captions sind in G4 zentriert, die heutigen Profile
   (`pdfas-config` und PDF-AS-Standard) setzen `halign=right`. Der Test setzt für G4
   `table.main.Style.halign=center` und bestätigt damit Zentrierung und DejaVuSansCondensed-Bold-Breiten.
3. **Logo:** Alle Referenzen wurden mit dem 200×210-px-Logo der PDF-AS-Standardkonfiguration erzeugt
   (PDF-Over liefert 326×342 px aus). Der Test verwendet das Referenzlogo; für die Breiten 225/230/495
   ergeben beide Logos dieselbe Zeichengröße (32×33, 33×34, 62×65).
4. **Farben:** PDF-AS schreibt `/DeviceRGB cs 1 1 1 sc` / `0 0 0 sc`; hier im Standard `1 g` / `0 g`
   (Plan §6.5, PDF/A-tauglich auch mit CMYK-OutputIntent), mit `pdfas_quirks` wie PDF-AS. Pixelgleich.
5. **Fonts:** Subset-Präfix (`AAABIF+` …) und Font-Programm unterscheiden sich (`subsetter` statt
   PDFBox `TTFSubsetter`); CIDs (= GIDs des Originalfonts), `/W` (gerundete Breiten, 1000 ausgelassen),
   `CIDToGIDMap`, `CIDSet` (außer PDF/A-3) und ToUnicode folgen PDFBox. Die kodierten Strings sind
   byte-gleich.
6. **Bilder:** Die Referenzen von 2021 enthalten das Logo unkomprimiert; hier wie im aktuellen
   `LosslessFactory` Flate mit PNG-Predictor 15 (Filterwahl je Zeile), Alpha als `/SMask`, PDF/A-1
   auf Weiß komponiert. Gleiche Pixel. Bei PDF/A (ohne `pdfas_quirks`) liegen Bildmarke und farbige
   Füllungen im Farbraum `[/ICCBased <sRGB>]` (Plan §6.5) statt DeviceRGB, damit der Block auch in
   PDF/A-Dokumenten mit CMYK-OutputIntent (z. B. FOGRA39) konform ist; das ICC-v2.1-Profil berechnet
   `src/icc.rs` aus IEC 61966-2-1 (kein fremdes Profil im Repository).
7. **Nicht nachgestellt:** `pdf/pdfas/pdf-as-web/data/dummy-pdf-signed.pdf` (eigenes dreispaltiges
   Profil mit LiberationMono, Font nicht im Lieferumfang) und die übrigen Formulare in
   `manySignFields_signed4.pdf` (Fremd-Widgets ohne PDF-AS-Tabelle).

## Review (adversarial, Paket C)

| Prüfung | Ergebnis |
|---|---|
| sRGB-ICC-Profil gegen LittleCMS (Pillow `ImageCms`, `tests/appearance.rs`) | gültig, sRGB → sRGB mit max. 1/255 Abweichung (Test-Grenze 2) |
| PDFium: Block mit `ICCBased`-Logo (PDF/A-2) gegen DeviceRGB (`tests/render.rs`) | **0 Pixel abweichend**, max. Kanalabweichung 0 |
| `qpdf --check` auf dem Mini-PDF eines PDF/A-Blocks mit ICC-Profil | ohne Befund |
| Tief geschachtelte Ausdrücke im Hinweistext (`${((((…`, 50 000 Ebenen) | `UnsupportedExpression` (vorher Stapelüberlauf, Prozessabbruch) |
| NaN/∞ in `imagescaletofit`, `border`, `padding`, Fontgröße, `ColsWidth` | `SigBlockError::Profile` (vorher `NaN` im Content-Stream) |
| Include-Zyklen (`include = *.properties` in mehreren Dateien) | terminiert sofort, Problem in `ProfileStore::problems` (vorher exponentiell viele Ladevorgänge) |
| Windows-Zeilenenden im Hinweistext | `\r\n`/`\r` wie `\n` (vorher `?` und Hinweis „Zeichen ersetzt“) |
| Eigenschaftstests (proptest) für Ausdrücke, Datumsmuster, Fontbeschreibungen, beliebige Profile | keine Panic, keine nicht endlichen Zahlen im Content-Stream |

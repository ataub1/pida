# ADR 0060 – Renderer (PDFium) und Positionierung (Paket F)

| | |
|---|---|
| **Status** | angenommen (14.10.2026, Paket F) |
| **Kontext** | `pdfover-render`, `pdfover-placement`; Plan §6.6–§6.8, analyse/04 §4–§6, ADR 0001 §8.1/§8.2 |

## 1. PDFium bereitstellen

- Gebunden wird **pdfium-binaries chromium/8100** (API-Stand `pdfium-render` 0.9.4 = `pdfium_7881`),
  dynamisch per `PdfiumBackend::bind`.
- Suchreihenfolge ohne Angabe: `PDFOVER_PDFIUM_DIR` → Verzeichnis der Binary (macOS-Bundle zusätzlich
  `../Frameworks`, `../Resources`, iOS `Frameworks`) → Systempfad. Mit `bind(Some(dir))` nur `dir`.
  Fehler: `RenderError::LibraryNotFound` mit allen durchsuchten Orten.
- **Systempfad ohne Arbeitsverzeichnis (CWE-427, Review 14.10.2026):** Unter macOS (`dlopen` mit
  bloßem Dateinamen, nicht eingeschränkte Prozesse) und Windows (`LoadLibrary`) schließt die Suche
  nach dem Dateinamen das Arbeitsverzeichnis ein – eine `libpdfium.dylib`/`pdfium.dll` in einem
  entpackten Archiv hätte fremden Code in die CLI geladen. Dort werden deshalb nur **absolute**
  Verzeichnisse geprüft (macOS: absolute Einträge von `DYLD_LIBRARY_PATH`/
  `DYLD_FALLBACK_LIBRARY_PATH`, `/opt/homebrew/lib`, `/usr/local/lib`, `/usr/lib`; Windows: absolute
  `PATH`-Einträge). Linux/Android behalten `dlopen("libpdfium.so")` (`ld.so` sucht nie im
  Arbeitsverzeichnis).
- Die Bindung gilt **einmal je Prozess** (globale Bindung in `pdfium-render`); weitere Aufrufe liefern
  sie erneut. `describe()` liest `BUILD=` aus der `VERSION`-Datei des Pakets („PDFium chromium/8100 (…)“).
- Entwicklung: `tools/pdfium/<plattform>/lib` im Hauptcheckout (nicht versioniert). Tests prüfen
  `PDFOVER_PDFIUM_DIR` und überspringen sich ohne Variable. **CI (Integration):** Release
  `chromium/8100` von `bblanchon/pdfium-binaries` je Plattform laden, entpacken und
  `PDFOVER_PDFIUM_DIR=<…>/lib` setzen; der Schritt „cargo check ohne PDFium“ baut
  `pdfover-render --no-default-features` (die PDFium-Testziele haben `required-features`).

## 2. Threads

PDFium ist nicht threadsicher. `pdfium-render` (`thread_safe`) serialisiert jeden FFI-Aufruf; zusätzlich
hält jede Operation (`open_document`, `page_size`, `render_page`, `render_forced_color`,
`image_objects`, `Drop`) eine prozessweite Sperre, damit mehrschrittige Abläufe nicht verzahnt laufen.
`PdfiumBackend` ist `Send + Sync + Copy`, `PdfiumRenderer` ist `Send`. Test:
`backend_is_usable_from_many_threads` (8 Threads, Ergebnisse bitgleich zur Einzelausführung, Drop in
fremdem Thread).

## 3. Forced-Color-Raster

- `FPDF_RenderPageBitmapWithColorScheme_Start` (alle Pfad-/Textfarben = Signalfarbe, Bilder
  unverändert, `FPDF_ANNOT`), Hintergrund per `FPDFBitmap_FillRect`, Raster
  `(int)(pt · dpi/72)` wie PDF-AS (`ForcedColorOptions::raster_size`), Anzeigebereich gerundet.
- `pdfium-render` bietet die Funktion nur roh an und gibt keine Handles heraus. Deshalb öffnet
  `ffi::RawDocument` über eine **zweite Bindung derselben Bibliothek** eine zweite Dokumentinstanz
  (lazy beim ersten Aufruf; Passwort nur im Speicher, `Zeroizing`). Das ist das einzige `unsafe` im
  Paket (Modul `ffi`, lokales `#![allow(unsafe_code)]`, jede Stelle mit `SAFETY`-Begründung).
- PDFium verlangt ein Pause-Objekt mit `version == 1`, sonst `FPDF_RENDER_FAILED`.
- Widgets zeichnet PDFium nur über `FPDF_FFLDraw` (Formularumgebung der zweiten Instanz) – in
  Originalfarben; sie zählen dennoch als Inhalt (Register KR-F06).
- `keep_images = false` (nicht PDF-AS) füllt die Hülle jedes Bildobjekts mit der Signalfarbe.
- Der Scan (`placement::RasterScanner`) läuft im **gedrehten, sichtbaren** Raum (KR-F04) und
  ignoriert Pixel mit einer Abweichung unter `SCAN_TOLERANCE = 6` (≈ 1/64 Deckung, Java2D-Marlin
  8 × 8 Subpixel) – KR-F10.

## 4. Bildobjekte und QR-Platzhalter

- `image_objects`: Seitenobjekte in PDFium-Reihenfolge, Form-XObjects in Tiefensuche (Matrix
  `Kind · Form`), Inline-Bilder inklusive; Bitmap `FPDFImageObj_GetBitmap` → RGBA
  (Gray/BGR/BGRx/BGRA mit Stride); nicht dekodierbare oder zu große Bilder (> 100 MP) leer.
- **Pixelbudget je Seite (Review 14.10.2026):** Ohne Obergrenze hielt `image_objects` jede
  dekodierte Kopie im Ergebnis – ein 80-KB-PDF mit einem 9000 × 9000-Graustufenbild, sechsmal
  gezeichnet, belegte 2 GB (linear in der Zahl der `Do`). Jetzt: erster Durchgang ohne Dekodieren
  (`/Width` × `/Height` über `FPDFImageObj_GetImageMetadata`), dekodiert werden die kleinsten Bilder
  zuerst, bis `MAX_IMAGE_PIXELS_PER_PAGE` = 64 MP (≈ 256 MiB RGBA) erreicht ist; das tatsächliche
  Ergebnis zählt ebenfalls gegen das Budget. Übrige Bildobjekte behalten Matrix und `order`, ihr
  Bitmap bleibt leer (Register KR-F12). Restrisiko: Ein JPEG, dessen tatsächliche Maße die
  deklarierten weit übersteigen, dekodiert PDFium einmal vollständig (danach verworfen).
- Zuordnung zu `ImageInvocation` primär über die Matrix (≤ 0,01), zusätzlich passende Pixelmaße
  (`ImageInvocation::pixel_width/height` = Bitmapgröße, wenn beide bekannt – sonst bekäme ein
  Inline-Bild mit gleicher Matrix den Namen des XObjects), `order` nur als Tie-Breaker (ADR 0001
  §8.2); Index nach der Matrixkomponente `e` (`O(n log n)` statt `O(n²)`). Dekodiert werden nur
  zugeordnete Bilder (Inline-Bilder und Form-Inhalte ohne `descend_forms` bleiben wie bei PDF-AS
  außen vor).
- QR: `rxing` `QRCodeReader` + `HybridBinarizer`, ohne TRY_HARDER, Luminanz wie ZXing
  `BufferedImageLuminanceSource`; ohne Treffer ein zweiter Versuch 2× vergrößert (KR-F07). Position
  aus `CTM · (Benutzerraum → sichtbar)`: `x = floor(e)`, `y = ceil(f + d)`, `w = ceil(a)`.

## 4a. Robustheit (Review 14.10.2026)

- Passwörter mit NUL-Zeichen liefern `RenderError::PasswordRequired`, bevor PDFium sie sieht:
  `pdfium-render` 0.9.4 bricht sonst in `CString::new(..).unwrap()` mit dem **Passwort in der
  Panic-Meldung** ab und vergiftet dabei seine globale Sperre – danach bricht jeder PDFium-Aufruf
  im Prozess ab.
- Klemmung (`clamp_block`, `nudge`, `default_start_position`) ist panikfrei auch bei `NaN`/`inf`
  (z. B. CropBox `[inf 0 inf 842]`); `lowest_content_row` rechnet mit Sättigung.
- Gegenprobe der Freiraumsuche (Plan §6.7): `PdfiumRenderer::lowest_object_edge` liefert die
  unterste Kante der Hüllrechtecke aller Seitenobjekte oberhalb der Fußzeile;
  `scan_cross_check.rs` vergleicht sie für alle 26 POS-Suiten und erzeugte Seiten mit dem Scan
  (oberhalb f:80 höchstens 1 pt Abstand).

## 5. Positionierung

`TablePos` folgt `TablePos.java` bis in die Java-Semantik (`String.split`, `Float.parseFloat`,
`x:auto` überschreibt beim Merge keinen festen Profilwert, `getFooterLine`). `resolve_width` /
`resolve_position` sind ein Nachbau von `Positioning.adjustSignatureTableandCalculatePosition`
(inklusive der Eigenheit „`p:<n>` ohne Platz ⇒ letzte Seite“). Abweichungen stehen im
Kompatibilitätsregister der Crate-Doku (`pdfover_placement`, KR-F01 … KR-F11); `CompatMode::PdfasQuirks`
reproduziert KR-F01/KR-F02 für Golden-Tests.

## 6. Golden-Tests

- **Positionen:** 24 der 26 PDF-AS-Suiten `public_pdfbox2/POS_*` (Eingaben kopiert nach
  `testdata/placement/pdfas-pos/`). Erwartete Werte = Rahmen des Signaturblocks in den
  Referenzbildern `example_ref.png` (signierte Seite, 288 dpi), BBox-Stauchung `W²/(W+1)`
  herausgerechnet, festgehalten in `golden.json`; ein Test prüft das Manifest gegen die Bilder, einer
  die eigene Berechnung (Seite, neue Seite, x, y, Breite, Widget-Rect ± 1 pt) in beiden
  Kompatibilitätsmodi. Ergebnis: Seite/neue Seite/x/Breite exakt, y in 8 Fällen exakt, in 16 Fällen
  (Glyphenrasterung von LiberationSerif) 1 pt tiefer. `*_MINIMAL` ohne Rahmen nicht messbar.
- **QR/Felder:** die PDF-AS-Tests `SignatureFieldsAndPlaceHolderExtractorTest` und
  `PDFBoxPlaceholderExtractorTest` (Image5/Image8/None/Im48, Signature_0…7); Widget-Rects der von
  PDF-AS signierten `new_qr_2_signed_signed.pdf` stimmen exakt mit Platzhalterposition +
  `widget_rect` überein.
- **Rotation/CropBox:** PDFium als Orakel (`rotation_oracle.rs`): Rechteck aus `widget_rect` wird
  für alle Seiten- und Blockrotationen mit/ohne CropBox-Versatz exakt an der sichtbaren Position
  gezeichnet; die Portierung von `createSignatureRectangle` dient als zweites Orakel.

## 7. Konsequenzen und offene Punkte

- Additive Schnittstellenänderungen (ADR 0001 Regel 3): `PositionContext::compat` und
  `PositionContext::new`, `PlacementNotice::ScanFailed`, `FreeSpaceScanner::region_occupied`
  (Default-Methode), `TablePos::merge` mit Java-Semantik für `auto`; neue Funktionen/Typen siehe
  Abschlussbericht. Review: `MAX_IMAGE_PIXELS_PER_PAGE`, `PdfiumRenderer::lowest_object_edge`,
  `default_start_placement`. Hinweis: `PositionContext` hat ein neues öffentliches Feld (`compat`);
  Struct-Literale in anderen Crates brauchen `..PositionContext::new(pages)`.
- QR-Bilder mit `/SMask`/`/Mask` (transparente PNG) werden nicht wie in PDF-AS dekodiert, weil
  `FPDFImageObj_GetBitmap` Masken ignoriert. Abhilfe bei Bedarf: `FPDFImageObj_GetRenderedBitmap`
  mit temporär nativer Matrix in der zweiten Dokumentinstanz.
- Der Renderer läuft im Prozess; der Hilfsprozess für feindliche PDFs (Plan §6.6) ist Sache der
  Integration.

# Eingebettete PDF-AS-Konfiguration (`pida-sigblock`)

Übernommen aus `reference/pdfas-config` (PDF-Over `PDFASConfig.zip`, PDF-AS 4, EUPL) und per
`include_bytes!` in `src/assets.rs` eingebettet (`BuiltinResources`, `ProfileStore::builtin`).

| Pfad | Inhalt | Lizenz | geändert |
|---|---|---|---|
| `cfg/config.properties`, `cfg/advancedconfig.properties` | Einstieg und Includes (`include.01 = profiles/*.properties`) | EUPL | nein |
| `cfg/profiles/*.properties` | 20 Profildateien (Basisprofile + die 12 PDF-Over-Profil-IDs) | EUPL | nein |
| `fonts/DejaVuSansMono.ttf`, `fonts/DejaVuSansCondensed-Bold.ttf`, `fonts/DejaVuSansCondensed.ttf` | Wertefont, PDF/A-Captionfont, PDF/A-Ersatz für Helvetica | Bitstream Vera / DejaVu (`licenses/DejaVu-LICENSE.txt`) | nein (Subsets entstehen erst beim Einbetten) |
| `images/signatur-logo_de.png`, `images/signatur-logo_en.png` (326×342 px), `images/amtsignatur-logo_demo.png` (236×235 px) | Standard-Bildmarken | EUPL | nein |
| `licenses/Adobe-Core14-AFM-License.html` | Lizenzhinweis der Adobe Core 14 AFM Files | Adobe | nein |
| `images/pida-seal.png` (326×342 px, RGBA) | PIDA-Siegel: Standard-Bildmarke von `SIGNATURBLOCK_SMALL_{DE,EN}[_NOTE][_PDFA]` (ADR 0005) | **eigenes Werk des Projekts, EUPL-1.2** (kein PDF-AS-Bestandteil) | – (erzeugt aus `design/brand/pida-seal.svg` mit `design/brand/tools/build.mjs`) |

`arial.ttf` aus `pdfas-config/fonts` wird bewusst **nicht** übernommen (analyse/05 §6.3).

## PIDA-Siegel als Standard-Bildmarke

Die Profildateien bleiben byte-gleich zu PDF-AS. `ProfileStore::builtin()` setzt beim Laden für die
acht kleinen Standardprofile `value.SIG_LABEL = ./images/pida-seal.png` (eigener Schlüssel des
Profils, ersetzt den aus `BASE_NORMAL[_EN]` geerbten Wert); Amtssignatur (`AMTSSIGNATURBLOCK_*`,
`amtsignatur-logo_demo.png`) und `BASE_LOGO` (`signatur-logo_de.png`) bleiben unverändert. Eine eigene
Bildmarke (Einstellung, CLI `-e`) geht weiterhin vor. `ProfileStore::builtin_pdfas()` liefert die
Konfiguration ohne diese Vorgabe (Golden-Abgleich mit PDF-AS); eigene Konfigurationsordner
(`ProfileStore::load_dir`) verwenden ihre eigenen Bildmarken.

Das Siegel hat dieselben Pixelmaße wie `signatur-logo_de/en.png` (326×342, Siegel 326×326 mittig):
Die Bildhöhe im Block folgt aus Spaltenbreite · Höhe/Breite, so bleiben Zeilenhöhen und Blockmaße
für jede Blockbreite gleich. Außerhalb des Kreises ist es transparent (voll transparente Pixel
tragen Weiß): PDF/A-1 komponiert auf Weiß, sonst wird eine `SMask` geschrieben.

## Standard-14-Metriken

`src/afm_data.rs` enthält nur die Breiten (`WX`) der WinAnsi-Zeichen sowie Ascender, Descender,
CapHeight, XHeight und FontBBox der sieben von PDF-AS akzeptierten Standard-14-Schnitte
(Helvetica, Helvetica-Bold, Courier, Courier-Bold, Times-Roman, Times-Bold, Times-Italic). Die
Tabellen sind aus den Adobe Core 14 AFM Files abgeleitet (Generator `tools/gen_afm.py`); die
Änderung (Extraktion, keine AFM-Dateien im Repository) ist im Kopf der Datei vermerkt, der
Lizenzhinweis liegt unter `licenses/`. Kontrolle: rechtsbündige Captions der PDF-AS-Referenz-PDFs
ergeben exakt diese Breiten (`afm::tests::helvetica_bold_widths_match_pdfas_output`).

## sRGB-ICC-Profil

Für PDF/A schreibt der Block Bildmarken und farbige Füllungen als `ICCBased` (Plan §6.5). Das
Profil (ICC v2.1, Monitor, Matrix/TRC) liegt **nicht** als Datei vor, sondern wird in
`src/icc.rs` aus den Normwerten von IEC 61966-2-1 berechnet; es gibt daher keine Lizenzauflage.

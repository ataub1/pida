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

`arial.ttf` aus `pdfas-config/fonts` wird bewusst **nicht** übernommen (analyse/05 §6.3).

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

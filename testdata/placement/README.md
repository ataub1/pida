# Testdaten `pdfover-placement` (Paket F)

| Pfad | Inhalt | Herkunft |
|---|---|---|
| `pdfas-pos/<SUITE>/example.pdf`, `config.properties` | Eingaben der PDF-AS-Positionssuiten `POS_*` | Kopie aus `pdf-as-4/pdf-as-tests/src/test/test-suites/public_pdfbox2` (PDF-AS 4.4.2, EUPL) |
| `pdfas-pos/auto_pos_example.properties` | gemeinsame Suite-Konfiguration (Default `x:auto;y:auto`) | ebenda |
| `pdfas-pos/golden.json` | erwartete Blockrahmen (Pixel bei 288 dpi) aus den Referenzbildern `testdata/pdf/pdfas-suites/<SUITE>/example_ref.png`, Profil-`pos`, Request-Position, Zielseite | abgeleitet (Test `manifest_matches_reference_images` prüft gegen die Bilder) |

Die Referenzbilder selbst liegen (5,5 MB) nur einmal unter `testdata/pdf/pdfas-suites/`. QR-, Rotations-
und Feld-Testdokumente erzeugen die Tests selbst (`crates/placement/tests/common`).

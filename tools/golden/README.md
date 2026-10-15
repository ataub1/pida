# PDF-AS 4.4.2 als Golden-Orakel (Plan §8.2)

Java wird hier **nur als Test-Orakel** verwendet, nie im Produkt. Die Rust-Tests lesen ausschließlich
`testdata/golden/pdfas-4.4.2/` und brauchen weder Java noch Netz.

| Datei | Zweck |
|---|---|
| `run-pdfas.sh` | Einstieg: lädt die offizielle PDF-AS-CLI 4.4.2 (Prüfsumme fest), erzeugt Eingaben, signiert alle Fälle, schreibt Manifeste |
| `run_cases.py` | Fallmatrix (12 Profil-IDs × Eingaben × Positionen, Hinweis-/Bildmarken-/Logo-Varianten), Konfigurationsvarianten mit PDF-Over-Overrides, parallele Ausführung, Index |
| `make_inputs.py` | deterministische Eingabe-PDFs und Bildmarken |
| `MakeQr.java` | QR-Modulmatrix mit ZXing aus der CLI-Distribution (Platzhalter `PDF-AS-POS;id=…`) |
| `PlaceholderSign.java` | Signieren mit QR-Platzhaltersuche wie PDF-Over (die CLI kann sie nicht einschalten) |
| `extract.py` | Manifest je Fall (qpdf `--json=2`, pyHanko, PDF-AS-DEBUG-Log) |
| `report.py`, `report_text.py` | erzeugt `docs/integration/golden-report.md` aus Manifesten und Testergebnissen |

## Herkunft und Prüfsummen

| Artefakt | Quelle | Prüfsumme |
|---|---|---|
| `pdf-as-cli-4.4.2.zip` (verwendet) | `https://apps.egiz.gv.at/releases/pdf-as/release/4.4.2/pdf-as-cli-4.4.2.zip` (2024-09-05) | SHA-256 `09c37cc004f14537e5d0d3d83e44687a833119a5903dd03b6de5d9d907e18eee` |
| `pdf-as-cli-4.4.2.zip` (Maven, nicht verwendet, anderer Build) | `https://apps.egiz.gv.at/maven/at/gv/egiz/pdfas/pdf-as-cli/4.4.2/` | SHA-1 `4af7bd24d3a053448d001c5772ae6af7b46d9b2d` (laut `.sha1`), SHA-256 `0c6bec14f1269a0f36a81d32ffa5f1509b717cefeea5660398baa30099b96f65` |

Die CLI enthält PDFBox 2.0.32, IAIK JCE/CMS (MOA-Lizenz, nur für den Testbetrieb hier) und ZXing 3.5.0.
Sie liegt nicht im Repository, sondern im Cache `target/golden/` (`PDFOVER_GOLDEN_CACHE`).

## Ablauf

```sh
bash tools/golden/run-pdfas.sh            # alle Fälle neu (≈ 5 min, 8 JVMs)
bash tools/golden/run-pdfas.sh --only 'qr' # nur passende Fall-IDs, Index wird zusammengeführt
bash tools/golden/run-pdfas.sh --reextract # Manifeste aus target/golden/work neu (nach --keep-work)
PDFOVER_GOLDEN_REPORT_DIR=target/golden/report cargo test -p pdfover-sigblock --test golden_pdfas442
PDFOVER_GOLDEN_REPORT_DIR=target/golden/report cargo test -p pdfover-placement --test golden_pdfas442
python3 tools/golden/report.py --golden testdata/golden/pdfas-4.4.2 --reports target/golden/report \
    --out docs/integration/golden-report.md
```

Voraussetzungen: Java ≥ 17 (`PDFOVER_JAVA`, Default `/opt/homebrew/opt/openjdk@21/bin/java`), qpdf,
Python mit pyHanko und Pillow (`.venv`), `$PDFOVER_REFERENCE/pdfas-config` (wird nur gelesen und je
Variante kopiert). Netz nur für den einmaligen Download der CLI.

Hinweise:

- Signiert wird mit `testdata/keystores/test-rsa2048.p12` (Alias `test-rsa2048`, Passwort `test`,
  nur Test). Die Uhr ist nicht fest; die Tests übernehmen `SIG_DATE` aus dem Manifest.
- Die DEBUG-Logs von PDF-AS (`target/golden/work/logs`) enthalten Dokument- und Zertifikatsdaten der
  Testfälle und werden nicht eingecheckt.
- `--only` lässt die übrigen Manifeste unverändert; ein vollständiger Lauf ersetzt
  `testdata/golden/pdfas-4.4.2/cases/` und das vorsignierte Eingabedokument.
- Manifeste und Index enthalten keine lokalen Pfade: Pfade in die Konfigurationskopie stehen als
  `<cfg>/…` (`run_cases.portable_overrides`). `--reextract` behält die PDF-AS-Laufzeit je Fall.
- Eingaben für den Rasterscan gedrehter Seiten (KR-F04): `landscape-rot90/270`, `wide-rot180`,
  `a4-rot270-low`, `rot90-842-low`, `half-rot90` (Prüfung der Java2D-Pixelregel in
  `pdfover_placement::position::pdfas_rotated_scan`).

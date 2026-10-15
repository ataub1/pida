# Golden-Referenzen PDF-AS 4.4.2

Erzeugt mit `tools/golden/run-pdfas.sh` (offizielle PDF-AS-4.4.2-CLI, Test-PKCS#12
`testdata/keystores/test-rsa2048.p12`, Konfiguration `reference/pdfas-config`). Nicht von Hand ändern.

| Pfad | Inhalt |
|---|---|
| `pdfas-4.4.2/index.json` | Fallliste (Profil, Eingabe, Position, Hinweis, Bildmarke, Logo-Größe, Platzhalter), Werkzeugversionen, Prüfsumme der CLI |
| `pdfas-4.4.2/inputs/` | Eingabe-PDFs und Bildmarken (`make_inputs.py`, deterministisch; `presigned.pdf` von PDF-AS vorsigniert) |
| `pdfas-4.4.2/cases/<Fall>/manifest.json` | Seite, Widget-Rect, Holder/Inner-Form (Operationen, Fonts, Bilder), `SIG_DATE`, Orakel (pyHanko, qpdf), Werte aus dem PDF-AS-Log |
| `pdfas-4.4.2/cases/<Fall>/signed.pdf` | signierte PDF-AS-Ausgabe (nur ausgewählte Fälle, Pixelvergleich) |

Leser: `crates/sigblock/tests/golden_pdfas442.rs`, `crates/placement/tests/golden_pdfas442.rs`.
Bericht: `docs/integration/golden-report.md`.

> **NUR FÜR TESTS.** Die Signaturen stammen vom selbstsignierten Testzertifikat „PDF-Over NG Test RSA
> (NUR TEST)“. Lizenz der PDF-AS-Ausgaben und der Konfiguration: EUPL (PDF-AS 4).

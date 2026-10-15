# Testdaten `pdf-incremental` (Paket A)

| Datei | Inhalt | Herkunft |
|---|---|---|
| `pyhanko_report.py` | Test-Orakel: validiert alle Signaturen einer PDF mit pyHanko (`validate_pdf_signature`, `strict=False` wie `--no-strict-syntax`, leeres Benutzerpasswort, **ohne Netzzugriff**: `allow_fetching=False`) und gibt je Signatur Integrität, Gültigkeit, Abdeckung und Änderungsstufe als JSON aus – ohne Zertifikatsinhalte | eigenes Skript |
| `pdfa1b-aes128-cleartext-metadata.pdf` | PDF/A-1b (`TestGhostscriptPdfA.pdf`) mit AES-128 (R4), ohne Benutzerpasswort, `EncryptMetadata false` – prüft, dass der Writer XMP-Metadaten unverschlüsselt und ungefiltert fortschreibt | `qpdf --static-id --static-aes-iv --encrypt --user-password= --owner-password=test-owner --bits=128 --use-aes=y --cleartext-metadata -- pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf …` |
| `RESULTS.md` | Ergebnis des Ende-zu-Ende-Tests je Korpusdatei (Increment → `openssl cms -sign` → einbetten → pyHanko/openssl/qpdf) | `PDFOVER_UPDATE_RESULTS=1 cargo test -p pdfover-pdf-incremental --test e2e_sign` |

Die Tests finden pyHanko über `PDFOVER_PYTHON`, ein `python3` neben `pyhanko` im `PATH` oder `python3`
im `PATH` (in Worktrees: `tools/env.sh` nimmt die `.venv` des Hauptcheckouts in den `PATH`). Fehlen
openssl, qpdf oder pyHanko, werden die jeweiligen Prüfungen übersprungen. Mit `PDFOVER_E2E_OUT=<ordner>`
bleiben die signierten Ausgaben zur Analyse erhalten.

Alle Schlüssel sind Testschlüssel (`testdata/keystores`, Passwort `test`, „NUR TEST“).

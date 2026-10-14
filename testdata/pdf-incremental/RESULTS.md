# pdf-incremental – Ergebnisse Ende-zu-Ende (Paket A)

Erzeugt von `cargo test -p pdfover-pdf-incremental --test e2e_sign` (mit `PDFOVER_UPDATE_RESULTS=1` wird diese Datei aktualisiert).

Ablauf je Datei: `SourcePdf::open` → Increment mit Signaturdictionary (Reserve 8192 Byte) und leerem, unsichtbarem Widget `PDF-AS Signatur<n>` auf Seite 1 (`/F 132`, `/Rect [0 0 0 0]`) → ByteRange-Daten mit `openssl cms -sign -binary -outform DER -nosmimecap -md sha256` und `testdata/keystores/test-rsa2048.p12` signieren → `Increment::embed_contents` → Prüfung mit `openssl cms -verify -noverify`, `qpdf --check` und pyHanko (`testdata/pdf-incremental/pyhanko_report.py`: `validate_pdf_signature`, `strict=False` wie `--no-strict-syntax`, ohne Netzzugriff).

Spalten: *Neue Signatur* = Integrität / kryptografische Gültigkeit / Abdeckung (Vertrauen bewusst ohne Bedeutung, selbstsigniertes Testzertifikat); *Vorsignaturen* = pyHanko-Änderungsstufe vorher → nachher (Ziel: höchstens `FORM_FILLING` für die zuvor vollständige Signatur; `OTHER` bei älteren PDF-AS-Signaturen stammt aus den PDF-AS-Increments selbst und ist unverändert).

Werkzeuge: OpenSSL 4.0.3 29 Sep 2026 (Library: OpenSSL 4.0.3 29 Sep 2026); qpdf version 12.4.2; pyHanko 0.37.0.

**61 von 61 Dateien bestanden.** `simple-aes256-userpw.pdf` (Benutzerpasswort, K5) wird erwartungsgemäß mit `CosError::PasswordRequired` abgelehnt und ist nicht aufgeführt.

| Datei | xref (letzte Rev.) | Verschlüsselung | Vorsig. | qpdf | openssl | Neue Signatur | Vorsignaturen (pyHanko) | Ergebnis |
|---|---|---|---|---|---|---|---|---|
| `pdf-incremental/pdfa1b-aes128-cleartext-metadata.pdf` | Tabelle | Aes128 (V4 R4) | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/manySignFields-xref-stream.pdf` | xref-Stream | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-aes128-owner.pdf` | Tabelle | Aes128 (V4 R4) | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-aes256-nosign.pdf` | Tabelle | Aes256 (V5 R6) | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-aes256-owner-xref-stream.pdf` | xref-Stream | Aes256 (V5 R6) | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-aes256-owner.pdf` | Tabelle | Aes256 (V5 R6) | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-linearized-xref-stream.pdf` | xref-Stream | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-linearized.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-rc4-128-owner.pdf` | Tabelle | Rc4_128 (V2 R3) | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-xref-hybrid.pdf` | Hybrid | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-xref-stream.pdf` | xref-Stream | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/generated/simple-xref-table.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE_NOTE/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_EN/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_EN_NOTE/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_ABSOLUT_PAGE_LAST/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_ABSOLUT_PAGE_NUM_HIGH/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_DE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_DE_SMALL/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_EN/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_EN_SMALL/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_AUTO/example.pdf` | Hybrid | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_AUTO_WITH_NEWPAGE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_LAST/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf` | Tabelle | – | 1 | ok | ok | INTACT / VALID / ENTIRE_FILE | PDF-AS Signatur1: NONE → FORM_FILLING | ok |
| `pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED_FORCE_NEW/example.pdf` | Tabelle | – | 1 | ok | ok | INTACT / VALID / ENTIRE_FILE | PDF-AS Signatur1: NONE → FORM_FILLING | ok |
| `pdf/pdfas-suites/POS_LAST_WITH_POS/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_MINIMAL/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_NOTE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_PDFA/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_PDFA_NOTE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_SMALL/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_SMALL_NOTE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_MINIMAL/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_NOTE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_PDFA/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_PDFA_NOTE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_SMALL/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_SMALL_NOTE/example.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-cli/rotate_090.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-cli/rotate_180.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-cli/rotate_270.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_0.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_180.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_270.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_90.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-legacy/simple.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/1Sign_manyQR.pdf` | xref-Stream | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/data/platzhalter_en_de_test.pdf` | Tabelle | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf` | xref-Stream | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf` | xref-Stream | – | 1 | ok | ok | INTACT / VALID / ENTIRE_FILE | Signature_4: NONE → FORM_FILLING | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf` | Hybrid | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf` | Tabelle | – | 1 | ok | ok | INTACT / VALID / ENTIRE_FILE | PDF-AS Signatur1: NONE → FORM_FILLING | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf` | Tabelle | – | 2 | ok | ok | INTACT / VALID / ENTIRE_FILE | PDF-AS Signatur1: OTHER → OTHER; PDF-AS Signatur2: NONE → FORM_FILLING | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf` | Tabelle | – | 3 | ok | ok | INTACT / VALID / ENTIRE_FILE | PDF-AS Signatur1: OTHER → OTHER; PDF-AS Signatur2: OTHER → OTHER; PDF-AS Signatur3: NONE → FORM_FILLING | ok |
| `pdf/pdfas/pdf-as-web/data/dummy-pdf-signed.pdf` | Tabelle | – | 1 | ok | ok | INTACT / VALID / ENTIRE_FILE | PDF-AS Signatur1: NONE → FORM_FILLING | ok |
| `pdf/pdfas/pdf-as-web/data/enc_own.pdf` | Tabelle | Rc4_128 (V2 R3) | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |
| `pdf/pdfas/pdf-as-web/data/placeholder_sigfield_and_qr.pdf` | xref-Stream | – | 0 | ok | ok | INTACT / VALID / ENTIRE_FILE | – | ok |

## Weitere Prüfungen (gleiche Testläufe)

- `tests/corpus_increment.rs`: jede Datei → Präfix-Invariante (I1), CRLF-Beginn, ByteRange `[0 a b c]` mit `b + c` = Dateilänge, `<`/`>` an `a` bzw. `b − 1`, `0`-gefüllter Platzhalter, 36-Zeichen-Feld mit Leerzeichen (I2), Re-Parse mit pdf-cos **und** lopdf (I5), Querverweis im Format der Vorrevision mit `/Prev`, `/ID[0]` beibehalten, `/ID[1]` neu, `/Encrypt` übernommen, `/Contents` unverschlüsselt, Schlüsselreihenfolge des Signaturdictionarys wie PDF-AS, `qpdf --check` ohne neue Befunde.
- Mehrfachsignatur: drei aufeinanderfolgende Increments je Tabelle, xref-Stream, Hybrid, linearisiert, AES-256, RC4-128 und vorsigniert (`manySignFields_signed4.pdf`); jede Runde endet an einer Revisionsgrenze.
- Erzwungene Formate (Tabelle, xref-Stream, Hybrid) auf `POS_LAST_ALREADY_SIGNED/example.pdf`: neue Signatur intakt/gültig, Vorsignatur höchstens `FORM_FILLING`, `qpdf --check` ohne Befund.
- Befüllen eines vorhandenen leeren Signaturfelds (`manySignFields.pdf` mit und ohne Object Streams): `/Rect` bleibt, Signatur intakt und gültig.
- `tests/builder.rs`: Fehlerfälle, Längenprüfung (N passt, N+1 und 2N abgewiesen), Platzhalter-Layout, Verschlüsselung neuer Objekte (RC4-128, AES-128, AES-256, xref-Stream), unverschlüsselte und ungefilterte XMP-Metadaten bei `EncryptMetadata false`, Originalschreibweise unveränderter Reals, Vorspann vor dem Header, korrigierter `startxref` (K2), rekonstruierter Querverweis wird abgelehnt.

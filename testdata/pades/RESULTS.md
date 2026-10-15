# pades – Ergebnisse Ende-zu-Ende (Welle 2, Paket I1)

Signier-Pipeline von `pdfover-pades` (Ersatz für PDF-AS `sign`) gegen den Korpus `testdata/pdf/`
mit externen Orakeln. Die Abschnitte unten erzeugen die Tests selbst; aktualisieren mit

```sh
PDFOVER_UPDATE_RESULTS=1 PDFOVER_PDFIUM_DIR=<pdfium/lib> cargo test -p pdfover-pades --test e2e_corpus --test e2e_matrix --test e2e_flows
```

Ohne `PDFOVER_UPDATE_RESULTS` landen die Tabellen nur unter `$CARGO_TARGET_TMPDIR/pades-results-*.md`.
Fehlt ein Werkzeug (qpdf, openssl, pyHanko in `.venv`, PDFium), wird die jeweilige Prüfung übersprungen
und in der Spalte mit `–` vermerkt.

**Ablauf je Fall (Modell A):** `SourcePdf::open` → `analyze` (Annahmeklasse K1–K5; K5 → `prepare`
lehnt mit dem Befund-Code ab) → `plan_signature` (Profil aus `ProfileStore::builtin`, Breite/Position
über `pdfover-placement` mit PDFium-Rasterscan, Widget-Rect, Rotation, QR-Platzhalter, PDF/A-Modus,
PDF/UA) → `prepare` (Increment über `pdf-incremental`, Reserve `estimate_cms_len` + 1 KiB) →
`KeystoreSession::sign` (`pdfover-backend-keystore`, Testschlüssel `testdata/keystores`, ADR 0001
§8.3: Hash der ByteRange-Daten → SignedAttrs → Hash → Rohsignatur) → `finish_raw` (`assemble` mit
demselben CMS-Profil, I3-Prüfung) → `SignedPdf::self_check` (I1–I6, mit PDFium) → Orakel.

**Modell B:** Fake-BKU als `SignatureBackend` (Abschnitt *Abläufe*): `DocumentSource` um `Prepared`
(Blackout-PDF + ExcludedByteRange), die BKU prüft die genullte Lücke und signiert die Bytes außerhalb
mit `openssl cms -sign -binary -nosmimecap -cades -md sha256` → `finish_cms` (Reserve 16 KiB).

**Orakel:** `qpdf --check` (keine neuen Befunde gegenüber dem Original), `openssl cms -verify -binary
-noverify` über die ByteRange-Daten, PDFium-Render vorher/nachher (Block mit Inhalt im erwarteten
Rechteck, außerhalb keine Pixeländerung; unsichtbar: Seite unverändert), pyHanko
(`testdata/pades/pyhanko_report.py`, `validate_pdf_signature`, `strict=False`, ohne Netz; neue Signatur
INTACT/VALID/vertraut/ENTIRE_FILE, Vorsignaturen behalten Integrität und Gültigkeit, die zuvor
vollständige erreicht höchstens `FORM_FILLING`).

Weitere Tests ohne eigene Tabelle: `analyze_synthetic.rs` (DocMDP P=1/2/3, `/Lock`, Seed Values,
XFA hybrid/dynamisch, UR3, Fremdinhalt nach `%%EOF`, Feldnummerierung), `e2e_flows.rs` (CMS zu groß →
`cms-too-large` → Neuvorbereitung mit `suggested_reserve`, falscher Signer/Digest, Persistenz
`persist`/`restore` mit Manipulationsprüfung, `write_atomically`, Selbstprüfung mit falschem
Original/Signer, neue Seite auf signiertem Dokument, PDF/UA-Struktur, PDF/A-OutputIntent-Strategie),
`robustness.rs` (Mutationen des Korpus: keine Panics; bei intaktem Querverweis besteht jede erzeugte
Signatur die Selbstprüfung).

**Review (adversarial, Welle 2):** `review_adversarial.rs` mit denselben Orakeln über `run_case`
(qpdf, openssl, PDFium-Pixel, pyHanko): `finish_cms` erzwingt das Zertifikat aus `prepare` auch gegen
eine abweichende Erwartung des Aufrufers; die Begleitdatei von `persist` ist über
`PreparedHandle::sha256` (PDF ‖ JSON) geschützt; K2-Befunde mit Vorsignaturen lehnt auch `prepare` ab;
ein von zwei Seiten geteiltes indirektes `/Annots` wird nicht verlängert (direkte Kopie auf der
Zielseite); CMS genau in Reservegröße passt, ein Byte mehr ergibt `cms-too-large`; vorhandenes
hierarchisches Feld `Unterschriften.Kunde` mit eigenem Widget-Kind (sichtbar und unsichtbar);
indirektes `/DR` mit direktem `/Font` (Vorgängerfassung meldete fälschlich I6); FieldMDP im
Signaturdictionary sperrt Felder ohne `/Lock`; unlesbares Signaturdictionary zählt als Vorsignatur;
zertifizierte Dokumente (`certified-docmdp-p2.pdf`, `certified-docmdp-p3.pdf`, erzeugt mit pyHanko über
`make_certified.py` aus `simple.pdf` und `test-ecp256.p12`): vorhandenes leeres Feld (sichtbar und
unsichtbar) und neue unsichtbare Signatur lassen die Zertifizierung bei `FORM_FILLING`
(`docmdp_ok`), ein neues sichtbares Feld ergab `OTHER`/`docmdp_ok=false` und wird jetzt abgelehnt
(`pades-new-visible-field-not-allowed`).
Die I6-Whitelist ist rollengenau (Unit-Tests in `src/check.rs`: fremde Arrays wie `/Contents`,
`/Kids` nicht-terminaler Felder, Inhalt eines direkten AcroForms).

**Welle 3 (Härtung):** Mutationstest `robustness.rs` nach der letzten Korrektur mit 32 Startwerten
× 2500 Mutationen je Datei (560 000 Mutationen, Release, je sichtbar und unsichtbar signiert): kein
Panic, bei intaktem Original keine Verletzung mehr, auch nicht I5. Behoben in
`pdf-cos`: Querverweiseinträge, die auf den Kopf eines anderen Objekts zeigen (lopdf überschrieb
dessen gültige Fassung, `repro-xref-misassigned.pdf`); `/StrF`/`/StmF` fehlend, `/Identity` oder
unbekannt (lopdf entschlüsselte mit RC4; jetzt spezifikationsgemäß, unbekannte Namen K5);
Originalschreibweise fehlerhafter Reals (`0.595.32`) wird nicht übernommen; komprimierte Einträge
in einem Container, der kein Object Stream ist, gelten als repariert (K2); ein Blatt des Seitenbaums
mit fremdem `/Type` (z. B. der Katalog nach `/Pages 1 0 R`) ist keine Seite. In `pades`:
Werte, die der PDFBox-Fixup ersetzt, weil sie beschädigt sind (`/DR /Font` kein Dictionary,
`/AcroForm` kein Dictionary, `/Fields` kein Array, `/DA` kein String), lässt I6 genau so zu;
eine Vorsignatur, deren `/ByteRange` über das Dateiende reicht, lehnen `analyze`/`prepare` vorab ab
(K5 `pdf-prior-signature-beyond-eof`, vorher I4 in der Selbstprüfung). Regressionstests:
`known_mutations_with_intact_xref_sign_cleanly` (seit dem Review auch `/StrF` bzw. `/StmF` fehlend
in `simple-aes128-owner.pdf`: Ausgabe mit pyHanko gültig und intakt, qpdf meldet nur die
Beschädigung der Eingabe) und `missing_or_unknown_crypt_filter_names_follow_the_spec` (pdf-cos). Bei beschädigtem Querverweis (K2) scheitert I5/I6
weiterhin vereinzelt fail-safe (die Ausgabe erbt die Beschädigung; qpdf liest sie ebenso wenig).
`fuzz_prepare.rs` (gesamter Korpus, `open` → `analyze` → `plan_signature` → `prepare`, 8 Threads):
201 300, 610 000 und zweimal 244 000 Mutationen ohne Panic und ohne Hänger (> 30 s je Fall). PDF/A:
`verapdf.rs` (Strukturprüfung immer; veraPDF 1.28.2 über `tools/verapdf/fetch.sh`, sonst übersprungen).

<!-- abschnitt:korpus -->
## Korpus (Modell A, Keystore-Backend)

Erzeugt von `cargo test -p pdfover-pades --test e2e_corpus`. Je Datei rotieren Schlüssel (RSA-2048, P-256, P-384), Profil (`INVISIBLE`, `SIGNATURBLOCK_SMALL_DE`, `SIGNATURBLOCK_SMALL_EN_NOTE`, `AMTSSIGNATURBLOCK_DE_RECOMMENDED`; PDF/A-Eingaben `SIGNATURBLOCK_SMALL_DE_PDFA`) und CMS-Profil (`pdfas-compat`, `pades-b-b`). Ablauf: `analyze` → `plan_signature` (PDFium-Rasterscan) → `prepare` → `KeystoreSession::sign` (Modell A) → `finish_raw` → `self_check` (I1–I6, PDFium) → Orakel.

Spalten: *qpdf* = `qpdf --check` ohne neue Befunde; *openssl* = `openssl cms -verify -noverify` über die ByteRange-Daten; *Bild* = PDFium-Render vorher/nachher (Block mit Inhalt im erwarteten Rechteck, außerhalb keine Änderung; unsichtbar: Seite unverändert); *Neue Signatur* = pyHanko Integrität / Gültigkeit / Abdeckung (Vertrauensanker = Testzertifikate); *Vorsignaturen* = pyHanko-Änderungsstufe vorher → nachher; *Hinweise* = `prepare`-/Positionierungshinweise.

Werkzeuge: OpenSSL 4.0.3 29 Sep 2026 (Library: OpenSSL 4.0.3 29 Sep 2026); qpdf version 12.4.2; pyHanko 0.37.0; PDFium chromium/8100.

**61 von 61 Dateien bestanden.**

| Datei | xref | Verschl. | Vorsig. | Klasse | Schlüssel | Profil | CMS | Ergebnis | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `pdf/generated/manySignFields-xref-stream.pdf` | xref-Stream | – | 0 | K1 | RSA-2048 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-aes128-owner.pdf` | Tabelle | Aes128 (R4) | 0 | K3 | P-256 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-aes256-nosign.pdf` | Tabelle | Aes256 (R6) | 0 | K5 | P-384 | INVISIBLE | pdfas-compat | abgelehnt (pdf-permissions) | – | – | – | – | – | – | ok |
| `pdf/generated/simple-aes256-owner-xref-stream.pdf` | xref-Stream | Aes256 (R6) | 0 | K3 | RSA-2048 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (4449 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-aes256-owner.pdf` | Tabelle | Aes256 (R6) | 0 | K3 | P-256 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (4548 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-aes256-userpw.pdf` | – | – | 0 | K5 | P-384 | – | pdfas-compat | abgelehnt beim Öffnen (pdf-user-password) | – | – | – | – | – | – | ok |
| `pdf/generated/simple-linearized-xref-stream.pdf` | xref-Stream | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_EN_NOTE | pdfas-compat | signiert | ok | ok | Block sichtbar (7875 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-linearized.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_EN_NOTE | pdfas-compat | signiert | ok | ok | Block sichtbar (7962 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-rc4-128-owner.pdf` | Tabelle | Rc4_128 (R3) | 0 | K3 | P-384 | SIGNATURBLOCK_SMALL_EN_NOTE | pdfas-compat | signiert | ok | ok | Block sichtbar (7839 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-xref-hybrid.pdf` | Hybrid | – | 0 | K1 | RSA-2048 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pdfas-compat | signiert | ok | ok | Block sichtbar (20383 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-xref-stream.pdf` | xref-Stream | – | 0 | K1 | P-256 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pdfas-compat | signiert | ok | ok | Block sichtbar (20799 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-xref-table.pdf` | Tabelle | – | 0 | K1 | P-384 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pdfas-compat | signiert | ok | ok | Block sichtbar (21405 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-cli/rotate_090.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | INVISIBLE | pades-b-b | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-cli/rotate_180.pdf` | Tabelle | – | 0 | K1 | P-256 | INVISIBLE | pades-b-b | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-cli/rotate_270.pdf` | Tabelle | – | 0 | K1 | P-384 | INVISIBLE | pades-b-b | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_0.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE | pades-b-b | signiert | ok | ok | Block sichtbar (4456 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_180.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE | pades-b-b | signiert | ok | ok | Block sichtbar (4553 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_270.pdf` | Tabelle | – | 0 | K1 | P-384 | SIGNATURBLOCK_SMALL_DE | pades-b-b | signiert | ok | ok | Block sichtbar (4799 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-cli/simple_rotated_90.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_EN_NOTE | pades-b-b | signiert | ok | ok | Block sichtbar (7871 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-legacy/simple.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_EN_NOTE | pades-b-b | signiert | ok | ok | Block sichtbar (7956 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/1Sign_manyQR.pdf` | xref-Stream | – | 0 | K1 | P-384 | SIGNATURBLOCK_SMALL_EN_NOTE | pades-b-b | signiert | ok | ok | Block sichtbar (7843 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/data/platzhalter_en_de_test.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE_PDFA | pades-b-b | signiert | ok | ok | Block sichtbar (5043 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf` | xref-Stream | – | 0 | K1 | P-256 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pades-b-b | signiert | ok | ok | Block sichtbar (20816 px) | INTACT / VALID / ENTIRE_FILE | – | placement.notice.overlaps_widget | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/manySignFields_signed4.pdf` | xref-Stream | – | 1 | K1 | P-384 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pades-b-b | signiert | ok | ok | Block sichtbar (21417 px) | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | pades-pdfua-skipped, placement.notice.overlaps_widget | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf` | Hybrid | – | 0 | K1 | RSA-2048 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf` | Tabelle | – | 1 | K1 | P-256 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | pades-pdfua-skipped | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed.pdf` | Tabelle | – | 2 | K1 | P-384 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | OTHER → OTHER; NONE → FORM_FILLING | pades-pdfua-skipped | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf` | Tabelle | – | 3 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (4458 px) | INTACT / VALID / ENTIRE_FILE | OTHER → OTHER; OTHER → OTHER; NONE → FORM_FILLING | pades-pdfua-skipped | ok |
| `pdf/pdfas/pdf-as-web/data/dummy-pdf-signed.pdf` | Tabelle | – | 1 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (4549 px) | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | pades-pdfua-skipped | ok |
| `pdf/pdfas/pdf-as-web/data/enc_own.pdf` | Tabelle | Rc4_128 (R3) | 0 | K3 | P-384 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (5308 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-web/data/placeholder_sigfield_and_qr.pdf` | xref-Stream | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_EN_NOTE | pdfas-compat | signiert | ok | ok | Block sichtbar (7877 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE_PDFA | pdfas-compat | signiert | ok | ok | Block sichtbar (4626 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE_NOTE/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | K1 | P-384 | SIGNATURBLOCK_SMALL_DE_PDFA | pdfas-compat | signiert | ok | ok | Block sichtbar (4876 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_EN/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE_PDFA | pdfas-compat | signiert | ok | ok | Block sichtbar (4532 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_EN_NOTE/TestGhostscriptPdfA.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE_PDFA | pdfas-compat | signiert | ok | ok | Block sichtbar (4629 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_ABSOLUT_PAGE_LAST/example.pdf` | Tabelle | – | 0 | K1 | P-384 | SIGNATURBLOCK_SMALL_DE_PDFA | pdfas-compat | signiert (neue Seite) | ok | ok | Block sichtbar (5374 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_ABSOLUT_PAGE_NUM_HIGH/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE_PDFA | pades-b-b | signiert (neue Seite) | ok | ok | Block sichtbar (5037 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_DE/example.pdf` | Tabelle | – | 0 | K1 | P-256 | INVISIBLE | pades-b-b | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_DE_SMALL/example.pdf` | Tabelle | – | 0 | K1 | P-384 | INVISIBLE | pades-b-b | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_EN/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE | pades-b-b | signiert | ok | ok | Block sichtbar (4457 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_AMTSSIGNATURBLOCK_EN_SMALL/example.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE | pades-b-b | signiert | ok | ok | Block sichtbar (4554 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_AUTO/example.pdf` | Hybrid | – | 0 | K1 | P-384 | SIGNATURBLOCK_SMALL_DE | pades-b-b | signiert | ok | ok | Block sichtbar (5309 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_AUTO_WITH_NEWPAGE/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE_PDFA | pades-b-b | signiert (neue Seite) | ok | ok | Block sichtbar (5040 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_LAST/example.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE_PDFA | pades-b-b | signiert (neue Seite) | ok | ok | Block sichtbar (5123 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED/example.pdf` | Tabelle | – | 1 | K1 | P-384 | SIGNATURBLOCK_SMALL_DE_PDFA | pades-b-b | signiert | ok | ok | Block sichtbar (5370 px) | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | pades-pdfua-skipped, placement.notice.new_page_not_allowed | ok |
| `pdf/pdfas-suites/POS_LAST_ALREADY_SIGNED_FORCE_NEW/example.pdf` | Tabelle | – | 1 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE_PDFA | pades-b-b | signiert | ok | ok | Block sichtbar (5033 px) | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | pades-pdfua-skipped, placement.notice.new_page_not_allowed | ok |
| `pdf/pdfas-suites/POS_LAST_WITH_POS/example.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE_PDFA | pades-b-b | signiert (neue Seite) | ok | ok | Block sichtbar (5132 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE/example.pdf` | Tabelle | – | 0 | K1 | P-384 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pades-b-b | signiert | ok | ok | Block sichtbar (21423 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_MINIMAL/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_NOTE/example.pdf` | Tabelle | – | 0 | K1 | P-256 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_PDFA/example.pdf` | Tabelle | – | 0 | K1 | P-384 | INVISIBLE | pdfas-compat | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_PDFA_NOTE/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (4460 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_SMALL/example.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (4553 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_DE_SMALL_NOTE/example.pdf` | Tabelle | – | 0 | K1 | P-384 | SIGNATURBLOCK_SMALL_DE | pdfas-compat | signiert | ok | ok | Block sichtbar (4799 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | SIGNATURBLOCK_SMALL_EN_NOTE | pdfas-compat | signiert | ok | ok | Block sichtbar (7880 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_MINIMAL/example.pdf` | Tabelle | – | 0 | K1 | P-256 | SIGNATURBLOCK_SMALL_EN_NOTE | pdfas-compat | signiert | ok | ok | Block sichtbar (7959 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_NOTE/example.pdf` | Tabelle | – | 0 | K1 | P-384 | SIGNATURBLOCK_SMALL_EN_NOTE | pdfas-compat | signiert | ok | ok | Block sichtbar (7836 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_PDFA/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pdfas-compat | signiert | ok | ok | Block sichtbar (20580 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_PDFA_NOTE/example.pdf` | Tabelle | – | 0 | K1 | P-256 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pdfas-compat | signiert | ok | ok | Block sichtbar (20810 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_SMALL/example.pdf` | Tabelle | – | 0 | K1 | P-384 | AMTSSIGNATURBLOCK_DE_RECOMMENDED | pdfas-compat | signiert | ok | ok | Block sichtbar (21416 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas-suites/POS_SIGNATURBLOCK_EN_SMALL_NOTE/example.pdf` | Tabelle | – | 0 | K1 | RSA-2048 | INVISIBLE | pades-b-b | signiert | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |

<!-- /abschnitt:korpus -->

<!-- abschnitt:ablaeufe -->
## Abläufe (Mehrfachsignatur, QR-Platzhalter, vorhandene Felder, Modell B)

Erzeugt von `cargo test -p pdfover-pades --test e2e_flows`. Jeder Schritt verwendet die Ausgabe des vorigen als Eingabe; *Vorsignaturen* zeigt die pyHanko-Änderungsstufe der älteren Signaturen vorher → nachher. Modell B: Die Fake-BKU erhält das Blackout-PDF und die ExcludedByteRange über `DocumentSource` (wie der Workflow-Adapter), prüft die genullte Lücke, signiert die Bytes außerhalb mit `openssl cms -sign -binary -nosmimecap -cades -md sha256` und liefert `SignResult::Cms`; `finish_cms` prüft I3 gegen das von `certificate()` gemeldete Zertifikat, die Reserve ist 16 KiB.

Werkzeuge: OpenSSL 4.0.3 29 Sep 2026 (Library: OpenSSL 4.0.3 29 Sep 2026); qpdf version 12.4.2; pyHanko 0.37.0; PDFium chromium/8100.

### Mehrfachsignatur (3 neue Felder nacheinander, Modell A)

**3 von 3 bestanden.**

| Fall | Vorsig. | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |
|---|---|---|---|---|---|---|---|---|
| `simple.pdf` Schritt 1 (RSA-2048, SIGNATURBLOCK_SMALL_DE) | 0 | ok | ok | Block sichtbar (4453 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` Schritt 2 (P-256, SIGNATURBLOCK_SMALL_EN_NOTE) | 1 | ok | ok | Block sichtbar (7963 px) | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | – | ok |
| `simple.pdf` Schritt 3 (P-384, SIGNATURBLOCK_SMALL_DE) | 2 | ok | ok | Block sichtbar (4798 px) | INTACT / VALID / ENTIRE_FILE | FORM_FILLING → FORM_FILLING; NONE → FORM_FILLING | – | ok |

### QR-Platzhalter nacheinander (benutzt: Image5, Image8)

**3 von 3 bestanden.**

| Fall | Vorsig. | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |
|---|---|---|---|---|---|---|---|---|
| `new_qr_2-2.pdf` Platzhalter Schritt 1 (RSA-2048) | 0 | ok | ok | Block sichtbar (4444 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `new_qr_2-2.pdf` Platzhalter Schritt 2 (P-256) | 1 | ok | ok | Block sichtbar (4530 px) | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | pades-pdfua-skipped | ok |
| `new_qr_2-2.pdf` Platzhalter Schritt 3 (P-384) | 2 | ok | ok | Block sichtbar (4793 px) | INTACT / VALID / ENTIRE_FILE | FORM_FILLING → FORM_FILLING; NONE → FORM_FILLING | pades-pdfua-skipped | ok |

### Vorhandene leere Signaturfelder (nacheinander)

**3 von 3 bestanden.**

| Fall | Vorsig. | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |
|---|---|---|---|---|---|---|---|---|
| `manySignFields.pdf` Feld 1 (SIGNATURBLOCK_SMALL_DE, RSA-2048) | 0 | ok | ok | Block sichtbar (4970 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `manySignFields.pdf` Feld 2 (INVISIBLE, P-256) | 1 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | pades-pdfua-skipped | ok |
| `manySignFields.pdf` Feld 3 (AMTSSIGNATURBLOCK_DE_RECOMMENDED, P-384) | 2 | ok | ok | Block sichtbar (1464 px) | INTACT / VALID / ENTIRE_FILE | FORM_FILLING → FORM_FILLING; NONE → FORM_FILLING | pades-pdfua-skipped | ok |

### Modell B (Fake-BKU: Blackout-PDF + ExcludedByteRange, `openssl cms -sign -cades`)

**7 von 7 bestanden.**

| Fall | Vorsig. | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |
|---|---|---|---|---|---|---|---|---|
| `pdf/pdfas/pdf-as-legacy/simple.pdf` Modell B (SIGNATURBLOCK_SMALL_DE, RSA-2048) | 0 | ok | ok | – | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-aes256-owner.pdf` Modell B (SIGNATURBLOCK_SMALL_DE, P-256) | 0 | ok | ok | – | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-xref-stream.pdf` Modell B (INVISIBLE, P-384) | 0 | ok | ok | – | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-xref-hybrid.pdf` Modell B (AMTSSIGNATURBLOCK_DE_RECOMMENDED, RSA-2048) | 0 | ok | ok | – | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/generated/simple-linearized.pdf` Modell B (SIGNATURBLOCK_SMALL_EN_NOTE, P-256) | 0 | ok | ok | – | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed.pdf` Modell B (SIGNATURBLOCK_SMALL_DE, P-384) | 1 | ok | ok | – | INTACT / VALID / ENTIRE_FILE | NONE → FORM_FILLING | – | ok |
| `pdf/pdfas-suites/PDFA_SIGNATURBLOCK_DE/TestGhostscriptPdfA.pdf` Modell B (SIGNATURBLOCK_SMALL_DE_PDFA, RSA-2048) | 0 | ok | ok | – | INTACT / VALID / ENTIRE_FILE | – | – | ok |


<!-- /abschnitt:ablaeufe -->

<!-- abschnitt:matrix -->
## Matrix (Modell A, Keystore-Backend)

Erzeugt von `cargo test -p pdfover-pades --test e2e_matrix`: `simple.pdf` × 3 Schlüssel × 5 Profile × 2 CMS-Profile, PDF/A-1b-Eingabe × 3 Schlüssel × `SIGNATURBLOCK_SMALL_DE_PDFA` × 2 CMS-Profile, alle 12 PDF-Over-Profil-IDs (BASE_LOGO mit dem Laufzeit-Overlay 23 mm). Orakel und Spalten wie im Korpus.

Werkzeuge: OpenSSL 4.0.3 29 Sep 2026 (Library: OpenSSL 4.0.3 29 Sep 2026); qpdf version 12.4.2; pyHanko 0.37.0; PDFium chromium/8100.

**48 von 48 Fällen bestanden.**

| Fall | Vorsig. | qpdf | openssl | Bild | Neue Signatur | Vorsignaturen | Hinweise | ok |
|---|---|---|---|---|---|---|---|---|
| `simple.pdf` RSA-2048 INVISIBLE pdfas-compat | 0 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 INVISIBLE pades-b-b | 0 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE pdfas-compat | 0 | ok | ok | Block sichtbar (4457 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE pades-b-b | 0 | ok | ok | Block sichtbar (4455 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_EN_NOTE pdfas-compat | 0 | ok | ok | Block sichtbar (7878 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_EN_NOTE pades-b-b | 0 | ok | ok | Block sichtbar (7879 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 AMTSSIGNATURBLOCK_DE_RECOMMENDED pdfas-compat | 0 | ok | ok | Block sichtbar (20580 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 AMTSSIGNATURBLOCK_DE_RECOMMENDED pades-b-b | 0 | ok | ok | Block sichtbar (20580 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4534 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE_PDFA pades-b-b | 0 | ok | ok | Block sichtbar (4534 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `TestGhostscriptPdfA.pdf` (PDF/A-1b) RSA-2048 SIGNATURBLOCK_SMALL_DE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4527 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `TestGhostscriptPdfA.pdf` (PDF/A-1b) RSA-2048 SIGNATURBLOCK_SMALL_DE_PDFA pades-b-b | 0 | ok | ok | Block sichtbar (4527 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 INVISIBLE pdfas-compat | 0 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 INVISIBLE pades-b-b | 0 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 SIGNATURBLOCK_SMALL_DE pdfas-compat | 0 | ok | ok | Block sichtbar (4558 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 SIGNATURBLOCK_SMALL_DE pades-b-b | 0 | ok | ok | Block sichtbar (4558 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 SIGNATURBLOCK_SMALL_EN_NOTE pdfas-compat | 0 | ok | ok | Block sichtbar (7969 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 SIGNATURBLOCK_SMALL_EN_NOTE pades-b-b | 0 | ok | ok | Block sichtbar (7969 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 AMTSSIGNATURBLOCK_DE_RECOMMENDED pdfas-compat | 0 | ok | ok | Block sichtbar (20818 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 AMTSSIGNATURBLOCK_DE_RECOMMENDED pades-b-b | 0 | ok | ok | Block sichtbar (20806 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 SIGNATURBLOCK_SMALL_DE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4625 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-256 SIGNATURBLOCK_SMALL_DE_PDFA pades-b-b | 0 | ok | ok | Block sichtbar (4625 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `TestGhostscriptPdfA.pdf` (PDF/A-1b) P-256 SIGNATURBLOCK_SMALL_DE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4619 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `TestGhostscriptPdfA.pdf` (PDF/A-1b) P-256 SIGNATURBLOCK_SMALL_DE_PDFA pades-b-b | 0 | ok | ok | Block sichtbar (4619 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 INVISIBLE pdfas-compat | 0 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 INVISIBLE pades-b-b | 0 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 SIGNATURBLOCK_SMALL_DE pdfas-compat | 0 | ok | ok | Block sichtbar (4792 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 SIGNATURBLOCK_SMALL_DE pades-b-b | 0 | ok | ok | Block sichtbar (4795 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 SIGNATURBLOCK_SMALL_EN_NOTE pdfas-compat | 0 | ok | ok | Block sichtbar (7837 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 SIGNATURBLOCK_SMALL_EN_NOTE pades-b-b | 0 | ok | ok | Block sichtbar (7837 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 AMTSSIGNATURBLOCK_DE_RECOMMENDED pdfas-compat | 0 | ok | ok | Block sichtbar (21396 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 AMTSSIGNATURBLOCK_DE_RECOMMENDED pades-b-b | 0 | ok | ok | Block sichtbar (21396 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 SIGNATURBLOCK_SMALL_DE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4871 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` P-384 SIGNATURBLOCK_SMALL_DE_PDFA pades-b-b | 0 | ok | ok | Block sichtbar (4871 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `TestGhostscriptPdfA.pdf` (PDF/A-1b) P-384 SIGNATURBLOCK_SMALL_DE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4871 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `TestGhostscriptPdfA.pdf` (PDF/A-1b) P-384 SIGNATURBLOCK_SMALL_DE_PDFA pades-b-b | 0 | ok | ok | Block sichtbar (4873 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE pdfas-compat | 0 | ok | ok | Block sichtbar (4454 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE_NOTE pdfas-compat | 0 | ok | ok | Block sichtbar (8095 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4522 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_DE_NOTE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (8201 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_EN pdfas-compat | 0 | ok | ok | Block sichtbar (4353 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_EN_NOTE pdfas-compat | 0 | ok | ok | Block sichtbar (7879 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_EN_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (4364 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 SIGNATURBLOCK_SMALL_EN_NOTE_PDFA pdfas-compat | 0 | ok | ok | Block sichtbar (7972 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 AMTSSIGNATURBLOCK_DE_RECOMMENDED pdfas-compat | 0 | ok | ok | Block sichtbar (20572 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 AMTSSIGNATURBLOCK_EN_RECOMMENDED pdfas-compat | 0 | ok | ok | Block sichtbar (20263 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 BASE_LOGO pdfas-compat | 0 | ok | ok | Block sichtbar (1936 px) | INTACT / VALID / ENTIRE_FILE | – | – | ok |
| `simple.pdf` RSA-2048 INVISIBLE pdfas-compat | 0 | ok | ok | unverändert | INTACT / VALID / ENTIRE_FILE | – | – | ok |

<!-- /abschnitt:matrix -->

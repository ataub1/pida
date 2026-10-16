# Testdaten Prüfmodus (ADR 0004 §3.9, §3.17)

> **NUR FÜR TESTS.** Signiert mit den Testschlüsseln aus `testdata/keystores` bzw. dem QES-Testzertifikat aus
> `keys/` (selbstsigniert, Passwort `test`, Subject mit `(NUR TEST)`). Keine echten Personendaten.

Erzeugt vom Generator des Kerns (`crates/pades/tests/verify_testdata.rs`); der Test prüft die Dateien gegen
die Tabelle (pyHanko bestätigt Integrität und Abdeckung):

```text
testdata/verify/make-keys.sh                                    # QES-Testzertifikat (openssl, einmalig)
PIDA_BLESS=1 cargo test -p pida-pades --test verify_testdata    # Dateien neu erzeugen (openssl ≥ 3)
```

Eingabe ist `pdf/generated/simple-xref-table.pdf` (eine Seite 612 × 792 pt), für `rotated-90.pdf`
`pdf/pdfas/pdf-as-cli/simple_rotated_90.pdf` (`/Rotate 90`).

| Datei | Inhalt | Erwartet |
|---|---|---|
| `unsigned.pdf` | ohne Signatur | `no_signatures` |
| `pdfas-signed.pdf` | eigene Engine, PDF-AS-kompatibel (`PdfasCompat`), RSA-2048, sichtbar | `intact`, `whole_document`, Unterzeichner `PDF-Over NG Test RSA (NUR TEST)`, SHA-256, `all_intact` |
| `tampered.pdf` | wie `pdfas-signed.pdf`, ein Byte im signierten Bereich geändert | `broken` (`cms-digest-mismatch`), Unterzeichner trotzdem genannt, `some_broken` |
| `pkcs7-sha1.pdf` | `/SubFilter /adbe.pkcs7.sha1`, CMS über den SHA-1-Wert (openssl) | `not_verifiable` (`pdf-signature-subfilter`), `some_not_verifiable` |
| `cades-sha1.pdf` | `ETSI.CAdES.detached` mit SHA-1 und ESS signing-certificate v1 (openssl) | `intact` mit `ng-verify-warning-sha1`, `ng-verify-warning-signing-certificate-v1` |
| `two-signatures.pdf` | RSA-Signatur, danach P-256-Signatur in einer späteren Revision | 1: `revision` (1 spätere Revision, 1 spätere Signatur), 2: `whole_document`; `all_intact` |
| `modified-after-signing.pdf` | `pdfas-signed.pdf` + Increment ohne Signatur (Katalog geändert) | `intact`, `revision` (1 spätere Revision, 0 Signaturen), `modified_after_last_signature` |
| `invisible.pdf` | Profil `INVISIBLE`, P-256 | `intact`, keine `appearance` |
| `qes.pdf` | QES-Testzertifikat P-256 mit qcStatements (QcCompliance, QcSSCD, QcType esign) | `qualified { qscd: true }` – „laut Zertifikat“ |
| `rotated-90.pdf` | gedrehte Seite, Block an `p:1;x:100;y:500` (sichtbarer Raum) | `appearance` auf Seite 0 mit `left = 100`, `top = 500` in `PAGE_ABS` |

Zusätzlich prüft `verify_testdata.rs` ohne eigene Datei: Bytes in der Lücke der `/ByteRange` neben `/Contents`
(Signature Wrapping, aus `pdfas-signed.pdf` im Speicher erzeugt) → `broken` (`pdf-signature-byterange`, wie pyHanko
„coverage unclear“), und die Ausgaben der echten PDF-AS-4.4.2-CLI (`testdata/golden/pdfas-4.4.2/cases/*/signed.pdf`,
`pdf/pdfas/pdf-as-pdfbox-2/new_qr_2_signed_signed_signed.pdf`) → `intact`, Abdeckung wie pyHanko.

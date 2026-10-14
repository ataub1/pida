# Testdaten `pdfover-cades` (Paket B)

> **NUR FÜR TESTS.** Eigene Test-PKI, unverschlüsselte PKCS#8-Testschlüssel, Subjects mit `(NUR TEST)`.

Erzeugt mit [`make-testdata.sh`](make-testdata.sh) (`openssl` ≥ 3.2, `python3` mit `asn1crypto`, z. B. aus der
`.venv` mit pyHanko; `--force` erzeugt neue Schlüssel).

| Datei | Inhalt |
|---|---|
| `content.bin` | deterministischer Signaturinhalt (~4,7 KiB, alle Bytewerte) |
| `test-*.key.p8.der` | PKCS#8 der gemeinsamen Testschlüssel aus `testdata/keystores` |
| `ca-root.crt.der`, `ca-intermediate.crt.der` | Test-PKI: Root (P-384) → Zwischen-CA (P-256) |
| `leaf-p521.*`, `leaf-rsa3072.*` | Endzertifikate der Zwischen-CA (mit SubjectKeyIdentifier) + PKCS#8 |
| `chain-p521.pem` | Kette Endzertifikat, Zwischen-CA, Root |
| `selfsigned-rsa4608.*`, `openssl-rsa4608-sha256-cades.cms.der` | selbstsigniertes RSA-4608 (über der 4096-Bit-Grenze von `rsa` beim DER-Import) + PKCS#8 + OpenSSL-CMS (Review-Zusatz, Abschnitt 0 des Skripts) |
| `openssl-*.cms.der` | CMS von `openssl cms -sign -binary` über `content.bin` (detached): RSA/P-256/P-384/P-521, SHA-1/256/384/512, `-cades` (ESS v1/v2), ohne ESS, `-noattr`, `-nocerts`, `-keyid`, RSA-PSS, zwei Signer |
| `golden-attrs-*.der` | SignedAttributes unabhängig mit asn1crypto gebaut (DER-sortiert): Profile `pdfas`/`padesbb`, SHA-256/384/512, Zeit 2026-10-14T12:00:00Z |

Die Golden-Tests gegen echte PDF-AS/IAIK-Signaturen nutzen die gemeinsamen Proben unter `testdata/pdf/pdfas/`.

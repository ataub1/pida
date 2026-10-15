# Testdaten `pida-pdf-cos` (Paket A)

| Datei | Inhalt | Herkunft |
|---|---|---|
| `empty-owner-rc4.pdf` | RC4-128 (R3), Benutzer- **und** Owner-Passwort leer, `/P = -1340` (kein Ändern, keine Annotationen, keine Formulare) | `qpdf --allow-weak-crypto --encrypt --user-password= --owner-password= --bits=128 --use-aes=n --modify=none --extract=n --annotate=n --form=n -- testdata/pdf/generated/simple-xref-table.pdf …` |
| `empty-owner-aes128.pdf` | wie oben, AES-128 (R4) | `qpdf --encrypt --user-password= --owner-password= --bits=128 --use-aes=y …` |
| `empty-owner-aes256.pdf` | wie oben, AES-256 (R6) | `qpdf --encrypt --user-password= --owner-password= --bits=256 …` |
| `simple-rc4-40-owner.pdf` | RC4-40 (V1, R2), ohne Benutzerpasswort, Owner `test-owner`, alle Rechte – deckt R2 aus „R2–R6“ ab | `qpdf --allow-weak-crypto --static-id --encrypt --user-password= --owner-password=test-owner --bits=40 -- testdata/pdf/generated/simple-xref-table.pdf …` |

Erwartung (gegengeprüft mit PDFBox 2.0.34): Das leere Passwort ist zugleich das Owner-Passwort,
PDFBox gewährt Owner-Rechte (`isOwnerPermission() == true`) und PDF-AS `checkPDFPermissions` lässt
das Signieren trotz `/P` zu → `Rights::permissions == Permissions::ALL`, `may_sign() == true`
(Test `empty_owner_password_grants_owner_rights_like_pdfbox`).

Inhalt wie `testdata/pdf/generated/simple-xref-table.pdf`; AES-Dateien sind wegen zufälliger Salze
nicht byte-genau reproduzierbar.

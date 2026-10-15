# Testdaten

Gemeinsame, kleine Testdaten für alle Pakete (Ziel < 15 MB, aktuell ≈ 8 MB). Erzeugt bzw. aktualisiert
mit [`tools/make-testdata.sh`](../tools/make-testdata.sh) (benötigt `openssl` ≥ 3, `qpdf` ≥ 11.7 und `python3`, kein Netz).
Der Korpus nach Plan §8.1 (≥ 450 Dateien, Klassen C1–C9) entsteht später unter `tools/corpus/` und liegt
nicht im Repository.

> **NUR FÜR TESTS.** Alle Schlüssel sind selbstsigniert, das Passwort ist überall `test`, die Subjects
> enthalten `(NUR TEST)`. Niemals für echte Signaturen, Zugänge oder Produktivsysteme verwenden.

## Aufbau

| Pfad | Inhalt | Herkunft |
|---|---|---|
| `keystores/test-rsa2048.p12` | RSA-2048, PBES2/AES-256-CBC, MAC SHA-256 | openssl |
| `keystores/test-ecp256.p12` | ECDSA P-256, PBES2/AES-256-CBC | openssl |
| `keystores/test-ecp384-title.p12` | ECDSA P-384 mit Titel-RDN `title=Dipl.-Ing. (NUR TEST)` (Golden-Test `subject.title`, SHA-256 bei P-384) | openssl |
| `keystores/test-rsa2048-legacy-3des.p12` | derselbe RSA-Schlüssel, Legacy-PBE `pbeWithSHA1And3-KeyTripleDES-CBC`, MAC SHA-1 | openssl |
| `keystores/*.crt.pem`, `*.crt.der` | die Zertifikate einzeln (z. B. für `pida_cades::Certificate::from_der`) | openssl |
| `pdf/pdfas/<modul>/…` | alle PDFs aus `reference/pdf-as-4/*/src/test/resources` (Rotation, QR-Platzhalter, Signaturfelder, bereits signiert 1–3×, `enc_own.pdf` mit Owner-Passwort) | PDF-AS 4.4.2, EUPL |
| `pdf/pdfas-suites/<SUITE>/` | PDF-AS-Testsuiten `public_pdfbox2` (`POS_*`, `PDFA_*`): `example.pdf`, `config.properties`, `example_ref.png` (visuelles Orakel, Plan §8.2) | PDF-AS 4.4.2, EUPL |
| `pdf/generated/simple-xref-table.pdf` | klassische xref-Tabelle | qpdf |
| `pdf/generated/simple-xref-stream.pdf`, `manySignFields-xref-stream.pdf` | xref-Stream + Object Streams (`--object-streams=generate`) | qpdf |
| `pdf/generated/simple-linearized*.pdf` | linearisiert (Tabelle bzw. Stream) | qpdf |
| `pdf/generated/simple-aes256-owner.pdf` | AES-256 (R6), **ohne** User-Passwort, Owner `test-owner` (K3) | qpdf |
| `pdf/generated/simple-aes128-owner.pdf` | AES-128 (R4), ohne User-Passwort (K3) | qpdf |
| `pdf/generated/simple-rc4-128-owner.pdf` | RC4-128 (R3), ohne User-Passwort (K3) | qpdf |
| `pdf/generated/simple-aes256-owner-xref-stream.pdf` | AES-256 + xref-Stream | qpdf |
| `pdf/generated/simple-aes256-nosign.pdf` | AES-256, Rechte ohne Ändern/Annotationen/Formulare → `Rights::may_sign() == false` | qpdf |
| `pdf/generated/simple-aes256-userpw.pdf` | AES-256 mit User-Passwort `test` (K5) | qpdf |
| `pdf/generated/simple-xref-hybrid.pdf` | Hybrid-Querverweis: xref-Tabelle + `/XRefStm`; Font-Objekt 5 liegt in einem Object Stream und steht **nur** im xref-Stream (Leser ohne `/XRefStm` finden es nicht) – `XrefKind::Hybrid` | `tools/make-hybrid-xref.py` |

Lizenz: Die PDF-AS-Dateien stehen unter der EUPL (PDF-AS 4, `reference/pdf-as-4/license`), wie dieses
Projekt (EUPL-1.2). Die erzeugten Dateien sind gemeinfreie Testartefakte.

## Regeln

- Gemeinsame Dateien hier ändert nur das Fundament bzw. die Integration (über das Skript).
- Jedes Paket legt **eigene** Testdaten unter `testdata/<crate>/` an (z. B. `testdata/sl12/`,
  `testdata/backend-atrust/` für aufgezeichnete A-Trust-Fixtures ohne Personendaten), siehe ADR 0001 §4.
- AES-256-Dateien sind nicht reproduzierbar (zufällige Salze); das Skript erzeugt fehlende Dateien und
  überschreibt vorhandene nur mit `--force`. Keystores ebenso (neue Schlüssel bei `--force`).
- Keine echten Personendaten, keine echten Zertifikate, keine Zugangsdaten.

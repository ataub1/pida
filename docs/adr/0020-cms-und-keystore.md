# ADR 0020 – CMS/CAdES-Kodierung und PKCS#12-Keystore (Paket B)

| | |
|---|---|
| **Status** | angenommen (14.10.2026) |
| **Kontext** | Umsetzung von `pida-cades` und `pida-backend-keystore` (ADR 0001 §4, §8.3; Plan §4.9, §6.10; analyse/03 §5, analyse/02 §5) |

## 1. Eigenes DER statt `cms`-Builder

SignedAttributes, SignedData und SignerInfo werden mit einem kleinen eigenen DER-Kodierer erzeugt; fremde CMS liest ein
BER-toleranter TLV-Leser (`der_util`). Signiert und geprüft wird immer über die **Originalbytes** der Attribute.
`cms` 0.2 bleibt Abhängigkeit (Typen der gepinnten Generation, Orakel in Tests), wird aber zum Prüfen nicht benutzt: Er
dekodiert die Attribute und kodiert sie sortiert neu, die signierten Originalbytes eines nicht DER-sortierten `SET OF`
gingen verloren (Test `verifier_uses_original_attribute_bytes_even_if_not_der_sorted`).

**Nachweis:** Die Golden-Tests reproduzieren die SignedAttributes **und** das komplette CMS der PDF-AS-Proben P1 (RSA-3072)
sowie P2/P3 (ECDSA P-256, vier Signaturen) byte-genau.

## 2. Kodierung je Profil (gemessen an IAIK)

| Element | `PdfasCompat` | `PadesBB` |
|---|---|---|
| digestAlgorithm(s), ESSCertIDv2.hashAlgorithm | mit NULL | ohne Parameter (RFC 5754) |
| signatureAlgorithm RSA | `sha256WithRSAEncryption` (kombinierter OID, wie IAIK in P1) mit NULL – **nicht** `rsaEncryption` wie OpenSSL | ebenso (RFC 4055) |
| signatureAlgorithm ECDSA | `ecdsa-with-SHA…` mit NULL (P2/P3) | ohne Parameter (RFC 5758) |
| signing-time | UTCTime 1950–2049, sonst GeneralizedTime (IAIK `ChoiceOfTime`) | – |
| ESS | v2 (hashAlgorithm nur ≠ SHA-256), v1 bei SHA-1; `issuerSerial` immer | ebenso |

Der Verifier akzeptiert beide Schreibweisen (`rsaEncryption` + digestAlgorithm, kombinierte OIDs, `id-ecPublicKey`),
kombinierte OIDs müssen zum digestAlgorithm passen.

## 3. Algorithmuswahl

`select_algorithm` folgt PDF-AS (`CertificateUtils.getAlgorithmIDs`) mit den Abweichungen aus Plan §4.9: EC mit
Feldgröße < 256 (PDF-AS: ecdsa-with-SHA1) und DSA werden abgelehnt. `pdfas_algorithm` liefert die exakte PDF-AS-Regel für
Diagnose/Kompatibilitätsregister. Signieren können `LocalSigner`/Keystore RSA (PKCS#1 v1.5, mit Blinding) und ECDSA
P-256/384/521 (RFC 6979) mit jedem SHA-2; kurze Hashes werden links aufgefüllt (bits2int), so geht z. B. P-521 + SHA-256.

## 4. Verifier

Wie PDF-AS (genau ein SignerInfo, message-digest, Signatur, ESS, erwartetes Zertifikat), zusätzlich: content-type =
eContentType, keine doppelten Pflichtattribute, Signer per IssuerAndSerialNumber **oder** SubjectKeyIdentifier,
Ausstellernamen DER-gleich oder textuell gleich. Abweichung: Sind ESS v1 **und** v2 vorhanden, werden beide geprüft (PDF-AS
nur v1). Politik: neue Signaturen nur SHA-256/384/512 mit PKCS#1 v1.5 bzw. ECDSA; Vorsignaturen zusätzlich SHA-1
(`CmsWarning::Sha1Digest`) und RSASSA-PSS. Weitere Hinweise: ESS v1, RSA < 2048 Bit, nicht DER-sortierte Attribute.

## 5. PKCS#12 und die Ein-Passwort-Grenze von `p12-keystore`

1. Vorprüfung ohne Passwort (DER-Struktur, MAC-Verfahren, PBE-Verfahren je Bag) → `UnsupportedPbe` mit Verfahrensnamen
   bzw. `Malformed`, bevor nach einem Passwort gefragt wird.
2. Kandidaten: Store-Passwort; bei abweichendem Schlüsselpasswort danach das Schlüsselpasswort als einziges Passwort.
3. MAC passt, Entschlüsselung der Schlüssel scheitert → `DifferentKeyPassword` (Neu-Export mit einheitlichem Passwort);
   sonst `WrongPassword`. Ein Low-Level-Pfad für getrennte Passwörter entsteht erst nach Spike S2 (Plan §4.9).
4. `pkcs5` 0.8.1 wird mit den Features `sha1-insecure` und `3des` eingebunden (Feature-Vereinigung mit `p12-keystore`):
   PBES2 mit PRF HMAC-SHA-1 und mit DES-EDE3-CBC werden lesbar (ältere OpenSSL-/Java-Exporte).

Alias exakt, sonst ohne Groß-/Kleinschreibung (Java speichert klein); ohne Alias der einzige Schlüssel. Selbstsignierte
Wurzeln kommen nicht in `CertChain::intermediates`. Schlüssel und Zertifikat müssen zusammenpassen.

**Verknüpfung (Review):** Schlüssel und Zertifikate verknüpft der Klon selbst über `p12_keystore::Pkcs12Archive` (eine
Entschlüsselung), nicht über `KeyStore::from_pkcs12(…, Strict)`: Dieses verwirft Schlüssel ohne `localKeyId` still,
Java `PKCS12KeyStore` (also PDF-Over) lädt sie über den friendlyName bzw. die Ein-Schlüssel-Regel. Reihenfolge:
`localKeyId` (bei mehreren Treffern bevorzugt mit gleichem friendlyName) → Zertifikat mit passendem öffentlichen
Schlüssel → gleicher friendlyName (ohne Groß-/Kleinschreibung). Kette über Aussteller = Subject (DER-Vergleich der
Namen); Java-Vertrauensanker gehören nicht dazu, sind aber eigene Einträge. Alias: friendlyName des Schlüssels, sonst
Subject-Text; Kollisionen `#2`, `#3` wie `p12-keystore`. Weil der Alias damit Personendaten tragen kann, zeigen `Debug`
und Logs nur seine Länge.

## 6. Backend-Dialoge

Fehlende Datei und fehlender Alias → Meldung mit Wiederholen/Abbrechen (wie PDF-Over `KSState`), Abbrechen =
`BackendError::Cancelled`. Endgültige Fehler (JKS/JCEKS, PBE, Schlüsseltyp, getrennte Passwörter) gehen ohne eigenen Dialog an
den Workflow (keine Doppelmeldung); der Konvertierungsassistent hängt an `backend-keystore-format` und nutzt
`conversion_hint`. Passwörter aus dem CLI-Overlay werden nie abgelegt; abgelegt wird nur ein abgefragtes Passwort bei
Speicherart `memory`/`disk`. Die PBKDF-Arbeit läuft unter Tokio in `spawn_blocking` und ist über das `CancelToken`
abbrechbar (das Ergebnis wird verworfen, der Schlüssel beim Drop genullt). Keystore-Dateien über 16 MiB werden nicht
gelesen (`Malformed`).

## 6a. Ergänzungen aus dem Review

- `verify_detached`: signing-time wird byteweise mit ASCII-Ziffern geparst (vorher Panic bei Nicht-ASCII im Offset,
  z. B. `+1é1`, durch `str`-Slicing an einer Nicht-Zeichengrenze – auslösbar durch jedes fremde CMS).
- RSA-Schlüssel bis 16384 Bit: `rsa` begrenzt öffentliche Schlüssel aus DER auf 4096 Bit; PDF-AS/Java kennt diese Grenze
  nicht (RSA-4608/8192-Keystores, Vorsignaturen). Der Exponent bleibt durch `rsa` auf ≤ 2³³ begrenzt.
- `LocalSigner::from_pkcs8_der`: PKCS#8-Algorithmus (und EC-Kurve) muss zur SPKI des Zertifikats passen, sonst
  `CmsError::KeyMismatch` (vorher ein irreführender Lesefehler).
- `RdnAttr` hat einen redigierenden `Debug` (Wert nur als Länge).

## 7. Offene Punkte

- `BackendError::Keystore(String)` trägt keinen Untercode; die `KeystoreError::code`-Werte gehen damit an der UI verloren
  (Änderungswunsch an die Integration).
- `p12-keystore` hält das entschlüsselte PKCS#8 kurz in nicht genullten Puffern (eigene Kopien sind `Zeroizing`).
- BER-kodierte PKCS#12 (unbestimmte Längen) liest `p12-keystore` nicht; Meldung mit Neu-Export-Hinweis.

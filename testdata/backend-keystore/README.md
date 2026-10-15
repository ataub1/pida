# Testdaten `pida-backend-keystore` (Paket B)

> **NUR FÜR TESTS.** Alle Schlüssel stammen aus `testdata/keystores` bzw. `testdata/cades` oder sind selbst erzeugt;
> Passwort `test` (Ausnahmen in der Tabelle).

Erzeugt mit [`make-testdata.sh`](make-testdata.sh) (`openssl` ≥ 3.4, JDK ≥ 17; [`GenKeystores.java`](GenKeystores.java)
nutzt die Java-KeyStore-API, weil `keytool` Passwörter unter 6 Zeichen ablehnt).

| Datei | Zweck |
|---|---|
| `openssl-legacy-rc2.p12` | `-legacy`: Zertifikate RC2-40, Schlüssel 3DES, MAC SHA-1 |
| `openssl-pbes2-3des.p12` | PBES2 mit DES-EDE3-CBC |
| `openssl-md5-des.p12` | PBES1 `pbeWithMD5AndDES-CBC` → `UnsupportedPbe` |
| `openssl-pbmac1.p12` | MAC PBMAC1 (RFC 9579) → `UnsupportedPbe` |
| `openssl-unencrypted-nomac.p12`, `openssl-nomac.p12` | unverschlüsselt bzw. ohne MAC |
| `openssl-empty-password.p12` | Passwort leer |
| `openssl-umlaut-password.p12` | Passwort `Pässwörd €!` (UTF-8) |
| `openssl-no-name.p12` | ohne friendlyName |
| `openssl-p521-chain.p12` | P-521 mit Zwischen-CA und Root (Alias `p521`) |
| `openssl-no-localkeyid.p12` | RSA-2048 ohne localKeyID, friendlyName `test-rsa2048`, unverschlüsselt, MAC SHA-256 (Review; [`strip_localkeyid.py`](strip_localkeyid.py)) |
| `openssl-no-localkeyid-chain.p12` | P-521 + Root + Zwischen-CA ohne jedes Bag-Attribut (Alias = Subject-Text, Kette über Aussteller) |
| `openssl-{brainpoolP256r1,ed25519,dsa,secp224r1}.p12` (+ `.crt.der`) | nicht unterstützte Schlüssel → `UnsupportedKey` |
| `java-modern.p12`, `java-legacy.p12`, `java-sha1-prf.p12` | JDK 21: PBES2/AES-256; Legacy RC2-40/3DES; PBES2 mit PRF HMAC-SHA-1 |
| `java-mixed-case-alias.p12` | Alias `Mein Signaturschluessel` (Java speichert klein) |
| `java-two-keys.p12` | Schlüssel `erster` (RSA), `zweiter` (P-256), Zertifikat `vertrauensanker` |
| `java-separate-keypass.p12` | Store-Passwort `test`, Schlüsselpasswort `schluessel` → `DifferentKeyPassword` |
| `java-key-mismatch.p12` | RSA-2048-Schlüssel mit RSA-3072-Zertifikat → `KeyCertificateMismatch` |
| `java-rsa2048.jks`, `java-rsa2048.jceks` | JKS/JCEKS → Konvertierungshinweis |

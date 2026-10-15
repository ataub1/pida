#!/usr/bin/env bash
# PIDA, Paket B – erzeugt die Test-Keystores für pida-backend-keystore unter
# testdata/backend-keystore/ (Varianten der Verschlüsselung, Java-Formate, Fehlerfälle).
#
#   testdata/backend-keystore/make-testdata.sh           # fehlende Dateien erzeugen
#   testdata/backend-keystore/make-testdata.sh --force   # alles neu
#
# Voraussetzungen: openssl >= 3.4 (Legacy-Provider, -pbmac1_pbkdf2), JDK >= 17 (java).
# Benötigt testdata/keystores (tools/make-testdata.sh) und testdata/cades (make-testdata.sh dort).
# Keine Netzwerkzugriffe. Passwort überall "test" (Ausnahmen unten dokumentiert).
#
# ALLE SCHLÜSSEL UND ZERTIFIKATE SIND NUR FÜR TESTS. Niemals für echte Signaturen verwenden.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TD="$HERE/.."
KS="$TD/keystores"
CA="$TD/cades"
FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1
command -v openssl >/dev/null || { echo "openssl fehlt" >&2; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cd "$HERE"
O="/C=AT/O=PDF-Over NG Test (NUR TEST)"

# --- Review-Zusatz: PKCS#12 ohne localKeyID (wird nur bei Fehlen erzeugt) ------------------------
# Java (PKCS12KeyStore, also PDF-Over) verknüpft Schlüssel und Zertifikat dann über den friendlyName
# bzw. die Ein-Schlüssel-Regel; der Klon über den öffentlichen Schlüssel. Unverschlüsselt erzeugt,
# localKeyID entfernt, MAC (HMAC-SHA-256) mit Passwort "test" neu berechnet (strip_localkeyid.py,
# braucht python3 mit asn1crypto, z. B. aus der .venv mit pyHanko).
if [[ $FORCE -eq 1 || ! -f openssl-no-localkeyid-chain.p12 ]]; then
  PY="${PYTHON:-python3}"
  "$PY" -c 'import asn1crypto' 2>/dev/null || { echo "python3 mit asn1crypto fehlt (source tools/env.sh)" >&2; exit 1; }
  openssl pkcs12 -in "$KS/test-rsa2048.p12" -passin pass:test -nocerts -nodes -out "$TMP/nl-rsa.key" 2>/dev/null
  openssl pkcs12 -export -inkey "$TMP/nl-rsa.key" -in "$KS/test-rsa2048.crt.pem" -name test-rsa2048 \
    -passout pass:test -keypbe NONE -certpbe NONE -nomac -out "$TMP/nl-rsa.p12"
  "$PY" "$HERE/strip_localkeyid.py" "$TMP/nl-rsa.p12" openssl-no-localkeyid.p12 test
  openssl pkey -inform DER -in "$CA/leaf-p521.key.p8.der" -out "$TMP/nl-p521.key"
  openssl x509 -inform DER -in "$CA/leaf-p521.crt.der" -out "$TMP/nl-p521.crt"
  openssl x509 -inform DER -in "$CA/ca-intermediate.crt.der" -out "$TMP/nl-inter.crt"
  openssl x509 -inform DER -in "$CA/ca-root.crt.der" -out "$TMP/nl-root.crt"
  cat "$TMP/nl-root.crt" "$TMP/nl-inter.crt" > "$TMP/nl-chain.crt"
  openssl pkcs12 -export -inkey "$TMP/nl-p521.key" -in "$TMP/nl-p521.crt" -certfile "$TMP/nl-chain.crt" \
    -passout pass:test -keypbe NONE -certpbe NONE -nomac -out "$TMP/nl-p521.p12"
  "$PY" "$HERE/strip_localkeyid.py" "$TMP/nl-p521.p12" openssl-no-localkeyid-chain.p12 test
  # Gegenprobe: OpenSSL prüft den neu berechneten MAC
  openssl pkcs12 -in openssl-no-localkeyid.p12 -passin pass:test -noout
  openssl pkcs12 -in openssl-no-localkeyid-chain.p12 -passin pass:test -noout
fi

if [[ $FORCE -eq 0 && -f "$HERE/openssl-legacy-rc2.p12" ]]; then
  echo "testdata/backend-keystore vorhanden (mit --force neu erzeugen)."
  exit 0
fi
command -v java >/dev/null || { echo "java (JDK) fehlt" >&2; exit 1; }

openssl pkcs12 -in "$KS/test-rsa2048.p12" -passin pass:test -nocerts -nodes -out "$TMP/rsa.key" 2>/dev/null
RSA_CRT="$KS/test-rsa2048.crt.pem"
p12() {
  # $1 = Ausgabe, $2 = Schlüssel, $3 = Zertifikat, $4.. = Optionen
  local out="$1" key="$2" crt="$3"; shift 3
  openssl pkcs12 -export -inkey "$key" -in "$crt" "$@" -out "$out"
}

# --- Verschlüsselungsvarianten (OpenSSL) --------------------------------------------------------
# OpenSSL -legacy: Zertifikate RC2-40, Schlüssel 3DES, MAC SHA-1 (wie alte Windows-/Java-Exporte)
p12 openssl-legacy-rc2.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout pass:test -legacy
# PBES2 mit 3DES statt AES
p12 openssl-pbes2-3des.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout pass:test \
  -keypbe DES-EDE3-CBC -certpbe DES-EDE3-CBC
# MD5-basiertes PBES1 (pbeWithMD5AndDES-CBC) – nicht unterstützt → eigener Fehler
p12 openssl-md5-des.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout pass:test -legacy \
  -keypbe PBE-MD5-DES -certpbe PBE-MD5-DES
# unverschlüsselt, ohne MAC
p12 openssl-unencrypted-nomac.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout pass:test \
  -keypbe NONE -certpbe NONE -nomac
# verschlüsselt, ohne MAC
p12 openssl-nomac.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout pass:test -nomac
# PBMAC1 (RFC 9579) – von p12-keystore nicht unterstützt
p12 openssl-pbmac1.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout pass:test -pbmac1_pbkdf2
# leeres Passwort
p12 openssl-empty-password.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout pass:
# ohne friendlyName (Alias fällt auf den Subject-Text zurück)
p12 openssl-no-name.p12 "$TMP/rsa.key" "$RSA_CRT" -passout pass:test
# Passwort mit Umlaut und Sonderzeichen (UTF-8)
p12 openssl-umlaut-password.p12 "$TMP/rsa.key" "$RSA_CRT" -name test-rsa2048 -passout "pass:Pässwörd €!"

# --- P-521 mit Kette (Endzertifikat, Zwischen-CA, Root) ----------------------------------------
openssl pkey -inform DER -in "$CA/leaf-p521.key.p8.der" -out "$TMP/p521.key"
openssl x509 -inform DER -in "$CA/leaf-p521.crt.der" -out "$TMP/p521.crt"
openssl x509 -inform DER -in "$CA/ca-intermediate.crt.der" -out "$TMP/inter.crt"
openssl x509 -inform DER -in "$CA/ca-root.crt.der" -out "$TMP/root.crt"
cat "$TMP/inter.crt" "$TMP/root.crt" > "$TMP/chain.crt"
p12 openssl-p521-chain.p12 "$TMP/p521.key" "$TMP/p521.crt" -name p521 -certfile "$TMP/chain.crt" -passout pass:test

# --- nicht unterstützte Schlüsseltypen ---------------------------------------------------------
selfsigned() {
  # $1 = Name, $2.. = genpkey-Optionen
  local name="$1"; shift
  openssl genpkey "$@" -out "$TMP/$name.key" 2>/dev/null
  openssl req -x509 -new -key "$TMP/$name.key" -days 3650 -subj "$O/CN=PDF-Over NG Test $name (NUR TEST)" \
    -out "$TMP/$name.crt" 2>/dev/null
  openssl x509 -in "$TMP/$name.crt" -outform DER -out "$name.crt.der"
  p12 "openssl-$name.p12" "$TMP/$name.key" "$TMP/$name.crt" -name "$name" -passout pass:test
}
selfsigned brainpoolP256r1 -algorithm EC -pkeyopt ec_paramgen_curve:brainpoolP256r1
selfsigned ed25519 -algorithm ED25519
selfsigned secp224r1 -algorithm EC -pkeyopt ec_paramgen_curve:secp224r1
openssl genpkey -genparam -algorithm DSA -pkeyopt dsa_paramgen_bits:2048 -out "$TMP/dsa.param" 2>/dev/null
selfsigned dsa -paramfile "$TMP/dsa.param"

# --- Java (keytool / KeyStore-API) -------------------------------------------------------------
jexport() {
  # $1 = Ausgabe, $2 = Alias, $3.. = JVM-Optionen (-Dkeystore.pkcs12.…)
  local out="$1" alias="$2"; shift 2
  java "$@" "$HERE/GenKeystores.java" "$TD" "$HERE" "export:$out:$alias"
}
# JDK-Standard (PBES2/AES-256, HmacPBESHA256)
jexport java-modern.p12 test-rsa2048
# JDK-Legacy wie Java 8 (RC2-40/3DES, MAC SHA-1)
jexport java-legacy.p12 test-rsa2048 -Dkeystore.pkcs12.legacy
# PBES2 mit PRF HMAC-SHA-1 (ältere Exporte)
jexport java-sha1-prf.p12 test-rsa2048 -Dkeystore.pkcs12.keyProtectionAlgorithm=PBEWithHmacSHA1AndAES_256 \
  -Dkeystore.pkcs12.certProtectionAlgorithm=PBEWithHmacSHA1AndAES_256
# Alias mit Großbuchstaben/Leerzeichen (Java speichert ihn ggf. kleingeschrieben)
jexport java-mixed-case-alias.p12 "Mein Signaturschluessel"
java "$HERE/GenKeystores.java" "$TD" "$HERE" main

cat > NUR-TEST-KEINE-ECHTEN-SCHLUESSEL.txt <<'EOF'
NUR FÜR TESTS – KEINE ECHTEN SCHLÜSSEL
=====================================
Alle Dateien in diesem Ordner wurden von testdata/backend-keystore/make-testdata.sh (openssl und
GenKeystores.java) aus den Test-Schlüsseln in testdata/keystores und testdata/cades erzeugt.
Passwort "test" (Ausnahmen: openssl-empty-password.p12 = leer, openssl-umlaut-password.p12 = "Pässwörd €!",
java-separate-keypass.p12: Store "test", Schlüssel "schluessel").
Niemals für echte Signaturen, Zugänge oder Produktivsysteme verwenden.
EOF
echo "testdata/backend-keystore erzeugt."

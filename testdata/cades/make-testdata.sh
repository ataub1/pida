#!/usr/bin/env bash
# PDF-Over NG, Paket B – erzeugt die Testdaten für pdfover-cades unter testdata/cades/.
#
#   testdata/cades/make-testdata.sh           # fehlende Dateien erzeugen
#   testdata/cades/make-testdata.sh --force   # alles neu (neue Schlüssel!)
#
# Voraussetzungen: openssl >= 3.2 (cms -cades), python3 mit asn1crypto (z. B. die .venv mit pyHanko).
# Keine Netzwerkzugriffe.
#
# ALLE SCHLÜSSEL UND ZERTIFIKATE SIND NUR FÜR TESTS. Selbst erzeugte Test-PKI, Subjects mit
# "(NUR TEST)". Niemals für echte Signaturen verwenden.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KS="$HERE/../keystores"
FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1
command -v openssl >/dev/null || { echo "openssl fehlt" >&2; exit 1; }
PY="${PYTHON:-python3}"
"$PY" -c 'import asn1crypto' 2>/dev/null || { echo "python3 mit asn1crypto fehlt (source tools/env.sh)" >&2; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cd "$HERE"

# ------------------------------------------------------------------------------------------------
# 0. Zusatz aus dem Review (unabhängig von den übrigen Dateien, wird nur bei Fehlen erzeugt):
#    selbstsigniertes RSA-4608 (> 4096 Bit, Grenze von `rsa` beim DER-Import) + OpenSSL-CMS
# ------------------------------------------------------------------------------------------------
if [[ $FORCE -eq 1 || ! -f selfsigned-rsa4608.crt.der ]]; then
  [[ -f content.bin ]] || { echo "content.bin fehlt (zuerst mit --force erzeugen)" >&2; exit 1; }
  openssl req -x509 -new -newkey rsa:4608 -nodes -sha256 -days 3650 -set_serial 0x0B0010 \
    -subj "/C=AT/O=PDF-Over NG Test (NUR TEST)/CN=PDF-Over NG Test RSA-4608 (NUR TEST)" \
    -addext "keyUsage=critical,digitalSignature,nonRepudiation" -addext "subjectKeyIdentifier=hash" \
    -keyout "$TMP/rsa4608.key" -out "$TMP/rsa4608.crt" 2>/dev/null
  openssl x509 -in "$TMP/rsa4608.crt" -outform DER -out selfsigned-rsa4608.crt.der
  openssl pkcs8 -topk8 -nocrypt -in "$TMP/rsa4608.key" -outform DER -out selfsigned-rsa4608.key.p8.der
  openssl cms -sign -binary -in content.bin -signer "$TMP/rsa4608.crt" -inkey "$TMP/rsa4608.key" -outform DER \
    -out openssl-rsa4608-sha256-cades.cms.der -md sha256 -cades -nosmimecap
fi

if [[ $FORCE -eq 0 && -f "$HERE/openssl-rsa2048-sha256-cades.cms.der" ]]; then
  echo "testdata/cades vorhanden (mit --force neu erzeugen)."
  exit 0
fi

# ------------------------------------------------------------------------------------------------
# 1. Inhalt (deterministisch, ~5 KiB, inkl. aller Bytewerte)
# ------------------------------------------------------------------------------------------------
"$PY" - <<'EOF'
data = b"".join(b"PDF-Over NG Testinhalt (NUR TEST) Zeile %04d\n" % i for i in range(100)) + bytes(range(256))
open("content.bin", "wb").write(data)
EOF

# ------------------------------------------------------------------------------------------------
# 2. PKCS#8 der gemeinsamen Testschlüssel (testdata/keystores, Passwort "test")
# ------------------------------------------------------------------------------------------------
for n in test-rsa2048 test-ecp256 test-ecp384-title; do
  openssl pkcs12 -in "$KS/$n.p12" -passin pass:test -nocerts -nodes -out "$TMP/$n.key.pem" 2>/dev/null
  openssl pkcs8 -topk8 -nocrypt -in "$TMP/$n.key.pem" -outform DER -out "$n.key.p8.der"
done

# ------------------------------------------------------------------------------------------------
# 3. Test-PKI: Root (P-384) → Zwischen-CA (P-256) → Endzertifikate P-521 und RSA-3072
# ------------------------------------------------------------------------------------------------
O="/C=AT/O=PDF-Over NG Test (NUR TEST)"
cat > "$TMP/ca.ext" <<'EOF'
basicConstraints=critical,CA:TRUE
keyUsage=critical,keyCertSign,cRLSign
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid
EOF
cat > "$TMP/leaf.ext" <<'EOF'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,nonRepudiation
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid
EOF
openssl req -x509 -new -newkey ec -pkeyopt ec_paramgen_curve:P-384 -nodes -sha384 -days 3650 \
  -set_serial 0x0B0001 -subj "$O/CN=PDF-Over NG Test Root CA (NUR TEST)" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign" \
  -keyout "$TMP/root.key" -out "$TMP/root.crt" 2>/dev/null
openssl req -new -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -subj "$O/CN=PDF-Over NG Test Zwischen-CA (NUR TEST)" \
  -keyout "$TMP/inter.key" -out "$TMP/inter.csr" 2>/dev/null
openssl x509 -req -in "$TMP/inter.csr" -CA "$TMP/root.crt" -CAkey "$TMP/root.key" -set_serial 0x0B0002 -days 3650 \
  -sha384 -extfile "$TMP/ca.ext" -out "$TMP/inter.crt" 2>/dev/null
make_leaf() {
  # $1 = Name, $2 = newkey, $3 = CN, $4 = Seriennummer
  # shellcheck disable=SC2086
  openssl req -new -newkey $2 -nodes -subj "$O/CN=$3" -keyout "$TMP/$1.key" -out "$TMP/$1.csr" 2>/dev/null
  openssl x509 -req -in "$TMP/$1.csr" -CA "$TMP/inter.crt" -CAkey "$TMP/inter.key" -set_serial "$4" -days 3650 \
    -sha256 -extfile "$TMP/leaf.ext" -out "$TMP/$1.crt" 2>/dev/null
  openssl x509 -in "$TMP/$1.crt" -outform DER -out "$1.crt.der"
  openssl pkcs8 -topk8 -nocrypt -in "$TMP/$1.key" -outform DER -out "$1.key.p8.der"
}
make_leaf leaf-p521 "ec -pkeyopt ec_paramgen_curve:P-521" "PDF-Over NG Test ECDSA P-521 (NUR TEST)" 0x0B0003
make_leaf leaf-rsa3072 "rsa:3072" "PDF-Over NG Test RSA-3072 (NUR TEST)" 0x0B0004
openssl x509 -in "$TMP/root.crt" -outform DER -out ca-root.crt.der
openssl x509 -in "$TMP/inter.crt" -outform DER -out ca-intermediate.crt.der
cat "$TMP/leaf-p521.crt" "$TMP/inter.crt" "$TMP/root.crt" > chain-p521.pem

# ------------------------------------------------------------------------------------------------
# 4. CMS von OpenSSL (detached, DER) über content.bin – Orakel für verify_detached
# ------------------------------------------------------------------------------------------------
sign() {
  # $1 = Ausgabe, $2 = Zertifikat (PEM), $3 = Schlüssel (PEM), $4.. = Optionen
  local out="$1" crt="$2" key="$3"; shift 3
  openssl cms -sign -binary -in content.bin -signer "$crt" -inkey "$key" -outform DER -out "$out" "$@"
}
RSA_CRT="$KS/test-rsa2048.crt.pem"; RSA_KEY="$TMP/test-rsa2048.key.pem"
P256_CRT="$KS/test-ecp256.crt.pem"; P256_KEY="$TMP/test-ecp256.key.pem"
P384_CRT="$KS/test-ecp384-title.crt.pem"; P384_KEY="$TMP/test-ecp384-title.key.pem"
sign openssl-rsa2048-sha256-cades.cms.der "$RSA_CRT" "$RSA_KEY" -md sha256 -cades -nosmimecap
sign openssl-rsa2048-sha256-plain.cms.der "$RSA_CRT" "$RSA_KEY" -md sha256
sign openssl-rsa2048-sha512-cades.cms.der "$RSA_CRT" "$RSA_KEY" -md sha512 -cades
sign openssl-rsa2048-sha1-cades.cms.der "$RSA_CRT" "$RSA_KEY" -md sha1 -cades -nosmimecap
sign openssl-rsa2048-sha256-noattr.cms.der "$RSA_CRT" "$RSA_KEY" -md sha256 -noattr
sign openssl-rsa2048-sha256-nocerts.cms.der "$RSA_CRT" "$RSA_KEY" -md sha256 -cades -nocerts
sign openssl-ecp256-sha256-cades.cms.der "$P256_CRT" "$P256_KEY" -md sha256 -cades -nosmimecap
sign openssl-ecp384-sha384-cades.cms.der "$P384_CRT" "$P384_KEY" -md sha384 -cades -nosmimecap
sign openssl-p521-sha512-cades-keyid.cms.der "$TMP/leaf-p521.crt" "$TMP/leaf-p521.key" -md sha512 -cades -keyid \
  -certfile "$TMP/inter.crt"
sign openssl-rsa3072-pss-sha256-cades.cms.der "$TMP/leaf-rsa3072.crt" "$TMP/leaf-rsa3072.key" -md sha256 -cades \
  -keyopt rsa_padding_mode:pss -keyopt rsa_pss_saltlen:32
openssl cms -sign -binary -in content.bin -outform DER -out openssl-two-signers.cms.der -md sha256 -cades \
  -signer "$RSA_CRT" -inkey "$RSA_KEY" -signer "$P256_CRT" -inkey "$P256_KEY"

# ------------------------------------------------------------------------------------------------
# 5. Golden-SignedAttributes, unabhängig mit asn1crypto gebaut (DER-sortiert)
# ------------------------------------------------------------------------------------------------
"$PY" - <<'EOF'
import hashlib, datetime
from asn1crypto import x509, cms, core, algos, tsp

content = open("content.bin", "rb").read()
when = datetime.datetime(2026, 10, 14, 12, 0, 0, tzinfo=datetime.timezone.utc)

def load(path):
    return x509.Certificate.load(open(path, "rb").read())

def attr_raw(oid, value_der):
    # Attribute ::= SEQUENCE { attrType OID, attrValues SET { value } }
    return _tlv(0x30, core.ObjectIdentifier(oid).dump() + _tlv(0x31, value_der))

def _len(n):
    if n < 0x80:
        return bytes([n])
    b = n.to_bytes((n.bit_length() + 7) // 8, "big")
    return bytes([0x80 | len(b)]) + b

def _tlv(tag, content):
    return bytes([tag]) + _len(len(content)) + content

def der_set(items):
    n = max(len(i) for i in items)
    items = sorted(items, key=lambda i: i + b"\x00" * (n - len(i)))
    return _tlv(0x31, b"".join(items))

def ess_v2(cert, hash_name, null_params):
    h = hashlib.new(hash_name, cert.dump()).digest()
    issuer_serial = tsp.IssuerSerial({
        "issuer": [x509.GeneralName(name="directory_name", value=cert.issuer)],
        "serial_number": cert.serial_number,
    })
    fields = {"cert_hash": h, "issuer_serial": issuer_serial}
    if hash_name != "sha256":
        fields["hash_algorithm"] = algos.DigestAlgorithm(
            {"algorithm": hash_name, "parameters": core.Null() if null_params else None})
    essid = tsp.ESSCertIDv2(fields)
    return tsp.SigningCertificateV2({"certs": [essid]}).dump()

def build(cert_path, hash_name, profile, out):
    cert = load(cert_path)
    md = hashlib.new(hash_name, content).digest()
    attrs = [attr_raw("1.2.840.113549.1.9.3", core.ObjectIdentifier("1.2.840.113549.1.7.1").dump())]
    if profile == "pdfas":
        attrs.append(attr_raw("0.4.0.1733.2.1", core.UTF8String("application/pdf").dump()))
        attrs.append(attr_raw("1.2.840.113549.1.9.5", core.UTCTime(when).dump()))
    attrs.append(attr_raw("1.2.840.113549.1.9.4", core.OctetString(md).dump()))
    attrs.append(attr_raw("1.2.840.113549.1.9.16.2.47", ess_v2(cert, hash_name, profile == "pdfas")))
    open(out, "wb").write(der_set(attrs))

build("../keystores/test-ecp384-title.crt.der", "sha256", "pdfas", "golden-attrs-pdfas-ecp384-sha256.der")
build("../keystores/test-ecp384-title.crt.der", "sha256", "padesbb", "golden-attrs-padesbb-ecp384-sha256.der")
build("leaf-p521.crt.der", "sha512", "pdfas", "golden-attrs-pdfas-p521-sha512.der")
build("leaf-p521.crt.der", "sha512", "padesbb", "golden-attrs-padesbb-p521-sha512.der")
build("../keystores/test-rsa2048.crt.der", "sha384", "padesbb", "golden-attrs-padesbb-rsa2048-sha384.der")
EOF

cat > NUR-TEST-KEINE-ECHTEN-SCHLUESSEL.txt <<'EOF'
NUR FÜR TESTS – KEINE ECHTEN SCHLÜSSEL
=====================================
Alle Dateien in diesem Ordner wurden von testdata/cades/make-testdata.sh mit openssl/asn1crypto erzeugt.
Eigene Test-PKI, unverschlüsselte PKCS#8-Testschlüssel, Subject enthält "(NUR TEST)".
Niemals für echte Signaturen, Zugänge oder Produktivsysteme verwenden.
EOF
echo "testdata/cades erzeugt."

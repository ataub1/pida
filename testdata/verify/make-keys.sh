#!/usr/bin/env bash
# PIDA – erzeugt das QES-Testzertifikat für die Signaturprüfung der Tests (ADR 0004 §3.17) unter testdata/verify/keys/.
#
#   testdata/verify/make-keys.sh            # nur, wenn es fehlt
#   testdata/verify/make-keys.sh --force    # neu erzeugen (neuer Schlüssel!)
#
# NUR FÜR TESTS: selbstsigniert, Passwort "test", Subject mit "(NUR TEST)". Die Erweiterung qcStatements
# (QcCompliance 0.4.0.1862.1.1, QcSSCD 0.4.0.1862.1.4, QcType esign 0.4.0.1862.1.6.1) erklärt das Zertifikat
# als „qualifiziert“ – das ist eine bloße Behauptung im Zertifikat und genau das, was `pades::verify_document`
# und die Fertig-Ansicht als „qualifiziert laut Zertifikat“ melden sollen. Kein Aussteller einer Vertrauensliste, keine echten Personendaten.
#
# Die signierten Testdateien erzeugt der Generator des Kerns:
#   PIDA_BLESS=1 cargo test -p pida-pades --test verify_testdata
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/keys"
FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1
command -v openssl >/dev/null || { echo "openssl fehlt" >&2; exit 1; }
mkdir -p "$DIR"
if [[ $FORCE -eq 0 && -f "$DIR/test-qes-ecp256.p12" ]]; then
  exit 0
fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# SEQUENCE { SEQUENCE { QcCompliance }, SEQUENCE { QcSSCD }, SEQUENCE { QcType, SEQUENCE { esign } } }
QC="3029""3008060604008e460101""3008060604008e460104""3013060604008e460106""3009060704008e46010601"

openssl req -x509 -new -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -sha256 -days 3650 \
  -set_serial 0x0A0B0C0D10 \
  -subj "/C=AT/O=PIDA Test (NUR TEST)/CN=PIDA Test QES P-256 (NUR TEST)" \
  -addext "keyUsage=critical,nonRepudiation" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "1.3.6.1.5.5.7.1.3=DER:${QC}" \
  -keyout "$TMP/qes.key.pem" -out "$DIR/test-qes-ecp256.crt.pem" 2>/dev/null
openssl x509 -in "$DIR/test-qes-ecp256.crt.pem" -outform DER -out "$DIR/test-qes-ecp256.crt.der"
openssl pkcs12 -export -inkey "$TMP/qes.key.pem" -in "$DIR/test-qes-ecp256.crt.pem" \
  -name test-qes-ecp256 -passout pass:test -out "$DIR/test-qes-ecp256.p12"
echo "QES-Testzertifikat erzeugt: $DIR/test-qes-ecp256.{p12,crt.pem,crt.der} (NUR TEST)"

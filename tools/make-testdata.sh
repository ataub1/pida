#!/usr/bin/env bash
# PIDA – erzeugt bzw. aktualisiert die gemeinsamen Testdaten unter testdata/.
#
#   tools/make-testdata.sh            # fehlende Keystores erzeugen, PDFs kopieren/erzeugen
#   tools/make-testdata.sh --force    # Keystores neu erzeugen (neue Schlüssel!)
#
# Voraussetzungen: openssl (>= 3), qpdf (>= 11.7), python3 (nur Standardbibliothek). Keine Netzwerkzugriffe.
#
# ALLE SCHLÜSSEL UND ZERTIFIKATE SIND NUR FÜR TESTS. Sie sind selbstsigniert, haben das bekannte
# Passwort "test" und tragen "NUR TEST" im Subject. Niemals für echte Signaturen verwenden.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TD="$ROOT/testdata"
REF="${PIDA_REFERENCE:-$ROOT/reference}/pdf-as-4"  # in Worktrees: source tools/env.sh
FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1

command -v openssl >/dev/null || { echo "openssl fehlt" >&2; exit 1; }
command -v qpdf >/dev/null || { echo "qpdf fehlt" >&2; exit 1; }

mkdir -p "$TD/keystores" "$TD/pdf/pdfas" "$TD/pdf/pdfas-suites" "$TD/pdf/generated"

# ------------------------------------------------------------------------------------------------
# 1. Keystores (PKCS#12, Passwort "test")
# ------------------------------------------------------------------------------------------------
KS="$TD/keystores"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

make_key_and_cert() {
  # $1 = Name, $2 = openssl-newkey-Argumente, $3 = Subject, $4 = Seriennummer
  local name="$1" newkey="$2" subj="$3" serial="$4"
  # shellcheck disable=SC2086
  openssl req -x509 -new -newkey $newkey -nodes -sha256 -days 3650 \
    -set_serial "$serial" -subj "$subj" \
    -addext "keyUsage=critical,digitalSignature,nonRepudiation" \
    -addext "basicConstraints=critical,CA:FALSE" \
    -keyout "$TMP/$name.key.pem" -out "$KS/$name.crt.pem" 2>/dev/null
  openssl x509 -in "$KS/$name.crt.pem" -outform DER -out "$KS/$name.crt.der"
}

export_p12() {
  # $1 = Name (Schlüssel/Zertifikat), $2 = Ausgabedatei, $3.. = zusätzliche pkcs12-Optionen
  local name="$1" out="$2"; shift 2
  openssl pkcs12 -export -inkey "$TMP/$name.key.pem" -in "$KS/$name.crt.pem" \
    -name "$name" -passout pass:test "$@" -out "$out"
}

if [[ $FORCE -eq 1 || ! -f "$KS/test-rsa2048.p12" || ! -f "$KS/test-ecp256.p12" || ! -f "$KS/test-ecp384-title.p12" || ! -f "$KS/test-rsa2048-legacy-3des.p12" ]]; then
  echo "Erzeuge Test-Keystores (NUR TEST) ..."
  make_key_and_cert test-rsa2048 "rsa:2048" \
    "/C=AT/O=PDF-Over NG Test (NUR TEST)/CN=PDF-Over NG Test RSA (NUR TEST)" 0x0A0B0C0D01
  make_key_and_cert test-ecp256 "ec -pkeyopt ec_paramgen_curve:P-256" \
    "/C=AT/O=PDF-Over NG Test (NUR TEST)/CN=PDF-Over NG Test ECDSA P-256 (NUR TEST)" 0x0A0B0C0D02
  # P-384 mit Titel-RDN (2.5.4.12): Golden-Test „subject.title“ (Plan §6.5) und SHA-256 für P-384.
  make_key_and_cert test-ecp384-title "ec -pkeyopt ec_paramgen_curve:P-384" \
    "/C=AT/O=PDF-Over NG Test (NUR TEST)/title=Dipl.-Ing. (NUR TEST)/CN=PDF-Over NG Test ECDSA P-384 (NUR TEST)" 0x0A0B0C0D03

  # Moderne Verschlüsselung (OpenSSL-3-Default: PBES2/AES-256-CBC, PBKDF2, HMAC-SHA-256)
  export_p12 test-rsa2048 "$KS/test-rsa2048.p12"
  export_p12 test-ecp256 "$KS/test-ecp256.p12"
  export_p12 test-ecp384-title "$KS/test-ecp384-title.p12"
  # Legacy-Verschlüsselung wie ältere Java-/Windows-Exporte (PBE-SHA1-3DES, MAC SHA-1)
  export_p12 test-rsa2048 "$KS/test-rsa2048-legacy-3des.p12" \
    -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1
else
  echo "Test-Keystores vorhanden (mit --force neu erzeugen)."
fi

cat > "$KS/NUR-TEST-KEINE-ECHTEN-SCHLUESSEL.txt" <<'EOF'
NUR FÜR TESTS – KEINE ECHTEN SCHLÜSSEL
=====================================
Alle Dateien in diesem Ordner wurden von tools/make-testdata.sh mit openssl erzeugt.
Selbstsignierte Zertifikate, Passwort "test", Subject enthält "(NUR TEST)".
Niemals für echte Signaturen, Zugänge oder Produktivsysteme verwenden.
EOF

# ------------------------------------------------------------------------------------------------
# 2. PDFs aus PDF-AS (EUPL, reference/pdf-as-4)
# ------------------------------------------------------------------------------------------------
if [[ -d "$REF" ]]; then
  echo "Kopiere PDF-AS-Test-PDFs ..."
  while IFS= read -r -d '' f; do
    rel="${f#"$REF"/}"                 # z. B. pdf-as-cli/src/test/resources/rotate_090.pdf
    module="${rel%%/*}"                # pdf-as-cli
    sub="${rel#*/src/test/resources/}" # rotate_090.pdf bzw. data/xyz.pdf
    dest="$TD/pdf/pdfas/$module/$sub"
    mkdir -p "$(dirname "$dest")"
    cp "$f" "$dest"
  done < <(find "$REF" -path '*/src/test/resources/*' -name '*.pdf' -print0)

  echo "Kopiere PDF-AS-Testsuiten (ohne css/js/fonts und ohne test.p12) ..."
  SUITES="$REF/pdf-as-tests/src/test/test-suites/public_pdfbox2"
  cp "$SUITES/auto_pos_example.properties" "$SUITES/config.properties" "$TD/pdf/pdfas-suites/"
  for d in "$SUITES"/POS_* "$SUITES"/PDFA_*; do
    name="$(basename "$d")"
    mkdir -p "$TD/pdf/pdfas-suites/$name"
    find "$d" -maxdepth 1 -type f \( -name '*.pdf' -o -name '*.png' -o -name '*.properties' \) -exec cp {} "$TD/pdf/pdfas-suites/$name/" \;
  done
else
  echo "WARNUNG: $REF fehlt – PDF-AS-Testdaten übersprungen." >&2
fi

# ------------------------------------------------------------------------------------------------
# 3. Mit qpdf erzeugte Varianten. AES-256 (R6) verwendet zufällige Salze/Dateischlüssel und ist
#    daher nicht reproduzierbar – vorhandene Dateien werden nur mit --force neu erzeugt, damit Git
#    keine unnötigen Änderungen sieht. --static-id/--static-aes-iv nur für Testdaten.
# ------------------------------------------------------------------------------------------------
GEN="$TD/pdf/generated"
SIMPLE="$TD/pdf/pdfas/pdf-as-legacy/simple.pdf"
FIELDS="$TD/pdf/pdfas/pdf-as-pdfbox-2/manySignFields.pdf"

gen() {
  # $1 = Zieldatei, Rest = qpdf-Argumente (Eingabe und Ausgabe werden angehängt: <in> <out>)
  local out="$1" in="$2"; shift 2
  if [[ $FORCE -eq 0 && -f "$out" ]]; then return 0; fi
  qpdf --static-id "$@" "$in" "$out"
}

if [[ -f "$SIMPLE" ]]; then
  echo "Erzeuge qpdf-Varianten (fehlende bzw. mit --force alle) ..."
  # xref-Tabelle normalisiert (Referenz für K1-Tabelle)
  gen "$GEN/simple-xref-table.pdf" "$SIMPLE" --deterministic-id --object-streams=disable
  # xref-Stream + Object Streams
  gen "$GEN/simple-xref-stream.pdf" "$SIMPLE" --deterministic-id --object-streams=generate
  gen "$GEN/manySignFields-xref-stream.pdf" "$FIELDS" --deterministic-id --object-streams=generate
  # linearisiert
  gen "$GEN/simple-linearized.pdf" "$SIMPLE" --deterministic-id --linearize
  gen "$GEN/simple-linearized-xref-stream.pdf" "$SIMPLE" --deterministic-id --linearize --object-streams=generate
  # verschlüsselt ohne User-Passwort (K3); Owner-Passwort "test-owner"
  gen "$GEN/simple-aes256-owner.pdf" "$SIMPLE" --static-aes-iv --encrypt --user-password= --owner-password=test-owner --bits=256 --
  gen "$GEN/simple-aes128-owner.pdf" "$SIMPLE" --static-aes-iv --encrypt --user-password= --owner-password=test-owner --bits=128 --use-aes=y --
  gen "$GEN/simple-rc4-128-owner.pdf" "$SIMPLE" --allow-weak-crypto --encrypt --user-password= --owner-password=test-owner --bits=128 --use-aes=n --
  # verschlüsselt, Signieren nicht erlaubt (kein Ändern, keine Annotationen, kein Formular) → Ablehnung
  gen "$GEN/simple-aes256-nosign.pdf" "$SIMPLE" --static-aes-iv --encrypt --user-password= --owner-password=test-owner --bits=256 --modify=none --annotate=n --form=n --
  # verschlüsselt mit Benutzerpasswort "test" (K5)
  gen "$GEN/simple-aes256-userpw.pdf" "$SIMPLE" --static-aes-iv --encrypt --user-password=test --owner-password=test-owner --bits=256 --
  # verschlüsselt + xref-Stream
  gen "$GEN/simple-aes256-owner-xref-stream.pdf" "$SIMPLE" --static-aes-iv --object-streams=generate --encrypt --user-password= --owner-password=test-owner --bits=256 --
fi

# QR-Platzhalter höher als der Signaturblock (Backlog Nr. 32): new_qr_2-2.pdf mit Image5 auf 225 × 120 pt gestreckt
# (statt 225 × 45; Oberkante bleibt 720). qdf-Form bearbeiten, fix-qdf setzt Längen und xref neu.
QR22="$TD/pdf/pdfas/pdf-as-pdfbox-2/new_qr_2-2.pdf"
if [[ -f "$QR22" ]] && command -v fix-qdf >/dev/null && [[ $FORCE -eq 1 || ! -f "$GEN/qr-tall-placeholder.pdf" ]]; then
  tmp="$(mktemp -d)"
  qpdf --static-id --qdf --object-streams=disable "$QR22" "$tmp/qdf.pdf"
  LC_ALL=C sed 's/^225 0 0 45 72 675 cm$/225 0 0 120 72 600 cm/' "$tmp/qdf.pdf" >"$tmp/edit.pdf"
  fix-qdf "$tmp/edit.pdf" >"$tmp/fixed.pdf"
  qpdf --static-id --deterministic-id "$tmp/fixed.pdf" "$GEN/qr-tall-placeholder.pdf"
  rm -rf "$tmp"
fi

# Hybrid-Querverweis (Tabelle + /XRefStm): qpdf schreibt so etwas nicht, daher ein eigenes,
# deterministisches Skript (nur Python-Standardbibliothek).
if command -v python3 >/dev/null; then
  if [[ $FORCE -eq 1 || ! -f "$GEN/simple-xref-hybrid.pdf" ]]; then
    python3 -I "$ROOT/tools/make-hybrid-xref.py" "$GEN/simple-xref-hybrid.pdf"
  fi
else
  echo "WARNUNG: python3 fehlt – simple-xref-hybrid.pdf übersprungen." >&2
fi

echo "Größe testdata/: $(du -sh "$TD" | cut -f1)"
echo "Fertig."

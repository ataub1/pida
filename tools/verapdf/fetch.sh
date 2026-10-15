#!/usr/bin/env bash
# PIDA – veraPDF (Greenfield-CLI) als PDF/A-Orakel für Tests laden und per SHA-256 prüfen.
#
#   tools/verapdf/fetch.sh                 # laden (falls nötig) und prüfen
#   tools/verapdf/fetch.sh --print-jar     # laden (falls nötig), Pfad der JAR auf stdout
#   eval "$(tools/verapdf/fetch.sh --print-env)"   # PIDA_VERAPDF_JAR (und PIDA_JAVA) setzen
#   tools/verapdf/fetch.sh --github-env    # CI: Variablen nach $GITHUB_ENV schreiben
#   tools/verapdf/fetch.sh --check         # nur prüfen, nichts laden (Exit 1, wenn es fehlt)
#
# Quelle: Maven Central, org.verapdf.apps:greenfield-apps (offizielle veraPDF-Greenfield-Anwendung,
# `jar-with-dependencies`, CLI-Einstieg org.verapdf.apps.GreenfieldCliWrapper – dieselbe JAR, die der
# veraPDF-Installer unter bin/ ablegt). Version und SHA-256 sind gepinnt (siehe unten; Prüfsumme =
# die von Maven Central veröffentlichte `.sha256`, Stand 15.10.2026). Neue Version: VERAPDF_VERSION,
# VERAPDF_SHA256 und VERAPDF_SIZE gemeinsam ändern und gegen die `.sha256`/`.asc` auf Maven Central
# abgleichen.
#
# Ziel: tools/verapdf/dist/ (nicht versioniert, tools/verapdf/.gitignore). veraPDF ist nur ein
# Test-Orakel (GPL-3.0-or-later bzw. MPL-2.0) und wird nie mit PIDA ausgeliefert.
# Laufzeit: Java ≥ 11 (PIDA_JAVA, sonst $JAVA_HOME/bin/java, sonst `java` im PATH).
# Benötigt: bash ≥ 3.2, curl (oder wget), sha256sum/shasum/openssl. Lädt nur über HTTPS.
# Umgebung (optional): VERAPDF_DEST (Zielordner), VERAPDF_BASE_URL (Spiegel von Maven Central).
set -euo pipefail

VERAPDF_VERSION="1.28.2"
VERAPDF_SHA256="687d4d8bcfec48c9f7e931cf78ed061f1eeccfba55d4cdc000ba9b4d8aa46ce6"
VERAPDF_SIZE="9485057"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEST="${VERAPDF_DEST:-$ROOT/tools/verapdf/dist}"
BASE_URL="${VERAPDF_BASE_URL:-https://repo1.maven.org/maven2}"
JAR_NAME="greenfield-apps-$VERAPDF_VERSION.jar"
URL="$BASE_URL/org/verapdf/apps/greenfield-apps/$VERAPDF_VERSION/$JAR_NAME"
JAR="$DEST/$JAR_NAME"

die() { echo "fetch-verapdf: $*" >&2; exit 1; }
log() { echo "fetch-verapdf: $*" >&2; }

usage() {
  sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 -r "$1" | awk '{print $1}'
  else
    die "kein SHA-256-Werkzeug (sha256sum, shasum oder openssl) gefunden"
  fi
}

size_of() {
  wc -c <"$1" | tr -d ' '
}

verified() {
  [ -f "$JAR" ] && [ "$(size_of "$JAR")" = "$VERAPDF_SIZE" ] && [ "$(sha256_of "$JAR")" = "$VERAPDF_SHA256" ]
}

download() {
  case "$URL" in https://*) ;; *) die "nur HTTPS: $URL" ;; esac
  mkdir -p "$DEST"
  local tmp="$JAR.part"
  rm -f "$tmp"
  log "lade $URL"
  if command -v curl >/dev/null 2>&1; then
    curl --fail --location --proto '=https' --tlsv1.2 --silent --show-error --output "$tmp" "$URL"
  elif command -v wget >/dev/null 2>&1; then
    wget --quiet --https-only --output-document="$tmp" "$URL"
  else
    die "weder curl noch wget gefunden"
  fi
  local got
  got="$(sha256_of "$tmp")"
  if [ "$got" != "$VERAPDF_SHA256" ] || [ "$(size_of "$tmp")" != "$VERAPDF_SIZE" ]; then
    rm -f "$tmp"
    die "Prüfsumme bzw. Größe passt nicht ($got, erwartet $VERAPDF_SHA256 / $VERAPDF_SIZE Bytes) – Datei verworfen"
  fi
  mv "$tmp" "$JAR"
  log "geprüft: $JAR"
}

find_java() {
  if [ -n "${PIDA_JAVA:-}" ] && [ -x "$PIDA_JAVA" ]; then
    echo "$PIDA_JAVA"
  elif [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/java" ]; then
    echo "$JAVA_HOME/bin/java"
  elif command -v java >/dev/null 2>&1 && java -version >/dev/null 2>&1; then
    command -v java
  fi
}

ensure() {
  if verified; then
    return
  fi
  if [ -f "$JAR" ]; then log "vorhandene JAR passt nicht zur Prüfsumme – neu laden"; fi
  download
}

mode="fetch"
case "${1:-}" in
  "") ;;
  -h | --help) usage 0 ;;
  --print-jar) mode="print-jar" ;;
  --print-env) mode="print-env" ;;
  --github-env) mode="github-env" ;;
  --check) mode="check" ;;
  *) usage 2 ;;
esac

case "$mode" in
  check)
    verified || die "veraPDF $VERAPDF_VERSION fehlt oder passt nicht: $JAR"
    log "ok: $JAR"
    ;;
  fetch)
    ensure
    java="$(find_java)"
    if [ -z "$java" ]; then log "Hinweis: kein Java gefunden (PIDA_JAVA oder JAVA_HOME setzen)"; fi
    ;;
  print-jar)
    ensure
    echo "$JAR"
    ;;
  print-env)
    ensure
    printf 'export PIDA_VERAPDF_JAR=%q\n' "$JAR"
    java="$(find_java)"
    if [ -n "$java" ]; then printf 'export PIDA_JAVA=%q\n' "$java"; fi
    ;;
  github-env)
    ensure
    [ -n "${GITHUB_ENV:-}" ] || die "GITHUB_ENV nicht gesetzt"
    echo "PIDA_VERAPDF_JAR=$JAR" >>"$GITHUB_ENV"
    java="$(find_java)"
    if [ -n "$java" ]; then echo "PIDA_JAVA=$java" >>"$GITHUB_ENV"; fi
    ;;
esac

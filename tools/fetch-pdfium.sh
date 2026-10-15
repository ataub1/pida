#!/usr/bin/env bash
# PDF-Over NG – PDFium-Binaries (bblanchon/pdfium-binaries) laden, per SHA-256 prüfen und entpacken.
#
#   tools/fetch-pdfium.sh                      # Plattform dieses Rechners
#   tools/fetch-pdfium.sh linux-x64 win-x64    # bestimmte Plattformen
#   tools/fetch-pdfium.sh --all                # alle Plattformen aus tools/pdfium.lock
#   tools/fetch-pdfium.sh --print-dir          # laden (falls nötig), Bibliotheksverzeichnis auf stdout
#   eval "$(tools/fetch-pdfium.sh --print-env)"   # laden und PDFOVER_PDFIUM_DIR in der Shell setzen
#   tools/fetch-pdfium.sh --github-env         # CI: PDFOVER_PDFIUM_DIR nach $GITHUB_ENV schreiben
#   PDFIUM_RELEASE=chromium/<n> tools/fetch-pdfium.sh --update-lock --all   # neue Release einpflegen
#
# Ziel: tools/pdfium/<plattform>/ (nicht versioniert, .gitignore), Bibliothek in lib/ bzw. unter
# Windows in bin/. Die erwarteten Hashes stehen in tools/pdfium.lock; eine Abweichung bricht ab und
# löscht das Archiv. Bereits entpackte Plattformen mit passendem Hash werden nicht neu geladen.
#
# Plattformen: mac-arm64 mac-x64 linux-x64 linux-arm64 win-x64 win-arm64 ios-device-arm64
#              ios-simulator-arm64 ios-simulator-x64 android-arm64 android-arm android-x64
#
# Umgebung (optional): PDFIUM_DEST (Zielordner, Standard tools/pdfium), PDFIUM_CACHE (Archive,
# Standard <Ziel>/.downloads), PDFIUM_RELEASE (nur mit --update-lock), PDFIUM_BASE_URL (Spiegel).
# Benötigt: bash ≥ 3.2, curl (oder wget), tar, sha256sum/shasum/openssl. Lädt nur über HTTPS.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/tools/pdfium.lock"
DEST="${PDFIUM_DEST:-$ROOT/tools/pdfium}"
ALL_PLATFORMS="mac-arm64 mac-x64 linux-x64 linux-arm64 win-x64 win-arm64 ios-device-arm64 ios-simulator-arm64 ios-simulator-x64 android-arm64 android-arm android-x64"

die() { echo "fetch-pdfium: $*" >&2; exit 1; }
log() { echo "fetch-pdfium: $*" >&2; }

usage() {
  sed -n '2,21p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

host_platform() {
  local os arch
  case "$(uname -s)" in
    Darwin) os=mac ;;
    Linux) os=linux ;;
    MINGW* | MSYS* | CYGWIN* | Windows_NT) os=win ;;
    *) die "unbekanntes Betriebssystem $(uname -s); Plattform bitte angeben" ;;
  esac
  case "$(uname -m)" in
    arm64 | aarch64) arch=arm64 ;;
    x86_64 | amd64) arch=x64 ;;
    *) die "unbekannte Architektur $(uname -m); Plattform bitte angeben" ;;
  esac
  echo "$os-$arch"
}

# Verzeichnis der Bibliothek innerhalb einer Plattform (Windows: bin/pdfium.dll, sonst lib/).
lib_subdir() {
  case "$1" in
    win-*) echo bin ;;
    *) echo lib ;;
  esac
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

lock_release() {
  [ -f "$LOCK" ] || die "$LOCK fehlt"
  awk '$1 == "release" { print $2; exit }' "$LOCK"
}

lock_sha() {
  awk -v f="pdfium-$1.tgz" '$2 == f && $1 ~ /^[0-9a-f]{64}$/ { print $1; exit }' "$LOCK"
}

download() {
  local url="$1" out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 --retry 3 --retry-delay 2 \
      --output "$out" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget --quiet --https-only --tries=3 --output-document="$out" "$url"
  else
    die "weder curl noch wget gefunden"
  fi
}

# Archiv darf nur relative Pfade ohne '..' enthalten.
check_members() {
  local members bad
  members="$(tar -tzf "$1")" || die "$1: kein lesbares tar.gz-Archiv"
  # awk liest alles (kein vorzeitiges Ende → kein SIGPIPE unter pipefail) und meldet den ersten Fund.
  bad="$(printf '%s\n' "$members" | awk '(/^\// || /(^|\/)\.\.(\/|$)/) && !found { print; found = 1 }')"
  [ -z "$bad" ] || die "$1: unzulässiger Pfad im Archiv: $bad"
}

valid_platform() {
  local p
  for p in $ALL_PLATFORMS; do [ "$p" = "$1" ] && return 0; done
  return 1
}

# --- Argumente ---------------------------------------------------------------------------------
PLATFORMS=""
MODE=fetch
GITHUB_ENV_OUT=0
UPDATE_LOCK=0
FORCE=0
while [ $# -gt 0 ]; do
  case "$1" in
    -h | --help) usage 0 ;;
    --all) PLATFORMS="$ALL_PLATFORMS" ;;
    --list) echo "$ALL_PLATFORMS" | tr ' ' '\n'; exit 0 ;;
    --print-dir) MODE=print-dir ;;
    --print-env) MODE=print-env ;;
    --github-env) GITHUB_ENV_OUT=1 ;;
    --update-lock) UPDATE_LOCK=1 ;;
    --force) FORCE=1 ;;
    --dest) shift; [ $# -gt 0 ] || die "--dest braucht ein Verzeichnis"; DEST="$1" ;;
    -*) log "unbekannte Option $1"; usage 2 ;;
    *) valid_platform "$1" || die "unbekannte Plattform '$1' (--list zeigt alle)"; PLATFORMS="$PLATFORMS $1" ;;
  esac
  shift
done
[ -n "${PLATFORMS// /}" ] || PLATFORMS="$(host_platform)"
FIRST="$(echo "$PLATFORMS" | awk '{print $1}')"

mkdir -p "$DEST"
DEST="$(cd "$DEST" && pwd)"

if [ "$UPDATE_LOCK" = 1 ]; then
  RELEASE="${PDFIUM_RELEASE:-$(lock_release)}"
else
  RELEASE="$(lock_release)"
  [ -z "${PDFIUM_RELEASE:-}" ] || [ "$PDFIUM_RELEASE" = "$RELEASE" ] \
    || die "PDFIUM_RELEASE=$PDFIUM_RELEASE weicht von tools/pdfium.lock ($RELEASE) ab; nur mit --update-lock"
fi
[ -n "$RELEASE" ] || die "keine Release in $LOCK"
case "$RELEASE" in chromium/[0-9]*) ;; *) die "ungültige Release '$RELEASE'" ;; esac
BASE_URL="${PDFIUM_BASE_URL:-https://github.com/bblanchon/pdfium-binaries/releases/download}/$RELEASE"
CACHE="${PDFIUM_CACHE:-$DEST/.downloads}/${RELEASE//\//-}"

# --- Laden, prüfen, entpacken ------------------------------------------------------------------
NEW_LOCK_LINES=""
for plat in $PLATFORMS; do
  archive="pdfium-$plat.tgz"
  target="$DEST/$plat"
  libdir="$target/$(lib_subdir "$plat")"
  marker="$target/.pdfium-archive.sha256"
  expected=""
  if [ "$UPDATE_LOCK" = 0 ]; then
    expected="$(lock_sha "$plat")"
    [ -n "$expected" ] || die "$archive fehlt in tools/pdfium.lock"
    if [ "$FORCE" = 0 ] && [ -f "$marker" ] && [ "$(cat "$marker")" = "$expected" ] && [ -d "$libdir" ]; then
      log "$plat: aktuell ($RELEASE)"
      continue
    fi
  fi

  mkdir -p "$CACHE"
  file="$CACHE/$archive"
  # Zwischengespeichertes Archiv nur verwenden, wenn der Hash passt (sonst verwerfen und neu laden).
  if [ -f "$file" ] && { [ "$FORCE" = 1 ] || { [ -n "$expected" ] && [ "$(sha256_of "$file")" != "$expected" ]; }; }; then
    rm -f "$file"
  fi
  if [ ! -f "$file" ]; then
    log "$plat: lade $BASE_URL/$archive"
    rm -f "$file.part"
    download "$BASE_URL/$archive" "$file.part" || { rm -f "$file.part"; die "$plat: Download fehlgeschlagen"; }
    mv "$file.part" "$file"
  fi

  actual="$(sha256_of "$file")"
  if [ "$UPDATE_LOCK" = 1 ]; then
    NEW_LOCK_LINES="$NEW_LOCK_LINES$actual  $archive
"
  elif [ "$actual" != "$expected" ]; then
    rm -f "$file"
    die "$plat: SHA-256 stimmt nicht (erwartet $expected, erhalten $actual) – Archiv gelöscht"
  fi

  check_members "$file"
  tmp="$DEST/.tmp-$plat-$$"
  rm -rf "$tmp"
  mkdir -p "$tmp"
  tar -xzf "$file" -C "$tmp"
  [ -d "$tmp/$(lib_subdir "$plat")" ] || { rm -rf "$tmp"; die "$plat: $(lib_subdir "$plat")/ fehlt im Archiv"; }
  echo "$actual" >"$tmp/.pdfium-archive.sha256"
  rm -rf "$target"
  mv "$tmp" "$target"
  log "$plat: entpackt nach $target ($(awk -F= '$1=="BUILD"{print "Build " $2}' "$target/VERSION" 2>/dev/null))"
done

if [ "$UPDATE_LOCK" = 1 ]; then
  # Kopfkommentar übernehmen, Release und Hashes ersetzen; nicht geladene Plattformen bleiben stehen.
  tmp_lock="$LOCK.tmp.$$"
  {
    awk '/^#/ { print; next } { exit }' "$LOCK"
    echo "release $RELEASE"
    for p in $ALL_PLATFORMS; do
      line="$(printf '%s' "$NEW_LOCK_LINES" | awk -v f="pdfium-$p.tgz" '$2 == f { print; exit }')"
      if [ -n "$line" ]; then
        echo "$line"
      else
        old="$(lock_sha "$p")"
        [ -z "$old" ] || echo "$old  pdfium-$p.tgz"
      fi
    done
  } >"$tmp_lock"
  mv "$tmp_lock" "$LOCK"
  log "tools/pdfium.lock aktualisiert ($RELEASE) – Hashes gegen die Release-Seite prüfen und Kopfkommentar anpassen"
fi

# --- Ausgabe -----------------------------------------------------------------------------------
FIRST_DIR="$DEST/$FIRST/$(lib_subdir "$FIRST")"
case "$MODE" in
  print-dir) echo "$FIRST_DIR" ;;
  print-env) printf 'export PDFOVER_PDFIUM_DIR=%q\n' "$FIRST_DIR" ;;
  *) ;;
esac
if [ "$GITHUB_ENV_OUT" = 1 ]; then
  [ -n "${GITHUB_ENV:-}" ] || die "--github-env: GITHUB_ENV ist nicht gesetzt"
  echo "PDFOVER_PDFIUM_DIR=$FIRST_DIR" >>"$GITHUB_ENV"
  log "PDFOVER_PDFIUM_DIR=$FIRST_DIR → \$GITHUB_ENV"
fi

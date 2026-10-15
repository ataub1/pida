#!/usr/bin/env bash
# PDF-Over NG – PDFium-Binaries (bblanchon/pdfium-binaries) laden, per SHA-256 prüfen und entpacken.
#
#   tools/fetch-pdfium.sh                      # Plattform dieses Rechners
#   tools/fetch-pdfium.sh linux-x64 win-x64    # bestimmte Plattformen
#   tools/fetch-pdfium.sh --all                # alle Plattformen aus tools/pdfium.lock
#   tools/fetch-pdfium.sh --print-dir          # laden (falls nötig), Bibliotheksverzeichnis auf stdout
#   eval "$(tools/fetch-pdfium.sh --print-env)"   # laden und PDFOVER_PDFIUM_DIR in der Shell setzen
#   tools/fetch-pdfium.sh --github-env         # CI: PDFOVER_PDFIUM_DIR nach $GITHUB_ENV schreiben
#   tools/fetch-pdfium.sh --xcframework        # iOS: <Ziel>/apple/PDFium.xcframework (Gerät + Simulator)
#   tools/fetch-pdfium.sh --jnilibs            # Android: <Ziel>/android/jniLibs/<abi>/libpdfium.so
#   PDFIUM_RELEASE=chromium/<n> tools/fetch-pdfium.sh --update-lock --all   # neue Release einpflegen
#
# Ziel: tools/pdfium/<plattform>/ (nicht versioniert, .gitignore), Bibliothek in lib/ bzw. unter
# Windows in bin/. Die erwarteten Hashes stehen in tools/pdfium.lock; eine Abweichung bricht ab und
# löscht das Archiv. Bereits entpackte Plattformen mit passendem Hash werden nicht neu geladen.
# --xcframework lädt ios-device-arm64, ios-simulator-arm64/-x64 und baut daraus ein Framework je Slice
# (install name @rpath/pdfium.framework/pdfium, Simulator als arm64+x86_64, ad-hoc signiert; nur macOS
# mit Xcode). --jnilibs lädt android-arm64/-arm/-x64 (ABIs arm64-v8a, armeabi-v7a, x86_64).
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
  sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
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
XCFRAMEWORK=0
JNILIBS=0
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
    --xcframework) XCFRAMEWORK=1; PLATFORMS="$PLATFORMS ios-device-arm64 ios-simulator-arm64 ios-simulator-x64" ;;
    --jnilibs) JNILIBS=1; PLATFORMS="$PLATFORMS android-arm64 android-arm android-x64" ;;
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

# --- Mobile: xcframework (iOS) und jniLibs (Android) --------------------------------------------
build_version() {
  awk -F= '$1 == "MAJOR" { major = $2 } $1 == "MINOR" { minor = $2 } $1 == "BUILD" { build = $2 } END { if (major != "") print major "." (minor == "" ? 0 : minor) "." build; else print "0.0.0" }' "$1"
}

# Ein Framework-Slice: pdfium.framework/{pdfium, Info.plist}, install name @rpath/…, ad-hoc signiert.
make_framework() {
  local dylib="$1" out="$2" platform="$3" version="$4" minos="$5"
  mkdir -p "$out/pdfium.framework"
  cp "$dylib" "$out/pdfium.framework/pdfium"
  chmod u+w "$out/pdfium.framework/pdfium"
  install_name_tool -id "@rpath/pdfium.framework/pdfium" "$out/pdfium.framework/pdfium"
  cat >"$out/pdfium.framework/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>pdfium</string>
  <key>CFBundleIdentifier</key><string>org.pdfium.pdfium</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>pdfium</string>
  <key>CFBundlePackageType</key><string>FMWK</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleSupportedPlatforms</key><array><string>$platform</string></array>
  <key>MinimumOSVersion</key><string>$minos</string>
</dict>
</plist>
PLIST
  # Simulator verlangt eine Signatur; Xcode signiert beim Einbetten („Embed & Sign“) mit dem Team neu.
  codesign --force --sign - --timestamp=none "$out/pdfium.framework" >/dev/null 2>&1 || die "codesign (ad hoc) fehlgeschlagen"
}

# Mindest-iOS aus LC_BUILD_VERSION der Bibliothek (pdfium-binaries chromium/8100: 17.0).
ios_minos() {
  otool -l "$1" | awk '/LC_BUILD_VERSION/ { found = 1 } found && $1 == "minos" { print $2; exit }'
}

if [ "$XCFRAMEWORK" = 1 ]; then
  [ "$(uname -s)" = Darwin ] || die "--xcframework nur unter macOS (xcodebuild, lipo, install_name_tool, codesign)"
  for tool in xcodebuild lipo install_name_tool codesign otool; do
    command -v "$tool" >/dev/null 2>&1 || die "--xcframework: $tool fehlt (Xcode installieren)"
  done
  device="$DEST/ios-device-arm64/lib/libpdfium.dylib"
  sim_arm="$DEST/ios-simulator-arm64/lib/libpdfium.dylib"
  sim_x64="$DEST/ios-simulator-x64/lib/libpdfium.dylib"
  for f in "$device" "$sim_arm" "$sim_x64"; do [ -f "$f" ] || die "--xcframework: $f fehlt"; done
  version="$(build_version "$DEST/ios-device-arm64/VERSION")"
  minos="$(ios_minos "$device")"
  [ -n "$minos" ] || minos=17.0
  work="$DEST/.tmp-xcframework-$$"
  rm -rf "$work"
  mkdir -p "$work/sim"
  lipo -create "$sim_arm" "$sim_x64" -output "$work/sim/libpdfium.dylib"
  make_framework "$device" "$work/device" iPhoneOS "$version" "$minos"
  make_framework "$work/sim/libpdfium.dylib" "$work/simulator" iPhoneSimulator "$version" "$minos"
  xcf="$DEST/apple/PDFium.xcframework"
  rm -rf "$xcf"
  mkdir -p "$DEST/apple"
  if ! xcodebuild -create-xcframework \
    -framework "$work/device/pdfium.framework" \
    -framework "$work/simulator/pdfium.framework" \
    -output "$xcf" >"$work/xcodebuild.log" 2>&1; then
    # z. B. „xcodebuild failed to load a required plug-in“ (Xcode ohne `xcodebuild -runFirstLaunch`):
    # dieselbe Struktur wie xcodebuild schreiben (Info.plist im Format XCFrameworkFormatVersion 1.0)
    log "xcodebuild -create-xcframework nicht möglich ($(grep -m1 -o 'xcodebuild failed[^.]*' "$work/xcodebuild.log" || echo 'siehe Log')) – xcframework wird direkt zusammengestellt"
    rm -rf "$xcf"
    mkdir -p "$xcf/ios-arm64" "$xcf/ios-arm64_x86_64-simulator"
    cp -R "$work/device/pdfium.framework" "$xcf/ios-arm64/"
    cp -R "$work/simulator/pdfium.framework" "$xcf/ios-arm64_x86_64-simulator/"
    cat >"$xcf/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>AvailableLibraries</key>
  <array>
    <dict>
      <key>BinaryPath</key><string>pdfium.framework/pdfium</string>
      <key>LibraryIdentifier</key><string>ios-arm64</string>
      <key>LibraryPath</key><string>pdfium.framework</string>
      <key>SupportedArchitectures</key><array><string>arm64</string></array>
      <key>SupportedPlatform</key><string>ios</string>
    </dict>
    <dict>
      <key>BinaryPath</key><string>pdfium.framework/pdfium</string>
      <key>LibraryIdentifier</key><string>ios-arm64_x86_64-simulator</string>
      <key>LibraryPath</key><string>pdfium.framework</string>
      <key>SupportedArchitectures</key><array><string>arm64</string><string>x86_64</string></array>
      <key>SupportedPlatform</key><string>ios</string>
      <key>SupportedPlatformVariant</key><string>simulator</string>
    </dict>
  </array>
  <key>CFBundlePackageType</key><string>XFWK</string>
  <key>XCFrameworkFormatVersion</key><string>1.0</string>
</dict>
</plist>
PLIST
    plutil -lint "$xcf/Info.plist" >/dev/null || { rm -rf "$work"; die "Info.plist des xcframework ungültig"; }
  fi
  cp "$DEST/ios-device-arm64/VERSION" "$DEST/apple/VERSION"
  rm -rf "$work"
  log "iOS: $xcf (pdfium $version, Mindest-iOS $minos; install name @rpath/pdfium.framework/pdfium)"
fi

if [ "$JNILIBS" = 1 ]; then
  jni="$DEST/android/jniLibs"
  rm -rf "$jni"
  for pair in android-arm64:arm64-v8a android-arm:armeabi-v7a android-x64:x86_64; do
    plat="${pair%%:*}"
    abi="${pair##*:}"
    so="$DEST/$plat/lib/libpdfium.so"
    [ -f "$so" ] || die "--jnilibs: $so fehlt"
    mkdir -p "$jni/$abi"
    cp "$so" "$jni/$abi/libpdfium.so"
  done
  cp "$DEST/android-arm64/VERSION" "$DEST/android/VERSION"
  log "Android: $jni/{arm64-v8a,armeabi-v7a,x86_64}/libpdfium.so"
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

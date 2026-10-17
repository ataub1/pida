#!/usr/bin/env bash
# Prüft die Codesignatur des macOS-Bundles (ad hoc oder Developer ID): PIDA.app samt Frameworks
# (libpdfium.dylib) und – falls gebaut – die App in der DMG.
#
# Bricht ab, wenn `codesign --verify --deep --strict` scheitert oder die Ressourcen nicht versiegelt sind
# („Sealed Resources=none“, nur vom Linker ad hoc signiert): Eine so gebaute App meldet macOS nach dem
# Herunterladen als „beschädigt“ statt „nicht verifizierter Entwickler“ (PIDA 0.2.0). Signiert wird beim
# Bündeln durch Tauri (`bundle.macOS.signingIdentity` in apps/app/src-tauri/tauri.conf.json, ad hoc "-" ohne
# Hardened Runtime; APPLE_SIGNING_IDENTITY ersetzt das für eine echte Identität, apps/app/scripts/bundle-config.mjs
# schaltet dann die Hardened Runtime ein). Die Gatekeeper-Bewertung (`spctl`) wird nur ausgegeben: Ad hoc
# signiert lautet sie weiterhin „rejected“ (macOS bietet „Dennoch öffnen“ statt „beschädigt“).
#
#   tools/verify-macos-bundle.sh [<bundle-ordner>]     Standard: target/release/bundle
set -euo pipefail

bundle="${1:-target/release/bundle}"
app="$bundle/macos/PIDA.app"

die() {
  echo "verify-macos-bundle: $*" >&2
  exit 1
}

[ "$(uname -s)" = Darwin ] || die "nur unter macOS"
[ -d "$app" ] || die "$app fehlt (zuerst bündeln)"

check_app() {
  local a="$1" info lib
  codesign --verify --deep --strict --verbose=2 "$a" || die "$a: codesign --verify --deep --strict scheitert"
  info="$(codesign -dv --verbose=4 "$a" 2>&1)"
  grep -q '^Sealed Resources version=' <<<"$info" || die "$a: Ressourcen nicht versiegelt (Sealed Resources=none)"
  # Ad hoc mit Hardened Runtime: deren Library Validation lehnt die ad hoc signierte libpdfium.dylib ab
  # („different Team IDs“) – die App liefe ohne Vorschau.
  if grep -q '^Signature=adhoc' <<<"$info" && grep -Eq '^CodeDirectory .*flags=[^ ]*runtime' <<<"$info"; then
    die "$a: ad hoc signiert mit Hardened Runtime (bundle.macOS.hardenedRuntime false setzen)"
  fi
  grep -E '^(Identifier|Format|CodeDirectory|Signature|TeamIdentifier|Sealed Resources)' <<<"$info" || true
  for lib in "$a"/Contents/Frameworks/*; do
    [ -e "$lib" ] || continue
    codesign --verify --strict --verbose=2 "$lib" || die "$lib: Signatur ungültig"
  done
  echo "spctl (nur Information): $(spctl -a -t exec -vv "$a" 2>&1 | tr '\n' ' ')"
}

check_app "$app"

mnt=""
cleanup() {
  if [ -n "$mnt" ]; then
    hdiutil detach "$mnt" -quiet || true
    rmdir "$mnt" 2>/dev/null || true
  fi
}
trap cleanup EXIT

for dmg in "$bundle"/dmg/*.dmg; do
  [ -e "$dmg" ] || continue
  echo "DMG: $dmg"
  codesign --verify --strict --verbose=2 "$dmg" 2>&1 || echo "(DMG selbst nicht signiert)"
  mnt="$(mktemp -d)"
  hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$dmg" >/dev/null
  [ -d "$mnt/PIDA.app" ] || die "$dmg: PIDA.app fehlt"
  check_app "$mnt/PIDA.app"
  cleanup
  mnt=""
done
echo "verify-macos-bundle: ok"

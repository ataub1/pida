#!/usr/bin/env bash
# Prüft die Codesignatur des macOS-Bundles (ad hoc oder Developer ID): PIDA.app samt Frameworks
# (libpdfium.dylib) und – falls gebaut – die App in der DMG.
#
# Bricht ab, wenn `codesign --verify --deep --strict` scheitert oder die Ressourcen nicht versiegelt sind
# („Sealed Resources=none“, nur vom Linker ad hoc signiert): Eine so gebaute App meldet macOS nach dem
# Herunterladen als „beschädigt“ statt „nicht verifizierter Entwickler“ (PIDA 0.2.0). Signiert wird beim
# Bündeln durch Tauri (`bundle.macOS.signingIdentity` in apps/app/src-tauri/tauri.conf.json, ad hoc "-" ohne
# Hardened Runtime; APPLE_SIGNING_IDENTITY ersetzt das für eine echte Identität, apps/app/scripts/bundle-config.mjs
# schaltet dann die Hardened Runtime ein). Geprüft wird jede Mach-O-Datei im Bundle (gültig signiert, Team-ID
# der App) sowie: ad hoc nie mit, eine echte Identität nie ohne Hardened Runtime (Notarisierung). Die
# Gatekeeper-Bewertung (`spctl`) wird nur ausgegeben: Ad hoc signiert lautet sie weiterhin „rejected“ (macOS
# bietet „Dennoch öffnen“ statt „beschädigt“).
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
  local a="$1" info team runtime f macho=0
  codesign --verify --deep --strict --verbose=2 "$a" || die "$a: codesign --verify --deep --strict scheitert"
  info="$(codesign -dv --verbose=4 "$a" 2>&1)"
  grep -q '^Sealed Resources version=' <<<"$info" || die "$a: Ressourcen nicht versiegelt (Sealed Resources=none)"
  runtime=0
  grep -Eq '^CodeDirectory .*flags=[^ ]*runtime' <<<"$info" && runtime=1
  if grep -q '^Signature=adhoc' <<<"$info"; then
    # Ad hoc mit Hardened Runtime: deren Library Validation lehnt die ad hoc signierte libpdfium.dylib ab
    # („different Team IDs“) – die App liefe ohne Vorschau.
    [ "$runtime" = 0 ] || die "$a: ad hoc signiert mit Hardened Runtime (bundle.macOS.hardenedRuntime false setzen)"
  else
    # Echte Identität: notarytool verlangt die Hardened Runtime (apps/app/scripts/bundle-config.mjs).
    [ "$runtime" = 1 ] || die "$a: mit Identität signiert, aber ohne Hardened Runtime (Notarisierung lehnt ab)"
  fi
  team="$(sed -n 's/^TeamIdentifier=//p' <<<"$info")"
  grep -E '^(Identifier|Format|CodeDirectory|Signature|TeamIdentifier|Sealed Resources)' <<<"$info" || true
  # Jede Mach-O-Datei im Bundle (Binary, Frameworks wie libpdfium.dylib, auch außerhalb der Standardorte, die
  # --deep nicht als Code prüft): gültig signiert und mit derselben Team-ID wie die App (Library Validation).
  while IFS= read -r -d '' f; do
    case "$(file -b "$f")" in Mach-O*) ;; *) continue ;; esac
    macho=$((macho + 1))
    codesign --verify --strict --verbose=2 "$f" || die "$f: Signatur ungültig"
    [ "$(codesign -dv --verbose=4 "$f" 2>&1 | sed -n 's/^TeamIdentifier=//p')" = "$team" ] ||
      die "$f: andere Team-ID als $a ($team)"
  done < <(find "$a" -type f -print0)
  [ "$macho" -ge 1 ] || die "$a: keine Mach-O-Datei gefunden"
  echo "Mach-O-Dateien geprüft: $macho"
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

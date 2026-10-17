# Entwicklungsumgebung für PIDA: in jeder Shell `source tools/env.sh` ausführen.
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

# Übergang zu PIDA (ADR 0002): Werte früherer Variablen aus der aufrufenden Umgebung übernehmen, sofern
# die neue Variable nicht gesetzt ist, und alle alten entfernen – danach gibt es nur noch PIDA_*.
[ -n "${PIDA_PDFIUM_DIR:-}" ] || [ -z "${PDFOVER_PDFIUM_DIR:-}" ] || export PIDA_PDFIUM_DIR="$PDFOVER_PDFIUM_DIR"  # rename-to-pida:keep
[ -n "${PIDA_REFERENCE:-}" ] || [ -z "${PDFOVER_REFERENCE:-}" ] || export PIDA_REFERENCE="$PDFOVER_REFERENCE"  # rename-to-pida:keep
[ -n "${PIDA_HOME:-}" ] || [ -z "${PDFOVER_HOME:-}" ] || export PIDA_HOME="$PDFOVER_HOME"  # rename-to-pida:keep
[ -n "${PIDA_PYTHON:-}" ] || [ -z "${PDFOVER_PYTHON:-}" ] || export PIDA_PYTHON="$PDFOVER_PYTHON"  # rename-to-pida:keep
[ -n "${PIDA_JAVA:-}" ] || [ -z "${PDFOVER_JAVA:-}" ] || export PIDA_JAVA="$PDFOVER_JAVA"  # rename-to-pida:keep
[ -n "${PIDA_VERAPDF_JAR:-}" ] || [ -z "${PDFOVER_VERAPDF_JAR:-}" ] || export PIDA_VERAPDF_JAR="$PDFOVER_VERAPDF_JAR"  # rename-to-pida:keep
[ -n "${PIDA_PDFAS_LIB:-}" ] || [ -z "${PDFOVER_PDFAS_LIB:-}" ] || export PIDA_PDFAS_LIB="$PDFOVER_PDFAS_LIB"  # rename-to-pida:keep
[ -n "${PIDA_NOTICES_DIR:-}" ] || [ -z "${PDFOVER_NOTICES_DIR:-}" ] || export PIDA_NOTICES_DIR="$PDFOVER_NOTICES_DIR"  # rename-to-pida:keep
[ -n "${PIDA_GOLDEN_CACHE:-}" ] || [ -z "${PDFOVER_GOLDEN_CACHE:-}" ] || export PIDA_GOLDEN_CACHE="$PDFOVER_GOLDEN_CACHE"  # rename-to-pida:keep
[ -n "${PIDA_LOG:-}" ] || [ -z "${PDFOVER_LOG:-}" ] || export PIDA_LOG="$PDFOVER_LOG"  # rename-to-pida:keep
if env | grep -q '^PDFOVER_'; then  # rename-to-pida:keep
  # shellcheck disable=SC2046 # Wortaufteilung gewollt: ein Variablenname je Wort
  unset $(env | sed -n 's/^\(PDFOVER_[A-Z0-9_]*\)=.*/\1/p')  # rename-to-pida:keep
fi

PIDA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-${(%):-%x}}")/.." && pwd)"
export PIDA_ROOT

# Hauptcheckout ermitteln: In einem `git worktree` (parallele Pakete, ADR 0001 §4) fehlen die
# nicht versionierten Ordner `reference/` und `.venv/`; sie werden aus dem Hauptcheckout genutzt.
_pida_common="$(git -C "$PIDA_ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)"
if [ -n "$_pida_common" ]; then
  PIDA_MAIN="$(dirname "$_pida_common")"
else
  PIDA_MAIN="$PIDA_ROOT"
fi
unset _pida_common
export PIDA_MAIN

# Referenzquellen (nur lesen): PDF-Over, PDF-AS 4, pdfas-config.
if [ -z "${PIDA_REFERENCE:-}" ]; then
  if [ -d "$PIDA_ROOT/reference" ]; then
    PIDA_REFERENCE="$PIDA_ROOT/reference"
  else
    PIDA_REFERENCE="$PIDA_MAIN/reference"
  fi
fi
export PIDA_REFERENCE

# Python-Umgebung mit pyHanko (Orakel für Tests/QA).
if [ -d "$PIDA_ROOT/.venv" ]; then
  export PATH="$PIDA_ROOT/.venv/bin:$PATH"
elif [ -d "$PIDA_MAIN/.venv" ]; then
  export PATH="$PIDA_MAIN/.venv/bin:$PATH"
fi

# PDFium des Hosts (tools/fetch-pdfium.sh), sofern vorhanden und nicht schon gesetzt.
if [ -z "${PIDA_PDFIUM_DIR:-}" ] && [ -d "$PIDA_MAIN/tools/pdfium/mac-arm64/lib" ] && [ "$(uname -sm)" = "Darwin arm64" ]; then
  export PIDA_PDFIUM_DIR="$PIDA_MAIN/tools/pdfium/mac-arm64/lib"
fi

# Mobile-Toolchains (Homebrew): JDK für Gradle, Android-SDK/NDK für Tauri-Android.
if [ -z "${JAVA_HOME:-}" ] && [ -d /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home ]; then
  export JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
fi
if [ -z "${ANDROID_HOME:-}" ] && [ -d /opt/homebrew/share/android-commandlinetools ]; then
  export ANDROID_HOME=/opt/homebrew/share/android-commandlinetools
fi
if [ -n "${ANDROID_HOME:-}" ] && [ -z "${NDK_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
  _pida_ndk="$(ls -1 "$ANDROID_HOME/ndk" 2>/dev/null | sort -V | tail -1)"
  [ -n "$_pida_ndk" ] && export NDK_HOME="$ANDROID_HOME/ndk/$_pida_ndk"
  unset _pida_ndk
fi
if [ -n "${ANDROID_HOME:-}" ]; then
  export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
fi

# Release-Builds ohne Benutzerpfade (docs/entwicklung.md §10 „Release bauen“), nur auf Wunsch:
#   PIDA_RELEASE_REMAP=1 source tools/env.sh
# setzt CARGO_ENCODED_RUSTFLAGS mit --remap-path-prefix für Cargo-Registry/-Git, rustup und dieses Repository:
# Panik-Orte und Debug-Infos nennen dann /cargo/registry/…, /rustup/…, /pida/… statt /Users/<name>/….
# CARGO_ENCODED_RUSTFLAGS (Trenner 0x1f) statt RUSTFLAGS, weil der Repo-Pfad Leerzeichen enthalten kann;
# vorhandene RUSTFLAGS werden übernommen. Für eine eigene Shell nur mit Release-Builds gedacht (geänderte Flags
# bauen neu); ohne die Variable bleiben Debug-Builds und Tests unverändert.
if [ "${PIDA_RELEASE_REMAP:-}" = 1 ]; then
  _pida_us="$(printf '\037')"
  _pida_flags="${CARGO_ENCODED_RUSTFLAGS:-}"
  if [ -z "$_pida_flags" ] && [ -n "${RUSTFLAGS:-}" ]; then
    _pida_flags="$(printf '%s' "$RUSTFLAGS" | tr -s ' \t' '\037\037')"
    _pida_flags="${_pida_flags#"$_pida_us"}"
    _pida_flags="${_pida_flags%"$_pida_us"}"
  fi
  _pida_cargo="${CARGO_HOME:-$HOME/.cargo}"
  for _pida_map in "$_pida_cargo/registry=/cargo/registry" "$_pida_cargo/git=/cargo/git" \
    "${RUSTUP_HOME:-$HOME/.rustup}=/rustup" "$PIDA_ROOT=/pida"; do
    case "$_pida_us$_pida_flags$_pida_us" in
      *"$_pida_us--remap-path-prefix=$_pida_map$_pida_us"*) ;; # schon gesetzt (env.sh erneut eingelesen)
      *) _pida_flags="${_pida_flags:+$_pida_flags$_pida_us}--remap-path-prefix=$_pida_map" ;;
    esac
  done
  export CARGO_ENCODED_RUSTFLAGS="$_pida_flags"
  unset _pida_us _pida_flags _pida_cargo _pida_map
fi

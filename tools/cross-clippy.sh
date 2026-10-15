#!/usr/bin/env bash
# PDF-Over NG – die blockierenden Clippy-Schritte des CI-Jobs `build` lokal für ein Fremdziel nachstellen
# (z. B. Windows oder Linux von macOS aus), bevor die CI sie auf windows-latest/ubuntu-latest ausführt.
#
#   tools/cross-clippy.sh x86_64-pc-windows-msvc     # strikt (-D warnings, ohne CLIPPY_LENIENT) + fido2-windows
#   tools/cross-clippy.sh x86_64-unknown-linux-gnu   # strikt + fido2-hid
#
# Nur Typprüfung und Lints (cfg(windows)/cfg(target_os = "linux")-Code, plattformabhängige FFI-Typen),
# kein Linken, keine Tests. Der C-Teil von ring braucht für Fremdziele eigentlich eine Sysroot; für die
# reine Prüfung genügen Platzhalter-Header (assert.h, string.h, stdlib.h) und unter MSVC ein
# lib.exe-Ersatz – die so erzeugten Objekte werden nie gelinkt und nie ausgeliefert.
# CLIPPY_LENIENT wird aus .github/workflows/ci.yml gelesen (eine Quelle).
#
# Benötigt: bash ≥ 3.2, rustup-Ziel (`rustup target add <ziel>`), clang, ar.
# Ausgabe nach target/cross-clippy (stört den normalen Build-Cache nicht), Header nach $TMPDIR.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
die() { echo "cross-clippy: $*" >&2; exit 1; }

TARGET="${1:-}"
case "$TARGET" in
  -h | --help | "") sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; [ -n "$TARGET" ]; exit $? ;;
  *-pc-windows-msvc) FIDO=fido2-windows ;;
  *-unknown-linux-gnu) FIDO=fido2-hid ;;
  *) die "nur *-pc-windows-msvc und *-unknown-linux-gnu (Apple-/Android-Ziele: cargo check --target, siehe docs/entwicklung.md §9)" ;;
esac
command -v clang >/dev/null 2>&1 || die "clang fehlt"
rustup target list --installed | grep -qx "$TARGET" || die "Ziel fehlt: rustup target add $TARGET"

LENIENT="$(sed -n 's/^[[:space:]]*CLIPPY_LENIENT:[[:space:]]*"\(.*\)"[[:space:]]*$/\1/p' "$ROOT/.github/workflows/ci.yml")"
grep -q '^[[:space:]]*CLIPPY_LENIENT:' "$ROOT/.github/workflows/ci.yml" || die "CLIPPY_LENIENT nicht in ci.yml gefunden"

OUT="$ROOT/target/cross-clippy"
# Fester Pfad (sonst baut ring bei jedem Lauf neu, weil sich CFLAGS ändern) ohne Leerzeichen: cc-rs
# trennt CFLAGS an Leerraum, der Checkout liegt aber evtl. unter „PDF-Over Clone“.
SHIM="${TMPDIR:-/tmp}"
SHIM="${SHIM%/}/pdfover-cross-clippy-$(id -u)"
case "$SHIM" in *[[:space:]]*) die "Pfad für Platzhalter-Header enthält Leerraum: $SHIM (TMPDIR anpassen)" ;; esac
mkdir -p "$SHIM/include"
cat >"$SHIM/include/assert.h" <<'EOF'
/* Platzhalter nur für tools/cross-clippy.sh (Typprüfung, wird nie gelinkt). */
#define assert(x) ((void)0)
#define static_assert _Static_assert
EOF
cat >"$SHIM/include/string.h" <<'EOF'
/* Platzhalter nur für tools/cross-clippy.sh (Typprüfung, wird nie gelinkt). */
#include <stddef.h>
void *memcpy(void *, const void *, size_t);
void *memmove(void *, const void *, size_t);
void *memset(void *, int, size_t);
int memcmp(const void *, const void *, size_t);
EOF
cat >"$SHIM/include/stdlib.h" <<'EOF'
/* Platzhalter nur für tools/cross-clippy.sh (Typprüfung, wird nie gelinkt). */
#include <stddef.h>
void *malloc(size_t);
void free(void *);
int posix_memalign(void **, size_t, size_t);
EOF
cat >"$SHIM/lib-exe" <<'EOF'
#!/usr/bin/env bash
# lib.exe-Ersatz für tools/cross-clippy.sh: "-out:<datei> [-nologo] <objekte…>" → ar crs.
out=""; files=()
for a in "$@"; do case "$a" in -out:*) out="${a#-out:}" ;; -*) ;; *) files+=("$a") ;; esac; done
[ -n "$out" ] || { echo "lib-exe: -out fehlt" >&2; exit 2; }
exec ar crs "$out" ${files[@]+"${files[@]}"}
EOF
chmod +x "$SHIM/lib-exe"

var="$(echo "$TARGET" | tr '-' '_')"
export "CC_$var=clang" "CFLAGS_$var=-isystem $SHIM/include"
case "$TARGET" in
  *-msvc) export "AR_$var=$SHIM/lib-exe" ;;
  *) export "AR_$var=ar" ;;
esac
export CARGO_TARGET_DIR="$OUT"

excludes=()
for c in $LENIENT; do excludes+=(--exclude "$c"); done
cd "$ROOT"
echo "cross-clippy: $TARGET – strikt (ohne: ${LENIENT:-keine})" >&2
cargo clippy --locked --target "$TARGET" --workspace ${excludes[@]+"${excludes[@]}"} --no-deps --all-targets -- -D warnings
echo "cross-clippy: $TARGET – pdfover-backend-atrust --features $FIDO" >&2
cargo clippy --locked --target "$TARGET" -p pdfover-backend-atrust --features "$FIDO" --no-deps --all-targets -- -D warnings
echo "cross-clippy: $TARGET ok" >&2

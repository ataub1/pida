# Entwicklungsumgebung für PDF-Over NG: in jeder Shell `source tools/env.sh` ausführen.
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
PDFOVER_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-${(%):-%x}}")/.." && pwd)"
export PDFOVER_ROOT

# Hauptcheckout ermitteln: In einem `git worktree` (parallele Pakete, ADR 0001 §4) fehlen die
# nicht versionierten Ordner `reference/` und `.venv/`; sie werden aus dem Hauptcheckout genutzt.
_pdfover_common="$(git -C "$PDFOVER_ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)"
if [ -n "$_pdfover_common" ]; then
  PDFOVER_MAIN="$(dirname "$_pdfover_common")"
else
  PDFOVER_MAIN="$PDFOVER_ROOT"
fi
unset _pdfover_common
export PDFOVER_MAIN

# Referenzquellen (nur lesen): PDF-Over, PDF-AS 4, pdfas-config.
if [ -z "${PDFOVER_REFERENCE:-}" ]; then
  if [ -d "$PDFOVER_ROOT/reference" ]; then
    PDFOVER_REFERENCE="$PDFOVER_ROOT/reference"
  else
    PDFOVER_REFERENCE="$PDFOVER_MAIN/reference"
  fi
fi
export PDFOVER_REFERENCE

# Python-Umgebung mit pyHanko (Orakel für Tests/QA).
if [ -d "$PDFOVER_ROOT/.venv" ]; then
  export PATH="$PDFOVER_ROOT/.venv/bin:$PATH"
elif [ -d "$PDFOVER_MAIN/.venv" ]; then
  export PATH="$PDFOVER_MAIN/.venv/bin:$PATH"
fi

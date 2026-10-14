# Entwicklungsumgebung für PDF-Over NG: in jeder Shell `source tools/env.sh` ausführen.
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
PDFOVER_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-${(%):-%x}}")/.." && pwd)"
export PDFOVER_ROOT
if [ -d "$PDFOVER_ROOT/.venv" ]; then
  export PATH="$PDFOVER_ROOT/.venv/bin:$PATH"
fi

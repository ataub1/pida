#!/bin/sh
# PIDA Web – Entfernen einer Installation aus dem Archiv (install.sh).
#
#   sudo /opt/pida-web/uninstall.sh           # Dienst und Programm entfernen, /etc/pida-web bleibt
#   sudo /opt/pida-web/uninstall.sh --purge   # auch /etc/pida-web löschen
set -eu

PREFIX=/opt/pida-web
UNIT=/etc/systemd/system/pida-web.service
PURGE=0
for arg in "$@"; do
    case "$arg" in
        --purge) PURGE=1 ;;
        -h | --help) sed -n '2,6p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "uninstall.sh: unbekanntes Argument $arg" >&2; exit 2 ;;
    esac
done
[ "$(id -u)" -eq 0 ] || { echo "uninstall.sh: bitte als root ausführen (sudo)" >&2; exit 1; }

if [ -d /run/systemd/system ] && [ -e "$UNIT" ]; then
    systemctl disable --now pida-web.service >/dev/null 2>&1 || true
fi
rm -f "$UNIT"
[ -d /run/systemd/system ] && systemctl daemon-reload || true
# nur das eigene Verzeichnis (Schutz vor einem leeren oder falschen PREFIX)
case "$PREFIX" in /opt/pida-web) rm -rf "$PREFIX" ;; esac
[ "$PURGE" = 1 ] && rm -rf /etc/pida-web
echo "PIDA Web entfernt$([ "$PURGE" = 1 ] && echo ' (samt /etc/pida-web)' || echo '; Einstellungen in /etc/pida-web bleiben (--purge löscht sie)')."

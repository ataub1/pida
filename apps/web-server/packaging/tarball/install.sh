#!/bin/sh
# PIDA Web – Installation aus dem Archiv (für Distributionen ohne .deb/.rpm). Auch für Updates: einfach das
# Archiv der neuen Version entpacken und erneut aufrufen – überschreibt die Programmdateien und startet den Dienst
# neu; /etc/pida-web/pida-web.env bleibt erhalten.
#
#   sudo ./install.sh            # nach /opt/pida-web, Dienst pida-web aktivieren und starten
#   sudo ./install.sh --no-start # nur installieren
#
# Entfernen: sudo /opt/pida-web/uninstall.sh   (--purge löscht auch /etc/pida-web)
set -eu

PREFIX=/opt/pida-web
UNIT=/etc/systemd/system/pida-web.service
START=1
for arg in "$@"; do
    case "$arg" in
        --no-start) START=0 ;;
        -h | --help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "install.sh: unbekanntes Argument $arg" >&2; exit 2 ;;
    esac
done
[ "$(id -u)" -eq 0 ] || { echo "install.sh: bitte als root ausführen (sudo)" >&2; exit 1; }
HERE="$(cd "$(dirname "$0")" && pwd)"
[ -x "$HERE/bin/pida-web-server" ] || { echo "install.sh: bin/pida-web-server fehlt – Archiv vollständig entpackt?" >&2; exit 1; }

# neue Dateien daneben vorbereiten, dann austauschen (ein laufender Dienst sieht nie einen halben Stand)
NEW="$PREFIX.new.$$"
rm -rf "$NEW"
mkdir -p "$NEW"
cp -R "$HERE/bin" "$HERE/share" "$NEW/"
cp "$HERE/uninstall.sh" "$HERE/VERSION" "$HERE/LICENSE" "$HERE/THIRD-PARTY-NOTICES.md" "$HERE/RUST-STD-COPYRIGHT.html" \
    "$HERE/apache-pida-web.conf" "$HERE/LIESMICH.txt" "$NEW/"
chmod 755 "$NEW/bin/pida-web-server" "$NEW/uninstall.sh"
OLD="$PREFIX.old.$$"
[ -e "$PREFIX" ] && mv "$PREFIX" "$OLD"
mv "$NEW" "$PREFIX"
rm -rf "$OLD"

mkdir -p /etc/pida-web
[ -e /etc/pida-web/pida-web.env ] || cp "$HERE/pida-web.env" /etc/pida-web/pida-web.env
cp "$HERE/pida-web.service" "$UNIT"

if [ -d /run/systemd/system ]; then
    systemctl daemon-reload
    if [ "$START" = 1 ]; then
        systemctl enable pida-web.service >/dev/null
        systemctl restart pida-web.service
    fi
    echo "PIDA Web $(cat "$PREFIX/VERSION") installiert ($PREFIX); Dienst: systemctl status pida-web"
else
    echo "PIDA Web $(cat "$PREFIX/VERSION") installiert ($PREFIX); kein systemd – Start: $PREFIX/bin/pida-web-server --dir $PREFIX/share/pida-web"
fi
echo "Webserver davor einrichten: Vorlage $PREFIX/apache-pida-web.conf"

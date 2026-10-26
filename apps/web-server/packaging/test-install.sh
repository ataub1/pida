#!/usr/bin/env bash
# PIDA Web – Installationstest der Linux-Pakete (CI, in einem frischen Container als root, ohne systemd):
# installieren, Server starten und abfragen, Update auf eine neuere Version (geänderte /etc/pida-web/pida-web.env
# bleibt erhalten), Deinstallation (und purge). Bricht beim ersten Fehler ab.
#
#   test-install.sh deb  <alt.deb>    <neu.deb>       # debian:12 / ubuntu
#   test-install.sh rpm  <alt.rpm>    <neu.rpm>       # rockylinux:9 / fedora
#   test-install.sh tar  <alt.tar.gz> <neu.tar.gz>    # beliebige Distribution
set -euo pipefail

FORMAT="${1:?Format deb|rpm|tar}"
OLD="$(readlink -f "${2:?altes Paket}")"
NEW="$(readlink -f "${3:?neues Paket}")"
ENV_FILE=/etc/pida-web/pida-web.env

step() { echo; echo "=== $*"; }
fail() { echo "FEHLER: $*" >&2; exit 1; }

# curl für die Abfragen
if ! command -v curl >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1; then
    apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq curl ca-certificates >/dev/null
  elif command -v dnf >/dev/null 2>&1; then
    dnf install -y -q curl >/dev/null
  fi
fi

case "$FORMAT" in
  deb) BIN=/usr/bin/pida-web-server DATA=/usr/share/pida-web UNIT=/lib/systemd/system/pida-web.service ;;
  rpm) BIN=/usr/bin/pida-web-server DATA=/usr/share/pida-web UNIT=/usr/lib/systemd/system/pida-web.service ;;
  tar) BIN=/opt/pida-web/bin/pida-web-server DATA=/opt/pida-web/share/pida-web UNIT=/etc/systemd/system/pida-web.service ;;
  *) fail "unbekanntes Format $FORMAT" ;;
esac

install_pkg() {
  case "$FORMAT" in
    deb) DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "$1" >/dev/null ;;
    rpm) dnf install -y -q "$1" >/dev/null ;;
    tar)
      local d
      d="$(mktemp -d)"
      tar -xzf "$1" -C "$d"
      (cd "$d"/pida-web-* && ./install.sh)
      rm -rf "$d"
      ;;
  esac
}

# Server wie der Dienst starten (Adresse aus der Einstellungsdatei), Antworten prüfen, beenden
check_server() {
  local want_version="$1" bind
  bind="$(sed -n 's/^PIDA_WEB_BIND=//p' "$ENV_FILE")"
  "$BIN" --bind "$bind" --dir "$DATA" >/tmp/pida-web.log 2>&1 &
  local pid=$!
  for _ in $(seq 1 50); do curl -fs "http://$bind/api/config" >/dev/null 2>&1 && break; sleep 0.2; done
  curl -fs "http://$bind/api/config" | grep -q 'service.a-trust.at' || { cat /tmp/pida-web.log; fail "/api/config"; }
  curl -fs "http://$bind/" | grep -qi '<html' || fail "Startseite"
  curl -fsI "http://$bind/pkg/pida_web_bg.wasm" | grep -qi 'application/wasm' || fail "WebAssembly-Typ"
  curl -fs "http://$bind/notices/THIRD-PARTY-NOTICES.md" | grep -q . || fail "Lizenzhinweise in der Oberfläche"
  [ "$(curl -s --path-as-is -o /dev/null -w '%{http_code}' "http://$bind/../../etc/passwd")" != 200 ] || fail "Pfad außerhalb"
  kill "$pid"
  wait "$pid" 2>/dev/null || true
  "$BIN" --version | grep -qF "$want_version" || fail "Version: $("$BIN" --version), erwartet $want_version"
}

version_of() {
  case "$FORMAT" in
    deb) dpkg-deb -f "$1" Version ;;
    rpm) rpm -qp --qf '%{VERSION}' "$1" 2>/dev/null ;;
    tar) tar -xzf "$1" --wildcards -O '*/VERSION' ;;
  esac
}
V_OLD="$(version_of "$OLD")"
V_NEW="$(version_of "$NEW")"
# das Programm selbst meldet die Cargo-Version (ohne Testzusatz)
V_BIN="${V_OLD%%+*}"

step "Installation $FORMAT $V_OLD"
install_pkg "$OLD"
for f in "$BIN" "$DATA/index.html" "$DATA/pkg/pida_web_bg.wasm" "$DATA/pdfium/pdfium.wasm" "$UNIT" "$ENV_FILE"; do
  [ -e "$f" ] || fail "$f fehlt"
done
[ ! -e "$DATA/samples" ] || fail "Testdaten im Paket"
grep -q "ExecStart=$BIN --bind" "$UNIT" || fail "Unit zeigt nicht auf $BIN"
grep -q -- "--dir $DATA" "$UNIT" || fail "Unit zeigt nicht auf $DATA"
case "$FORMAT" in
  deb) dpkg -s pida-web | grep -q '^Status: install ok installed' || fail "dpkg-Status" ;;
  rpm) rpm -q pida-web >/dev/null || fail "rpm -q" ;;
esac
check_server "$V_BIN"

step "Update $V_OLD → $V_NEW (eigene Einstellung bleibt)"
sed -i 's/^PIDA_WEB_BIND=.*/PIDA_WEB_BIND=127.0.0.1:1443/' "$ENV_FILE"
install_pkg "$NEW"
grep -q '^PIDA_WEB_BIND=127.0.0.1:1443$' "$ENV_FILE" || fail "Einstellung beim Update überschrieben"
case "$FORMAT" in
  deb) [ "$(dpkg-query -W -f '${Version}' pida-web)" = "$V_NEW" ] || fail "Version nach Update" ;;
  rpm) [ "$(rpm -q --qf '%{VERSION}' pida-web)" = "$V_NEW" ] || fail "Version nach Update"; [ "$(rpm -q pida-web | wc -l)" = 1 ] || fail "zwei Versionen installiert" ;;
  tar) [ "$(cat /opt/pida-web/VERSION)" = "$V_NEW" ] || fail "Version nach Update"; for d in /opt/pida-web.*; do [ ! -e "$d" ] || fail "Reste des Austauschs: $d"; done ;;
esac
check_server "$V_BIN"

step "Deinstallation"
case "$FORMAT" in
  deb)
    apt-get remove -y -qq pida-web >/dev/null
    [ ! -e "$BIN" ] && [ ! -e "$DATA" ] && [ ! -e "$UNIT" ] || fail "Dateien nach remove"
    [ -e "$ENV_FILE" ] || fail "Conffile nach remove gelöscht (erst purge)"
    apt-get purge -y -qq pida-web >/dev/null
    [ ! -e /etc/pida-web ] || fail "/etc/pida-web nach purge"
    ! dpkg -s pida-web >/dev/null 2>&1 || fail "Paket noch registriert"
    ;;
  rpm)
    dnf remove -y -q pida-web >/dev/null
    [ ! -e "$BIN" ] && [ ! -e "$DATA" ] && [ ! -e "$UNIT" ] || fail "Dateien nach remove"
    ! rpm -q pida-web >/dev/null 2>&1 || fail "Paket noch registriert"
    # geänderte Conffiles hebt rpm als .rpmsave auf
    rm -f "$ENV_FILE.rpmsave"
    ;;
  tar)
    /opt/pida-web/uninstall.sh
    [ ! -e /opt/pida-web ] && [ ! -e "$UNIT" ] || fail "Dateien nach uninstall"
    [ -e "$ENV_FILE" ] || fail "Einstellungen ohne --purge gelöscht"
    install_pkg "$NEW"
    /opt/pida-web/uninstall.sh --purge
    [ ! -e /opt/pida-web ] && [ ! -e /etc/pida-web ] || fail "Reste nach uninstall --purge"
    ;;
esac

echo
echo "OK: $FORMAT – Installation, Update ($V_OLD → $V_NEW), Deinstallation"

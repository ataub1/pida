#!/usr/bin/env python3
"""PIDA – App Bundle in Google Play hochladen (Track „internal“), docs/integration/mobile-android.md §8.

Nur Python-Standardbibliothek und `openssl` (RS256-Signatur des Dienstkonto-JWT), keine Google-Bibliotheken.

    tools/play-upload.py --check                      # Zugang prüfen (Bearbeitung anlegen, Tracks lesen, verwerfen)
    tools/play-upload.py dist/release-0.5.4/PIDA_0.5.4_android.aab \\
        --notes-de "Was ist neu …" [--notes-en "What's new …"] [--status draft]

Schlüssel des Dienstkontos: `PIDA_PLAY_SERVICE_ACCOUNT`, sonst `~/.pida-release/play-service-account.json`.
Das Skript gibt weder den Schlüssel noch das Zugriffstoken aus. Es schreibt nur in den Track „internal“;
geschlossene, offene und Produktions-Releases bleiben der Play Console vorbehalten.
"""

import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

PACKAGE = "at.asit.pida.app"
TRACK = "internal"
SCOPE = "https://www.googleapis.com/auth/androidpublisher"
TOKEN_URL = "https://oauth2.googleapis.com/token"
API = f"https://androidpublisher.googleapis.com/androidpublisher/v3/applications/{PACKAGE}"
UPLOAD = f"https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/{PACKAGE}"
DEFAULT_KEY = os.path.expanduser("~/.pida-release/play-service-account.json")
NOTES_MAX = 500  # Zeichen je Sprache (Play)


class PlayError(Exception):
    pass


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def sign_rs256(message: bytes, private_key_pem: str) -> bytes:
    # Schlüssel nur kurz in einer Datei mit Rechten 0600 im privaten Temp-Ordner (openssl liest ihn von dort)
    with tempfile.TemporaryDirectory() as tmp:
        key_path = os.path.join(tmp, "key.pem")
        fd = os.open(key_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as f:
            f.write(private_key_pem)
        result = subprocess.run(
            ["openssl", "dgst", "-sha256", "-sign", key_path], input=message, capture_output=True, check=False
        )
    if result.returncode != 0:
        raise PlayError("openssl konnte das Anmelde-Token nicht signieren (Schlüsseldatei prüfen)")
    return result.stdout


def access_token(key_file: str) -> tuple[str, str]:
    try:
        with open(key_file, encoding="utf-8") as f:
            sa = json.load(f)
    except (OSError, ValueError) as e:
        raise PlayError(f"Schlüsseldatei nicht lesbar: {key_file} ({type(e).__name__})") from None
    if sa.get("type") != "service_account" or "private_key" not in sa or "client_email" not in sa:
        raise PlayError(f"{key_file} ist kein JSON-Schlüssel eines Dienstkontos")
    now = int(time.time())
    header = b64url(json.dumps({"alg": "RS256", "typ": "JWT"}).encode())
    claims = b64url(
        json.dumps({"iss": sa["client_email"], "scope": SCOPE, "aud": TOKEN_URL, "iat": now, "exp": now + 600}).encode()
    )
    unsigned = f"{header}.{claims}".encode()
    jwt = f"{header}.{claims}.{b64url(sign_rs256(unsigned, sa['private_key']))}"
    body = urllib.parse.urlencode(
        {"grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer", "assertion": jwt}
    ).encode()
    reply = request("POST", TOKEN_URL, body=body, content_type="application/x-www-form-urlencoded")
    return reply["access_token"], sa["client_email"]


def request(method, url, token=None, body=None, content_type="application/json"):
    req = urllib.request.Request(url, data=body, method=method)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    if body is not None:
        req.add_header("Content-Type", content_type)
    try:
        with urllib.request.urlopen(req, timeout=600) as resp:
            data = resp.read()
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")
        try:
            detail = json.loads(detail).get("error", {}).get("message", detail) or detail
        except ValueError:
            pass
        raise PlayError(f"{method} {url.split('?')[0]}: HTTP {e.code}: {detail}") from None
    except urllib.error.URLError as e:
        raise PlayError(f"{method} {url.split('?')[0]}: {e.reason}") from None
    return json.loads(data) if data else {}


def api_json(method, path, token, payload=None):
    body = json.dumps(payload).encode() if payload is not None else None
    return request(method, f"{API}{path}", token, body)


def release_notes(de, en):
    notes = []
    for lang, text in (("de-DE", de), ("en-US", en)):
        if text:
            text = text.strip()
            if len(text) > NOTES_MAX:
                raise PlayError(f"Versionshinweise {lang}: {len(text)} Zeichen, Play erlaubt {NOTES_MAX}")
            notes.append({"language": lang, "text": text})
    return notes


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("bundle", nargs="?", help="signiertes App Bundle (.aab)")
    ap.add_argument("--check", action="store_true", help="nur Zugang prüfen, nichts ändern")
    ap.add_argument("--notes-de", help="Versionshinweise Deutsch (≤ 500 Zeichen)")
    ap.add_argument("--notes-en", help="Versionshinweise Englisch (nur mit englischem Store-Eintrag)")
    ap.add_argument("--name", help="Release-Name (Vorgabe: „<versionCode> (<versionName>)“ wie in der Console)")
    ap.add_argument(
        "--status",
        choices=["completed", "draft"],
        default="completed",
        help="completed = an die Tester ausrollen (Vorgabe); draft = nur als Entwurf anlegen",
    )
    args = ap.parse_args()
    if not args.check and not args.bundle:
        ap.error("App Bundle angeben oder --check")
    if args.bundle and not args.bundle.endswith(".aab"):
        ap.error("Google Play nimmt für neue Apps nur App Bundles (.aab)")

    key_file = os.environ.get("PIDA_PLAY_SERVICE_ACCOUNT") or DEFAULT_KEY
    token, email = access_token(key_file)
    edit = api_json("POST", "/edits", token, {})["id"]
    committed = False
    try:
        if args.check:
            tracks = api_json("GET", f"/edits/{edit}/tracks", token).get("tracks", [])
            print(f"Zugang ok: {email} → {PACKAGE}")
            for t in tracks:
                for r in t.get("releases", []):
                    print(f"  {t['track']}: {r.get('name', '?')} [{r.get('status')}] versionCodes {r.get('versionCodes')}")
            return
        notes = release_notes(args.notes_de, args.notes_en)
        size = os.path.getsize(args.bundle)
        print(f"Lade {os.path.basename(args.bundle)} hoch ({size / 1e6:.1f} MB) …")
        with open(args.bundle, "rb") as f:
            bundle = request(
                "POST",
                f"{UPLOAD}/edits/{edit}/bundles?uploadType=media",
                token,
                f.read(),
                "application/octet-stream",
            )
        code = str(bundle["versionCode"])
        name = args.name or f"{code} ({version_name(args.bundle) or '?'})"
        release = {"name": name, "versionCodes": [code], "status": args.status}
        if notes:
            release["releaseNotes"] = notes
        api_json("PUT", f"/edits/{edit}/tracks/{TRACK}", token, {"track": TRACK, "releases": [release]})
        api_json("POST", f"/edits/{edit}:commit", token)
        committed = True
        what = "an die internen Tester ausgerollt" if args.status == "completed" else "als Entwurf angelegt"
        print(f"Release „{name}“ (versionCode {code}) im Track {TRACK} {what}.")
    finally:
        if not committed:
            try:
                api_json("DELETE", f"/edits/{edit}", token)
            except PlayError:
                pass


def version_name(bundle_path):
    # „PIDA_0.5.3_android.aab“ → „0.5.3“ (Namensschema aus dem Build)
    parts = os.path.basename(bundle_path).split("_")
    return parts[1] if len(parts) >= 3 and parts[0] == "PIDA" else None


if __name__ == "__main__":
    try:
        main()
    except PlayError as e:
        print(f"play-upload: {e}", file=sys.stderr)
        sys.exit(1)

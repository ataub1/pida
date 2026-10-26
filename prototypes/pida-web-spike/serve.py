#!/usr/bin/env python3
"""Nur für die Prüfung des Durchstichs: liefert web/ aus und speichert per PUT /pruefung/<name> hochgeladene Dateien
(das im Browser signierte PDF) unter target/pruefung/, damit sie mit qpdf, openssl und pyHanko geprüft werden können.
Bindet nur an 127.0.0.1; nicht für den Betrieb."""
import os, sys
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from functools import partial

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "target", "pruefung")

class Handler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

    def do_PUT(self):
        name = os.path.basename(self.path)
        if not self.path.startswith("/pruefung/") or not name.endswith(".pdf"):
            self.send_error(404)
            return
        n = int(self.headers.get("Content-Length", "0"))
        if n > 64 << 20:
            self.send_error(413)
            return
        os.makedirs(OUT, exist_ok=True)
        with open(os.path.join(OUT, name), "wb") as f:
            f.write(self.rfile.read(n))
        self.send_response(204)
        self.end_headers()

port = int(sys.argv[1]) if len(sys.argv) > 1 else 1441
ThreadingHTTPServer(("127.0.0.1", port), partial(Handler, directory=os.path.join(HERE, "web"))).serve_forever()

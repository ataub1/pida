#!/usr/bin/env python3
"""PDF-Over NG – Test-Orakel (pades, Welle 2): pyHanko-Validierung als JSON, mehrere Dateien.

Aufruf:  python3 -I pyhanko_report.py --trust <pem> [--trust <pem> ...] <datei.pdf> [...]

Ausgabe (eine Zeile JSON): {"<datei>": [ {field, intact, valid, trusted, coverage,
modification_level, docmdp_ok, error}, ... ], ...}

- Keine Netzzugriffe: ValidationContext mit allow_fetching=False, Revocation „soft-fail“.
- Leniente Syntax (strict=False) wie `pyhanko sign validate --no-strict-syntax` (Hybrid-xref).
- Verschlüsselte Dateien (Standard-Handler ohne Benutzerpasswort) werden mit "" geöffnet.
- Die Ausgabe enthält keine Zertifikatsinhalte (nur Feldnamen und Status); Feldnamen der
  Testdokumente sind synthetisch.

Wird von `crates/pades/tests/*` aufgerufen; fehlt pyHanko, überspringen die Tests die Prüfung.
"""

import json
import sys


def validate(path, trust):
    from pyhanko.pdf_utils.reader import PdfFileReader
    from pyhanko.sign.validation import validate_pdf_signature
    from pyhanko_certvalidator import ValidationContext

    results = []
    with open(path, "rb") as f:
        reader = PdfFileReader(f, strict=False)
        if reader.encrypted:
            reader.decrypt(b"")
        for sig in reader.embedded_signatures:
            entry = {"field": sig.field_name}
            try:
                vc = ValidationContext(
                    trust_roots=trust, allow_fetching=False, revocation_mode="soft-fail"
                )
                st = validate_pdf_signature(sig, signer_validation_context=vc)
                level = st.modification_level
                entry.update(
                    intact=bool(st.intact),
                    valid=bool(st.valid),
                    trusted=bool(st.trusted),
                    coverage=st.coverage.name if st.coverage is not None else None,
                    modification_level=level.name if level is not None else None,
                    docmdp_ok=st.docmdp_ok,
                    error=None,
                )
            except Exception as exc:  # Validierungsfehler als Befund melden
                entry.update(
                    intact=False,
                    valid=False,
                    trusted=False,
                    coverage=None,
                    modification_level=None,
                    docmdp_ok=None,
                    error=type(exc).__name__,
                )
            results.append(entry)
    return results


def main(argv):
    try:
        from pyhanko.keys import load_cert_from_pemder
    except ImportError as exc:  # pragma: no cover - Umgebung
        print(json.dumps({"skip": "pyhanko fehlt: %s" % exc}))
        return 3
    trust_paths = []
    files = []
    args = iter(argv[1:])
    for a in args:
        if a == "--trust":
            trust_paths.append(next(args))
        else:
            files.append(a)
    trust = [load_cert_from_pemder(p) for p in trust_paths]
    out = {}
    for path in files:
        try:
            out[path] = validate(path, trust)
        except Exception as exc:  # Datei nicht lesbar
            out[path] = [{"field": None, "intact": False, "valid": False, "trusted": False,
                          "coverage": None, "modification_level": None, "docmdp_ok": None,
                          "error": "read:" + type(exc).__name__}]
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

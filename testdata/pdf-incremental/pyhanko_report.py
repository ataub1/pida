#!/usr/bin/env python3
"""PIDA – Test-Orakel (Paket A): pyHanko-Validierung als JSON.

Aufruf:  python3 -I pyhanko_report.py <datei.pdf> [<vertrauensanker.pem> ...]

Gibt je eingebetteter Signatur ein JSON-Objekt aus (eine Zeile, Liste):
  field, intact, valid, trusted, coverage, modification_level, docmdp_ok, error

- Keine Netzzugriffe: ValidationContext mit allow_fetching=False, Revocation „soft-fail“.
- Leniente Syntax (strict=False) wie `pyhanko sign validate --no-strict-syntax` – nötig für
  Hybrid-Querverweise.
- Die Ausgabe enthält keine Zertifikatsinhalte (nur Feldnamen und Status).

Wird von `crates/pdf-incremental/tests/*` aufgerufen; fehlt pyHanko, überspringen die Tests.
"""

import json
import sys


def main(argv):
    if len(argv) < 2:
        print("Aufruf: pyhanko_report.py <datei.pdf> [trust.pem ...]", file=sys.stderr)
        return 2
    try:
        from pyhanko.pdf_utils.reader import PdfFileReader
        from pyhanko.sign.validation import validate_pdf_signature
        from pyhanko_certvalidator import ValidationContext
        from pyhanko.keys import load_cert_from_pemder
    except ImportError as exc:  # pragma: no cover - Umgebung
        print(json.dumps({"skip": "pyhanko fehlt: %s" % exc}))
        return 3

    trust = [load_cert_from_pemder(p) for p in argv[2:]]
    results = []
    with open(argv[1], "rb") as f:
        reader = PdfFileReader(f, strict=False)
        if reader.encrypted:
            # Standard-Handler ohne Benutzerpasswort (K3)
            reader.decrypt(b"")
        for sig in reader.embedded_signatures:
            entry = {"field": sig.field_name}
            try:
                vc = ValidationContext(
                    trust_roots=trust,
                    allow_fetching=False,
                    revocation_mode="soft-fail",
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
    print(json.dumps(results))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

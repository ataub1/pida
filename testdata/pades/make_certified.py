#!/usr/bin/env python3
"""PIDA – Testdaten (pades, Review Welle 2): zertifiziertes PDF (DocMDP) mit pyHanko.

Aufruf:  python3 -I make_certified.py <eingabe.pdf> <test.p12> <ausgabe.pdf> <P>

Legt ein leeres Signaturfeld „Leer“ (Seite 1, [300 100 530 150]) an und zertifiziert das
Dokument mit DocMDP P=1/2/3 (Feld „Zertifizierung“). Nur mit den Testschlüsseln aus
`testdata/keystores` (Passwort `test`, NUR TEST) verwenden; kein Netzverkehr.

Erzeugt `certified-docmdp-p2.pdf` und `certified-docmdp-p3.pdf` aus
`testdata/pdf/pdfas/pdf-as-legacy/simple.pdf` mit `testdata/keystores/test-ecp256.p12`.
"""

import sys

from pyhanko.pdf_utils.incremental_writer import IncrementalPdfFileWriter
from pyhanko.sign import fields, signers
from pyhanko.sign.fields import MDPPerm, SigFieldSpec


def main(argv):
    src, p12, out, perm = argv[1], argv[2], argv[3], int(argv[4])
    signer = signers.SimpleSigner.load_pkcs12(p12, passphrase=b"test")
    perms = {1: MDPPerm.NO_CHANGES, 2: MDPPerm.FILL_FORMS, 3: MDPPerm.ANNOTATE}[perm]
    with open(src, "rb") as f:
        w = IncrementalPdfFileWriter(f)
        fields.append_signature_field(w, SigFieldSpec(sig_field_name="Leer", on_page=0, box=(300, 100, 530, 150)))
        meta = signers.PdfSignatureMetadata(field_name="Zertifizierung", certify=True, docmdp_permissions=perms)
        with open(out, "wb") as o:
            signers.sign_pdf(w, meta, signer=signer, output=o)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

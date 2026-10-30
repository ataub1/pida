# PDF signatures: technology and verification

Translation of content/de/04-pdf-signaturen.md.

## How is a signature stored in a PDF? {#wie-steckt-eine-signatur-in-einem-pdf}

- The signature is appended to the PDF as an **incremental update**; the original content is preserved byte for byte.
  The signature covers the whole document except the gap in which it is stored itself (ByteRange).
- The format follows the European standard **PAdES** (PDF Advanced Electronic Signatures, ETSI EN 319 142);
  internally it contains a CMS signature (CAdES).
- **Several signatures:** each further signature is appended again; earlier signatures remain valid as long as only
  permitted changes are made afterwards.
- **Visible or invisible:** the signature block is only the visual representation; the actual signature is in the
  data. An invisible signature is just as valid.
- **Signature fields:** forms can contain empty signature fields; PIDA can sign into them.
- **Certified documents:** a document can be “certified” by its author and restrict changes (up to “no changes
  allowed”). Signing is then only partly possible or not possible at all.
- **Protected documents:** PIDA cannot sign a PDF that requires a password to open.

## Signature levels (PAdES baseline) {#signaturniveaus-pades-baseline}

- **B-B:** basic signature.
- **B-T:** additionally a timestamp from a timestamping service.
- **B-LT / B-LTA:** additionally validation data (certificates, revocation information) or archive timestamps for
  long-term validation.
- PIDA signs (like PDF-AS) at level B-B.

## Verifying a signature {#signatur-prufen}

- **Self-check in PIDA:** after signing, PIDA checks that the document is unchanged and the signature is
  cryptographically intact. PIDA does not check the chain of trust or revocation status.
- **Full verification:** www.signaturpruefung.gv.at (signature verification service of RTR) checks the signature,
  certificate, chain of trust and revocation status.
- **Adobe Acrobat/Reader shows “Validity unknown” or a warning:** often Adobe does not trust the certificate issuer
  because the European trusted lists (EUTL) are not enabled in its settings. This does not make the signature itself
  invalid; what counts is a verification e.g. at signaturpruefung.gv.at.
- **“Document has been modified after signing”:** any change after signing (even filling in a form or adding a
  comment in an area not intended for it) can invalidate a signature. Finish editing first, then sign.

## PDF/A {#pdf-a}

PDF/A is an ISO standard for the long-term archiving of PDF documents. PIDA detects PDF/A documents and signs them so
that they remain PDF/A-compliant.

## Preparing a document for others (QR placeholder) {#dokument-fur-andere-vorbereiten-qr-platzhalter}

If you create a document that others will sign, a **QR placeholder** determines where the signature goes. PIDA,
PDF-Over and PDF-AS (including the accessible signing page PDF-AS-WAI) recognise it and put the signature block in
its place.

1. Choose **“Create QR placeholder …”** – on the start page below the drop area or in the command list.
2. Set a **number**. For several signatures, each placeholder gets its own number, e.g. 0001 and 0002. Tell the
   signers their number.
3. Choose the **signature block** and **language**. “As set by the signer” uses their own signature block.
4. Save as **PNG** or **SVG** and insert it in your word processor at the desired place, as wide as the signature
   block should be. It is saved 8.5 cm wide, which fits the standard signature block.
5. Save the document as PDF.

When signing, PIDA finds the placeholder if the placeholder search for QR code placeholders is switched on under
Settings › Document. Without a placeholder, millimetre values also work: give the signers the page and the distance
of the top left corner from the left and from the bottom (“Signature placement” › “Position in millimetres”).

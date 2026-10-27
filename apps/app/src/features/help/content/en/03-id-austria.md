# ID Austria and A-Trust

Translation of content/de/03-id-austria.md. Current details are available at www.oesterreich.gv.at and
www.id-austria.gv.at.

## What is ID Austria? {#was-ist-id-austria}

ID Austria is the electronic proof of identity of the Republic of Austria and the successor of the mobile phone
signature (Handy-Signatur) and the citizen card (Bürgerkarte). It lets you identify yourself online and create
qualified electronic signatures. The signature is created by the qualified trust service provider A-Trust (remote
signing: the key is stored securely at A-Trust).

## How do I sign with ID Austria in PIDA? {#wie-signiert-man-mit-id-austria-in-pida}

1. In PIDA, choose “Sign with ID Austria”.
2. Log in with your user name or phone number and your signature password.
3. Confirm: in the “ID Austria” app on your smartphone (first compare the reference value in PIDA with the one in the
   app), alternatively by SMS TAN or with a FIDO2 security key (on the desktop).
4. A-Trust creates the signature; PIDA embeds it in the PDF and checks it.

The **reference value** is a short string that PIDA and the app show at the same time. If they do not match, decline
the request in the app – it then does not come from your own signing process.

## Frequently asked questions {#haufige-fragen}

- **Forgotten password or ID Austria blocked:** this is handled not by PIDA but by ID Austria or A-Trust (see
  www.oesterreich.gv.at). PIDA keeps the signature password at most until the app is closed.
- **No notification in the app:** open the “ID Austria” app and check your internet connection; the request expires
  after a few minutes, then choose “Try again” in PIDA.
- **What does A-Trust see?** For signing, the PDF document is sent to A-Trust (see A-Trust's privacy policy).
- **Several documents:** in a batch, one login is enough; each document is confirmed individually.
- **Users from other countries:** ID Austria requires registration in Austria; citizens of other EU countries use
  their own country's signature solution (QES are recognised throughout the EU).

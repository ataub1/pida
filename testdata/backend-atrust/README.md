# Testdaten `pdfover-backend-atrust` (Paket D)

> **NUR FÜR TESTS.** Synthetische, anonymisierte Seiten – **kein** Original-Markup von A-Trust, keine
> Mitschnitte, keine Personendaten, keine echten Zugangsdaten.

Die Seiten sind ausschließlich aus den Selektoren und URL-Mustern in
[docs/analyse/02 §3.5/§8](../../docs/analyse/02-signatur-backends-protokolle.md) konstruiert
(`#handynummer`, `#signaturpasswort`, `#Button_Identification`, `#input_tan`, `#vergleichswert`,
`#qrimage`, `#jsLongPoll script` mit `qrpoll("URL", "TOKEN");`, `#smartphoneAnimation`,
`#biometricimage`, `.ida-signtype #SignType`, `#fidoBlock`, `#credentialOptions`, `#credentialResult`,
`#FidoContinue`, `#SmsButton`, `#FidoButton`, `#LinkList a[href*="ShowSigobj.aspx"]`, `#Label1`,
`#LabelDetail`, `#Label2`, `#NextBtn`, `#Button_Next`, `#Button_Back`, `#Button_Cancel`).

Platzhalter `{{NAME}}` ersetzt der Mock-Server der Tests (`crates/backend-atrust/tests/support`):

| Datei | Block | Platzhalter | Besonderheiten |
|---|---|---|---|
| `login.html` | UsernamePassword | `SID` | Hidden-Felder (`__VIEWSTATE`), Checkbox ohne `checked`, `select`/`textarea` (nicht gesendet), Input ohne Namen, drei Submit-Buttons (einer `type="SUBMIT"`), Signaturdaten-Link |
| `usernameinfo.html` | AutoSkip (`/UsernameInfo.aspx`, `/tanAppInfo.aspx`) | `ACTION` | `NextBtn` und zusätzlicher Cancel-Button |
| `expiresinfo.html` | Interstitial (`/ExpiresInfo.aspx`) | `SID` | `#Label2` mit verschachteltem `<b>` und `<br>` (jsoup-`ownText`-Semantik) |
| `sms.html` | SMS-TAN | `SID`, `REF` | `#FidoButton`, `#LinkList` mit zwei Links |
| `qr.html` | QR-Code | `SID`, `REF`, `POLLID`, `TOKEN` | relativer Poll-Pfad `../poll/…`, `#SmsButton`, `#FidoButton` |
| `app.html` | App-Warten | `SID`, `REF`, `POLLID`, `TOKEN` | absoluter Poll-Pfad |
| `app-biometric.html` | App-Warten (`#biometricimage`) | – | Vergleichswert und Poll-Skript (beide Pflicht wie in PDF-Over; ohne sie ist die Seite unbekannt) |
| `app-signtype.html` | App-Warten (`.ida-signtype #SignType`) | – | – |
| `fido2.html` | FIDO2 | `SID`, `OPTIONS` (HTML-escaped JSON) | `#SmsButton` |
| `error.html` | Error (`/error*.aspx`, `/SessionClosed.aspx`) | `ACTION`, `BUTTONS` | `#Label1` + `#LabelDetail`; Buttons je Test |
| `meta-refresh.html` | – | `TARGET` | `<meta http-equiv="Refresh">` |
| `unknown.html`, `two-forms.html`, `two-blocks.html` | Fehlerfälle | – | unbekannte Seite, zwei Formulare, zwei Blöcke |

Das Testzertifikat in den SL-Antworten ist `testdata/keystores/test-rsa2048.crt.der`.

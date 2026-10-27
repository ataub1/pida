# ID Austria und A-Trust

ENTWURF für den Machbarkeitstest – fachlich zu prüfen. Aktuelle Details stehen auf www.oesterreich.gv.at bzw.
www.id-austria.gv.at.

## Was ist ID Austria?

ID Austria ist der elektronische Identitätsnachweis der Republik Österreich und Nachfolger der Handy-Signatur und der
Bürgerkarte. Damit kann man sich online ausweisen und qualifiziert elektronisch signieren. Die Signatur erstellt der
qualifizierte Vertrauensdiensteanbieter A-Trust (Fernsignatur: der Schlüssel liegt geschützt bei A-Trust).

## Wie signiert man mit ID Austria in PIDA?

1. In PIDA „Signieren mit ID Austria“ wählen.
2. Anmelden mit Benutzername bzw. Telefonnummer und Signatur-Passwort.
3. Bestätigen: in der App „ID Austria“ am Smartphone (vorher den Vergleichswert in PIDA mit dem in der App
   vergleichen), alternativ per SMS-TAN oder mit einem FIDO2-Sicherheitsschlüssel (am Desktop).
4. A-Trust erstellt die Signatur; PIDA bettet sie ins PDF ein und prüft sie.

Der **Vergleichswert** ist eine kurze Zeichenfolge, die PIDA und die App gleichzeitig anzeigen. Stimmen sie nicht
überein, die Anfrage in der App ablehnen – sie stammt dann nicht vom eigenen Signiervorgang.

## Häufige Fragen

- **Passwort vergessen oder ID Austria gesperrt:** Das regelt nicht PIDA, sondern ID Austria bzw. A-Trust (Hinweise
  auf www.oesterreich.gv.at). PIDA speichert das Signatur-Passwort höchstens bis zum Beenden der App.
- **Keine Benachrichtigung in der App:** App „ID Austria“ öffnen, Internetverbindung prüfen; die Anfrage läuft nach
  einigen Minuten ab, dann „Erneut versuchen“ in PIDA.
- **Was sieht A-Trust?** Für die Signatur wird das PDF-Dokument an A-Trust übermittelt (siehe Datenschutzerklärung von
  A-Trust).
- **Mehrere Dokumente:** Im Stapel genügt eine Anmeldung; jedes Dokument wird einzeln bestätigt.
- **Ausländische Nutzerinnen und Nutzer:** ID Austria setzt eine Registrierung in Österreich voraus; andere
  EU-Bürgerinnen und -Bürger nutzen die Signaturlösung ihres Landes (QES werden EU-weit anerkannt).

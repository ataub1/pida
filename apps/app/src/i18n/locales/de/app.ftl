### PDF-Over NG – Texte der Oberfläche, deutsch (EUPL-1.2)
###
### Eigene IDs der Oberfläche (`ng-ui-*`), ergänzend zu crates/i18n/locales/de/pdfover.ftl und
### pdfover-ng.ftl. Die Integration übernimmt sie nach pdfover-ng.ftl (ADR 0001 Regel 11); jede ID hat
### einen Eintrag in de/ und en/ mit denselben Variablen (geprüft in src/i18n/i18n.test.ts).

## Allgemein

ng-ui-yes = Ja
    .accesskey = J
ng-ui-no = Nein
    .accesskey = N
ng-ui-close = Schließen
ng-ui-continue = Weiter
    .accesskey = W
ng-ui-skip-to-content = Zum Inhalt springen
ng-ui-link-external = (öffnet im Browser)
ng-ui-page-n = Seite { $page }
ng-ui-page-previous = Vorherige Seite
ng-ui-page-next = Nächste Seite
ng-ui-unit = Einheit
ng-ui-starting = PDF-Over NG wird gestartet
ng-ui-finished = PDF-Over NG wurde beendet.
ng-ui-finished-code = Beendet mit Code { $code }.
ng-ui-mainbar-label = Arbeitsschritte
ng-ui-step-done = erledigt
ng-ui-lang-system = Systemsprache
ng-ui-lang-de = Deutsch
ng-ui-lang-en = Englisch
ng-ui-drop-not-here = Dokumente können im Schritt „Öffnen“ oder auf dem Ergebnis abgelegt werden.

## Fehler der Oberfläche bzw. der Shell

ng-ui-core-unavailable = Der Signaturkern konnte nicht gestartet werden. Bitte versuchen Sie es erneut; bleibt der Fehler bestehen, prüfen Sie die Protokolle (Einstellungen › Über).
ng-ui-event-invalid = Diese Aktion wird von dieser Version des Signaturkerns noch nicht unterstützt.
ng-ui-bridge-error = Die Verbindung zum Signaturkern ist gestört.
ng-ui-error-unknown = Unerwarteter Fehler ({ $code }).
ng-ui-open-failed = Das Öffnen ist fehlgeschlagen.
ng-ui-dialog-failed = Der Dateidialog konnte nicht geöffnet werden.

## Öffnen

ng-ui-open-title = Dokument öffnen
ng-ui-open-analyzing = Dokument wird geprüft …
ng-ui-dropzone-label = Ablagefläche für PDF-Dokumente
ng-ui-dropzone-hint = Mehrere Dokumente werden nacheinander signiert. Tastatur: Strg+O bzw. ⌘O öffnet den Dateidialog.
ng-ui-dropzone-release = Loslassen, um die Dokumente zu öffnen
ng-ui-queue-title = Warteschlange ({ $count })
ng-ui-queue-current = aktuell

## Platzhalter

ng-ui-placeholder-title = Signaturplatzhalter gefunden
ng-ui-placeholder-qr = QR-Code-Platzhalter auf Seite { $page }
ng-ui-placeholder-field = Signaturfeld „{ $name }“
ng-ui-placeholder-kind = Art der Platzhalter

## Positionieren

ng-ui-position-title = Signatur positionieren
ng-ui-position-help = Ziehen Sie den Signaturblock an die gewünschte Stelle oder verwenden Sie die Pfeiltasten (Umschalt: 1 pt, Strg/⌘: 50 pt), Bild auf/ab für die Seite und Eingabe zum Signieren.
ng-ui-position-block-role = verschiebbarer Signaturblock
ng-ui-position-announce = Seite { $page } von { $pages }, { $left } mm von links, { $bottom } mm von unten
ng-ui-position-pages = Seiten
ng-ui-position-page-alt = Seite { $page } von { $pages } – { $name }
ng-ui-position-new-page-alt = Neue leere Seite
ng-ui-position-preview-failed = Die Seitenvorschau ist nicht verfügbar.
ng-ui-position-coordinates = Position
ng-ui-position-x = X von links ({ $unit })
ng-ui-position-y = Y Oberkante von unten ({ $unit })
ng-ui-position-page-field = Seite
ng-ui-position-auto = Automatisch
    .accesskey = A
ng-ui-position-auto-done = Der Signaturblock wurde automatisch platziert.
ng-ui-position-invisible = Unsichtbar signieren
    .accesskey = U
ng-ui-position-signed-hint = Das Dokument ist bereits signiert; eine neue Seite kann nicht angefügt werden.

## Signiermittel

ng-ui-backend-title = Signiermittel wählen
ng-ui-backend-hint = Ein Standard-Signiermittel lässt sich in den Einstellungen (Erweitert) festlegen.
ng-ui-backend-atrust-desc = Signieren mit ID Austria: Bestätigung in der App, per SMS-TAN oder mit FIDO2-Sicherheitsschlüssel.
ng-ui-backend-local-desc = Signieren mit der Bürgerkarte über eine lokal installierte Bürgerkartenumgebung.
ng-ui-backend-keystore-desc = Signieren mit einer PKCS#12-Datei (Keystore).
ng-ui-backend-unavailable = Derzeit nicht verfügbar.
ng-ui-backend-local-unavailable = Keine lokale Bürgerkartenumgebung gefunden.
ng-ui-backend-keystore-unavailable = Keystore nicht eingerichtet (Einstellungen › Keystore).
ng-ui-backend-leaves-device = Das Dokument wird zum Signieren an A-Trust übermittelt.
ng-ui-backend-qes = Qualifizierte Signatur
ng-ui-factor-app = App
ng-ui-factor-sms = SMS
ng-ui-factor-card = Karte
ng-ui-privacy-title = Datenschutz
ng-ui-privacy-lead = Bitte lesen Sie den Datenschutzhinweis, bevor das Dokument übermittelt wird.
ng-ui-privacy-accept = Zustimmen und fortfahren
    .accesskey = Z
ng-ui-privacy-decline = Ablehnen
    .accesskey = b

## Signieren

ng-ui-progress-connecting = Verbindung wird aufgebaut …
ng-ui-progress-certificate = Zertifikat wird gelesen …
ng-ui-progress-server = Warten auf den Signaturdienst …
ng-ui-progress-document = Dokument { $index } von { $total } wird signiert …
ng-ui-progress-done = Signatur erstellt.
ng-ui-sms-title = SMS-TAN eingeben
ng-ui-qr-title = QR-Code scannen
ng-ui-qr-alt = QR-Code zum Scannen mit der App „ID Austria“
ng-ui-app-title = Bestätigung in der App
ng-ui-fido2-title = FIDO2-Sicherheitsschlüssel
ng-ui-fido2-hint = Verbinden Sie Ihren Sicherheitsschlüssel und folgen Sie den Anweisungen des Systems (PIN eingeben, Schlüssel berühren).
ng-ui-fido2-failed = FIDO2 ist fehlgeschlagen: { $detail }
ng-ui-countdown = Verbleibende Zeit: { $time }
ng-ui-countdown-expired = Die Zeit ist abgelaufen.
ng-ui-login-rejected = Die Anmeldung wurde abgelehnt. Bitte geben Sie das Passwort erneut ein.
ng-ui-username-required = Bitte geben Sie den Benutzernamen oder die Telefonnummer ein.
ng-ui-secret-title = Passwort
ng-ui-secret-prompt = Bitte geben Sie das Passwort ein:
ng-ui-secret-retry = Das Passwort ist falsch.
ng-ui-secret-subject = Für: { $subject }
ng-ui-proxy-password-prompt = Bitte geben Sie das Proxy-Passwort ein:
ng-ui-proxy-password-retry = Das Proxy-Passwort ist falsch.
ng-ui-notice-keystore-conversion = JKS- und JCEKS-Keystores werden nicht unterstützt. Bitte wandeln Sie den Keystore in PKCS#12 um (z. B. mit keytool -importkeystore).

## Ergebnis

ng-ui-output-reveal = Im Ordner anzeigen
    .accesskey = O
ng-ui-output-saving = Das signierte Dokument wird gespeichert …
ng-ui-output-next = Nächstes Dokument: { $name }
ng-ui-output-next-button = Nächstes Dokument signieren
    .accesskey = N
ng-ui-output-more = Weitere Dokumente öffnen
    .accesskey = W
ng-ui-output-drop-hint = Sie können weitere Dokumente hier ablegen.
ng-ui-output-saved-next = { $file } wurde gespeichert, weiter mit dem nächsten Dokument.

## Einstellungen

ng-ui-settings-interface = Oberfläche
ng-ui-settings-reset = Einstellungen zurücksetzen …
    .accesskey = z
ng-ui-settings-reset-confirm = Alle Einstellungen werden auf die Standardwerte zurückgesetzt; die bisherigen werden als Sicherung aufbewahrt.
ng-ui-settings-reset-confirm-button = Zurücksetzen
    .accesskey = Z
ng-ui-settings-preview-alt = Vorschau des Signaturblocks
ng-ui-settings-preview-note = Die Vorschau zeigt die gespeicherten Einstellungen.
ng-ui-emblem-none = Keine Bildmarke gewählt
ng-ui-emblem-drop-hint = Sie können eine Bilddatei (PNG, JPEG, GIF) auf die Vorschau ziehen.
ng-ui-theme = Darstellung
ng-ui-theme-system = Wie System
ng-ui-theme-light = Hell
ng-ui-theme-dark = Dunkel
ng-ui-cms-profile = Signaturformat (Keystore)
ng-ui-cms-profile-hint = Gilt für lokal erstellte Signaturen.
ng-ui-cms-pdfas = Kompatibel zu PDF-AS
ng-ui-cms-pades = PAdES-BASELINE-B
ng-ui-postfix-hint = Leer bedeutet „_signed“.
ng-ui-proxy-mode = Proxy verwenden
ng-ui-proxy-system = Proxy des Systems
ng-ui-proxy-none = Kein Proxy
ng-ui-proxy-manual = Manuell
ng-ui-proxy-password-hint = Das Passwort wird bei Bedarf abgefragt und im Schlüsselbund gespeichert.
ng-ui-certcache = Signaturzertifikat zwischenspeichern
ng-ui-certcache-hint = Speichert nur das öffentliche Zertifikat, damit die Vorschau den echten Signaturblock zeigt.
ng-ui-keystore-keychain = Im Schlüsselbund des Betriebssystems speichern
ng-ui-keystore-alias-hint = Leer lassen, wenn der Keystore nur einen Schlüssel enthält.
ng-ui-problem-required = Bitte füllen Sie dieses Feld aus.
ng-ui-problem-port = Der Port muss zwischen 1 und 65535 liegen.
ng-ui-problem-invalid = Der Wert ist ungültig.
ng-ui-about-version = Version { $version }
ng-ui-about-thirdparty = Die Lizenzhinweise der verwendeten Komponenten liegen dem Programmpaket bei.

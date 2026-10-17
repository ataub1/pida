### PIDA – eigene Texte, deutsch (EUPL-1.2)
###
### Von Hand gepflegt. Jede ID braucht einen Eintrag in `de/` und `en/` mit denselben Variablen
### (Test `crates/i18n/tests/code_catalog.rs`). Zwei Arten von IDs:
###
### - Eigene Texte (Paket E und UI): Präfix `ng-`, damit sie nie mit den aus PDF-Over erzeugten IDs
###   (`pdfover.ftl`) kollidieren.
### - Stabile Codes der Crates (ADR 0001 Regel 11, Plan §7.5): Die ID ist der Code selbst
###   (`cms-der`, `backend-network`, `fido2-timeout` …). Codes mit Punkt (`render.failed`,
###   `placement.notice.clamped`) sind in Fluent nicht zulässig; ihre ID entsteht wie bei den
###   PDF-Over-Keys mit `pida_i18n::fluent_id_for_java_key` (Punkt → Bindestrich:
###   `render-failed`, `placement-notice-clamped`).
###
### Variablen heißen wie die Felder der Fehlervariante (`$detail` für einen Freitext). Der Test prüft,
### dass jeder Code, den der Quelltext liefert (`fn code`, `fn message_id`, `NOTICE_*`, Code-Literale),
### hier in beiden Sprachen steht.

## Konfiguration: Fehler (`pida_config::ConfigError::message_id`)

ng-config-error-io = Ein-/Ausgabefehler bei den Einstellungen: { $detail }
ng-config-error-parse = Die Einstellungen sind nicht lesbar: { $detail }
ng-config-error-serialize = Die Einstellungen konnten nicht gespeichert werden: { $detail }
ng-config-error-unsupported-schema = Die Einstellungen stammen von einer neueren Version (Schema { $found }, unterstützt bis { $supported }).
ng-config-error-legacy = Die PDF-Over-Konfiguration ist nicht lesbar: { $detail }
ng-config-error-no-home = Der Konfigurationsordner kann nicht ermittelt werden.
ng-config-error-secret = Fehler beim Zugriff auf den Schlüsselbund: { $detail }
ng-config-file-missing = Die Konfigurationsdatei „{ $path }“ wurde nicht gefunden.
ng-config-reset-done = Die bisherigen Einstellungen wurden nach „{ $path }“ verschoben.
ng-secrets-not-persistent = Kein Schlüsselbund verfügbar: Passwörter werden nur bis zum Programmende gespeichert.

## Konfiguration: Lesen von PDF-Over.config (`pida_config::LegacyWarning::code`)

ng-config-legacy-invalid-bku = BKU: Der Wert „{ $value }“ ist ungültig; es gibt keine Voreinstellung.
ng-config-legacy-invalid-proxy-port = PROXY_PORT: „{ $value }“ ist keine gültige Zahl und wird ignoriert.
ng-config-legacy-proxy-port-out-of-range = PROXY_PORT: { $value } liegt außerhalb von 1 bis 65535 und wird ignoriert.
ng-config-legacy-invalid-value = { $key }: Der Wert „{ $value }“ ist ungültig; der Standardwert gilt.
ng-config-legacy-unknown-locale = { $key }: „{ $value }“ ist keine gültige Sprache; die Systemsprache gilt.
ng-config-legacy-unsupported-locale = { $key }: Die Sprache „{ $value }“ wird nicht unterstützt; es gilt { $lang }.
ng-config-legacy-unknown-profile = SIGNATURE_PROFILE: „{ $value }“ ist unbekannt; es gilt der Standard-Signaturblock.
ng-config-legacy-unknown-pass-store-type = KEYSTORE_PASS_STORE_TYPE: „{ $value }“ ist unbekannt; Keystore-Passwörter werden nicht gespeichert.

## Konfiguration: Import aus ~/.pdf-over (`pida_config::migrate`)

ng-migrate-offer-title = Einstellungen von PDF-Over übernehmen?
ng-migrate-offer-text = Es wurden Einstellungen einer früheren PDF-Over-Installation gefunden („{ $path }“). Sollen sie übernommen werden? Die alte Datei bleibt unverändert.
ng-migrate-secrets-consent = In der alten Konfiguration sind Passwörter im Klartext gespeichert. Sollen sie in den Schlüsselbund des Betriebssystems übernommen werden?
ng-migrate-remove-plaintext = Die Passwörter stehen weiterhin im Klartext in „{ $path }“. Sie können sie dort entfernen lassen.
ng-migrate-done = Übernommen: { $imported } Einstellungen; nicht übernommen: { $skipped }.
ng-migrate-undo = Import rückgängig machen
ng-migrate-key-ignored = { $key } wird nicht übernommen (in PIDA ohne Funktion).
ng-migrate-unknown-key = Der unbekannte Eintrag { $key } wird ignoriert.
ng-migrate-path-missing = { $key }: Der Pfad „{ $path }“ existiert nicht.
ng-migrate-path-dropped = { $key }: Der Pfad „{ $path }“ existiert nicht und wird nicht übernommen.
ng-migrate-emblem-unsupported = EMBLEM: „{ $path }“ ist kein unterstütztes Bild (PNG, JPEG, GIF) und wird nicht übernommen.
ng-migrate-emblem-from-cache = EMBLEM: Das Original „{ $path }“ fehlt; die von PDF-Over zwischengespeicherte Bildmarke wird übernommen.
ng-migrate-emblem-default = EMBLEM: „{ $path }“ ist die Standard-Bildmarke von PDF-AS und keine eigene Bildmarke; sie wird nicht übernommen (es bleibt das PIDA-Siegel).
ng-migrate-secret-ignored = { $key }: Das Passwort wird nicht übernommen, weil die Speicherart nicht „Permanent“ ist.
ng-migrate-secret-no-target = { $key }: Das Passwort kann ohne zugehörige Keystore-Datei bzw. Proxy-Host und -Benutzer nicht zugeordnet werden.
ng-migrate-signature-field-discarded = Das in PDF-Over dauerhaft gespeicherte Signaturfeld wird verworfen (B-05); die Auswahl gilt künftig nur für das jeweilige Dokument.
ng-migrate-custom-profile = Eigenes PDF-AS-Profil gefunden: „{ $file }“.
ng-migrate-note-default = Der Signaturhinweis entspricht dem Standardtext und folgt künftig Profil und Sprache.
ng-migrate-placeholder-search-off = Die Platzhaltersuche war aktiv, aber weder QR-Code- noch Signaturfeld-Suche gewählt; sie bleibt ausgeschaltet.
ng-migrate-proxy-without-host = PROXY_PORT bzw. PROXY_USER ohne PROXY_HOST werden nicht übernommen.
ng-migrate-keystore-jks = Keystore-Typ { $value }: JKS- bzw. JCEKS-Keystores müssen vor der Verwendung in PKCS#12 umgewandelt werden.

## CMS-Signaturen (`pida_cades::CmsError::code`)

cms-der = Die Signaturdaten (CMS) sind nicht lesbar.
cms-certificate = Ein Zertifikat ist nicht lesbar.
cms-unsupported-algorithm = Der Signatur- oder Hashalgorithmus wird nicht unterstützt.
cms-not-signed-data = Die Daten sind keine CMS-Signatur (SignedData).
cms-signer-count = Die Signatur enthält nicht genau einen Unterzeichner.
cms-missing-attribute = Ein Pflichtattribut der Signatur fehlt.
cms-digest-mismatch = Die Prüfsumme der Signatur passt nicht zum Dokument.
cms-invalid-signature = Der Signaturwert ist ungültig.
cms-signer-certificate-missing = Das Zertifikat des Unterzeichners fehlt in der Signatur.
cms-signing-certificate-mismatch = Das Signaturattribut verweist auf ein anderes Zertifikat.
cms-unexpected-signer = Die Signatur wurde mit einem anderen als dem erwarteten Zertifikat erstellt.
cms-key = Der Schlüssel ist nicht verwendbar.
cms-key-mismatch = Der private Schlüssel passt nicht zum Zertifikat.
cms-too-large = Die Signatur ist größer als der reservierte Platz.
cms-io = Lesefehler beim Prüfen der Signatur.

## Keystore (`pida_backend_keystore::KeystoreError::code`)

keystore-file-not-found = Die Keystore-Datei wurde nicht gefunden.
keystore-wrong-password = Das Keystore-Passwort ist falsch.
keystore-alias-not-found = Der Alias wurde im Keystore nicht gefunden.
keystore-not-a-key-entry = Der Alias ist kein Schlüsseleintrag.
keystore-conversion-needed = Das Keystore-Format (JKS/JCEKS) wird nicht unterstützt – bitte nach PKCS#12 konvertieren.
keystore-unsupported-pbe = Die Verschlüsselung des Keystores wird nicht unterstützt – bitte neu exportieren.
keystore-different-key-password = Der Schlüssel ist mit einem eigenen Passwort geschützt – bitte das PKCS#12 mit einheitlichem Passwort exportieren.
keystore-unsupported-key = Der Schlüsseltyp wird nicht unterstützt.
keystore-cms = Zertifikat oder Schlüssel im Keystore sind nicht verwendbar.
keystore-io = Die Keystore-Datei ist nicht lesbar.
keystore-malformed = Die Datei ist kein gültiger PKCS#12-Keystore.
keystore-no-key-entry = Der Keystore enthält keinen Schlüssel mit Zertifikat.
keystore-alias-required = Der Keystore enthält mehrere Schlüssel – bitte einen Alias angeben.
keystore-key-mismatch = Schlüssel und Zertifikat im Keystore passen nicht zusammen.

## Signatur-Backends (`pida_backend_api::BackendError::code`); Variablen = Felder der Variante

backend-cancelled = Der Vorgang wurde abgebrochen.
    .title = Vorgang abgebrochen
    .action = Erneut versuchen
backend-unavailable = Das Signiermittel ist nicht verfügbar: { $detail }
backend-network = Die Verbindung zum Signaturdienst ist fehlgeschlagen: { $detail }
    .title = Der Signaturdienst ist gerade nicht erreichbar
    .text = Die Verbindung zum Signaturdienst ist fehlgeschlagen. Prüfen Sie die Internetverbindung und versuchen Sie es erneut; die genaue Ursache steht unter „Technische Details“.
    .action = Erneut versuchen
backend-proxy-auth = Der Proxy-Server verlangt eine Anmeldung.
backend-document-too-large = Das Dokument überschreitet die Größengrenze des Signaturdienstes.
backend-http = Der Signaturdienst hat die Anfrage abgelehnt: HTTP { $status } { $reason }
backend-security-layer = Vorgang vom Signaturserver abgebrochen ({ $code }): { $info }
backend-unknown-page = Unbekannte Seite des Signaturdienstes ({ $diagnostic }). Der Vorgang wurde abgebrochen.
backend-loop = Der Signaturdienst antwortet in einer Endlosschleife; der Vorgang wurde abgebrochen.
backend-protocol = Unerwartete Antwort des Signaturdienstes: { $detail }
backend-wrong-password = Das Passwort ist falsch.
backend-keystore = Fehler beim Keystore: { $detail }
backend-keystore-format = Das Keystore-Format { $format } wird nicht unterstützt – bitte nach PKCS#12 konvertieren.
backend-certificate-mismatch = Die Signatur wurde mit einem anderen als dem erwarteten Zertifikat erstellt.
backend-fido2 = FIDO2-Fehler: { $detail }
backend-timeout = Zeitüberschreitung: Der Signaturdienst hat nicht rechtzeitig geantwortet.
    .title = Die Zeit ist abgelaufen
    .action = Neue Bestätigung anfordern
backend-internal = Interner Fehler: { $detail }

## Meldungen und Masken der Backends ohne PDF-Over-Key (`NoticeCode::AtrustInterstitial` als Titel,
## `FormProblem::LoginRejected`); Zuordnung aller Varianten: crates/i18n/tests/code_catalog.rs

ng-notice-atrust-interstitial = Hinweis von A-Trust
ng-form-login-rejected = Die Anmeldung wurde nicht angenommen. Bitte das Passwort erneut eingeben.

## ID Austria über A-Trust, Szenario A (`pida_backend_atrust`)

atrust-unknown-page = Unbekannte Seite des Signaturdienstes ({ $diagnostic }). Der Vorgang wurde abgebrochen.
atrust-signature-data-link-rejected = Der Link „Signaturdaten anzeigen“ führt nicht zu A-Trust und wird nicht geöffnet.

## FIDO2 (`pida_backend_atrust::Fido2Error::code` und Detailcodes der Provider)

fido2-unavailable = FIDO2 ist auf diesem Gerät nicht verfügbar.
fido2-cancelled = Vorgang am Sicherheitsschlüssel abgebrochen.
fido2-invalid-options = Die FIDO2-Anfrage des Signaturdienstes ist nicht lesbar.
fido2-device = Fehler am Sicherheitsschlüssel: { $detail }
fido2-pin-unsupported = Dieser Sicherheitsschlüssel verlangt eine PIN; die PIN-Eingabe ist auf diesem System noch nicht verfügbar – bitte App oder SMS verwenden.
fido2-no-credential = Auf diesem Sicherheitsschlüssel ist keine ID-Austria-Registrierung vorhanden.
fido2-timeout = Zeitüberschreitung: Der Sicherheitsschlüssel wurde nicht berührt.
fido2-locked = Der Sicherheitsschlüssel ist gesperrt.
fido2-pin-auth-blocked = Zu viele falsche PIN-Eingaben in Folge. Bitte den Sicherheitsschlüssel ab- und wieder anstecken und es erneut versuchen.
fido2-u2f-only = Der Sicherheitsschlüssel unterstützt kein FIDO2 (nur U2F).
fido2-no-user-verification = Der Sicherheitsschlüssel kann die Nutzerin nicht verifizieren (keine PIN oder Biometrie eingerichtet).
fido2-hid-permission = Kein Zugriff auf den Sicherheitsschlüssel – die udev-Regel (uaccess) für FIDO-Geräte fehlt.
fido2-no-device = Kein FIDO2-Sicherheitsschlüssel gefunden. Bitte stecken Sie Ihren Sicherheitsschlüssel an und versuchen Sie es erneut.

## Signaturblock (`pida_sigblock::SigBlockError::code`, `NOTICE_CHARS_REPLACED`)

sigblock-unknown-profile = Das Signaturprofil „{ $profile }“ existiert nicht.
sigblock-profile = Das Signaturprofil ist fehlerhaft: { $detail }
sigblock-unsupported-expression = Der Ausdruck { $expr } im Signaturprofil wird nicht unterstützt.
sigblock-unsupported-date-pattern = Das Datumsformat „{ $pattern }“ im Signaturprofil wird nicht unterstützt.
sigblock-font = Die Schrift für den Signaturblock konnte nicht geladen werden: { $detail }
sigblock-missing-glyph = Das Zeichen „{ $char }“ ist in der Schrift des Signaturblocks nicht verfügbar.
sigblock-image = Die Bildmarke konnte nicht gelesen werden (unterstützt: PNG, JPEG, GIF).
sigblock-missing-resource = Eine Ressource des Signaturprofils fehlt: { $detail }
sigblock-pdf = Das Erscheinungsbild der Signatur konnte nicht erzeugt werden.
sigblock-io = Fehler beim Lesen der Signaturblock-Konfiguration.
sigblock-chars-replaced = Einige Zeichen sind in der Schrift des Signaturblocks nicht verfügbar und wurden durch „?“ ersetzt: { $chars }

## Darstellung (`pida_render::RenderError::code`, Code `render.…` → ID `render-…`)

render-library_not_found = Die PDF-Darstellungsbibliothek (PDFium) wurde nicht gefunden: { $details }
render-load_failed = Das Dokument konnte nicht geladen werden: { $details }
render-password_required = Das Dokument ist durch ein Passwort geschützt.
render-page_out_of_range = Seite { $page } existiert nicht (das Dokument hat { $count } Seiten).
render-failed = Die Seite konnte nicht dargestellt werden: { $details }

## Positionierung (`pida_placement::PlacementError::code`, `PlacementNotice::code`; `placement.…` → `placement-…`)

placement-invalid_position = Ungültige Positionsangabe: { $position }
placement-no_such_page = Seite { $page } existiert nicht.
placement-placeholder_not_found = Es wurde kein passender Signaturplatzhalter gefunden.
placement-not_enough_space = Für den Signaturblock ist nicht genug Platz, und auf bereits signierten Dokumenten darf keine neue Seite angefügt werden.
placement-notice-new_page_not_allowed = Das Dokument ist bereits signiert – der Signaturblock wurde auf der letzten Seite statt auf einer neuen Seite platziert.
placement-notice-overlaps_content = Der Signaturblock überdeckt Inhalt der Seite.
placement-notice-overlaps_widget = Der Signaturblock überdeckt ein vorhandenes Formular- oder Signaturfeld.
placement-notice-clamped = Die Position wurde an die Seite angepasst.
placement-notice-scan_failed = Der freie Platz auf der Seite konnte nicht ermittelt werden; der Signaturblock wurde oben auf der Seite platziert.

## Eingangsanalyse (`pida_pades::Finding::code`, Plan §6.1)

pdf-xref-repaired = Der Querverweis des Dokuments ist beschädigt und wurde beim Lesen korrigiert. Das Signieren ist möglich, ändert die Beschädigung aber nicht.
pdf-xref-reconstructed = Der Querverweis des Dokuments fehlt oder ist unlesbar. Das Dokument kann ohne Reparatur nicht signiert werden.
pdf-tail-garbage = Nach dem Dateiende-Marker (%%EOF) stehen fremde Daten.
pdf-prior-signature-beyond-eof = Eine vorhandene Signatur reicht über das Dateiende hinaus – das Dokument wurde nach dem Signieren gekürzt oder beschädigt und kann nicht weiter signiert werden.
pdf-docmdp-p1 = Das Dokument ist zertifiziert und erlaubt keine Änderungen (DocMDP P=1); es kann nicht signiert werden.
pdf-user-password = Das Dokument ist mit einem Kennwort zum Öffnen geschützt und kann nicht signiert werden.
pdf-xfa-dynamic = Das Dokument ist ein dynamisches XFA-Formular und kann nicht signiert werden.
pdf-usage-rights = Das Dokument enthält erweiterte Adobe-Reader-Rechte. Das Signieren macht sie ungültig; Adobe Reader zeigt danach eine Warnung.

## Oberfläche: Allgemein

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
ng-ui-starting = PIDA wird gestartet
ng-ui-finished = PIDA wurde beendet.
ng-ui-finished-code = Beendet mit Code { $code }.
ng-ui-mainbar-label = Arbeitsschritte
ng-ui-step-done = erledigt
ng-ui-lang-system = Systemsprache
ng-ui-lang-de = Deutsch
ng-ui-lang-en = Englisch
ng-ui-drop-not-here = Dokumente können im Schritt „Öffnen“ oder auf dem Ergebnis abgelegt werden.

## Oberfläche: Fehler der Oberfläche bzw. der Shell

ng-ui-core-unavailable = Der Signaturkern konnte nicht gestartet werden. Bitte versuchen Sie es erneut; bleibt der Fehler bestehen, prüfen Sie die Protokolle (Einstellungen › Über).
ng-ui-event-invalid = Diese Aktion wird von dieser Version des Signaturkerns noch nicht unterstützt.
ng-ui-bridge-error = Die Verbindung zum Signaturkern ist gestört.
ng-ui-error-unknown = Unerwarteter Fehler ({ $code }).
ng-ui-open-failed = Das Öffnen ist fehlgeschlagen.
ng-ui-dialog-failed = Der Dateidialog konnte nicht geöffnet werden.

## Oberfläche: Öffnen

ng-ui-open-title = Dokument öffnen
ng-ui-open-analyzing = Dokument wird geprüft …
ng-ui-dropzone-label = Ablagefläche für PDF-Dokumente
ng-ui-dropzone-hint = Mehrere Dokumente werden nacheinander signiert. Tastatur: Strg+O bzw. ⌘O öffnet den Dateidialog.
ng-ui-dropzone-release = Loslassen, um die Dokumente zu öffnen
ng-ui-queue-title = Warteschlange ({ $count })
ng-ui-queue-current = aktuell

## Oberfläche: Platzhalter

ng-ui-placeholder-title = Signaturplatzhalter gefunden
ng-ui-placeholder-qr = QR-Code-Platzhalter auf Seite { $page }
ng-ui-placeholder-field = Signaturfeld „{ $name }“
ng-ui-placeholder-kind = Art der Platzhalter

## Oberfläche: Positionieren

ng-ui-position-title = Signatur positionieren
ng-ui-position-help = Ziehen Sie den Signaturblock an die gewünschte Stelle oder verwenden Sie die Pfeiltasten (Umschalt: 1 pt, Strg/⌘: 50 pt), Bild auf/ab für die Seite und Eingabe zum Signieren.
ng-ui-position-block-role = verschiebbarer Signaturblock
ng-ui-position-announce = Seite { $page } von { $pages }, linke Kante { $left } mm von links, Oberkante { $top } mm von unten
ng-ui-position-pages = Seiten
ng-ui-position-page-alt = Seite { $page } von { $pages } – { $name }
ng-ui-position-new-page-alt = Neue leere Seite
ng-ui-position-preview-failed = Die Seitenvorschau ist nicht verfügbar.
ng-ui-position-auto = Automatisch
    .accesskey = A
ng-ui-position-auto-done = Der Signaturblock wurde automatisch platziert.
ng-ui-position-invisible = Unsichtbar signieren
    .accesskey = U
ng-ui-position-signed-hint = Das Dokument ist bereits signiert; eine neue Seite kann nicht angefügt werden.

## Oberfläche: Signiermittel

ng-ui-backend-title = Signiermittel wählen
ng-ui-backend-hint = Ein Standard-Signiermittel lässt sich in den Einstellungen (Erweitert) festlegen.
ng-ui-backend-atrust-desc = Signieren mit ID Austria: Bestätigung in der App, per SMS-TAN oder mit FIDO2-Sicherheitsschlüssel.
ng-ui-backend-local-desc = Signieren mit der Chipkarte über eine lokal installierte Chipkarten-Software.
ng-ui-backend-keystore-desc = Signieren mit einer PKCS#12-Datei (Keystore).
ng-ui-backend-unavailable = Derzeit nicht verfügbar.
ng-ui-backend-local-unavailable = Keine lokale Chipkarten-Software gefunden.
ng-ui-backend-keystore-unavailable = Keystore nicht eingerichtet (Einstellungen › Keystore).
ng-ui-backend-leaves-device = Das Dokument wird zum Signieren an A-Trust übermittelt.
ng-ui-backend-qes = Qualifizierte Signatur
ng-ui-factor-app = App
ng-ui-factor-sms = SMS
ng-ui-factor-card = Karte
ng-ui-privacy-title = Datenschutz
    .accesskey = t

## Oberfläche: Signieren

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

## Oberfläche: Ergebnis

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

## Oberfläche: Einstellungen

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

## Eingangsanalyse (`pida_pades::Finding::code`, Plan §6.1) – Welle 2

pdf-docmdp-p2 = Das Dokument ist zertifiziert (Formulare ausfüllen und signieren erlaubt): nur vorhandene leere Signaturfelder oder unsichtbare Signaturen.
pdf-docmdp-p3 = Das Dokument ist zertifiziert (Formulare, Signaturen und Kommentare erlaubt): nur vorhandene leere Signaturfelder oder unsichtbare Signaturen.
pdf-permissions = Das Dokument ist geschützt; die Rechte erlauben kein Signieren.
pdf-encrypted = Das Dokument ist verschlüsselt; die Signatur wird mit derselben Verschlüsselung ergänzt.
pdf-xfa-hybrid = Das Dokument enthält ein XFA-Formular; XFA-Anzeigeprogramme zeigen den Signaturblock eventuell nicht.
pdf-field-locked = Einige Signaturfelder sind durch vorhandene Signaturen gesperrt und können nicht gewählt werden.
pdf-seed-value = Das Signaturfeld enthält Vorgaben für die Signatur; sie werden erfüllt.
pdf-seed-value-incompatible = Das Signaturfeld enthält Vorgaben, die diese Signatur nicht erfüllen kann.
pdf-pdfa-need-appearances = Das PDF/A-Dokument verlangt die Neuerzeugung von Formularansichten und ist bereits nicht konform.
pdf-prior-signature-invalid = Eine vorhandene Signatur ist ungültig.
pdf-prior-signature-unverifiable = Eine vorhandene Signatur kann nicht geprüft werden.
pdf-public-key = Das Dokument ist mit Zertifikaten verschlüsselt und kann nicht signiert werden.
pdf-unsupported-security = Die Verschlüsselung des Dokuments wird nicht unterstützt.
pdf-no-pages = Das Dokument hat keine Seiten.
pdf-not-a-pdf = Die Datei ist kein PDF-Dokument.
pdf-unreadable = Das Dokument ist beschädigt und kann nicht gelesen werden.
pdf-limit-exceeded = Das Dokument überschreitet eine Verarbeitungsgrenze.
pdf-decryption = Das Dokument konnte nicht entschlüsselt werden.
pdf-io = Das Dokument konnte nicht gelesen werden (Lesefehler).
pdf-signature-subfilter = Das Signaturverfahren einer vorhandenen Signatur wird nicht geprüft.
pdf-signature-byterange = Der signierte Bereich einer vorhandenen Signatur ist ungültig.
pdf-signature-dictionary = Eine vorhandene Signatur hat kein lesbares Signaturdictionary.

## PAdES-Engine (`pida_pades::PadesError::code`, Invarianten I1–I6, Hinweise) – Welle 2

pades-pdf-unreadable = Das Dokument konnte nicht gelesen werden.
pades-write = Die Signatur konnte nicht in das Dokument geschrieben werden.
pades-field-already-signed = Das gewählte Signaturfeld ist bereits signiert.
pades-field-locked = Das gewählte Signaturfeld ist gesperrt.
pades-certificate-not-valid = Das Zertifikat ist zum Signierzeitpunkt nicht gültig.
pades-io = Lese- oder Schreibfehler.
pades-field-not-found = Das gewählte Signaturfeld wurde nicht gefunden.
pades-new-page-not-allowed = Für dieses Dokument darf keine neue Seite angehängt werden.
pades-invalid-plan = Die Signatur kann so nicht platziert werden.
pades-invalid-state = Der gespeicherte Signaturvorgang ist ungültig und wird verworfen.
pades-invariant-i1 = Die Selbstprüfung der Signatur ist fehlgeschlagen (Original nicht unverändert); es wurde nichts gespeichert.
pades-invariant-i2 = Die Selbstprüfung der Signatur ist fehlgeschlagen (signierter Bereich); es wurde nichts gespeichert.
pades-invariant-i3 = Die Selbstprüfung der Signatur ist fehlgeschlagen (Signaturdaten oder Zertifikat); es wurde nichts gespeichert.
pades-invariant-i4 = Die Selbstprüfung der Signatur ist fehlgeschlagen (vorhandene Signaturen); es wurde nichts gespeichert.
pades-invariant-i5 = Die Selbstprüfung der Signatur ist fehlgeschlagen (Ergebnis nicht lesbar); es wurde nichts gespeichert.
pades-invariant-i6 = Die Selbstprüfung der Signatur ist fehlgeschlagen (unerlaubte Änderung); es wurde nichts gespeichert.
pades-pdfua-skipped = Die Barrierefreiheits-Struktur (PDF/UA) wurde nicht ergänzt.
pades-output-intent-skipped = Der Farbraum für PDF/A (OutputIntent) wurde nicht ergänzt, weil das Dokument bereits signiert ist.
pades-i5-without-pdfium = Die Prüfung mit PDFium wurde übersprungen.
pades-new-visible-field-not-allowed = Das Dokument ist zertifiziert: Ein neues sichtbares Signaturfeld würde die Zertifizierung verletzen. Bitte ein vorhandenes leeres Signaturfeld oder eine unsichtbare Signatur wählen.
pades-not-self-checked = Die Selbstprüfung der Signatur wurde nicht bestanden; es wird nichts gespeichert.

## Signatur-Backends (`BackendError::code`, `FormProblem::code`) – Welle 2

backend-login-rejected = Die Anmeldung wurde abgelehnt. Bitte die Zugangsdaten prüfen.

## FIDO2 (`Fido2Error::code`) – Welle 2

fido2-pin-required = Der Sicherheitsschlüssel verlangt eine PIN.
fido2-pin-invalid = Die PIN des Sicherheitsschlüssels ist falsch.
fido2-internal = Interner Fehler bei der Anmeldung mit dem Sicherheitsschlüssel.

## Kommandozeile (`apps/cli`) – Welle 2

ng-cli-help-title = PIDA – PDF-Signatur mit ID Austria
ng-cli-help-usage = Aufruf: pida-cli [Optionen] [Dateien …] | pida-cli <Befehl> [Optionen]
ng-cli-help-options = Optionen (kompatibel zu PDF-Over):
ng-cli-help-new-options = Neue Optionen:
ng-cli-help-commands = Befehle:
ng-cli-help-headless = Ohne Rückfragen signieren (Keystore mit -kspass/--kspass-stdin/--kspass-env, lokale BKU); ohne Terminal automatisch
ng-cli-help-json = Ergebnis maschinenlesbar (JSON) auf stdout ausgeben
ng-cli-help-kspass-stdin = Keystore-Passwort von stdin lesen (erste Zeile)
ng-cli-help-kspass-env = Keystore-Passwort aus der Umgebungsvariable lesen
ng-cli-help-log-level = Log-Stufe (error, warn, info, debug, trace)
ng-cli-help-version = Version anzeigen
ng-cli-help-cmd-sign = Dokumente signieren (Dateien, Ordner, Muster wie *.pdf)
ng-cli-help-cmd-verify = Signaturen eines PDFs prüfen
ng-cli-help-cmd-profiles = Signaturprofile auflisten
ng-cli-help-cmd-migrate = Einstellungen aus ~/.pdf-over übernehmen (--from <Ordner>, --dry-run)
ng-cli-help-cmd-render-block = Signaturblock als PDF/PNG ausgeben (--profile <ID> <Datei>)
ng-cli-help-exit-codes = Exit-Codes: 0 erfolgreich, 1 allgemeiner Fehler, 2 Aufruf, 3 Dokument abgelehnt, 4 abgebrochen, 5 Signiermittel/Netz, 6 Selbstprüfung (nichts gespeichert), 7 Schreibfehler
ng-cli-no-input = Kein Dokument angegeben (Hilfe: pida-cli -h).
ng-cli-keypass-mismatch = -kskeypass weicht von -kspass ab: PKCS#12 hat nur ein Passwort. Bitte den Keystore mit einheitlichem Passwort exportieren.
ng-cli-kspass-env-missing = Die Umgebungsvariable { $name } ist nicht gesetzt.
ng-cli-stdin-failed = Passwort von stdin nicht lesbar: { $detail }
ng-cli-signed = signiert: { $input } → { $output }
ng-cli-cancelled = abgebrochen: { $input }
ng-cli-failed = fehlgeschlagen: { $input }: { $message }
ng-cli-overwrite = Datei { $name } existiert bereits. Überschreiben? [j/N]{ " " }
ng-cli-retry = Wiederholen? [j/N]{ " " }
ng-cli-continue = Fortfahren? [j/N]{ " " }
ng-cli-username = Benutzername/Telefonnummer:{ " " }
ng-cli-password = Passwort:{ " " }
ng-cli-tan = TAN (Vergleichswert { $reference }):{ " " }
ng-cli-qr = Vergleichswert { $reference }: Bitte den QR-Code mit der App „ID Austria“ scannen ({ $path }).
ng-cli-app = Vergleichswert { $reference }: Bitte in der App „ID Austria“ bestätigen.
ng-cli-countdown = noch { $seconds } s …
ng-cli-fido2 = FIDO2: Bitte den Sicherheitsschlüssel berühren …
ng-cli-keystore-password = Keystore-Passwort für { $name }:{ " " }
ng-cli-key-password = Schlüssel-Passwort für { $name }:{ " " }
ng-cli-proxy-password = Proxy-Passwort für { $name }:{ " " }
ng-cli-wrong-password = Passwort falsch.
ng-cli-progress = … { $step }
ng-cli-valid = gültig: { $field } ({ $signer })
ng-cli-invalid = ungültig: { $field } ({ $error })
ng-cli-no-signatures = Keine Signaturen gefunden.
ng-cli-written = geschrieben: { $path }
ng-cli-no-legacy = Keine PDF-Over-Konfiguration gefunden ({ $path }).
ng-cli-migrate-dry-run = Probelauf – es wurde nichts geschrieben.
ng-cli-unknown-profile = Profil { $id } existiert nicht (Liste: pida-cli profiles).

## Fortschritt (`pida_core_api::messages::progress_message`) – Welle 2

ng-progress-connecting = Verbindung wird aufgebaut
ng-progress-certificate = Zertifikat wird gelesen
ng-progress-waiting = Warten auf den Signaturdienst
ng-progress-document = Dokument { $index } von { $total }
ng-progress-done = Fertig

## Einstellungsprüfung (`pida-core-api`, Feldprobleme) – Welle 2

ng-settings-invalid-postfix = Der Dateinamenzusatz darf keine Pfadtrenner (/ oder \) enthalten.
ng-settings-proxy-host-missing = Für einen manuellen Proxy ist ein Host anzugeben.
ng-settings-invalid-logo-size = Die Größe der Bildmarke muss größer als 0 und höchstens 120 mm sein.

## Prüfung vorhandener Signaturen (`verify`) – Welle 2

ng-verify-failed = Die Datei konnte nicht geprüft werden: { $detail }
ng-verify-no-signature-dictionary = Signaturfeld ohne Signaturdaten.
ng-verify-no-byte-range = Signatur ohne /ByteRange bzw. /Contents.
ng-verify-byte-range = Die /ByteRange der Signatur ist ungültig.

## Workflow und Kern (`pida-workflow`, `pida-core-api`) – Welle 2

ng-invariant-violated = Die Selbstprüfung der Signatur ist fehlgeschlagen ({ $invariant }: { $detail }). Es wurde nichts gespeichert.
ng-cms-too-large = Die Signatur ist größer als der reservierte Platz ({ $needed } statt { $reserve } Bytes). „Wiederholen“ bereitet das Dokument mit mehr Platz neu vor; dafür ist eine erneute Bestätigung nötig.
    .title = Die Signatur braucht mehr Platz
    .action = Erneut versuchen
ng-backend-choice-required = Mehrere Signiermittel verfügbar ({ $detail }) – bitte mit -b wählen.
ng-no-backend-available = Kein Signiermittel verfügbar.
ng-default-backend-unavailable = Das gewählte Signiermittel ({ $detail }) ist nicht verfügbar.
ng-document-rejected = Das Dokument kann nicht signiert werden.
    .title = Dieses Dokument kann nicht signiert werden
    .action = Anderes Dokument öffnen
ng-pdf-no-sign-rights = Die Rechte des Dokuments erlauben kein Signieren.
ng-field-already-signed = Das Signaturfeld ist bereits signiert.
ng-field-locked = Das Signaturfeld ist gesperrt.
ng-certificate-not-valid = Das Zertifikat ist zum Signierzeitpunkt nicht gültig.
ng-clock-skew = Die Uhr dieses Geräts weicht um etwa { $minutes } Minuten von der Zeit des Signaturdienstes ab.
ng-atrust-login-rejected = Die Anmeldung wurde abgelehnt. Bitte das Passwort erneut eingeben.
ng-unknown-document = Das Dokument ist nicht (mehr) geöffnet.
ng-core-stopped = Der Programmkern wurde beendet.
ng-no-preview = Keine Vorschau des Signaturblocks verfügbar.

## Welle 3 (Integration: Bridge, Shell, CLI-Argumente der GUI, Zertifikats-Cache)

ng-ui-warning-title = Hinweis zum Dokument
ng-ui-warning-lead = Beim Prüfen des Dokuments ist Folgendes aufgefallen:
ng-ui-warning-question = Soll das Dokument trotzdem signiert werden?
ng-ui-warning-proceed = Trotzdem signieren
ng-ui-warning-discard = Dokument verwerfen
ng-ui-done-title = Erledigt ({ $count })
ng-ui-done-signed = signiert: { $file }
ng-ui-done-cancelled = abgebrochen
ng-ui-done-failed = fehlgeschlagen: { $message }
ng-ui-settings-read-only = Diese Einstellungen werden nicht gespeichert (Konfiguration per -c bzw. nicht lesbare Einstellungsdatei). Änderungen gelten bis zum Programmende.
ng-cli-fido2-pin = PIN des Sicherheitsschlüssels:{ " " }
ng-certcache-mismatch = Das Zertifikat hat sich geändert (z. B. erneuert oder ein anderes Konto). Die Signatur wurde verworfen. „Wiederholen“ liest das Zertifikat neu, erstellt den Signaturblock mit den neuen Angaben und fragt erneut nach der Bestätigung.
ng-placeholder-profile-unknown = Der Signaturplatzhalter nennt das unbekannte Profil „{ $profile }“; signiert wird mit dem eingestellten Profil.
ng-ui-args-cli-only = Die Option { $option } gibt es nur in der Kommandozeile (pida-cli).
ng-ui-about-notices = Drittlizenzen anzeigen

## Mobile (Welle Mobile, Paket M0: Lebenszyklus, „Signatur fortsetzen“, PKCS#12-Import, Teilen; Plan §5.2)

ng-fido2-unavailable-mobile = FIDO2-Sicherheitsschlüssel sind auf diesem Gerät nicht verfügbar. Bitte bestätigen Sie in der App „ID Austria“ oder per SMS-TAN.
ng-resume-expired = Eine unterbrochene Signatur war älter als eine Stunde und wurde verworfen.
ng-resume-restore-failed = Die unterbrochene Signatur konnte nicht wiederhergestellt werden; das Dokument wird neu vorbereitet.
ng-resume-backend-unavailable = Das Signiermittel der unterbrochenen Signatur ist nicht mehr verfügbar. Bitte wählen Sie ein anderes.
ng-keystore-imported = Der Keystore „{ $name }“ wurde importiert.
ng-keystore-import-busy = Während eines Signiervorgangs kann kein Keystore importiert werden.
ng-keystore-import-unreadable = Die Datei konnte nicht gelesen werden oder ist zu groß.
ng-keystore-import-format = Die Datei ist keine PKCS#12-Datei (.p12, .pfx).
ng-keystore-import-failed = Der Keystore konnte nicht in die App übernommen werden.
ng-share-failed = Das Dokument konnte nicht geteilt werden.
ng-export-done = „{ $name }“ wurde gesichert.
ng-export-failed = Das Dokument konnte nicht gesichert werden.
ng-mobile-unsupported = Auf diesem Gerät nicht verfügbar.
ng-mobile-not-implemented = Diese Funktion ist in dieser Version noch nicht verfügbar.
ng-mobile-cancelled = Abgebrochen.
ng-mobile-timeout = Das System hat nicht rechtzeitig geantwortet.
ng-mobile-native = Die Funktion des Betriebssystems ist fehlgeschlagen.
ng-ui-output-share = Teilen
    .accesskey = T
ng-ui-output-export = Sichern unter …
    .accesskey = u
ng-ui-output-mobile-hint = Das signierte Dokument liegt in den Dokumenten dieser App.
ng-ui-resume-title = Signatur fortsetzen?
ng-ui-resume-text = Die Signatur von „{ $name }“ wurde { $minutes ->
        [0] gerade eben
        [one] vor einer Minute
       *[other] vor { $minutes } Minuten
    } unterbrochen (die App wurde vom System beendet). Sie können sie mit derselben Position fortsetzen.
ng-ui-resume-again = Anmeldung und Bestätigung sind erneut nötig.
ng-ui-resume-pending-one = Danach folgt ein weiteres Dokument.
ng-ui-resume-pending = Danach folgen { $count } weitere Dokumente.
ng-ui-resume-continue = Fortsetzen
    .accesskey = F
ng-ui-resume-discard = Verwerfen
    .accesskey = V
ng-ui-keystore-import = PKCS#12-Datei importieren …
    .accesskey = I
ng-ui-keystore-import-title = PKCS#12-Keystore importieren
ng-ui-keystore-import-lead = Die Datei wird in den geschützten Bereich dieser App kopiert; das Original bleibt unverändert.
ng-ui-keystore-import-choose = Datei wählen …
    .accesskey = D
ng-ui-keystore-import-file = Gewählte Datei: { $name }
ng-ui-keystore-import-password = Passwort des Keystores
ng-ui-keystore-import-remember = Passwort sicher auf diesem Gerät speichern
ng-ui-keystore-import-start = Importieren
    .accesskey = m
ng-ui-keystore-import-done = Fertig
ng-ui-keystore-import-stored = Das Passwort ist im sicheren Speicher des Geräts abgelegt.
ng-ui-keystore-import-not-stored = Das Passwort wurde nicht gespeichert; es wird beim Signieren abgefragt.
ng-ui-keystore-current = Importierte Datei: { $name }
ng-ui-backend-atrust-desc-mobile = Signieren mit ID Austria: Bestätigung in der App „ID Austria“ oder per SMS-TAN.
ng-ui-backend-keystore-import-hint = Noch kein Keystore auf diesem Gerät.
ng-ui-fido2-unavailable-badge = FIDO2 nicht verfügbar
ng-ui-browse-mobile = Dokument wählen …
    .accesskey = w
ng-ui-dropzone-hint-mobile = Sie können PDF-Dokumente auch aus anderen Apps über „Teilen“ bzw. „Öffnen in“ an PIDA senden.
ng-ui-position-help-mobile = Tippen Sie auf die gewünschte Stelle oder ziehen Sie den Signaturblock mit dem Finger; er bleibt dabei über dem Finger sichtbar.

## Datenschutz ohne Zustimmungsdialog (ADR 0003): Kurzhinweis beim Signiermittel, Einstellungen › Datenschutz, CLI

ng-document-leaves-device = Hinweis: Das Dokument wird zum Signieren an A-Trust übermittelt (Datenschutzerklärung von A-Trust: https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf).
ng-ui-privacy-statement = Die Software PIDA verarbeitet die für den Signaturvorgang benötigten Daten lokal auf dem installierten Gerät. Der Signaturvorgang mit der Chipkarte wird lokal durchgeführt. Beim Signaturvorgang mittels ID Austria wird das zu signierende PDF-Dokument an die Firma A-Trust gesendet. Die Speicherdauer sowie weitere Datenschutzinformationen zur ID-Austria-Signatur entnehmen Sie bitte der Datenschutzerklärung der Firma A-Trust: <a>https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf</a>.
ng-ui-privacy-details = Details
ng-ui-privacy-details-label = Details zum Datenschutz
ng-ui-privacy-local = Mit einem Keystore oder der Chipkarte (lokale BKU) wird auf diesem Gerät signiert; das Dokument wird dabei nicht hochgeladen.
ng-ui-privacy-local-mobile = Mit einem Keystore wird auf diesem Gerät signiert; das Dokument wird dabei nicht hochgeladen.
ng-ui-privacy-atrust-link = Datenschutzerklärung von A-Trust: <a>https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf</a>
ng-ui-privacy-about = Wohin Ihre Dokumente beim Signieren gehen, steht im Abschnitt <a>Datenschutz</a>.
ng-ui-certcache-title = Zertifikats-Cache

## Neue Oberfläche „Arbeitsbereich“ (ADR 0004 §3.16): Meldungen des Kerns. Wortlaut nach ADR 0004 §8 F3 –
## nie „gültig“ oder „rechtsgültig“. UI-Texte der Pakete stehen in `ui-workspace.ftl` bzw. `ui-tools.ftl`.

ng-signer-unavailable = Das gewählte Signiermittel ist gerade nicht verfügbar. Bitte wählen Sie ein anderes.
ng-signer-localbku-not-running = Die Chipkarten-Software (lokale BKU) läuft nicht. Starten Sie sie und versuchen Sie es erneut.
ng-signer-keystore-missing = Es ist kein Keystore eingerichtet oder die Datei fehlt (Einstellungen › Signiermittel).
ng-signer-keystore-import = Auf diesem Gerät ist noch kein Keystore – importieren Sie eine PKCS#12-Datei.
ng-signer-policy = Dieses Signiermittel ist von Ihrer Organisation nicht freigegeben.
ng-signing-cancelled = Signieren abgebrochen – es wurde nichts signiert.
ng-signing-declined = In der App abgelehnt oder abgelaufen – es wurde nichts signiert.
    .title = In der App abgelehnt oder abgelaufen
    .action = Erneut versuchen
ng-constraint-signed = Das Dokument ist bereits signiert; eine neue Seite würde die vorhandenen Signaturen verändern.
ng-constraint-certified = Das Dokument ist zertifiziert; es darf nur in vorhandenen Signaturfeldern oder unsichtbar signiert werden.
ng-constraint-invisible-profile = Das gewählte Profil signiert unsichtbar (Einstellungen › Signaturbild).
ng-batch-added =
    { $count ->
        [one] Ein weiteres Dokument wurde hinzugefügt.
       *[other] { $count } weitere Dokumente wurden hinzugefügt.
    }
ng-batch-item-signed = „{ $file }“ wurde signiert.
ng-parked-files-added =
    { $count ->
        [one] Das Dokument wurde zur offenen Arbeit hinzugefügt.
       *[other] { $count } Dokumente wurden zur offenen Arbeit hinzugefügt.
    }
ng-open-duplicates =
    { $count ->
        [one] „{ $file }“ ist bereits geöffnet und wurde nicht noch einmal hinzugefügt.
       *[other] { $count } der gewählten Dokumente sind bereits geöffnet und wurden nicht noch einmal hinzugefügt.
    }
ng-open-missing =
    { $count ->
        [one] Eine gewählte Datei wurde nicht übernommen – sie ist nicht (mehr) vorhanden oder keine Datei.
       *[other] { $count } gewählte Dateien wurden nicht übernommen – sie sind nicht (mehr) vorhanden oder keine Dateien.
    }
ng-batch-done = { $signed } von { $total } Dokumenten signiert.
ng-batch-nothing-to-sign = Im Stapel ist kein Dokument zum Signieren bereit.
ng-batch-output-folder-required = Bitte wählen Sie vor dem Start einen Ordner für die signierten Dokumente.
ng-batch-aborted =
    { $signed ->
        [one] Stapel abgebrochen – ein Dokument wurde bereits signiert und gespeichert.
       *[other] Stapel abgebrochen – { $signed } Dokumente wurden bereits signiert und gespeichert.
    }
ng-batch-paused = Der Stapel ist angehalten. Bitte entscheiden Sie, wie es weitergeht.
ng-batch-large = { $count } Dokumente: Jedes Dokument wird einzeln in der App bestätigt.
ng-batch-pending-app-request = Eine noch offene Anfrage in der App lehnen Sie bitte ab; vergleichen Sie immer den Vergleichswert.
ng-verify-open-failed = Die Datei konnte nicht zum Prüfen geöffnet werden.
ng-verify-busy = Während des Signierens kann kein Dokument geprüft werden.
ng-verify-warning-sha1 = Die Signatur verwendet das veraltete Hashverfahren SHA-1; sie ist unverändert, gilt aber nach heutigem Stand nicht mehr als sicher.
ng-verify-warning-signing-certificate-v1 = Die Signatur bindet ihr Zertifikat nur über das ältere Attribut „signing-certificate“ (SHA-1).
ng-verify-warning-weak-key = Der Schlüssel dieser Signatur ist nach heutigem Stand zu kurz.
ng-recent-missing = Die Datei ist nicht mehr am gespeicherten Ort.
ng-recent-policy = „Zuletzt signiert“ ist von Ihrer Organisation abgeschaltet.
ng-migration-item-block = Signaturbild (Profil, Sprache, Hinweistext)
ng-migration-item-emblem = Bildmarke
ng-migration-item-username = Benutzername für ID Austria
ng-migration-item-output = Speicherort und Dateiname
ng-migration-item-keystore = Keystore
ng-migration-item-proxy = Proxy
ng-migration-item-placeholder = Platzhaltersuche
ng-migration-item-position = Automatische Positionierung
ng-migration-item-language = Sprache
ng-migration-not-found = Auf diesem Gerät wurden keine PDF-Over-Einstellungen gefunden.
ng-firstrun-done = Einrichtung abgeschlossen. Sie können alles jederzeit in den Einstellungen ändern.
ng-settings-field-unknown = Unbekannte Einstellung.
ng-settings-invalid-value = Dieser Wert ist für die Einstellung nicht gültig.
ng-settings-invalid-port = Der Port muss zwischen 1 und 65535 liegen.
ng-settings-policy = Diese Einstellung ist von Ihrer Organisation festgelegt.
ng-settings-unavailable-mobile = Diese Einstellung gibt es auf Mobilgeräten nicht.
ng-window-title = PIDA – PDF-Signatur ID Austria
ng-keystore-file-not-found = Die gewählte Keystore-Datei wurde nicht gefunden. Bitte wählen Sie sie erneut aus.
ng-keystore-password-needed = Bitte geben Sie das Passwort des Keystores ein.
ng-keystore-file-not-found-at = Die Keystore-Datei „{ $name }“ wurde nicht gefunden. Bitte wählen Sie sie unter Einstellungen › Signiermittel erneut aus.

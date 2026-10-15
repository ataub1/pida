### PDF-Over NG – eigene Texte, deutsch (EUPL-1.2)
###
### Von Hand gepflegt. Jede ID braucht einen Eintrag in `de/` und `en/` mit denselben Variablen
### (Test `crates/i18n/tests/code_catalog.rs`). Zwei Arten von IDs:
###
### - Eigene Texte (Paket E und UI): Präfix `ng-`, damit sie nie mit den aus PDF-Over erzeugten IDs
###   (`pdfover.ftl`) kollidieren.
### - Stabile Codes der Crates (ADR 0001 Regel 11, Plan §7.5): Die ID ist der Code selbst
###   (`cms-der`, `backend-network`, `fido2-timeout` …). Codes mit Punkt (`render.failed`,
###   `placement.notice.clamped`) sind in Fluent nicht zulässig; ihre ID entsteht wie bei den
###   PDF-Over-Keys mit `pdfover_i18n::fluent_id_for_java_key` (Punkt → Bindestrich:
###   `render-failed`, `placement-notice-clamped`).
###
### Variablen heißen wie die Felder der Fehlervariante (`$detail` für einen Freitext). Der Test prüft,
### dass jeder Code, den der Quelltext liefert (`fn code`, `fn message_id`, `NOTICE_*`, Code-Literale),
### hier in beiden Sprachen steht.

## Konfiguration: Fehler (`pdfover_config::ConfigError::message_id`)

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

## Konfiguration: Lesen von PDF-Over.config (`pdfover_config::LegacyWarning::code`)

ng-config-legacy-invalid-bku = BKU: Der Wert „{ $value }“ ist ungültig; es gibt keine Voreinstellung.
ng-config-legacy-invalid-proxy-port = PROXY_PORT: „{ $value }“ ist keine gültige Zahl und wird ignoriert.
ng-config-legacy-proxy-port-out-of-range = PROXY_PORT: { $value } liegt außerhalb von 1 bis 65535 und wird ignoriert.
ng-config-legacy-invalid-value = { $key }: Der Wert „{ $value }“ ist ungültig; der Standardwert gilt.
ng-config-legacy-unknown-locale = { $key }: „{ $value }“ ist keine gültige Sprache; die Systemsprache gilt.
ng-config-legacy-unsupported-locale = { $key }: Die Sprache „{ $value }“ wird nicht unterstützt; es gilt { $lang }.
ng-config-legacy-unknown-profile = SIGNATURE_PROFILE: „{ $value }“ ist unbekannt; es gilt der Standard-Signaturblock.
ng-config-legacy-unknown-pass-store-type = KEYSTORE_PASS_STORE_TYPE: „{ $value }“ ist unbekannt; Keystore-Passwörter werden nicht gespeichert.

## Konfiguration: Import aus ~/.pdf-over (`pdfover_config::migrate`)

ng-migrate-offer-title = Einstellungen von PDF-Over übernehmen?
ng-migrate-offer-text = Es wurden Einstellungen einer früheren PDF-Over-Installation gefunden („{ $path }“). Sollen sie übernommen werden? Die alte Datei bleibt unverändert.
ng-migrate-secrets-consent = In der alten Konfiguration sind Passwörter im Klartext gespeichert. Sollen sie in den Schlüsselbund des Betriebssystems übernommen werden?
ng-migrate-remove-plaintext = Die Passwörter stehen weiterhin im Klartext in „{ $path }“. Sie können sie dort entfernen lassen.
ng-migrate-done = Übernommen: { $imported } Einstellungen; nicht übernommen: { $skipped }.
ng-migrate-undo = Import rückgängig machen
ng-migrate-key-ignored = { $key } wird nicht übernommen (in PDF-Over NG ohne Funktion).
ng-migrate-unknown-key = Der unbekannte Eintrag { $key } wird ignoriert.
ng-migrate-path-missing = { $key }: Der Pfad „{ $path }“ existiert nicht.
ng-migrate-path-dropped = { $key }: Der Pfad „{ $path }“ existiert nicht und wird nicht übernommen.
ng-migrate-emblem-unsupported = EMBLEM: „{ $path }“ ist kein unterstütztes Bild (PNG, JPEG, GIF) und wird nicht übernommen.
ng-migrate-emblem-from-cache = EMBLEM: Das Original „{ $path }“ fehlt; die von PDF-Over zwischengespeicherte Bildmarke wird übernommen.
ng-migrate-secret-ignored = { $key }: Das Passwort wird nicht übernommen, weil die Speicherart nicht „Permanent“ ist.
ng-migrate-secret-no-target = { $key }: Das Passwort kann ohne zugehörige Keystore-Datei bzw. Proxy-Host und -Benutzer nicht zugeordnet werden.
ng-migrate-signature-field-discarded = Das in PDF-Over dauerhaft gespeicherte Signaturfeld wird verworfen (B-05); die Auswahl gilt künftig nur für das jeweilige Dokument.
ng-migrate-custom-profile = Eigenes PDF-AS-Profil gefunden: „{ $file }“.
ng-migrate-note-default = Der Signaturhinweis entspricht dem Standardtext und folgt künftig Profil und Sprache.
ng-migrate-placeholder-search-off = Die Platzhaltersuche war aktiv, aber weder QR-Code- noch Signaturfeld-Suche gewählt; sie bleibt ausgeschaltet.
ng-migrate-proxy-without-host = PROXY_PORT bzw. PROXY_USER ohne PROXY_HOST werden nicht übernommen.
ng-migrate-keystore-jks = Keystore-Typ { $value }: JKS- bzw. JCEKS-Keystores müssen vor der Verwendung in PKCS#12 umgewandelt werden.

## CMS-Signaturen (`pdfover_cades::CmsError::code`)

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

## Keystore (`pdfover_backend_keystore::KeystoreError::code`)

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

## Signatur-Backends (`pdfover_backend_api::BackendError::code`); Variablen = Felder der Variante

backend-cancelled = Der Vorgang wurde abgebrochen.
backend-unavailable = Das Signiermittel ist nicht verfügbar: { $detail }
backend-network = Die Verbindung zum Signaturdienst ist fehlgeschlagen: { $detail }
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
backend-internal = Interner Fehler: { $detail }

## Meldungen und Masken der Backends ohne PDF-Over-Key (`NoticeCode::AtrustInterstitial` als Titel,
## `FormProblem::LoginRejected`); Zuordnung aller Varianten: crates/i18n/tests/code_catalog.rs

ng-notice-atrust-interstitial = Hinweis von A-Trust
ng-form-login-rejected = Die Anmeldung wurde nicht angenommen. Bitte das Passwort erneut eingeben.

## ID Austria über A-Trust, Szenario A (`pdfover_backend_atrust`)

atrust-unknown-page = Unbekannte Seite des Signaturdienstes ({ $diagnostic }). Der Vorgang wurde abgebrochen.
atrust-signature-data-link-rejected = Der Link „Signaturdaten anzeigen“ führt nicht zu A-Trust und wird nicht geöffnet.

## FIDO2 (`pdfover_backend_atrust::Fido2Error::code` und Detailcodes der Provider)

fido2-unavailable = FIDO2 ist auf diesem Gerät nicht verfügbar.
fido2-cancelled = Vorgang am Sicherheitsschlüssel abgebrochen.
fido2-invalid-options = Die FIDO2-Anfrage des Signaturdienstes ist nicht lesbar.
fido2-device = Fehler am Sicherheitsschlüssel: { $detail }
fido2-pin-unsupported = Dieser Sicherheitsschlüssel verlangt eine PIN; die PIN-Eingabe ist auf diesem System noch nicht verfügbar – bitte App oder SMS verwenden.
fido2-no-credential = Auf diesem Sicherheitsschlüssel ist keine ID-Austria-Registrierung vorhanden.
fido2-timeout = Zeitüberschreitung: Der Sicherheitsschlüssel wurde nicht berührt.
fido2-locked = Der Sicherheitsschlüssel ist gesperrt.
fido2-u2f-only = Der Sicherheitsschlüssel unterstützt kein FIDO2 (nur U2F).
fido2-no-user-verification = Der Sicherheitsschlüssel kann die Nutzerin nicht verifizieren (keine PIN oder Biometrie eingerichtet).
fido2-hid-permission = Kein Zugriff auf den Sicherheitsschlüssel – die udev-Regel (uaccess) für FIDO-Geräte fehlt.

## Signaturblock (`pdfover_sigblock::SigBlockError::code`, `NOTICE_CHARS_REPLACED`)

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

## Darstellung (`pdfover_render::RenderError::code`, Code `render.…` → ID `render-…`)

render-library_not_found = Die PDF-Darstellungsbibliothek (PDFium) wurde nicht gefunden: { $details }
render-load_failed = Das Dokument konnte nicht geladen werden: { $details }
render-password_required = Das Dokument ist durch ein Passwort geschützt.
render-page_out_of_range = Seite { $page } existiert nicht (das Dokument hat { $count } Seiten).
render-failed = Die Seite konnte nicht dargestellt werden: { $details }

## Positionierung (`pdfover_placement::PlacementError::code`, `PlacementNotice::code`; `placement.…` → `placement-…`)

placement-invalid_position = Ungültige Positionsangabe: { $position }
placement-no_such_page = Seite { $page } existiert nicht.
placement-placeholder_not_found = Es wurde kein passender Signaturplatzhalter gefunden.
placement-not_enough_space = Für den Signaturblock ist nicht genug Platz, und auf bereits signierten Dokumenten darf keine neue Seite angefügt werden.
placement-notice-new_page_not_allowed = Das Dokument ist bereits signiert – der Signaturblock wurde auf der letzten Seite statt auf einer neuen Seite platziert.
placement-notice-overlaps_content = Der Signaturblock überdeckt Inhalt der Seite.
placement-notice-overlaps_widget = Der Signaturblock überdeckt ein vorhandenes Formular- oder Signaturfeld.
placement-notice-clamped = Die Position wurde an die Seite angepasst.
placement-notice-scan_failed = Der freie Platz auf der Seite konnte nicht ermittelt werden; der Signaturblock wurde oben auf der Seite platziert.

## Eingangsanalyse (`pdfover_pades::Finding::code`, Plan §6.1)

pdf-xref-repaired = Der Querverweis des Dokuments ist beschädigt und wurde beim Lesen korrigiert. Das Signieren ist möglich, ändert die Beschädigung aber nicht.
pdf-xref-reconstructed = Der Querverweis des Dokuments fehlt oder ist unlesbar. Das Dokument kann ohne Reparatur nicht signiert werden.
pdf-tail-garbage = Nach dem Dateiende-Marker (%%EOF) stehen fremde Daten.
pdf-docmdp-p1 = Das Dokument ist zertifiziert und erlaubt keine Änderungen (DocMDP P=1); es kann nicht signiert werden.
pdf-user-password = Das Dokument ist mit einem Kennwort zum Öffnen geschützt und kann nicht signiert werden.
pdf-xfa-dynamic = Das Dokument ist ein dynamisches XFA-Formular und kann nicht signiert werden.
pdf-usage-rights = Das Dokument enthält erweiterte Adobe-Reader-Rechte. Das Signieren macht sie ungültig; Adobe Reader zeigt danach eine Warnung.

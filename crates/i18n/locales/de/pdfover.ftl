### PIDA – deutscher Katalog (EUPL-1.2)
###
### Erzeugt mit `pida_i18n::convert::convert_pdfover_messages` aus `messages_de.properties` von PDF-Over
### (A-SIT, EUPL). NICHT von Hand bearbeiten: Korrekturen gehören in die Tabellen des Konverters,
### eigene Texte von PIDA in `pida.ftl`. Neu erzeugen:
### PIDA_I18N_BLESS=1 cargo test -p pida-i18n --test catalogs
###
### Konventionen: Fluent-ID = Java-Key mit „-“ statt „.“; `&`-Mnemonic → `.accesskey`;
### `%s`/`%d` → `{ $arg1 }`, `{ $arg2 }` … in Reihenfolge; `<a>…</a>` bzw. `<a href="…">…</a>`
### bleiben als Link-Markup im Text (`pida_i18n::markup`).

BKU-KS = Keystore
# angepasst (PIDA): Terminologie der Oberfläche („Chipkarte“, „Chipkarten-Software“)
BKU-LOCAL = Lokale Chipkarte
BKU-MOBILE = ID Austria
BKU-NONE = Keine Voreinstellung
# unused (in PDF-Over 4.4.x nicht referenziert)
BKUSelectionComposite-btnMobile-text = ID Austria
# unused (in PDF-Over 4.4.x nicht referenziert)
TrustedSocketFactory-FailedToCreateSecureConnection = Konnte keine sichere Verbindung erstellen
AdvancedConfigurationComposite-lblSaveFilePostFix-text = Ausgabedatei Postfix:
advanced_config-AutoPosition = Automatische Positionierung
advanced_config-AutoPosition_ToolTip = Aktivieren Sie diese Option, um die Signatur automatisch zu positionieren
advanced_config-BKUSelection_Title = BKU-Auswahl
    .accesskey = B
advanced_config-BKUSelection_ToolTip = Wählen Sie die BKU, mit der die Signatur durchgeführt werden soll
advanced_config-KeystoreEnabled = Keystore-Signatur aktivieren
advanced_config-KeystoreEnabled_ToolTip = Aktiviert die Möglichkeit, mit einem lokalen Keystore zu signieren
advanced_config-LocaleSelection_Title = Interface-Sprache
    .accesskey = p
advanced_config-LocaleSelection_ToolTip = Wählen Sie die Sprache für das Benutzerinterface
advanced_config-OutputFolder = Standard-Ausgabeordner:
advanced_config-OutputFolder-Dialog = Wählen Sie einen Ordner
advanced_config-OutputFolder-Dialog_Title = Ausgabeordner wählen
advanced_config-OutputFolder_Title = Ausgabe Einstellungen
advanced_config-OutputFolder_ToolTip = Wählen Sie einen Ordner, in dem signierte Dokumente automatisch abgelegt werden (deaktivert, wenn das Feld leer ist; ein einzelner Punkt (".") wählt das Verzeichnis des Ursprungsdokuments)
advanced_config-PdfACompat = PDF/A-Kompatibilität
advanced_config-PdfACompat_ToolTip = Ob die Signatur PDF/A-kompatibel sein soll (erhöht PDF-Größe). Diese Option setzt voraus, dass das original Dokument bereits im PDF/A Format ist.
advanced_config-ProxyHost = Host:
advanced_config-ProxyHost_Template = Hostname oder IP des Proxy-Servers
advanced_config-ProxyHost_ToolTip = Um einen Proxy zu verwenden Hostname/IP hier eingeben
advanced_config-ProxyPass = Passwort:
advanced_config-ProxyPass_Template = Passwort für den Proxy-Server
advanced_config-ProxyPass_ToolTip = Für Proxy-Authentifizierung Passwort hier eingeben
advanced_config-ProxyPort = Port:
advanced_config-ProxyPort_Template = Port des Proxy-Servers [1-65535]
advanced_config-ProxyPort_ToolTip = Um einen Proxy zu verwenden Portnummer hier eingeben
advanced_config-ProxyUser = Benutzer:
advanced_config-ProxyUser_Template = Benutzername für den Proxy-Server
advanced_config-ProxyUser_ToolTip = Für Proxy-Authentifizierung Benutzername hier eingeben
advanced_config-Proxy_Title = Proxy
    .accesskey = P
# unused (in PDF-Over 4.4.x nicht referenziert)
advanced_config-SigPHTransparency = Signaturplatzhalter-Transparenz
# unused (in PDF-Over 4.4.x nicht referenziert)
advanced_config-SigPHTransparencyMax = Undurchsichtig
# unused (in PDF-Over 4.4.x nicht referenziert)
advanced_config-SigPHTransparencyMin = Unsichtbar
# unused (in PDF-Over 4.4.x nicht referenziert)
advanced_config-SigPHTransparencyTooltip = Wählt die Transparenz des Signaturplatzhalters im Schritt "Positionierung"
advanced_config-Signature_Title = Signatur
    .accesskey = g
advanced_config-Placeholder_Title = Platzhalter
advanced_config-Placeholder_Enabled = Platzhalter Suche aktivieren
advanced_config-UpdateCheck = Automatisch auf Updates überprüfen
# unused (in PDF-Over 4.4.x nicht referenziert)
advanced_config-UpdateCheck_Title = Updates
advanced_config-UpdateCheck_ToolTip = Zeigt eine Benachrichtigung, wenn eine neue Version verfügbar ist
advanced_config-UseMarker = Nach QR-Code Signaturplatzhaltern suchen
advanced_config-UseMarker_ToolTip = Aktivieren Sie diese Option, um das Dokument nach Signaturplatzhaltern zu durchsuchen. Sollte ein Platzhatler gefunden werden werden Sie gefragt ob Sie die Signatur in den Platzhalter einfügen wollen.
advanced_config-UseSignatureFields = Nach Signaturfelder-Platzhaltern suchen
advanced_config-UseSignatureFields_ToolTip = Aktivieren Sie diese Option, um das Dokument nach Signaturplatzhaltern zu durchsuchen. Sollte ein Platzhatler gefunden werden werden Sie gefragt ob Sie die Signatur in den Platzhalter einfügen wollen.
argument-error-output = ist kein Ordner
argument-help-autopos = Aktiviert automatische Positionierung der Signatur
argument-help-bku = Wählen Sie die BKU. Gültige Werte: LOCAL, MOBILE, KS. Bsp.: -b MOBILE (für die Verwendung der ID Austria)
argument-help-config = Auswahl der Konfigurationsdatei. Bsp.: -c <Dateiname>
argument-help-emblem = Wählt die Bildmarke für die Signatur. Bsp.: -e <Dateiname>
argument-help-help = Zeigt diese Hilfe an
argument-help-input = Wählt das zu signierende Dokument. Bsp.: -i <Dateiname>
argument-help-keystorealias = Wählt das Keystore Key-Alias. Bsp.: -ksalias <Alias>
argument-help-keystorefile = Wählt die Keystore-Datei. Bsp.: -ks <Dateiname>
argument-help-keystorekeypass = Wählt das Keystore Key-Password. Bsp.: -kskeypass <Passwort>
argument-help-keystorestorepass = Wählt das Keystore-Password. Bsp.: -kspass <Passwort>
argument-help-keystoretype = Wählt den Keystore-Typ. Bsp.: -kstype [PKCS12|JKS|JCEKS]
argument-help-number = Wählt die Telefonnummer bzw. den Benutzernamen für die ID Austria. Bsp.: -n <Nummer>
argument-help-output = Wählt den Ausgabeordner oder die Ausgabedatei. Bsp.: -o <Ordner>
argument-help-password = Wählt das Passwort für die ID Austria. Bsp.: -p <Passwort>
argument-help-proxyhost = Wählt den Proxy-Server. Bsp.: -proxy <Hostname/IP>
argument-help-proxypass = Wählt das Proxy-Passwort. Bsp.: -proxypass <Passwort>
argument-help-proxyport = Wählt den Proxy-Server-Port. Bsp.: -proxyport <Portnummer>
argument-help-proxyuser = Wählt den Proxy-Benutznamen. Bsp.: -proxyuser <Benutzername>
argument-help-skipfinish = Aktiviert das Überspringen des "Fertig"-Dialogs (falls Speichern erfolgreich)
argument-info-help = Die folgenden Optionen sind verfügbar:
argument-invalid-bku = BKU-Auswahl ungültig! Verwendung:
argument-invalid-config = Konfigurationsdatei ungültig! Verwendung:
argument-invalid-emblem = Bildmarke ungültig! Verwendung:
argument-invalid-input = Dokument ungültig! Verwendung:
argument-invalid-keystorealias = Keystore-Alias ungültig! Verwendung:
argument-invalid-keystorefile = Keystore-Datei ungültig! Verwendung:
argument-invalid-keystorekeypass = Keystore Key-Passwort ungültig! Verwendung:
argument-invalid-keystorestorepass = Keystore-Passwort ungültig! Verwendung:
argument-invalid-keystoretype = Keystore-Typ ungültig! Verwendung:
argument-invalid-number = Telefonnummer ungültig! Verwendung:
argument-invalid-output = Ausgabeordner ungültig! Verwendung:
argument-invalid-password = ID Austria-Passwort ungültig! Verwendung:
argument-invalid-proxyhost = Proxy-Host ungültig! Verwendung:
argument-invalid-proxypass = Proxy-Passwort ungültig! Verwendung:
argument-invalid-proxyport = Proxy-Port ungültig! Verwendung:
argument-invalid-proxyuser = Proxy-Benutzername ungültig! Verwendung:
bku_selection-card = Karte
    .accesskey = K
bku_selection-ks = KeyStore
    .accesskey = S
bku_selection-mobile = ID-A
    .accesskey = A
common-AllExtension_Description = Alle Dateien
common-Cancel = Abbrechen
    .accesskey = A
common-GIFExtension_Description = GIF-Bilder
common-ImageExtension_Description = Alle unterstützen Bilder
common-JPGExtension_Description = JPG-Bilder
common-KeystoreExtension_Description = Keystores
common-KSExtension_Description = Java Keystores
common-Ok = OK
    .accesskey = K
common-PDFExtension_Description = PDF-Dokumente
common-PKCS12Extension_Description = PKCS12 Keystores
common-PNGExtension_Description = PNG-Bilder
common-Save = Speichern
    .accesskey = S
common-browse = Durchsuchen
    .accesskey = D
common-info = Information
common-open = Öffnen
common-warning = Warnung
common-working = In Arbeit...
common-unavailable = Nicht verfügbar
config-About = Über { $arg1 }
    .accesskey = b
config-AboutText =
    Diese Software wird von A-SIT unter den Bedingungen der EUPL frei zur Verfügung gestellt. Lizenbedingungen unter <a>https://technology.a-sit.at/lizenzbedingungen/</a>. Technischer Support unter <a href="mailto:software@egiz.gv.at">software@egiz.gv.at</a>.

    Beachten Sie, dass Komponenten unter eigenen Lizenzen zur Verfügung gestellt werden, die teilweise nur für EUPL-lizensierte Software zur freien Verwendung vorgesehen sind.
config-DataProtection = Datenschutzinformation
# angepasst (PIDA): Terminologie der Oberfläche („Chipkarte“, „Chipkarten-Software“)
config-DataProtectionStatement = Die Software PDF-Over verarbeitet für den Signaturvorgang benötigte Daten lokal auf dem installierten Gerät.  Der Signaturvorgang mit der Chipkarte wird lokal durchgeführt.  Beim Signaturvorgang mittels ID-Austria wird das zu signierende PDF Dokument an die Firma A-Trust gesendet.  Die Speicherdauer sowie weitere Datenschutzinformationen zur ID Austria-Signatur entnehmen Sie bitte der Datenschutzerklärung der Firma A-Trust: <a>https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf</a>.
config-DataProtectionURL = https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf
config-FIDO2 = FIDO2
    .accesskey = F
config-Advanced = Erweitert
    .accesskey = w
config-Keystore = Keystore
    .accesskey = K
config-Simple = Einfach
    .accesskey = E
config-ShowLogDirectory = Logs anzeigen
config-fido2-AboutFIDO =
    Nutzer:innen mit ID Austria können einen Hardware-Sicherheitstoken als Ersatz zum Smartphone zur Auslösung von PDF-Signaturen verwenden.

    Die Eingabe von Benutzername und Passwort ist weiterhin notwendig, aber die Freigabe über die App wird durch die Freigabe über den Hardware-Token ersetzt.

    Weitere Informationen erhalten Sie auf <a>der Informationsseite der ID-Austria</a>.
config-fido2-UnsupportedTryLibFido2 =
    Für die Verwendung von FIDO2-Tokens wird benötigt:
    {" "}● <a>LibFido2</a>

    Bitte installieren Sie die o.a. Voraussetzungen und starten sie dann PDF-Over neu.
config-fido2-UnsupportedPlatform = Ihr System unterstützt keine Authentifizierung mittels FIDO2-Token.
config-fido2-InfoURL = https://www.oesterreich.gv.at/id-austria/haeufige-fragen/allgemeines-zu-id-austria#fido
config-fido2-Group = FIDO2-Einstellungen
config-fido2-ByDefault = FIDO2 standardmäßig auswählen
dataSourceSelection-DropLabel = Dokument hierher ziehen
dataSourceSelection-DropLabel2 = oder...
dataSourceSelection-browse = Durchsuchen um ein Dokument zu wählen
    .accesskey = D
dataSourceSelection-usePlaceholderTitle = Info
dataSourceSelection-usePlaceholderText = Im vorliegenden Dokument wurde ein Signaturplatzhalter gefunden. Soll dieser verwendet werden?
error-CmdLineArgs = Fehler bei den Befehlszeilen-Parametern:
# unused (in PDF-Over 4.4.x nicht referenziert)
error-Details = Details
error-EnteredReferenceValue = Sie haben den Vergleichswert eingegeben!
# unused (in PDF-Over 4.4.x nicht referenziert)
error-FailedToGetSignedDocument = Konnte signiertes Dokument nicht erhalten.
error-FailedToLoadEmblem = Konnte Bildmarke nicht speichern
error-FailedToLoadQRCode = Konnte den QR code nicht laden
error-FailedToOpenDocument = Konnte signiertes Dokument nicht öffnen: { $arg1 }.
error-FailedToSaveSettings = Konnte Konfigurationsdatei nicht speichern!
error-FileNotExist = Datei { $arg1 } existiert nicht!
error-IOError = Ein-/Ausgabe-Fehler
# korrigiert: Tippfehler „bittem un“ (docs/analyse/01 §6.2)
error-Initialization =
    Initialisierung fehlgeschlagen.
    Wir bitten um Verzeihung für die Unannehmlichkeiten.

    Eventuell lässt sich Ihr Problem beheben, indem Sie PDF-Over auf Werkseinstellungen zurücksetzen.
    Möchten Sie dies nun versuchen?
# korrigiert (B-09): zweites Vorkommen von error.Initialization in messages_de.properties
# korrigiert: Tippfehler „bittem un“ (docs/analyse/01 §6.2)
error-ConfigInitialization =
    Der Einstellungsbildschirm konnte nicht geladen werden.
    Wir bitten um Verzeihung für die Unannehmlichkeiten.

    Eventuell lässt sich Ihr Problem beheben, indem Sie PDF-Over auf Werkseinstellungen zurücksetzen.
    Möchten Sie dies nun versuchen?
error-InvalidBKU = Ungültige BKU-Auswahl. Bitte überprüfen.
# unused (in PDF-Over 4.4.x nicht referenziert)
error-InvalidLocale = Ungültige Sprache
error-InvalidPhoneNumber = Telefonnummer ungültig! Beispiel: +43664123456789
# unused (in PDF-Over 4.4.x nicht referenziert)
error-InvalidSettings = Ungültige Einstellungen vorhanden. Bitte überprüfen.
error-KeyStore = Fehler beim Laden des KeyStores. Falsches Passwort?
error-KeyStoreAliasExist = Key-Alias { $arg1 } nicht im Keystore gefunden
error-KeyStoreAliasNoKey = Alias { $arg1 } ist kein Schlüssel
error-KeyStoreFileNotExist = Keystore-Datei { $arg1 } existiert nicht!
error-KeyStoreStorePass = Keystore-Passwort nicht akzeptiert
error-KeyStoreKeyPass = Schlüssel-Passwort nicht akzeptiert
# angepasst (PIDA): Terminologie der Oberfläche („Chipkarte“, „Chipkarten-Software“)
error-LocalBKU =
    Bitte prüfen sie, ob Ihre lokale Chipkarten-Software (BKU) läuft

    Sie benötigen eine BKU, um auf Ihre Chipkarte zuzugreifen. Weitere Informationen unter www.buergerkarte.at
error-MayNotBeAPDF = Dies ist möglicherweise keine PDF-Datei
error-NoTan = Keine TAN eingeben
# unused (in PDF-Over 4.4.x nicht referenziert)
error-PDFProtected = Diese PDF-Datei ist verschlüsselt und kann daher nicht signiert werden
error-PDFPwdProtected = Diese PDF-Datei ist Passwortgeschützt und kann daher nicht signiert werden
error-PositioningNotPossible = Positionsauswahl ist im Moment nicht verfügbar wegen eines Java Fehlers. Die Position wird automatisch bestimmt.
error-PrepareDocument = Konnte Dokument nicht zur Signatur vorbereiten.
# unused (in PDF-Over 4.4.x nicht referenziert)
error-Retry = Wiederholen
# unused (in PDF-Over 4.4.x nicht referenziert)
error-SWTLib = Fehler beim Laden der SWT-Bibliothek
# unused (in PDF-Over 4.4.x nicht referenziert)
error-SaveOutputFolder = Konnte Dokument nicht in den konfigurierten Ausgabeordner speichern.
error-Signatur = Signatur-Fehler
error-TanTooLong = Eingegebene TAN zu lang
error-SLError =
    Vorgang vom Signaturserver abgebrochen ({ $arg1 }):
    { $arg2 }
error-Title = Fehler
# unused (in PDF-Over 4.4.x nicht referenziert)
error-TitleFatal = Fataler Fehler
error-Unexpected = Unerwarteter Fehler
error-ATrustConnection = Verbindung zu A-Trust konnte nicht aufgebaut werden
error-CouldNotResolveHostname =
    Server '{ $arg1 }' wurde nicht gefunden.
    Überprüfen Sie Ihre Internetverbindung.
error-FailedToConnect =
    Verbindung zum Signaturserver fehlgeschlagen:
    { $arg1 }
error-ProxyAuthRequired =
    HTTP-Proxy lehnt Verbindung ab:
    HTTP/1.1 407 Proxy Authentication Required
exception-InvalidEmblemFile = { $arg1 } ist eine ungültige Bildmarken-Datei!
exception-InvalidPort = { $arg1 } ist ungültig: muss eine Nummer zwischen { $arg2 } und { $arg3 } sein.
exception-PasswordTooLong = Eingegebenes Passwort ist zu lange!
exception-PasswordTooShort = Eingegebenes Passwort ist zu kurz!
exception-PathNotDirectory = Pfad { $arg1 } ist kein gültiger Ordner!
exception-PathNotExist = Pfad { $arg1 } existiert nicht!
keystore-KeystoreStorePassEntry = Keystore-Passwort eingeben:
keystore-KeystoreKeyPassEntry = Schlüssel-Passwort eingeben:
keystore_config-Keystore_Title = Keystore
keystore_config-KeystoreAlias = Schlüssel-Alias
keystore_config-KeystoreFile = Keystore-Datei
keystore_config-KeystoreFile_ToolTip = Pfad zur Keystore-Datei
# unused (in PDF-Over 4.4.x nicht referenziert)
keystore_config-KeystoreFile-Dialog = Wählen Sie den Keystore
# unused (in PDF-Over 4.4.x nicht referenziert)
keystore_config-KeystoreFile-Dialog_Title = Keystore wählen
keystore_config-SaveToWhere-Header = Keystore-Passwörter speichern?
keystore_config-SaveToWhere-None = Nicht speichern (Eingabe bei jeder Signatur)
keystore_config-SaveToWhere-Memory = Temporär speichern (Eingabe bei der ersten Signatur)
keystore_config-SaveToWhere-Disk = Permanent speichern (einmalige Eingabe)
keystore_config-KeystoreKeyPass = Schlüssel-Passwort
keystore_config-KeystoreKeyPass_ToolTip = Achtung: das Passwort wird im Klartext gespeichert.
keystore_config-KeystoreStorePass = Keystore-Passwort
keystore_config-KeystoreStorePass_ToolTip = Achtung: das Passwort wird im Klartext gespeichert.
keystore_config-KeystoreType = Keystore-Typ
keystore_config-KeystoreType_JKS = Java-Keystore
keystore_config-KeystoreType_PKCS12 = PKCS12
keystore_config-Load = Keystore laden
keystore_config-Load_ToolTip = Keystore laden, um die verfügbaren Schlüssel-Aliases anzuzeigen
main-about = Über { $arg1 }
main-configuration = Konfiguration
main-done = Fertig
main-hide = { $arg1 } ausblenden
main-position = Positionierung
main-quit = { $arg1 } beenden
main-signature = Signatur
mobileBKU-aTrustDisclaimer = Dieses Service wird von der Fa. A-Trust zu Verfügung gestellt
# unused (in PDF-Over 4.4.x nicht referenziert)
mobileBKU-notice = Nachricht vom Server:
mobileBKU-number = Benutzername:
mobileBKU-password = Passwort:
mobileBKU-show = <a>Signaturdaten anzeigen</a>
mobileBKU-show_tooltip = Zeigt die zu signierenden Daten an
# unused (in PDF-Over 4.4.x nicht referenziert)
mobileBKU-tan_tries_exceeded = Zu viele TAN-Versuche, neue TAN anfordern?
# unused (in PDF-Over 4.4.x nicht referenziert)
mobileBKU-wrong_tan = TAN nicht akzeptiert
mobileBKU-rememberPassword = Daten merken
    .accesskey = m
mobileBKU-rememberPasswordNote = Wählen Sie diese Box aus, um bis zum nächsten Start von PDF-Over nicht erneut gefragt zu werden
mobileBKU-authorize = Autorisierung starten
output-file_ask_overwrite = Datei { $arg1 } existiert bereits, wollen Sie sie überschreiben?
output-link_open_message = Sie können das signierte Dokument <a>hier</a> öffnen.
output-link_save_message = Sie können das signierte Dokument speichern
output-save_failed = Datei { $arg1 } konnte nicht gespeichert werden: { $arg2 }
output-success_message = Signatur war erfolgreich
positioning-newPage = Neue Seite anlegen
    .accesskey = N
positioning-page = Seite { $arg1 } von { $arg2 }
positioning-placeholder = Bitte wählen Sie den gewünschten Platzhalter aus der Liste aus:
positioning-removeNewPage = Neue Seite rückgängig
    .accesskey = N
positioning-sign = Signieren
    .accesskey = S
# unused (in PDF-Over 4.4.x nicht referenziert)
positioning-signature = Signatur
simple_config-ReplaceEmblem = Logo auswählen
    .accesskey = o
simple_config-ClearEmblem = Logo zurücksetzen
    .accesskey = r
simple_config-Preview_Title = Vorschau
simple_config-ExampleNumber = +43676123456789
simple_config-ExampleNumber_ToolTip = Um die Handynummer bzw. den Benutzernamen voreinzustellen hier eingeben
simple_config-MobileBKU_Title = ID Austria
simple_config-Note = Text:
simple_config-Note_Default_Standard = Dieses mit einer qualifizierten elektronischen Signatur versehene Dokument hat gemäß Art. 25 Abs. 2 der Verordnung (EU) Nr. 910/2014 vom 23. Juli 2014 ("eIDAS-VO") die gleiche Rechtswirkung wie ein handschriftlich unterschriebenes Dokument.
simple_config-Note_Default_OfficialSignature = Dieses Dokument wurde amtssigniert. Auch ein Ausdruck dieses Dokuments hat gemäß § 20 E-Government-Gesetz die Beweiskraft einer öffentlichen Urkunde.
# unused (in PDF-Over 4.4.x nicht referenziert)
simple_config-Note_Default_Empty = { "" }
simple_config-Note_SetDefault = Standard wiederherstellen
simple_config-Note_Title = Signaturhinweis
    .accesskey = n
simple_config-Note_Tooltip = Ein optionaler Hinweis, der in Ihrer Signatur angezeigt wird
simple_config-PhoneNumber = Benutzername:
simple_config-SigBlockLang_Title = Sprache des Signaturblocks:
    .accesskey = r
simple_config-SigBlockLang_ToolTip = Wählen Sie eine Sprache für den Signaturblock, der am signierten Dokument erscheint
simple_config-SigProfile_Title = Signatur-Profil
simple_config-SIGNATURBLOCK_SMALL = Standard Signaturblock
simple_config-AMTSSIGNATURBLOCK = Amtssignatur
simple_config-BASE_LOGO = Nur Bildmarke
simple_config-INVISIBLE = Unsichtbar
simple_config-LogoOnlyTargetSize_Title = Größe der Bildmarke
tanEnter-QR = QR Code:
tanEnter-FP =
    Bitte öffnen Sie die ID Austria-App
    und bestätigen Sie die Signatur.
tanEnter-ReferenceValue = Vergleichswert:
tanEnter-SMS = SMS anfordern
    .accesskey = S
tanEnter-FIDO2 = FIDO2
    .accesskey = F
tanEnter-TAN = TAN:
# unused (in PDF-Over 4.4.x nicht referenziert)
tanEnter-APPTAN =
    Bitte öffnen Sie die ID Austria-App
    TAN:
# unused (in PDF-Over 4.4.x nicht referenziert)
tanEnter-tries = { $arg1 } Versuche übrig!
# unused (in PDF-Over 4.4.x nicht referenziert)
tanEnter-try = Nur noch 1 Versuch übrig!
version_check-UpdateText =
    Version { $arg1 } ist verfügbar
    Jetzt Download-Seite öffnen?
version_check-UpdateTitle = Neue Version verfügbar!
waiting-message = Signaturerstellung läuft...
# unused (in PDF-Over 4.4.x nicht referenziert)
waiting_for_app-message = Bitte öffnen Sie die ID Austria-App!
config-UpdateStatus-NOT_CHECKED = Keine Versions-Infos. <a>Jetzt abfragen</a>?
config-UpdateStatus-CHECKING = Versions-Infos werden abgefragt...
config-UpdateStatus-OUTDATED = Neue Version verfügbar. <a>Herunterladen</a>?
config-UpdateStatus-UP_TO_DATE = Ihre Version ist aktuell. <a>Nochmals prüfen</a>?
config-UpdateStatus-FAILED = Versions-Check fehlgeschlagen. <a>Nochmal versuchen</a>?
atrusterror-http_413 =
    a-trust.at lehnt Anfrage ab:
    Dateigröße des PDF-Dokuments überschreitet Grenzwert.
atrusterror-http_generic =
    a-trust.at lehnt Anfrage ab:
    HTTP { $arg1 } { $arg2 }
atrusterror-message =
    a-trust.at meldet:
    { $arg1 }
# unused (in PDF-Over 4.4.x nicht referenziert)
# ergänzt (PIDA): fehlte in dieser Sprache
PlaceholderSelectionGui-lblNewLabel-text = Neue Beschriftung
# unused (in PDF-Over 4.4.x nicht referenziert)
# ergänzt (PIDA): fehlte in dieser Sprache
PlaceholderSelectionGui-shlInfo-text = Info
# ergänzt (PIDA): fehlte in dieser Sprache
argument-help-vis = Macht die Signatur unsichtbar (die Werte false, 0, no, nein oder off lassen sie sichtbar). Bsp.: -v true
# ergänzt (PIDA): fehlte in dieser Sprache
argument-invalid-vis = Sichtbarkeits-Argument ungültig! Verwendung:

### PIDA – Texte der neuen Oberfläche, UI-B (Einstellungen, Prüfmodus, Befehlspalette, Kürzel, Ersteinrichtung, Zuletzt signiert), deutsch (EUPL-1.2)
###
### Von Hand gepflegt (ADR 0004 §4.5). IDs: `ng-ui-<bereich>-<name>`, Bereiche dieser Datei:
### set, vf, pal, sh, ob, recent. Alle Kataloge einer Sprache bilden ein Fluent-Bündel: keine ID aus `pida.ftl`,
### `pdfover.ftl` oder der anderen `ui-*.ftl` wiederverwenden; jede ID mit denselben Variablen in
### `de/` und `en/` (Test `crates/i18n/tests/catalogs.rs`). Einzahl/Mehrzahl über Selektoren.
###
### Texte aus dem Prototyp (`prototypes/pida-workspace/src/i18n/de.ts`), angepasst an den Kern. Die Zeilen der
### Einstellungen tragen Synonyme für die Suche als Attribut `.keywords` (Leerzeichen getrennt, klein).

## Einstellungen: Rahmen, Suche, Bereiche (ADR 0004 §3.11)

ng-ui-set-title = Einstellungen
ng-ui-set-instant = Änderungen gelten sofort.
ng-ui-set-search = Einstellungen durchsuchen
ng-ui-set-search-clear = Suche löschen
ng-ui-set-search-empty = Keine Einstellung gefunden für „{ $q }“.
ng-ui-set-search-hits =
    { $n ->
        [0] Keine Treffer
        [one] Ein Treffer
       *[other] { $n } Treffer
    }
ng-ui-set-sections = Bereiche
ng-ui-set-sec-signature = Signaturbild
ng-ui-set-sec-document = Dokument
ng-ui-set-sec-signers = Signiermittel
ng-ui-set-sec-save = Speichern
ng-ui-set-sec-appearance = Darstellung
ng-ui-set-sec-privacy = Datenschutz
ng-ui-set-sec-advanced = Erweitert
ng-ui-set-sec-about = Über PIDA
ng-ui-set-policy = Von Ihrer Organisation vorgegeben.

## Einstellungen › Signaturbild (Live-Editor)

ng-ui-set-preview = Vorschau
    .keywords = signaturbild signaturblock vorschau aussehen
ng-ui-set-preview-alt = Vorschau des Signaturblocks: { $profile }, { $lang }
ng-ui-set-preview-alt-logo = Vorschau des Signaturblocks: { $profile }
ng-ui-set-preview-invisible = Unsichtbare Signatur – auf dem Dokument erscheint kein Signaturbild.
ng-ui-set-profile = Profil
    .keywords = profil amtssignatur standard behörde amt unsichtbar
ng-ui-set-profile-small = Standard (klein)
ng-ui-set-profile-small-desc = Unterzeichner, Datum, Prüfhinweis
ng-ui-set-profile-amt = Amtssignatur
ng-ui-set-profile-amt-desc = Für Behörden, mit Aussteller und Seriennummer
ng-ui-set-profile-logo = Nur Bildmarke
ng-ui-set-profile-logo-desc = Die Bildmarke allein, ohne Text
ng-ui-set-profile-invisible = Unsichtbar
ng-ui-set-profile-invisible-desc = Signatur ohne sichtbares Signaturbild
ng-ui-set-profile-more = Weitere Profile
ng-ui-set-block-lang = Sprache des Signaturblocks
    .keywords = sprache blocksprache deutsch englisch english
ng-ui-set-lang-system = Wie System
ng-ui-set-lang-de = Deutsch
ng-ui-set-lang-en = English
ng-ui-set-emblem = Bildmarke
    .keywords = logo bildmarke emblem bild siegel grafik firmenlogo
ng-ui-set-emblem-default = Standard-Bildmarke (PIDA-Siegel)
ng-ui-set-emblem-current = Aktuell: { $name }
ng-ui-set-emblem-hint = Bild hierher ziehen · PNG, JPEG oder GIF. Das Bild bleibt auf diesem Gerät.
ng-ui-set-emblem-choose = Datei wählen …
ng-ui-set-emblem-reset = Standard verwenden
ng-ui-set-emblem-dialog = Bildmarke wählen
ng-ui-set-logo-size = Größe der Bildmarke
    .keywords = logo größe bildmarke zentimeter
ng-ui-set-note = Hinweistext
    .keywords = hinweis text hinweistext eidas rechtswirkung
ng-ui-set-note-hint = Leer lassen für einen Block ohne Hinweis.
ng-ui-set-note-reset = Standardtext wiederherstellen

## Einstellungen › Dokument (PDF/A, Platzhaltersuche wie PDF-Over USE_MARKER/USE_FIELDS)

ng-ui-set-pdfa = PDF/A-kompatibel signieren
    .keywords = pdf/a pdfa archiv archivierung langzeit langzeitarchivierung konform iso 19005
ng-ui-set-pdfa-hint = PDF/A-Dokumente erkennt PIDA automatisch und signiert sie PDF/A-konform. Diese Option erzwingt das auch für Dokumente ohne PDF/A-Kennung.
ng-ui-set-placeholder = Signaturplatzhalter automatisch suchen
    .keywords = platzhalter signaturplatzhalter qr qr-code signaturfeld signaturfelder feld formular acroform marker automatisch
ng-ui-set-placeholder-hint = Ein gefundener Platzhalter bestimmt beim Öffnen, wo die Signatur hinkommt.
ng-ui-set-placeholder-kind = Suchen nach
ng-ui-set-placeholder-qr = QR-Code-Platzhalter
ng-ui-set-placeholder-qr-desc = Die Signatur kommt auf den QR-Code im Dokument.
ng-ui-set-placeholder-field = Leere Signaturfelder
ng-ui-set-placeholder-field-desc = Die Signatur kommt in ein leeres Feld des PDF-Formulars.

## Einstellungen › Signiermittel

ng-ui-set-default = Standard-Signiermittel
    .keywords = standard signiermittel voreinstellung vorauswahl id austria keystore chipkarte bürgerkarte fragen
ng-ui-set-signer-ida = ID Austria
ng-ui-set-signer-ida-desc = Bestätigung in der App, mit FIDO2-Schlüssel oder per SMS-TAN
ng-ui-set-signer-ida-desc-mobile = Bestätigung in der App „ID Austria“ oder per SMS-TAN
ng-ui-set-signer-keystore = Keystore
ng-ui-set-signer-keystore-desc = Zertifikatsdatei (PKCS#12) auf diesem Gerät
ng-ui-set-signer-keystore-off = Zuerst unten den Keystore einrichten.
ng-ui-set-signer-card = Chipkarte
ng-ui-set-signer-card-desc = Karte im Lesegerät mit lokaler Chipkarten-Software
ng-ui-set-signer-ask = Jedes Mal fragen
ng-ui-set-signer-ask-desc = Signiermittel bei jedem Dokument wählen
ng-ui-set-ida-user = ID Austria: Telefonnummer oder Benutzername
    .keywords = id austria telefon telefonnummer handy mobil benutzer benutzername konto anmeldung
ng-ui-set-ida-remember = Benutzername merken
ng-ui-set-ida-remember-hint = Gespeichert wird nur der Benutzername, nie das Passwort.
ng-ui-set-fido2 = FIDO2-Sicherheitsschlüssel bevorzugen
    .keywords = fido2 fido sicherheitsschlüssel schlüssel yubikey token
ng-ui-set-fido2-hint = Überspringt die App-Bestätigung, wenn A-Trust FIDO2 anbietet.
ng-ui-set-keystore = Keystore
    .keywords = keystore pkcs#12 pkcs12 p12 pfx zertifikat zertifikatsdatei alias schlüssel
ng-ui-set-keystore-enabled = Keystore verwenden
ng-ui-set-keystore-file = Keystore-Datei
ng-ui-set-keystore-none = Keine Datei gewählt
ng-ui-set-keystore-choose = Datei wählen …
ng-ui-set-keystore-type = Typ
ng-ui-set-keystore-alias = Alias
ng-ui-set-keystore-storage = Passwort
ng-ui-set-keystore-storage-none = Nicht speichern
ng-ui-set-keystore-storage-memory = Bis Programmende
ng-ui-set-keystore-storage-keychain = Schlüsselbund
ng-ui-set-card = Chipkarte
    .keywords = chipkarte bürgerkarte karte lesegerät kartenleser bku chipkarten-software
ng-ui-set-card-status = Chipkarten-Software
ng-ui-set-card-found = Erkannt
ng-ui-set-card-missing = Nicht gefunden

## Einstellungen › Speichern

ng-ui-set-target = Speicherort
    .keywords = ordner speicherort ausgabe ausgabeordner zielordner speichern
ng-ui-set-target-beside = Neben dem Original
ng-ui-set-target-beside-desc = Im selben Ordner wie das Dokument
ng-ui-set-target-folder = In festem Ordner
ng-ui-set-target-ask = Jedes Mal fragen
ng-ui-set-target-ask-desc = Nach dem Signieren den Speicherort wählen
ng-ui-set-folder = Ordner
ng-ui-set-folder-none = Noch kein Ordner gewählt
ng-ui-set-folder-choose = Ordner wählen …
ng-ui-set-postfix = Dateinamenzusatz
    .keywords = dateiname name zusatz postfix endung signiert signed muster
ng-ui-set-postfix-hint = Wird an den Namen des Dokuments angehängt. Leer bedeutet „_signed“.
ng-ui-set-example = Beispiel

## Einstellungen › Darstellung

ng-ui-set-theme = Farbschema
    .keywords = farbschema farbe dunkel hell modus nachtmodus theme
ng-ui-set-theme-system = Wie System
ng-ui-set-theme-light = Hell
ng-ui-set-theme-dark = Dunkel
ng-ui-set-ui-lang = Sprache der Oberfläche
    .keywords = sprache oberfläche deutsch englisch english

## Einstellungen › Datenschutz (ADR 0003, ADR 0004 §3.10)

ng-ui-set-privacy-info = Datenschutzinformation
    .keywords = datenschutz a-trust hochladen upload übermittlung dsgvo
ng-ui-set-certcache = Signaturzertifikat zwischenspeichern
    .keywords = zertifikat zertifikats-cache cache zwischenspeichern
ng-ui-set-recent = „Zuletzt signiert“ merken
    .keywords = zuletzt signiert verlauf liste chronik
ng-ui-set-recent-hint = Gespeichert werden nur Name, Ordner und Zeitpunkt der zuletzt signierten Dokumente (höchstens 10), nur auf diesem Gerät.
ng-ui-set-recent-count =
    { $n ->
        [0] Die Liste ist leer.
        [one] Ein Eintrag in der Liste.
       *[other] { $n } Einträge in der Liste.
    }
ng-ui-set-recent-clear = Liste leeren
ng-ui-set-recent-cleared = Die Liste „Zuletzt signiert“ ist geleert.

## Einstellungen › Erweitert

ng-ui-set-proxy = Proxy
    .keywords = proxy netzwerk host port internet firewall
ng-ui-set-proxy-host = Host
ng-ui-set-proxy-port = Port
ng-ui-set-proxy-user = Benutzer (optional)
ng-ui-set-updates = Automatisch nach Updates suchen
    .keywords = update updates aktualisierung version
ng-ui-set-cms = Signaturformat (Keystore)
    .keywords = signaturformat format cms pades pdf-as keystore
ng-ui-set-reset = Zurücksetzen
    .keywords = zurücksetzen standard standardwerte werkseinstellungen

## Einstellungen › Über

ng-ui-set-about = PIDA
    .keywords = über version lizenz lizenzen drittlizenzen
ng-ui-set-logs = Protokolle
    .keywords = protokoll protokolle log logs fehlersuche

## Zuletzt signiert (Startansicht, ADR 0004 §3.10)

ng-ui-recent-title = Zuletzt signiert
ng-ui-recent-empty = Noch keine signierten Dokumente.
ng-ui-recent-open = „{ $name }“ im Prüfmodus öffnen
ng-ui-recent-remove = „{ $name }“ aus der Liste entfernen
ng-ui-recent-missing = Datei nicht mehr vorhanden
ng-ui-recent-verify-later = Der Prüfmodus folgt mit einer der nächsten Versionen.

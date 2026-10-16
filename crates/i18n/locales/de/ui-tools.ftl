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
# „Automatische Positionierung“ wie PDF-Over (SIGNATURE_POSITION=auto, ADR 0004 Nachtrag 8)
ng-ui-set-auto-position = Automatische Positionierung
    .keywords = position positionierung platzieren platzierung automatisch auto signaturblock lage sofort signieren
ng-ui-set-auto-position-hint = Der Signaturblock wird beim Öffnen automatisch platziert und das Dokument ist sofort bereit zum Signieren. Die Lage lässt sich bei Bedarf trotzdem ändern.

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
    .keywords = signaturformat format cms pades pdf-as keystore
ng-ui-set-reset = Zurücksetzen
    .keywords = zurücksetzen standard standardwerte werkseinstellungen
ng-ui-set-migration = PDF-Over-Einstellungen übernehmen
    .keywords = pdf-over migration übernehmen import importieren altkonfiguration alte einstellungen ersteinrichtung
ng-ui-set-migration-hint = Sucht auf diesem Gerät nach den Einstellungen einer früheren PDF-Over-Installation und bietet sie zur Übernahme an.
ng-ui-set-migration-search = Suchen …

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

## Prüfmodus (ADR 0004 §3.9; Wortlaut §8 F3: PIDA sagt nie „gültig“ oder „rechtsgültig“)

ng-ui-vf-title = Prüfmodus
ng-ui-vf-preview = Seiten des geprüften Dokuments
ng-ui-vf-page = Seite { $n } von { $total }
ng-ui-vf-meta = { $pages } · { $revisions }
ng-ui-vf-pages =
    { $n ->
        [one] eine Seite
       *[other] { $n } Seiten
    }
ng-ui-vf-revisions =
    { $n ->
        [one] eine Revision
       *[other] { $n } Revisionen
    }
ng-ui-vf-loading = Signaturen werden geprüft …
ng-ui-vf-failed = Die Datei konnte nicht geprüft werden
ng-ui-vf-summary-none = Dieses Dokument enthält keine Signatur
ng-ui-vf-summary-none-text = Es gibt nichts zu prüfen. Sie können das Dokument signieren.
ng-ui-vf-summary-intact =
    { $n ->
        [one] Die Signatur ist unverändert und kryptografisch intakt
       *[other] Alle { $n } Signaturen sind unverändert und kryptografisch intakt
    }
ng-ui-vf-summary-intact-text = Das Dokument wurde seit der letzten Signatur nicht verändert.
ng-ui-vf-summary-broken =
    { $n ->
        [one] Achtung: Eine Signatur ist nicht intakt
       *[other] Achtung: { $n } Signaturen sind nicht intakt
    }
ng-ui-vf-summary-broken-text = Das Dokument wurde nach dem Signieren verändert oder eine Signatur ist beschädigt. Verwenden Sie die signierte Originaldatei oder fordern Sie eine neue, signierte Fassung an.
ng-ui-vf-summary-unverifiable = Nicht alle Signaturen konnten geprüft werden
ng-ui-vf-summary-unverifiable-text = Keine Signatur ist verletzt, aber mindestens eine verwendet ein Verfahren, das PIDA nicht prüfen kann.
ng-ui-vf-summary-modified = Nach der letzten Signatur ergänzt oder geändert
ng-ui-vf-summary-modified-text = Alle Signaturen sind intakt, die Datei wurde aber nach der letzten Signatur noch ergänzt oder geändert. Dieser Teil ist von keiner Signatur abgedeckt.
ng-ui-vf-scope = PIDA prüft auf diesem Gerät, ob das Dokument seit dem Signieren unverändert ist, und zeigt die Angaben der Zertifikate. Vertrauenskette und Sperrstatus prüft PIDA nicht.
ng-ui-vf-full-check = Vollständige Prüfung bei signaturpruefung.gv.at
ng-ui-vf-signatures = Signaturen
ng-ui-vf-unknown-signer = Signatur { $n } (Unterzeichner unbekannt)
ng-ui-vf-status-intact = Intakt
ng-ui-vf-status-broken = Verändert
ng-ui-vf-status-unverifiable = Nicht prüfbar
ng-ui-vf-time = Signiert am { $time } (Angabe des Signierenden)
ng-ui-vf-time-none = Ohne Angabe des Signaturzeitpunkts
ng-ui-vf-qes = Qualifizierte elektronische Signatur laut Zertifikat
ng-ui-vf-qc = Qualifiziertes Zertifikat ohne qualifizierte Signaturerstellungseinheit (laut Zertifikat)
ng-ui-vf-not-qualified = Nicht qualifiziert (laut Zertifikat)
ng-ui-vf-qual-unknown = Qualifikation nicht feststellbar
ng-ui-vf-qual-hint = Angabe aus dem Zertifikat, nicht gegen die EU-Vertrauensliste geprüft.
ng-ui-vf-whole = Dokument seit dieser Signatur unverändert
ng-ui-vf-revision = Stand dieser Signatur unverändert
ng-ui-vf-later =
    Danach { $revisions ->
        [one] eine weitere Revision
       *[other] { $revisions } weitere Revisionen
    }, { $signatures ->
        [0] ohne weitere Signatur
        [one] davon eine mit Signatur
       *[other] davon { $signatures } mit Signatur
    }
ng-ui-vf-broken = Dokument nach dieser Signatur verändert oder Signatur beschädigt
ng-ui-vf-unverifiable = Diese Signatur kann PIDA nicht prüfen
ng-ui-vf-time-outside = Der angegebene Zeitpunkt liegt außerhalb der Gültigkeitsdauer des Zertifikats.
ng-ui-vf-details = Zertifikatsdetails
ng-ui-vf-cert-org = Organisation
ng-ui-vf-cert-issuer = Aussteller
ng-ui-vf-cert-serial = Seriennummer
ng-ui-vf-cert-validity = Gültigkeitsdauer
ng-ui-vf-cert-range = { $from } bis { $to }
ng-ui-vf-cert-algorithm = Verfahren
ng-ui-vf-cert-format = Format
ng-ui-vf-cert-qualification = Qualifikation
ng-ui-vf-cert-field = Signaturfeld
ng-ui-vf-cert-appearance = Signaturbild
ng-ui-vf-cert-page = Seite { $n }
ng-ui-vf-cert-invisible = unsichtbar
ng-ui-vf-cert-trust = Vertrauenskette
ng-ui-vf-cert-trust-not-checked = Nicht geprüft
ng-ui-vf-cert-code = Technischer Code
ng-ui-vf-cert-unknown = unbekannt
ng-ui-vf-format-pades = PAdES (ETSI.CAdES.detached)
ng-ui-vf-format-pkcs7 = PKCS#7 (adbe.pkcs7.detached)
ng-ui-vf-format-pkcs7-sha1 = PKCS#7 mit SHA-1 (adbe.pkcs7.sha1)
ng-ui-vf-format-other = Anderes Format
ng-ui-vf-show = Auf der Seite zeigen
ng-ui-vf-shown = Signaturbild von { $name } auf Seite { $page } hervorgehoben.
ng-ui-vf-sign = Dieses Dokument signieren
ng-ui-vf-other = Anderes Dokument prüfen
ng-ui-vf-exit = Prüfmodus verlassen

## Befehlspalette (⌘/Strg+K; die Befehle stehen in ui-workspace.ftl, Bereich shell)

ng-ui-pal-title = Befehle
ng-ui-pal-search = Befehl suchen …
ng-ui-pal-empty = Keine Treffer
ng-ui-pal-group-actions = Aktionen
ng-ui-pal-group-view = Ansicht
ng-ui-pal-group-help = Hilfe

## Kürzelübersicht („?“)

ng-ui-sh-title = Tastenkürzel
ng-ui-sh-group-general = Allgemein
ng-ui-sh-group-workspace = Arbeitsbereich
ng-ui-sh-palette = Befehlspalette

## Ersteinrichtung (ADR 0004 §3.12; das PIDA-Siegel bleibt, ADR 0005)

ng-ui-ob-title = Willkommen bei PIDA
ng-ui-ob-progress = Schritt { $n } von { $total }
ng-ui-ob-offer-title = PDF-Over-Einstellungen übernehmen
ng-ui-ob-import-title = PDF-Over-Einstellungen gefunden
ng-ui-ob-import-text = Auf diesem Gerät gibt es Einstellungen einer früheren PDF-Over-Installation. Sollen sie übernommen werden? Die alte Datei bleibt unverändert.
ng-ui-ob-import-list = Übernommen werden:
ng-ui-ob-import-item = { $item }: { $detail }
ng-ui-ob-import-seal = Bildmarke: Es bleibt das PIDA-Siegel.
ng-ui-ob-import-accept = Übernehmen
ng-ui-ob-import-decline = Nicht übernehmen
ng-ui-ob-signer-title = Womit signieren Sie meistens?
ng-ui-ob-signer-text = Das wird bei jedem Dokument vorausgewählt. Sie können jederzeit wechseln.
ng-ui-ob-signer-keystore-off = Später in den Einstellungen einrichten.
ng-ui-ob-block-title = So sieht Ihre Signatur aus
ng-ui-ob-block-text = Das Signaturbild erscheint auf dem Dokument. Profil, Bildmarke und Hinweistext lassen sich später in den Einstellungen ändern.
ng-ui-ob-back = Zurück
ng-ui-ob-next = Weiter
ng-ui-ob-finish = Los geht’s
ng-ui-ob-later = Später einrichten

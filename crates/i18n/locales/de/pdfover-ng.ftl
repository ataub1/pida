### PDF-Over NG – eigene Texte, deutsch (EUPL-1.2)
###
### Von Hand gepflegt. Alle IDs beginnen mit `ng-`, damit sie nie mit den aus PDF-Over erzeugten IDs
### (`pdfover.ftl`) kollidieren. Neue Meldungs-IDs anderer Pakete trägt die Integration hier ein
### (ADR 0001 Regel 11); jede ID braucht einen Eintrag in `de/` und `en/` mit denselben Variablen.

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

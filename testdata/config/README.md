# Testdaten `pdfover-config` (Paket E)

> **NUR FÜR TESTS.** Alle Namen, Nummern, Pfade und Passwörter sind erfunden (`test-…`, `NUR TEST`,
> `*.example.test`). Keine echten Zugangsdaten.

| Datei | Zweck |
|---|---|
| `pdf-over-generated-de.config` | wie von PDF-Over geschrieben (`Properties.store`, ISO-8859-1, `\uXXXX`, sortiert, Kopf- und Datumszeile); DE-Standardhinweis, Windows-Pfade escaped, Proxy, Keystore `memory` |
| `pdf-over-generated-en-keystore.config` | wie oben, EN, `BASE_LOGO`, Keystore `disk` mit Klartext-Testpasswörtern, `UPDATE_CHECK=false`, `THEME`, `#` im Pfad |
| `special-cases.config` | Sonderfälle der Lese-Regeln (docs/analyse/05 §1.5): `UPDATE_CHECK=FALSE`, `MOBILE_REMEMBER_PASSWORD=TRUE`, `USE_FIELDS` vor `USE_MARKER`, leerer `SIGNATURE_NOTE`, Auto-Migration `KEYSTORE_PASS_STORE_TYPE`, ungültiger `PROXY_PORT`, `42.5d`, `1024,abc`, `en_GB`/`fr`, `BKU=mobile`, `THEME`/`CRASH`, unbekannter Key |
| `note-missing-amts-en.config` | fehlender `SIGNATURE_NOTE` (Profil-Default), Klartext bei `memory`, Port außerhalb des Bereichs, negative Fenstergröße, `NaN` |
| `windows-crlf.config` | CRLF, escapte Windows- und UNC-Pfade, `\u00FC` im Pfad, `JKS`, mehrzeiliger Hinweis mit `\:`, `\=`, `\#` |
| `latin1-raw.config` | rohe ISO-8859-1-Bytes (ungültiges UTF-8 → Latin-1) |
| `handedited-utf8.config` | von Hand als UTF-8 gespeichert: rohe Umlaute, `:`/Leerraum als Trenner, nicht escapte Windows-Pfade, Fortsetzungszeile, `8080\u0020` |
| `handedited-utf8-bom-crlf.config` | UTF-8 mit BOM und CRLF |
| `legacy-home/.pdf-over/` | Altordner: `cfg/config.properties` und `cfg/profiles/INVISIBLE.properties` unverändert (PDF-AS 4.4.x), `SIGNATURBLOCK_SMALL_DE.properties` geändert, `MEIN_PROFIL.properties` eigenes Profil, `advancedconfig.properties` mit `signature_field_name` (B-05), `.emblem.png` (1×1 PNG); `PDF-Over.config` und `.emblem.properties` erzeugt der Test (absolute Pfade) |
| `oracle/LegacyOracle.java` | Java-Orakel: Nachbau von `ConfigurationManager.loadFromDisk` mit den echten JDK-Funktionen; `--locales` liefert `Locale.getAvailableLocales()` (Quelle von `crates/config/assets/java21-available-locales.txt`) |

Herkunft: `pdf-over-generated-*.config` mit `java.util.Properties.store` (JDK 21) erzeugt; die Dateien unter
`legacy-home/.pdf-over/cfg/` stammen aus der PDF-AS-Konfiguration von PDF-Over (A-SIT, EUPL) bzw. sind
davon abgeleitet. Alles andere ist von Hand geschrieben.

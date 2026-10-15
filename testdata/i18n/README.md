# Testdaten `pdfover-i18n` (Paket E)

| Datei | Zweck |
|---|---|
| `messages.properties`, `messages_de.properties` | unveränderte Kopien aus PDF-Over (`pdf-over-gui/src/main/resources/at/asit/pdfover/gui/`, A-SIT, EUPL; `messages_en.properties` ist dort ein Symlink auf `messages.properties`). Quelle der erzeugten Kataloge `crates/i18n/locales/<lang>/pdfover.ftl` |
| `oracle/DumpBundle.java` | Java-Orakel: liest die Dateien wie PDF-Over (`PropertyResourceBundle`) und formatiert jeden Wert mit `String.format` |
| `wave1-message-ids.json` | Kopie von `docs/integration/wave1-message-ids.json` (108 Meldungs-IDs der Welle 1, DE/EN). `tests/code_catalog.rs` prüft, dass jede ID wortgleich in `locales/<lang>/pdfover-ng.ftl` steht (Platzhalter `{x}` → Fluent `{ $x }`, Codes mit Punkt → ID mit Bindestrich) |

Neu erzeugen der Kataloge nach einer Änderung dieser Dateien oder der Korrekturtabellen in
`crates/i18n/src/convert.rs`: `PDFOVER_I18N_BLESS=1 cargo test -p pdfover-i18n --test catalogs`.

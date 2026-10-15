# ADR 0050 – Konfiguration, Migration und i18n (Paket E)

| | |
|---|---|
| **Status** | angenommen (14.10.2026) |
| **Kontext** | Umsetzung von `pida-config`, `pida-i18n` (Plan §7.1–§7.5, analyse/05 §1, analyse/01 §5–§6) |
| **Grundlage** | [ADR 0001](0001-workspace-und-schnittstellen.md) §4, §8 |

## Entscheidungen

1. **Kataloge in zwei Dateien je Sprache.** `locales/<lang>/pdfover.ftl` wird aus `messages*.properties`
   **erzeugt** (Test `catalogs` prüft Gleichheit; neu erzeugen mit `PIDA_I18N_BLESS=1`), eigene Texte stehen
   in `locales/<lang>/pida.ftl` (IDs mit Präfix `ng-`). Neue Meldungs-IDs anderer Pakete trägt die
   Integration in `pida.ftl` ein. Die UI lädt beide Dateien.
2. **Fluent-Konventionen.** ID = Java-Key mit `-` statt `.`; `&`-Mnemonic → `.accesskey`; `%s`/`%d` →
   `{ $arg1 }`, `{ $arg2 }` … in Reihenfolge; `<a>…</a>`/`<a href="…">…</a>` bleiben als Link-Markup
   (`pida_i18n::markup`, CLI: `to_plain`). Keine Unicode-Isolationszeichen um Platzhalter.
3. **Korrekturen an den PDF-Over-Texten** stehen als Tabellen im Konverter: B-09 (DE-Duplikat →
   `error.ConfigInitialization`), B-08 (EN-Hilfetexte `-kspass`/`-kskeypass` getauscht), B-07 (Hilfetext `-v`),
   Tippfehler „bittem un“, fehlende Keys je Sprache ergänzt; 29 von PDF-Over 4.4.x nicht referenzierte Keys
   sind als `# unused` markiert, bleiben aber im Katalog.
4. **Sprach-Fallback.** UI-Sprache → `Lang::system` (von der Shell gesetzt, sonst `LC_ALL`/`LC_MESSAGES`/
   `LANG`; `C`/`POSIX` → EN wie die JVM unter Linux) → DE; fehlende Nachricht: DE, dann EN, dann `!id!`.
   `fluent-bundle` löst je Nachricht höchstens 100 Placeables auf; der Konverter fasst benachbarte
   maskierte Zeichen zu einem Literal zusammen und warnt darüber hinaus.
5. **Lese-Semantik von `PDF-Over.config`** exakt wie `ConfigurationManager.loadFromDisk` (Java-Orakel im Test),
   mit gemeldeten Abweichungen statt Startabbruch: ungültiger `PROXY_PORT`, `LOGO_ONLY_SIZE` NaN/∞/≤ 0,
   negative Fenstergröße, UTF-8-BOM. Locales werden gegen die eingebettete Liste von
   `Locale.getAvailableLocales()` (JDK 21) geprüft; nicht unterstützte gültige Locales: UI → EN
   (Basis-Bundle), Signaturblock → DE (`_DE`).
6. **Migration** ist rein (`migrate`), Schreiben getrennt (`apply_migration`, nur neue Dateien, Protokoll
   `import-pdf-over.log`, `undo_migration`). Ein gespeicherter Hinweis, der exakt dem Standardtext von Profil
   und Sprache entspricht, wird zu `NoteSetting::Default`. Platzhaltersuche ohne gewählte Art bleibt aus.
   Klartext-Passwörter nur bei `disk` und nur nach Zustimmung in den `SecretStore`. `apply_migration`
   überschreibt nie eine vorhandene Datei (freie Namen für Sicherung und Bildmarke) und nimmt bei einem
   Fehler alles zurück (Dateien, Secrets, Sicherung). Angeboten wird der Import nur, solange es keine
   `settings.toml` gibt (`import_offer`).
7. **Unbekannte Settings-Keys** werden als TOML-Punktpfad in `Settings::unknown` gehalten (`future`,
   `signature.future_key`), damit sie auch den JSON-Rundlauf über die Tauri-Bridge überstehen. Beim
   Schreiben wird ein unbekannter Eintrag unter einer bekannten Tabelle nur übernommen, wenn die bekannten
   Werte danach unverändert lesbar sind; sonst wird er verworfen (die Datei bleibt immer lesbar).
   `Settings::sanitize` ersetzt ungültige Werte (`logo_only_size_mm` NaN/±∞/≤ 0 → 23 mm); die Shell ruft
   es nach dem JSON-Rundlauf auf. Neue Enum-Werte erfordern eine neue `schema_version`.
8. **Ordnernamen:** Windows `PIDA`, macOS `at.pida`, Linux `pida` (wie
   `/etc/pida/policy.toml`); Logs unter `~/Library/Logs/at.pida` bzw. `$XDG_STATE_HOME`.
   Die Tauri-Shell sollte ihren Identifier daran ausrichten.
9. **Richtlinien** dateibasiert (`%ProgramData%\PIDA\policy.toml`, `/Library/Application
   Support/at.pida/policy.toml`, `/etc/pida/policy.toml`); Registry und Managed Preferences
   liest die Shell und übergibt sie per `Policy::from_toml`/`from_json`. Keine eigene Umgebungsvariable
   zum Umleiten; unter Windows kommt `%ProgramData%` aber aus der Prozessumgebung – manipulationsfest ist
   dort nur `HKLM\Software\Policies\…` (Shell). Ein vorgegebenes Profil gewinnt auch gegen CLI `-v`.
10. **Effektive Werte beim Signieren** liefert `Effective` (Einstellung + Overlay + Richtlinie), auch die
    PDF-AS-Profil-ID (`Effective::pdfas_profile_id`) und der Hinweistext für das effektive Profil;
    `Settings::pdfas_profile_id` kennt weder `-v` noch Richtlinien.
11. **CLI `-c` mit Altdatei:** `load_config_source` verwendet Klartext-Passwörter nicht (wie der Import
    ohne Zustimmung); `load_config_source_with_secrets` liefert sie zusätzlich **nur für den RAM**
    (Sitzungsspeicher), damit Skripte mit `KEYSTORE_PASS_STORE_TYPE=disk` weiterlaufen können – ob die
    Shell davon Gebrauch macht, entscheidet die Integration.

# ADR 0050 – Konfiguration, Migration und i18n (Paket E)

| | |
|---|---|
| **Status** | angenommen (14.10.2026) |
| **Kontext** | Umsetzung von `pdfover-config`, `pdfover-i18n` (Plan §7.1–§7.5, analyse/05 §1, analyse/01 §5–§6) |
| **Grundlage** | [ADR 0001](0001-workspace-und-schnittstellen.md) §4, §8 |

## Entscheidungen

1. **Kataloge in zwei Dateien je Sprache.** `locales/<lang>/pdfover.ftl` wird aus `messages*.properties`
   **erzeugt** (Test `catalogs` prüft Gleichheit; neu erzeugen mit `PDFOVER_I18N_BLESS=1`), eigene Texte stehen
   in `locales/<lang>/pdfover-ng.ftl` (IDs mit Präfix `ng-`). Neue Meldungs-IDs anderer Pakete trägt die
   Integration in `pdfover-ng.ftl` ein. Die UI lädt beide Dateien.
2. **Fluent-Konventionen.** ID = Java-Key mit `-` statt `.`; `&`-Mnemonic → `.accesskey`; `%s`/`%d` →
   `{ $arg1 }`, `{ $arg2 }` … in Reihenfolge; `<a>…</a>`/`<a href="…">…</a>` bleiben als Link-Markup
   (`pdfover_i18n::markup`, CLI: `to_plain`). Keine Unicode-Isolationszeichen um Platzhalter.
3. **Korrekturen an den PDF-Over-Texten** stehen als Tabellen im Konverter: B-09 (DE-Duplikat →
   `error.ConfigInitialization`), B-08 (EN-Hilfetexte `-kspass`/`-kskeypass` getauscht), B-07 (Hilfetext `-v`),
   Tippfehler „bittem un“, fehlende Keys je Sprache ergänzt; 29 von PDF-Over 4.4.x nicht referenzierte Keys
   sind als `# unused` markiert, bleiben aber im Katalog.
4. **Sprach-Fallback.** UI-Sprache → `Lang::system` (von der Shell gesetzt, sonst `LC_ALL`/`LC_MESSAGES`/
   `LANG`) → DE; fehlende Nachricht: DE, dann EN, dann `!id!`.
5. **Lese-Semantik von `PDF-Over.config`** exakt wie `ConfigurationManager.loadFromDisk` (Java-Orakel im Test),
   mit gemeldeten Abweichungen statt Startabbruch: ungültiger `PROXY_PORT`, `LOGO_ONLY_SIZE` NaN/∞/≤ 0,
   negative Fenstergröße, UTF-8-BOM. Locales werden gegen die eingebettete Liste von
   `Locale.getAvailableLocales()` (JDK 21) geprüft; nicht unterstützte gültige Locales: UI → EN
   (Basis-Bundle), Signaturblock → DE (`_DE`).
6. **Migration** ist rein (`migrate`), Schreiben getrennt (`apply_migration`, nur neue Dateien, Protokoll
   `import-pdf-over.log`, `undo_migration`). Ein gespeicherter Hinweis, der exakt dem Standardtext von Profil
   und Sprache entspricht, wird zu `NoteSetting::Default`. Platzhaltersuche ohne gewählte Art bleibt aus.
   Klartext-Passwörter nur bei `disk` und nur nach Zustimmung in den `SecretStore`.
7. **Unbekannte Settings-Keys** werden als TOML-Punktpfad in `Settings::unknown` gehalten (`future`,
   `signature.future_key`), damit sie auch den JSON-Rundlauf über die Tauri-Bridge überstehen.
8. **Ordnernamen:** Windows `PDF-Over NG`, macOS `at.pdf-over-ng`, Linux `pdf-over-ng` (wie
   `/etc/pdf-over-ng/policy.toml`); Logs unter `~/Library/Logs/at.pdf-over-ng` bzw. `$XDG_STATE_HOME`.
   Die Tauri-Shell sollte ihren Identifier daran ausrichten.
9. **Richtlinien** dateibasiert (`%ProgramData%\PDF-Over NG\policy.toml`, `/Library/Application
   Support/at.pdf-over-ng/policy.toml`, `/etc/pdf-over-ng/policy.toml`); Registry und Managed Preferences
   liest die Shell und übergibt sie per `Policy::from_toml`/`from_json`. Keine Umgebungsvariable zum
   Umleiten.

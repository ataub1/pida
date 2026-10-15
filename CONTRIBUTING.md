# Mitarbeit an PIDA

*English summary below.*

Danke für Ihr Interesse! PIDA ist ein PDF-Signaturwerkzeug für ID Austria (Rust-Kern + Tauri 2, Lizenz
**EUPL-1.2**). Beiträge sind willkommen – Fehlerberichte, Verbesserungsvorschläge, Dokumentation und Code.

## Bevor Sie loslegen

- **Sicherheitslücken** bitte nie öffentlich melden, sondern wie in [SECURITY.md](SECURITY.md) beschrieben.
- Für größere Änderungen bitte zuerst ein **Issue** eröffnen und den Ansatz abstimmen. Architektur und
  verbindliche Verträge stehen in [docs/00-architektur-und-umsetzungsplan.md](docs/00-architektur-und-umsetzungsplan.md)
  und den ADRs unter [docs/adr/](docs/adr/) (insbesondere ADR 0001 – Workspace und Schnittstellen,
  ADR 0004 – neue Oberfläche).

## Entwicklungsumgebung

Einrichtung, Werkzeuge (Rust, Node, PDFium, pyHanko, qpdf), Tests und CI beschreibt
[docs/entwicklung.md](docs/entwicklung.md). Kurzfassung:

```bash
source tools/env.sh
cargo test --workspace --no-fail-fast
cd apps/app && npm ci && npm test
```

## Regeln für Beiträge

1. **Pull-Requests gegen `main`.** Die CI (Build und Tests auf Windows, Linux, macOS, Mobile-Ziele,
   Lizenzprüfung, Formatierung) muss grün sein.
2. **Qualität:** `cargo fmt`, `cargo clippy` ohne neue Warnungen, Tests für neues Verhalten; Signatur-Themen
   immer gegen externe Prüfwerkzeuge testen (pyHanko, qpdf, OpenSSL, veraPDF – siehe `docs/entwicklung.md`).
3. **Keine echten Zugangsdaten und keine Requests an A-Trust** in Tests oder Beispielen; Testdaten nur aus
   `testdata/`. Logs und `Debug`-Ausgaben dürfen keine Passwörter, TANs, Tokens oder Dokumentinhalte enthalten.
4. **Texte zweisprachig:** neue Meldungen in Deutsch und Englisch (`crates/i18n/locales/{de,en}/`).
5. **Abhängigkeiten:** nur mit EUPL-kompatibler Lizenz (keine GPL/AGPL; siehe `deny.toml`), Versionen exakt
   pinnen.
6. **Bezüge auf das Original-PDF-Over** (Import aus `~/.pdf-over`, `PDF-Over.config`, kompatible
   Kommandozeilen-Argumente) bleiben unverändert – siehe ADR 0002.
7. **Herkunft (DCO):** Mit Ihrem Beitrag bestätigen Sie, dass Sie ihn unter der EUPL-1.2 einreichen dürfen.
   Bitte signieren Sie Ihre Commits mit `git commit -s` (Zeile `Signed-off-by:` gemäß
   [Developer Certificate of Origin](https://developercertificate.org/)).

## Lizenz

Beiträge werden unter der **EUPL-1.2** veröffentlicht, derselben Lizenz wie das Projekt (siehe [LICENSE](LICENSE)).

---

## English summary

Contributions are welcome. Please open an issue before large changes, never report security issues publicly
(see [SECURITY.md](SECURITY.md)), send pull requests against `main` with a green CI, add tests, keep texts in
German and English, use only EUPL-compatible dependencies, never use real credentials or contact A-Trust in
tests, and sign off your commits (`git commit -s`, DCO). Contributions are licensed under the EUPL-1.2.
Developer setup: [docs/entwicklung.md](docs/entwicklung.md).

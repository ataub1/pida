# ADR 0003 – Datenschutzhinweis ohne Zustimmungsdialog

| | |
|---|---|
| **Status** | angenommen (15.10.2026) |
| **Kontext** | Prototyp: Vor der ersten Signatur mit einem Signiermittel, bei dem das Dokument das Gerät verlässt (`Capabilities::document_leaves_device`, ID Austria/A-Trust), erschien ein **blockierender** Dialog „Datenschutzinformation“ mit „Zustimmen und fortfahren“/„Ablehnen“ (Plan §4.2/§7.8); der Kern wartete auf `privacy_acknowledged`. |
| **Entscheidung** | Auftraggeber, 15.10.2026: Der Dialog entfällt. Der Hinweis gehört als Kurzhinweis zum Signiermittel und vollständig in die Einstellungen. |
| **Betrifft** | `pida-workflow`, `pida-core-api`, `pida-config`, `pida-cli`, Oberfläche (Desktop und Mobile), Kataloge `pida.ftl` |

## 1. Entscheidung

1. **Kurzhinweis (nicht blockierend).** Beim Signiermittel mit `document_leaves_device` steht „Das Dokument
   wird zum Signieren an A-Trust übermittelt.“ mit dem Link „Details“:
   - in der Signiermittel-Auswahl unter der Karte von ID Austria (Keystore und lokale BKU: kein Hinweis),
   - im Signierbereich, solange ID Austria läuft (`StepView::Signing::document_leaves_device`) – auch wenn
     das Signiermittel ohne Auswahl startete (F-53/F-54 nach „Signieren“).
   „Details“ öffnet die Einstellungen beim Tab „Datenschutz“. Während des Signierens sind die Einstellungen
   wie in PDF-Over gesperrt; dort zeigt „Details“ dieselben Angaben als schließbares Fenster.
2. **Einstellungen › Datenschutz.** Neuer Tab vor „Über“ mit dem bisherigen Datenschutztext aus PDF-Over
   (`config-DataProtectionStatement`, Link zur Datenschutzerklärung von A-Trust
   <https://www.a-trust.at/downloads/documents/datenschutzerklaerung.pdf>), dem Hinweis, dass Keystore und
   Bürgerkarte lokal signieren (kein Upload), und dem Zertifikats-Cache (bisher „Erweitert › Datenschutz“).
   Ein eigener Text der Organisation (Richtlinie `privacy_notice_text`, `ViewState::privacy_text`) ersetzt den
   Text aus PDF-Over; der Link zu A-Trust steht dann eigens darunter. „Über“ verweist auf den Tab.
3. **Kern ohne Zustimmungsschritt.** Entfernt: `State::PrivacyNotice`, `Event::PrivacyAnswered`,
   `Effect::AcknowledgePrivacy`, `Context::privacy_acknowledged`, `PrivacyNoticeView`,
   `StepView::Backend::privacy_notice`, `PRIVACY_TEXT_VERSION`, `pida_config::Settings::privacy`.
   Kompatibilität:
   - `UiEvent::AcknowledgePrivacy` wird weiter gelesen und ignoriert (`#[ts(skip)]`: nicht mehr in den
     TypeScript-Typen) – ältere Aufrufer scheitern nicht an `ng-ui-event-invalid`.
   - `settings.toml` mit `[privacy.acknowledged]` lädt weiter. Der Eintrag ist ausgemustert
     (`pida_config::RETIRED_KEYS`) und wird beim Lesen verworfen; andere Einträge unter `[privacy]` bleiben
     wie unbekannte erhalten. Eine neue `schema_version` ist nicht nötig.
   - Eine Richtlinienoption, die den Dialog weiter erzwingt, gibt es bewusst nicht.
4. **Nie ohne Nutzeraktion zu A-Trust (Autostart-Befund F-54, docs/integration/mobile.md R-1).** Der Dialog war
   bisher (bis zur Bestätigung) auch die Schranke vor dem automatischen Start. Neu im Reducer
   (`State::ChoosingBackend::sign_requested`): Die automatische Wahl – Standard-Signiermittel (F-53) bzw. „nur
   ID Austria verfügbar“ (F-54) – startet ein Signiermittel mit `document_leaves_device` **nur nach
   „Signieren“** (Position bestätigt bzw. „Unsichtbar signieren“). Ohne diese Aktion (automatische
   Positionierung, unsichtbares Profil, Platzhalter, nächstes Dokument eines Batches) erscheint die Auswahl mit
   dem Kurzhinweis; erst die Wahl dort startet die Sitzung. „Signatur fortsetzen“ ist ebenfalls eine
   ausdrückliche Aktion. Signiermittel, bei denen das Dokument auf dem Gerät bleibt (Keystore, lokale BKU),
   starten wie bisher automatisch. Der eingeschaltete, aber fehlende Keystore verhindert F-54 weiterhin.
5. **CLI.** Der Aufruf (`pida-cli -b MOBILE …`) ist die Nutzeraktion. Statt der Rückfrage „Fortfahren? [j/N]“
   gibt die CLI eine einzeilige Info aus: Hinweis des Kerns `ng-document-leaves-device` (nur ohne Oberfläche,
   `Context::headless`), über die normale Ausgabe der Hinweise.
6. **Mobile (iOS/Android)** verwendet dieselbe Oberfläche und denselben Kern.

## 2. Folgen

- Wer ID Austria als Standard-Signiermittel eingestellt hat und automatisch positioniert, wählt ID Austria
  künftig je Dokument in der Auswahl (ein Klick; mit Positionierung genügt „Signieren“). Soll das
  Standard-Signiermittel selbst als ausdrückliche Zustimmung gelten, wäre das eine neue Entscheidung.
- Katalog: entfernt `ng-ui-privacy-lead`, `ng-ui-privacy-accept`, `ng-ui-privacy-decline`, `ng-cli-privacy`
  (in `RETIRED_WAVE2_IDS` des Katalogtests); neu `ng-document-leaves-device`, `ng-ui-privacy-details`,
  `ng-ui-privacy-details-label`, `ng-ui-privacy-local`, `ng-ui-privacy-atrust-link`, `ng-ui-privacy-about`,
  `ng-ui-certcache-title`; `ng-ui-privacy-title` ist jetzt der Tab (Zugriffstaste DE `t`, EN `y`).

## 3. Tests

- Reducer (`crates/workflow/tests/transitions.rs`, `lifecycle.rs`): kein Zustimmungsschritt; ID Austria startet
  ohne „Signieren“ weder über F-54 noch über F-53 (auch nicht bei erneuter Prüfung); nach „Signieren“ bzw.
  „Unsichtbar signieren“ startet F-54; eingeschalteter, fehlender Keystore verhindert F-54 weiter; CLI:
  einzeiliger Hinweis; Fortsetzen ohne Zustimmung.
- Kern (`crates/core-api/tests/flow.rs`, `mobile.rs`, `pdfium.rs`): Auswahl statt Autostart, keine Sitzung vor
  der Wahl, Kurzhinweis-Flag im Signierbereich, alte `settings.toml` mit Zustimmungsfeld, Richtlinientext.
- Einstellungen (`crates/config/tests/settings_roundtrip.rs`): ausgemusterter Eintrag lädt und verschwindet.
- CLI (`apps/cli/src/lib.rs`): `-b MOBILE` im Terminal – eine Info-Zeile, keine Rückfrage.
- Oberfläche (`BackendScreen`, `SigningScreen`, `SettingsDialog`, `App`, `Mobile`): kein Dialog, Kurzhinweis
  bei ID Austria und nicht beim Keystore, „Details“ öffnet Einstellungen › Datenschutz, Tab mit Link.

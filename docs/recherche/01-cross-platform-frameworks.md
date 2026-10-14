# 01 – Cross-Platform-Frameworks für den PDF-Over-Klon

Stand der Recherche: **2026-10-14**. Referenz-Code: PDF-Over `main` @ `c07c5ec` (4.4.10-SNAPSHOT).
Alle Pfade mit Zeilennummern beziehen sich auf `ref/PDF-Over/` (relativ zum Repo-Root von PDF-Over).

## Legende für den Verifikationsgrad

| Tag | Bedeutung |
|---|---|
| **[C]** | Im PDF-Over-Originalcode verifiziert (Pfad:Zeile angegeben) |
| **[W]** | Offizielle Doku / Release-Notes / Paketregister, am 2026-10-14 abgerufen |
| **[S]** | Sekundärquelle (Fachpresse, Blog, Forum); plausibel, aber nicht offiziell |
| **[I]** | Eigene Interpretation / Bewertung / Vermutung |
| **[E]** | Erfahrungs- oder Schätzwert, in dieser Recherche **nicht gemessen** |

---

## 0. Kurzfazit

1. **Harte K.O.-Kriterien** (alle fünf Plattformen aus *einer* Codebasis, kein JVM beim Endnutzer) eliminieren:
   Compose Multiplatform (Desktop = JVM), .NET MAUI (kein Linux), React Native (kein Linux), Electron allein (kein Mobile).
   Eine reine PWA scheitert **funktional** (A-Trust-Flow über CORS, FIDO2 für fremde RP-ID, lokale BKU) – nicht technisch-plattformseitig.
2. **Top-3** (gewichtete Matrix, Abschnitt 6):
   **(1) Tauri 2** (84,2 / 100) – **(2) Flutter** (82,0; als Variante „Flutter-UI + Rust-Core" 83,0) – **(3) Avalonia UI 12** (81,3).
   Der Abstand ist klein; die Rangfolge kippt je nach Gewichtung (Sensitivität in 6.3): bei „Mobile-first" gewinnt Flutter,
   bei „Krypto/Integration-first" bleibt Tauri knapp vor Avalonia.
3. **Empfehlung [I]:** Signatur-Kern (PDF-AS-Äquivalent, Security-Layer/A-Trust-Protokoll, CMS/PAdES, FIDO2-Brücke) als
   **UI-unabhängige Rust-Bibliothek** bauen und mit **Tauri 2** ausliefern. Damit bleibt der Umstieg auf Flutter-UI
   (über `flutter_rust_bridge`) als Plan B offen, ohne den Kern neu zu schreiben.
4. **Das größte Risiko ist kein Framework-Thema**, sondern plattformpolitisch: PDF-Over führt eine WebAuthn/FIDO2-Assertion
   als *nativer Client* für die RP `https://service.a-trust.at` aus [C]. Auf **iOS/Android** (und im Browser) ist das für
   Fremd-Apps ohne Mitwirkung von A-Trust (Associated Domains / Digital Asset Links / Related Origins) **nicht erlaubt** [W].
   Auf Windows (`webauthn.dll`) und Linux/macOS (direkt über `libfido2`) ist es möglich [W]/[I]. Das gilt für **jedes** Framework.

---

## 1. Ziel, Umfang, Methode

- Gesucht: Technologie für eine App auf **Windows, macOS, Linux, iOS, Android**, ohne Java/JVM, mit kleiner bzw. keiner Installation.
- Bewertet wurden 14 Kandidaten: Tauri 2, Flutter, Compose Multiplatform/KMP, .NET MAUI, Avalonia UI, Uno Platform, Qt 6/QML,
  Slint, Dioxus, React Native (+ Windows/macOS), Capacitor/Ionic (+ Electron), Electron, Wails, reine PWA.
- Quellen: offizielle Release-Seiten (GitHub Releases, NuGet, pub.dev, crates.io), Herstellerdoku, ergänzend Fachpresse.
  Versionsangaben „Stand 2026-10-14". Wo GitHub-Release-Seiten nur Tag/Monat zeigen, ist das Jahr aus dem Kontext erschlossen [I].
- Die konkrete Neuimplementierung von PDF-AS (PAdES, Signaturblock) ist **nicht** Gegenstand dieses Dokuments; das Krypto-/PDF-Ökosystem
  je Sprache geht nur als Bewertungskriterium ein (Abschnitt 5.7).

---

## 2. Anforderungen aus PDF-Over, die die Framework-Wahl treiben

Die folgenden Punkte sind die „nativen Berührungspunkte" des Originals. Sie bestimmen die Kriterien und Gewichte.

| # | Anforderung | Fundstelle im Original | Konsequenz für den Klon |
|---|---|---|---|
| R1 | **PDF-Seiten rendern** und Signaturblock darauf positionieren (Seitenbild in Skalierung) | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/SignaturePanel.java:37` (Import `org.apache.pdfbox.rendering.PDFRenderer`), `:153` (`new PDFRenderer(pdf)`), `:283` (`renderer.renderImage(whichPage-1, …)`) [C] | Framework braucht performantes PDF-Rendering (pdf.js, PDFium, PDFKit, PdfRenderer) plus Overlay/Drag eines Rechtecks |
| R2 | **Drag & Drop** von Dateien ins Fenster | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/composites/DataSourceSelectComposite.java:170`, `…/OutputComposite.java:182` (`new DropTarget(…)`) [C] | Desktop-DnD von Dateien nötig; mobil: Datei-Picker/„Öffnen in" |
| R3 | **Signierte Datei mit Standard-App öffnen**, Log-Ordner öffnen | `…/composites/OutputComposite.java:141` (`Desktop.getDesktop().open(f)`), `:144` (`Program.launch`), `…/composites/configuration/AboutComposite.java:86` [C] | „Open with default app"/Opener; mobil: Share-Sheet / „Öffnen in" |
| R4 | **System-Browser öffnen** (Links, Signaturdaten-Ansicht, Info-Seiten) | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/utils/SWTUtils.java:259-272` (`openURL`), `…/AboutComposite.java:62`, `…/mobilebku/MobileBKUFido2Composite.java:116` [C] | Opener/URL-Launcher auf allen Plattformen |
| R5 | **HTTP-Flow zu A-Trust** (POST Security-Layer-Request, dann HTML-Seiten parsen) | `pdf-over-commons/src/main/java/at/asit/pdfover/commons/Constants.java:108-109` (`MOBILE_BKU_URL`, Test-URL), `pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/MobileBKUConnector.java:201` (`new HttpPost(Constants.MOBILE_BKU_URL)`), `:33-35` (jsoup-Imports), `…/bku/mobile/ATrustParser.java:11` [C] | Nativer HTTP-Client mit Cookie-Handling und HTML-Parser; **in WebView-/Browser-Frameworks darf das nicht über `fetch` aus der Seite laufen** (CORS) [I] |
| R6 | **Lokale Bürgerkartenumgebung** über `http://127.0.0.1:3495/http-security-layer-request` | `pdf-over-commons/…/Constants.java:105`, `pdf-over-gui/…/bku/LocalBKUConnector.java:48` (`new Socket("127.0.0.1", 3495)`) [C] | Desktop-only; im Browser durch *Local Network Access*-Prompt und CORS erschwert (5.3) |
| R7 | **FIDO2/WebAuthn als nativer Client** für RP-Origin `https://service.a-trust.at` | `pdf-over-gui/…/composites/mobilebku/MobileBKUFido2Composite.java:61` (`WebAuthN.buildGetFromJSON(…).get("https://service.a-trust.at")`), `:104` (`WebAuthN.isAvailable()`), `pdf-over-gui/…/bku/MobileBKUConnector.java:56`, `:338-340` [C]; Abhängigkeit `at.a-sit:webauthn-client-java:0.0.1` in `pdf-over-gui/pom.xml:72-74` [C]; Hinweis auf fehlendes `libfido2` in `…/composites/configuration/Fido2ConfigurationComposite.java:40` und `pdf-over-gui/src/main/resources/at/asit/pdfover/gui/messages_en.properties:266-268` [C] | Zugriff auf Plattform-WebAuthn-API bzw. `libfido2` mit **fremder RP-ID** – auf Mobile/Web blockiert (5.1) |
| R8 | **Datei-Keystore** PKCS#12 und „JKS" (intern `JCEKS`) | `pdf-over-gui/…/composites/configuration/KeystoreConfigurationComposite.java:545-546`, `pdf-over-gui/…/keystore/KeystoreUtils.java:14-18` [C] | PKCS#12 ist überall lösbar; **JCEKS ist ein Java-proprietäres Format** – außerhalb Java kaum Bibliotheken [I] |
| R9 | **Geheimnisse persistieren**: Keystore-Passwort optional auf Disk, Handy-Signatur-Passwort merken | `pdf-over-gui/…/workflow/config/ConfigurationDataInMemory.java:133-139` (`KeyStorePassStorageType { MEMORY, DISK }`), `pdf-over-commons/…/Constants.java:154-155` [C] | Im Klon: OS-Keychain/Keystore/libsecret statt Klartext [I] |
| R10 | **Update-Check** (Textdatei abrufen, auf Download-Seite verlinken; kein Auto-Update) | `pdf-over-gui/…/utils/UpdateCheckManager.java:52`, `pdf-over-commons/…/Constants.java:115-118` [C] | Auto-Update ist „nice to have", nicht Paritätspflicht |
| R11 | **Kommandozeilen-Argumente** (Datei beim Start übergeben) | `pdf-over-gui/src/main/java/at/asit/pdfover/gui/Main.java:36-64` (`new StateMachine(args)`) [C] | Dateizuordnung/„Öffnen mit" auf Desktop |
| R12 | **Heutige Auslieferung**: gebündelte JRE 21 je Plattform, launch4j, IzPack | `jre-bundles/` (ZIP/TAR 41–52 MB je Plattform, per `ls -la` gemessen) [C]; `pdf-over-gui/pom.xml:376` (launch4j), `:544` (izpack) [C]; SWT 3.134.0 in `pom.xml:116` [C] | **Baseline für „keine große Installation"**: allein die JRE ist ≈ 41–52 MB komprimiert |

---

## 3. Kriterien, K.O.-Regeln und Gewichte

### 3.1 K.O.-Kriterien (vor der Gewichtung)

| K.O. | Regel |
|---|---|
| K1 | Alle fünf Zielplattformen müssen vom Framework **selbst** (offiziell oder mit klar gepflegtem Erstanbieter-Target) aus einer Codebasis bedient werden. |
| K2 | Kein Java/JVM auf dem Endgerät (auch keine gebündelte JRE). |
| K3 | Die Kernfunktionen R5–R7 müssen auf Desktop ohne Fremd-Server implementierbar sein (Datenschutz: Handy-Signatur-Zugangsdaten dürfen keinen Proxy passieren) [I]. |

K.O.-Kandidaten werden in der Matrix trotzdem bewertet (zur Transparenz), aber nicht empfohlen.

### 3.2 Gewichtete Kriterien (Summe 100)

| Kürzel | Kriterium | Gewicht | Begründung |
|---|---|---|---|
| P | Plattformabdeckung & Reife je Plattform | **20** | Kernanforderung des Nutzers |
| N | Native Integrationen (Dateien, DnD, „Öffnen mit", Share/„Öffnen in", Deep/Universal Links, Secure Storage, System-Browser, nativer HTTP ohne CORS, FIDO2-Zugriff, Smartcard/PKCS#11) | **20** | R2–R9 |
| K | Krypto-/PDF-Ökosystem der Sprache (PDF-Rendering, CMS/PAdES, X.509, PKCS#12, HTML-Parsing) | **15** | PDF-AS muss neu gebaut werden |
| G | Größe & Laufzeitvoraussetzungen | **12** | „keine große Installation" |
| D | Distribution (Stores, Code-Signing, Notarisierung, MSIX/winget, Flatpak/AppImage) & Auto-Update | **10** | Betrieb |
| X | Entwicklungsgeschwindigkeit / DX | **10** | Aufwand |
| O | Ökosystem-Gesundheit / Langfristigkeit | **8** | Wartbarkeit über Jahre (eGovernment) |
| L | Lizenz (inkl. Verträglichkeit mit EUPL-1.2 und App-Store-Vertrieb) | **5** | Rechtssicherheit |

Skala je Kriterium 1 (schlecht) – 5 (sehr gut), Halbpunkte erlaubt. Gesamt = Σ(Gewicht × Note) / 5 → 0–100.

---

## 4. Kandidatenprofile

> Jeder Steckbrief: Status/Version · Plattformen · Größe/Laufzeit · Integrationen · PDF · Distribution/Update · DX/Ökosystem/Lizenz · Fit für PDF-Over.

### 4.1 Tauri 2 (Rust-Backend + System-WebView, inkl. Mobile)

- **Status [W]:** `tauri` v2.12.3 (10.10.2026), `tauri-cli` v2.12.2 (10.10.2026); 2.0 stable seit 02.10.2024.
  Parallel läuft eine **Tauri-3-Alpha** mit austauschbarem Runtime-Layer (`tauri-runtime-wry` vs. `tauri-runtime-cef`) und
  einem Bundler-Schalter `webview_runtime` = Wry | **Cef** | Other – d. h. optional gebündeltes Chromium statt System-WebView
  ([tauri-v3.0.0-alpha.0](https://newreleases.io/project/github/tauri-apps/tauri/release/tauri-v3.0.0-alpha.0),
  [tauri-bundler-v3.0.0-alpha.0](https://newreleases.io/project/github/tauri-apps/tauri/release/tauri-bundler-v3.0.0-alpha.0)).
  Ein offenes Issue (März 2026) zeigt, dass die CEF-API noch unvollständig ist [S].
- **Plattformen:** Windows, macOS (≥ 10.15), Linux, Android, iOS [W] ([Prerequisites](https://v2.tauri.app/start/prerequisites/)).
  Desktop gilt als produktionsreif; Mobile ist offiziell unterstützt, aber weniger ausgereift (Plugin-Lücken) [S]/[I].
- **Laufzeit [W]:** Windows: **WebView2** (ab Windows 10 1803 vorinstalliert; NSIS/WiX-Installer enthalten den WebView2-Installationsschritt).
  Linux: **webkit2gtk-4.1** (Pakete `libwebkit2gtk-4.1-dev` usw.; AppImage bündelt die webkit2gtk-Helferprozesse).
  macOS/iOS: WKWebView. Android: System WebView.
- **Größe:** Doku: minimale App „< 600 KB" [W] ([What is Tauri](https://v2.tauri.app/start/)); Messung Dritter: Hello World 3,2 MB,
  komplexe App 8,6 MB vs. Electron 85/244 MB [S] ([tech-insider](https://tech-insider.org/tauri-vs-electron-2026/)).
  Linux-AppImage deutlich größer, weil WebKitGTK-Teile gebündelt werden [W]/[E].
- **Integrationen [W]** ([Plugin-Liste](https://v2.tauri.app/plugin/)): `dialog` (Datei-Dialoge), `fs`, `opener` (Dateien/URLs extern öffnen → R3/R4),
  `http` (Rust-HTTP-Client → **kein CORS-Problem**, R5), `deep-link` (Custom Schemes Desktop; App Links/Universal Links mobil, mit
  `assetlinks.json`/AASA; auf macOS nur per Config, nicht zur Laufzeit) ([Deep Linking](https://v2.tauri.app/plugin/deep-linking/)),
  `single-instance`, `updater`, `store`, `stronghold` (verschlüsselte DB; keine Deprecation-Notiz auf der Plugin-Seite),
  `biometric` (nur Android/iOS), `barcode-scanner` (mobil).
  **Dateizuordnung:** `bundle.fileAssociations` inkl. Mobile (Android-Intent-Filter `Send`/`SendMultiple`/`View`, iOS Document Types,
  `exportedType`); Dateien kommen als `RunEvent::Opened` auf macOS/iOS/Android, auf Windows/Linux als Argumente
  ([Mobile File Associations](https://v2.tauri.app/learn/mobile-file-associations/)) → R11, „Öffnen in".
  **Kein offizielles Share-Sheet- und kein offizielles Keychain-Plugin** (nur Community, z. B. `tauri-plugin-sharesheet`) [W].
  Drag & Drop von Dateien ist als Fenster-Event im Kern vorhanden [E – aus API-Kenntnis, nicht erneut geprüft].
  FIDO2/Smartcard: aus Rust direkt über `windows`-Crate (`webauthn.dll`), FFI zu `libfido2`, `pcsc`-/`cryptoki`-Crates [I];
  mobil eigene Swift/Kotlin-Plugins möglich.
- **PDF:** pdf.js in der WebView (überall gleich) oder `pdfium-render` 0.9.4 (MIT/Apache-2.0, 06.09.2026) [W]
  ([crates.io](https://crates.io/crates/pdfium-render)). Risiko: pdf.js-Performance auf WebKitGTK und älteren Android-WebViews [I].
- **Distribution [W]** ([Distribute](https://v2.tauri.app/distribute/)): Debian, RPM, AppImage, Snap, **Flathub**, AUR; macOS-Bundle, DMG,
  **App Store**; Windows Installer (MSI/NSIS), **Microsoft Store**; **Google Play**; **iOS App Store**; Signing-Guides für alle fünf.
  **Updater** ([Updater](https://v2.tauri.app/plugin/updater/)): Desktop (AppImage, `.app.tar.gz`, MSI/NSIS), Signatur **nicht abschaltbar**
  (Schlüssel über `TAURI_SIGNING_PRIVATE_KEY`); Mobile-Updates laufen über die Stores [I].
- **DX/Ökosystem/Lizenz:** Zwei Sprachen (TypeScript-UI + Rust); Frontend-HMR auch auf Mobile [S]; sehr aktive Releases;
  MIT/Apache-2.0. Risiko Migration v2→v3 [I].
- **Fit [I]:** Sehr gut für Desktop-Parität (R1–R11) bei minimaler Größe. Rust ist zugleich eine gute Sprache für den Signatur-Kern
  (RustCrypto `cms`/`x509-cert`, PDF-Crates, WASM-fähig). Mobile benötigt eigene Plugin-Arbeit (Share, Keychain).

### 4.2 Flutter (Dart, Skia/Impeller, alle 5 Plattformen)

- **Status [W]:** Flutter **3.47** (12.08.2026, Dart 3.13 laut [S]); nächste Stable **3.50** im November 2026
  ([Archiv/Release-Fenster](https://docs.flutter.dev/release/archive)). 3.47: **Impeller ist Standard-Renderer auf macOS, Windows, Linux**,
  experimentelles Multi-Window (Linux/Windows), Mindest-OS **iOS 15** und **macOS 12**, `material_ui`/`cupertino_ui` 1.0 als separate
  Pakete; eingebaute Material/Cupertino-Bibliotheken werden im November-Release formal deprecated
  ([What's new in 3.47](https://flutter.dev/blog/whats-new-in-flutter-3-47)).
- **Governance:** Auf der Google I/O 2026 wurde **Canonical** als Lead-Maintainer/„strategic steward" von Flutter Desktop
  (Windows, macOS, Linux) benannt [S] ([OMG! Ubuntu](https://www.omgubuntu.co.uk/2026/05/flutter-desktop-canonical-maintained)).
  Bewertung: stärkt Linux, verteilt aber Verantwortung – Langfristrisiko „Google zieht sich vom Desktop zurück" [I].
- **Laufzeit [W]:** Linux: GTK 3 (`libgtk-3-0 libblkid1 liblzma5`) ([Linux building](https://docs.flutter.dev/platform-integration/linux/building));
  Windows: neben `.exe` alle DLLs inkl. `flutter_windows.dll`, `data/`-Ordner und VC++-Redistributables (`msvcp140.dll`, `vcruntime140.dll`,
  `vcruntime140_1.dll`) ([Windows building](https://docs.flutter.dev/platform-integration/windows/building)).
- **Größe:** Kein offizieller Desktop-Wert; Sekundärquelle nennt 20–30 MB für eine minimale Desktop-App [S]; mobil
  einstellige bis niedrige zweistellige MB [E].
- **Integrationen:** `flutter_secure_storage` 11.2.0 auf **allen 6 Targets** (Keychain, Android-KeyStore-basiert, libsecret, Windows,
  WebCrypto) [W] ([pub.dev](https://pub.dev/packages/flutter_secure_storage)); `file_selector`, `url_launcher`, `app_links`,
  Desktop-DnD (`desktop_drop`/`super_drag_and_drop`), Share-Intents (`receive_sharing_intent`/`share_handler`) [E – verbreitete Pakete,
  nicht einzeln geprüft]. `flutter_web_auth_2` 5.1.0: ASWebAuthenticationSession (iOS/macOS), Chrome Auth Tab (Android) [W]
  ([pub.dev](https://pub.dev/packages/flutter_web_auth_2)). `passkeys` 2.23.1: Android/iOS/macOS/Web/Windows, **kein Linux**, RP-ID muss zur
  eigenen Domain passen [W] ([pub.dev](https://pub.dev/packages/passkeys)) → für R7 untauglich; nötig ist eigenes FFI zu `webauthn.dll`/`libfido2` [I].
  HTTP über `dart:io` ohne CORS [I]. Smartcard/PKCS#11 nur über `dart:ffi` [I].
- **PDF [W]:** `pdfrx` 2.6.5 (MIT; PDFium auf Android/iOS/Windows/macOS/Linux/Web-WASM, optional CoreGraphics auf Apple)
  ([pub.dev](https://pub.dev/packages/pdfrx)). PAdES in Dart: `pdf_plus` (Fork von dart_pdf, inkrementelles Signieren PAdES B-B/B-T,
  externes Signieren, Validierung) und `nkpdfsign` 0.1.1 (nur B-B, unsichtbar) [S] – jung, kleine Maintainer-Basis [I].
- **Distribution:** Kein einheitlicher Desktop-Bundler im SDK; Windows-MSIX über Paket `msix` (Store signiert selbst; Self-Hosting braucht `.pfx`)
  [W]; Linux: Snap Store offiziell empfohlen, Flatpak/AppImage nicht in der Doku [W]. Auto-Update nur über Drittpakete [E].
- **DX/Ökosystem/Lizenz:** Ein Sprach-/UI-Stack für alle Plattformen, sehr gutes Hot Reload, Widget Previews stable [W]; BSD-3-Clause.
- **Variante „Flutter-UI + Rust-Core":** `flutter_rust_bridge` 2.13.0, MIT, Android/iOS/Linux/macOS/Web/Windows, „Flutter Favorite" [W]
  ([pub.dev](https://pub.dev/packages/flutter_rust_bridge)). Erlaubt denselben Rust-Signatur-Kern wie bei Tauri.
- **Fit [I]:** Beste Mobile-Reife bei weiterhin guter Desktop-Unterstützung. Schwächen: Dart-Krypto/PAdES-Ökosystem,
  Desktop-Packaging/Update selbst zusammenstellen, FIDO2 nur via FFI.

### 4.3 Compose Multiplatform / Kotlin Multiplatform

- **Status [W]:** Compose Multiplatform **1.12.1** (22.09.2026), 1.13.0-alpha02 (06.10.2026)
  ([Releases](https://github.com/JetBrains/compose-multiplatform/releases)). Stabilität: Android, iOS, **Desktop (JVM)** stable, Web (Wasm) Beta
  ([Supported platforms](https://kotlinlang.org/docs/multiplatform/supported-platforms.html)).
- **Native Desktop-Targets?** **Nein.** Die Kompatibilitätsseite formuliert Desktop-Anforderungen in JDK-Begriffen (JDK ≥ 11, Packaging via
  `jpackage` ab JDK 17) [W] ([Compatibility](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)); die
  Release-Notes 1.13.0-alpha02 sprechen von „JRE fonts … stripped from the distributable" – also weiter JRE-basiert [W].
  Es gibt nur Community-Experimente (z. B. `compose-native` für Linux/Windows, GraalVM-Native-Image-Versuche) [S].
- **Bewertung:** **K.O. nach K2** (JVM). Ironie [I]: Gerade auf der JVM könnte man PDF-AS/PDFBox/BouncyCastle direkt weiterverwenden –
  das widerspricht aber der Nutzervorgabe. Größe Desktop ≈ Größe einer gejlinkten JRE + App; zur Einordnung: PDF-Overs JRE-Bündel
  sind 41–52 MB komprimiert [C].

### 4.4 .NET MAUI

- **Status [W]:** Offiziell unterstützt: Android, iOS, **Mac Catalyst**, Windows (WinUI 3) – **kein Linux**
  ([Supported platforms](https://learn.microsoft.com/en-us/dotnet/maui/supported-platforms), Seite vom 08.09.2026, Monikers bis .NET 11).
  Tizen über Samsung.
- **Linux-Ausweg:** Avalonia entwickelt ein **MAUI-Backend** (Linux + WebAssembly) – erste Preview März 2026 auf .NET 11 Preview;
  bleibt Preview bis zur .NET-11-GA (November 2026); Wayland noch nicht, Storage/Media-APIs unvollständig [S]
  ([The Register](https://theregister.com/2026/03/23/maui_linux_avalonia), [Avalonia-Blog 11/2025](https://avaloniaui.net/blog/net-maui-is-coming-to-linux-and-the-browser-powered-by-avalonia)).
- **Bewertung:** **K.O. nach K1** (Stand heute). Stärken wären MAUI Essentials (FilePicker, Share, SecureStorage, Launcher, WebAuthenticator)
  und das .NET-Krypto-Ökosystem (5.7). Fachpresse berichtet über Bugs und schwierige .NET-9→10-Migration [S].

### 4.5 Avalonia UI 12

- **Status [W]:** Avalonia **12.1.4** (09.10.2026) ([NuGet](https://www.nuget.org/packages/Avalonia)); 12.0 am 07.04.2026
  ([Blog „Avalonia 12"](https://avaloniaui.net/blog/avalonia-12)) mit Schwerpunkt Performance/Stabilität, Mobile-Navigation
  (TabbedPage, DrawerPage …), Android-Start 1960 → 460 ms mit NativeAOT (Herstellerangabe), Linux-Barrierefreiheit über AT-SPI2,
  Mac Catalyst in Avalonia.iOS, **WebView-Komponente jetzt Open Source** (nutzt System-WebView), Wayland in privater Preview.
  **Achtung:** Die NuGet-Paketbeschreibung spricht weiterhin von „experimental support for Android, iOS and WebAssembly" [W].
- **Plattformen:** Windows/macOS/Linux sehr reif (X11/XWayland); iOS/Android nutzbar, offiziell noch „experimental" [W]/[I].
- **Integrationen [W]:** `StorageProvider` (Öffnen/Speichern/Ordner) auf Windows, macOS, Linux, Browser, Android, iOS – Pfad-basierte
  Lookups nicht auf Mobile/Browser ([Doku](https://docs.avaloniaui.net/docs/concepts/services/storage-provider/)). Launcher, DnD (Desktop)
  vorhanden [E]. Secure Storage, Share-Sheet, Deep Links mobil: über .NET-for-iOS/Android-APIs selbst anbinden [I].
  FIDO2: `DSInternals.Win32.WebAuthn` (Windows-WebAuthn-Wrapper) [S], `libfido2` per P/Invoke [I]; PKCS#11 über `Pkcs11Interop`,
  PC/SC über `PCSC`-Pakete [E].
- **Krypto/PDF:** .NET-Krypto nutzt OS-Bibliotheken; **PKCS#12 lesen/schreiben auf allen Plattformen inkl. iOS/Android** [W]
  ([Cross-platform cryptography](https://learn.microsoft.com/en-us/dotnet/standard/security/cross-platform-cryptography)).
  Einschränkung: **Brainpool-Kurven auf Apple-Plattformen nicht unterstützt** [W] → relevant, falls Keystores/Zertifikate Brainpool nutzen
  (offene Frage). PAdES: z. B. `SimpleSign.PAdES` 0.6.0 (MIT, ETSI EN 319 142) [S]; BouncyCastle .NET, PDFsharp (MIT),
  iText (AGPL/kommerziell) [E].
- **Größe [E]:** Self-contained .NET + Skia: grob 30–80 MB; mit NativeAOT/Trimming kleiner. Kein Runtime-Install beim Nutzer.
- **Distribution/Update:** Standard-.NET-Publishing; Desktop-Auto-Update z. B. mit **Velopack** (Rust-Kern, Bindings für C#, Rust, JS, C++, Python;
  Windows/macOS/Linux) [W]/[S] ([Velopack](https://docs.velopack.io/)).
- **Lizenz/Ökosystem:** Kern MIT; bezahlte Pro-Komponenten/Tools; Marke „Accelerate" 2026 abgelöst [S]. Firma AvaloniaUI OÜ, hohe Release-Kadenz.
- **Fit [I]:** Stärkstes Krypto-Fundament der Top-3, exzellenter Linux-Desktop; Mobile ist der Schwachpunkt.

### 4.6 Uno Platform 6.6

- **Status [W]:** 6.6 (InfoQ 07.08.2026): Native AOT auf Android/iOS/Linux/macOS/Windows, Vulkan opt-in (Linux/Android), Screenreader nur
  Windows/macOS/WebAssembly – **Android/iOS „in progress", Linux „planned"** ([InfoQ](https://infoq.com/news/2026/08/uno-platform-6-6)).
  Seit 6.0 einheitlicher Skia-Renderer auf allen Zielen außer WinAppSDK [S].
- **Lizenz [W]:** Framework **Apache-2.0**; Uno Platform Studio (Hot Design) kostenpflichtig (Pro 39 $/Monat …)
  ([Pricing](https://platform.uno/pricing/)).
- **Fit [I]:** Wie Avalonia (.NET-Krypto), WinUI-API-Modell; Barrierefreiheit auf Linux/Mobile ist für eine eGovernment-App ein Malus
  (öffentliche Stellen: Barrierefreiheitsanforderungen).

### 4.7 Qt 6 / QML

- **Status [W]:** **Qt 6.12 LTS** (30.09.2026; 5 Jahre Wartung; QML Hot Reload; Canvas Painter stabil)
  ([Qt-Blog](https://qt.io/blog/qt-6.12-released)). LTS-Patch-Releases kommen **zuerst/teilweise nur für Kommerzkunden**
  („immediate access … limited to commercial customers") [W] ([Qt Releases](https://doc.qt.io/qt-6/qt-releases.html)).
- **PDF [W]:** Qt PDF (PDFium-basiert), LGPLv3/GPLv2/kommerziell ([Qt PDF](https://doc-snapshots.qt.io/qtpdf/qtpdf-index.html));
  Mobile-Support von Qt PDF dort nicht ausgewiesen → zu verifizieren.
- **Lizenz-Risiko iOS [S]:** LGPL verlangt Relink-Möglichkeit; auf iOS (statisches Linken, App Store) umstritten, viele Entwickler
  greifen zur kommerziellen Lizenz ([Qt-Wiki](https://wiki.qt.io/Licensing-talk-about-mobile-platforms), alte Quelle).
- **Integrationen [I]:** Native C/C++ ist der direkteste Weg zu `webauthn.dll`, `libfido2`, PKCS#11, OpenSSL-CMS, PoDoFo – technisch die
  beste Integrationsbasis, aber höchster Entwicklungsaufwand (C++, Speicher­sicherheit).
- **Größe [E]:** 25–60 MB je nach Modulen (QML + PDF + Netzwerk).
- **Fit [I]:** Technisch reif auf allen fünf Plattformen, aber DX und Lizenzlage (LGPL auf iOS, LTS-Politik) sprechen gegen Qt für ein
  kleines Team/Open-Source-Projekt.

### 4.8 Slint

- **Status [W]:** Slint **1.18.1** (21.09.2026) ([Releases](https://github.com/slint-ui/slint/releases)); iOS-Support seit 1.12 (Juni 2025),
  **auf iOS nur Rust** (Winit + Skia) ([iOS-Doku](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/ios/)).
- **Lizenz [W]:** GPLv3 **oder** Royalty-free-Lizenz (proprietäre Desktop/Mobile-Apps, nicht Embedded) **oder** kommerziell
  ([Pricing](https://slint.dev/pricing)).
- **Fit [I]:** Kleinste Binaries, aber keine eingebauten Datei-Dialoge/WebView/Mobile-Integrationen (Crates wie `rfd` nötig); PDF-Rendering
  nur als Bild (PDFium). Für eine dokumentzentrierte App mit vielen OS-Integrationen zu „roh".

### 4.9 Dioxus

- **Status [W]:** Stable **0.7.10** (30.07.2026), **0.8.0-alpha.1** (31.07.2026) ([Releases](https://github.com/DioxusLabs/dioxus/releases)).
  0.7 (Sept. 2025) brachte Mobile-Tooling und den nativen Renderer **Blitz** (WGPU) – **experimentell**; Standard auf Desktop/Mobile ist die
  System-WebView (wry) [W]/[S] ([Release 0.7](https://dioxuslabs.com/blog/release-070)). Bundler stammte ursprünglich aus Tauri v2, jetzt vendored [W].
- **Fit [I]:** Rust-only, kleines Binary, aber pre-1.0 mit Breaking Changes und deutlich kleinerem Plugin-Ökosystem als Tauri.

### 4.10 React Native (+ react-native-windows / react-native-macos)

- **Status [W]/[S]:** RN-Kern zweimonatlich, 0.88 am 12.10.2026 geplant/erschienen ([Releases](https://reactnative.dev/docs/releases));
  `react-native-macos` 0.83.0 (30.09.) synchron zu RN 0.83.10 [W] ([Releases](https://github.com/microsoft/react-native-macos/releases));
  `react-native-windows` 0.84 (Juni 2026) [S]. Desktop-Forks hinken dem Kern also mehrere Minor-Versionen hinterher.
- **Linux:** Kein offizielles/erstanbieterseitiges Target; nur Community-Renderer (z. B. GTK-Bridges) [S].
- **Bewertung:** **K.O. nach K1.**

### 4.11 Capacitor (Ionic) + Electron

- **Status [W]:** Capacitor **8** (08.12.2025; SPM-Standard auf iOS, Edge-to-Edge Android), Capacitor 9 für Ende November 2026 angekündigt [S]
  ([Ionic-Blog](https://ionic.io/blog/announcing-capacitor-8)). Capacitor selbst: iOS, Android, Web.
  Desktop: `@capacitor-community/electron` **nicht mehr gepflegt** (letztes Release 2023) [S]; Nachfolger `@capawesome/capacitor-electron`
  (MIT, Juli 2026; Capacitor ≥ 6, Electron ≥ 28; Plugins brauchen eigene Electron-Implementierung) [W]
  ([Capawesome](https://capawesome.io/blog/announcing-the-capacitor-electron-platform/)).
- **Fit [I]:** Eine Web-Codebasis, sehr schnelle UI-Entwicklung, gutes Plugin-Ökosystem auf Mobile; Desktop-Größe = Electron (siehe 4.12);
  die Capacitor↔Electron-Brücke ist Drittanbieter-abhängig. HTTP muss nativ laufen (CapacitorHttp bzw. Electron-Main-Prozess) wegen CORS.

### 4.12 Electron

- **Status [W]/[S]:** Unterstützt werden die letzten drei Majors; Electron 45 soll am 20.10.2026 stable werden, 44.4.5 am 23.09.2026 [S]
  ([Timeline](https://www.electronjs.org/docs/latest/tutorial/electron-timelines), [endoflife.date](https://endoflife.date/electron)).
- **Größe [S]:** Installer typischerweise 80–150 MB komprimiert; Hello World ≈ 85 MB.
- **Bewertung:** Exzellente Desktop-DX und Distribution (electron-builder, Auto-Update), aber **kein Mobile → K.O. nach K1**;
  widerspricht außerdem „keine große Installation" (größer als die heutige JRE-Lösung).

### 4.13 Wails (Go + System-WebView)

- **Status [W]:** **v3.0.0-beta.29** (13.10.2026), alle v3-Releases als Pre-release markiert („API is stable, but you may still encounter issues
  before the final 3.0 release"); v2 bleibt die stabile Linie ([Releases](https://github.com/wailsapp/wails/releases)).
  Mobile (iOS WKWebView, Android WebView) in v3 – in der „next"-Doku als **EXPERIMENTAL** markiert [W]
  ([Mobile Guide](https://wails.io/docs/next/guides/mobile)).
- **Fit [I]:** Go hat ein sehr gutes Krypto-/PDF-Ökosystem (Standardbibliothek `crypto/x509`, PDF-Signier-Bibliotheken) und kleine Binaries;
  Mobile-Reife und Release-Status (Dauer-Beta) sind aber zu riskant.

### 4.14 Reine PWA (Zero-Install)

- **Plattform:** läuft überall; iOS 26 öffnet zum Home-Bildschirm hinzugefügte Seiten standardmäßig als Web-App; Home-Screen-Web-Apps bleiben
  in der EU erhalten [S].
- **Funktionale Blocker [W]/[I]:**
  1. **A-Trust-Flow (R5):** Cross-Origin-Requests aus dem Browser zu `service.a-trust.at` brauchen CORS-Freigaben von A-Trust; ohne diese nur über
     einen Proxy-Server, der Handy-Signatur-Zugangsdaten sähe → **K3 verletzt** [I] (CORS-Verhalten von A-Trust nicht verifiziert).
  2. **FIDO2 (R7):** WebAuthn im Browser nur für die eigene RP-ID; Ausnahme *Related Origin Requests* (Chrome ≥ 129, Safari ≥ 18) erfordert eine
     `/.well-known/webauthn`-Datei **auf der A-Trust-Domain** [W]/[S] ([Chrome-Blog](https://developer.chrome.com/blog/passkeys-updates-chrome-129)).
  3. **Lokale BKU (R6):** Chrome ≥ 142 verlangt für Zugriffe öffentlicher Seiten auf `127.0.0.1` eine **Local-Network-Access-Berechtigung** [S]/[W]
     ([Chrome LNA](https://developer.chrome.com/blog/local-network-access)); zusätzlich CORS auf BKU-Seite nötig [I].
  4. **„Öffnen mit" (R11):** File Handling API nur Chromium-Desktop [W] ([MDN](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/How_to/Associate_files_with_your_PWA)).
- **Bewertung:** **Funktionales K.O. (K3)** für Parität. Sinnvoll höchstens als spätere „Lite"-Variante (z. B. Positionieren/Prüfen) mit
  WASM-kompiliertem Rust-Kern [I].

---

## 5. Querschnittsthemen (framework-übergreifend)

### 5.1 FIDO2/WebAuthn mit fremder RP-ID (`service.a-trust.at`) – der kritische Pfad

Im Original holt `MobileBKUConnector` einen FIDO2-Link aus der A-Trust-HTML-Seite (`MobileBKUConnector.java:338-340`, `:384`), und
`MobileBKUFido2Composite` führt die Assertion mit der Origin `https://service.a-trust.at` aus (`MobileBKUFido2Composite.java:61`) [C].
Das Modul `webauthn-client-java` 0.0.1 ist nicht im Referenz-Quellbaum; sein POM nennt JNA, `jna-platform`, CBOR und eine Public-Suffix-List [W]
([POM](https://apps.egiz.gv.at/maven/at/a-sit/webauthn-client-java/0.0.1/webauthn-client-java-0.0.1.pom)). **Vermutung [I]:** Windows über
`webauthn.dll`, Linux/macOS über `libfido2` (die GUI verweist bei fehlender Unterstützung auf die libfido2-Installation, `Fido2ConfigurationComposite.java:40`).

| Plattform | Nativer Weg für fremde RP-ID | Bewertung |
|---|---|---|
| Windows | Win32-WebAuthn-API (`webauthn.dll`): native Apps dürfen als WebAuthn-Client auftreten [W] ([Microsoft Learn](https://learn.microsoft.com/en-us/windows/security/identity-protection/hello-for-business/webauthn-apis)) | machbar (Rust/C#/C++/Dart-FFI) |
| macOS | AuthenticationServices verlangt Associated Domains (`webcredentials:`) bzw. das restriktive Browser-Entitlement `com.apple.developer.web-browser.public-key-credential` [W]/[S] ([Apple](https://developer.apple.com/documentation/authenticationservices/supporting-passkeys), [Forum](https://developer.apple.com/forums/thread/829169)); Alternative: **CTAP2 direkt über `libfido2`** (nur Security-Keys, kein iCloud-Passkey) [I] | machbar über libfido2; Mac-App-Store-Sandbox-Freigaben ungeklärt |
| Linux | `libfido2` über hidraw (udev-Rechte) [I] | machbar |
| iOS | Passkeys/Security-Keys nur für Associated Domains; für fremde Domains Apple-Empfehlung: **ASWebAuthenticationSession** [W]/[S] ([passkeys.dev](https://passkeys.dev/docs/reference/ios)) | nur mit Browser-basiertem Flow → Architekturänderung |
| Android | Origin setzen nur für **privilegierte Apps** (Allowlist beim Credential Provider, Android ≥ 14) [W] ([Android](https://developer.android.com/identity/sign-in/privileged-apps)); sonst Digital Asset Links der RP-Domain; WebView-WebAuthn ebenfalls nur für eigene, verknüpfte Domain [W] ([Credential Manager WebView](https://developer.android.com/identity/sign-in/credential-manager-webview)) | wie iOS |
| Web/PWA | nur eigene RP-ID oder Related Origin Requests der RP [W]/[S] | ohne A-Trust unmöglich |

**Konsequenz [I]:** Auf Mobile FIDO2 zunächst weglassen (PDF-Over bietet FIDO2 ohnehin nur optional an: `Constants.java:221`
`CFG_FIDO2_BY_DEFAULT`, `messages_en.properties:266`) oder mit A-Trust klären. Für die Framework-Wahl heißt das: **direkter FFI-Zugang zu C-Bibliotheken
(`libfido2`, `webauthn.dll`) ist Pflicht** – Rust, C#, C++, Go und Dart-FFI können das; reine Web-Stacks nur über Native-Module.

### 5.2 HTTP, Cookies, HTML-Parsing in WebView-Frameworks

PDF-Over spricht A-Trust mit Apache HttpClient an und parst HTML mit jsoup (`MobileBKUConnector.java:33-35,201`; `ATrustParser.java:11`) [C].
In Tauri/Wails/Dioxus/Capacitor/Electron muss das **im nativen Teil** passieren (Tauri `http`-Plugin/Rust `reqwest`+`scraper`, CapacitorHttp,
Electron-Main) – `fetch` aus der WebView scheitert an CORS [I]. Flutter (`dart:io`), .NET (`HttpClient`, AngleSharp) und Qt (`QNetworkAccessManager`)
haben dieses Problem nicht [I].

### 5.3 Lokale BKU (127.0.0.1:3495)

Desktop-only (R6). Native Frameworks: trivial. Browser: LNA-Prompt (Chrome ≥ 142) + CORS [S]. iOS/Android: eine lokale BKU ist dort nicht zu
erwarten [I]; App Transport Security bzw. Android-Cleartext-Policy wären für `http://127.0.0.1` zusätzlich zu konfigurieren [I].

### 5.4 PDF-Rendering-Optionen

| Option | Plattformen | Lizenz | Passt zu |
|---|---|---|---|
| **pdf.js** v6.4.299 (03.10.2026) [W] ([Releases](https://github.com/mozilla/pdf.js/releases)) | jede WebView/Browser | Apache-2.0 | Tauri, Wails, Dioxus, Capacitor, Electron, PWA |
| **PDFium**-Bindings: `pdfium-render` 0.9.4 (Rust) [W], `pdfrx` 2.6.5 (Flutter) [W], .NET-Wrapper [E], Qt PDF [W] | alle 5 | BSD-artig (PDFium) + Wrapper-Lizenz | Rust/Flutter/.NET/Qt |
| **PDFKit** (iOS/macOS), **PdfRenderer**/`androidx.pdf` (1.0.0-beta01 vom 26.08.2026) [W] ([Release-Notes](https://developer.android.com/jetpack/androidx/releases/pdf)) | jeweils nativ | System | mobile Plugins |

Empfehlung [I]: Für exakte, plattformgleiche Darstellung (Signaturblock-Vorschau muss mit dem späteren PDF übereinstimmen) **ein** Renderer
für alle Plattformen – pdf.js (WebView-Stacks) oder PDFium (native Stacks).

### 5.5 Store-Distribution, Code-Signing, Notarisierung

- **Apple [W]/[I]:** Notarisierung für Direktvertrieb macOS; App-Store-Review-Richtlinie 4.2 („Minimum Functionality") betrifft reine
  Web-Wrapper – eine PDF-Signatur-App mit nativen Funktionen sollte unkritisch sein [S]/[I]. Exportkontroll-Angabe (Verschlüsselung) im
  App Store Connect nötig [E].
- **Windows [W]/[S]:** Microsoft **Artifact Signing** (vormals Trusted Signing, Umbenennung Jan. 2026): Public-Trust-Zertifikate für
  **Organisationen** in EU/UK/USA/Kanada, **Einzelentwickler nur USA/Kanada**; ab 9,99 $/Monat
  ([Quickstart](https://learn.microsoft.com/azure/trusted-signing/quickstart), [DevClass](https://devclass.com/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/)).
  MSIX/Store möglich (Store signiert selbst) [W].
- **Linux:** Flathub/AppImage/deb/rpm (Tauri dokumentiert alle) [W]; Flutter offiziell nur Snap [W].
- **Lizenz × App Store [I]:** Copyleft-Lizenzen (GPL; vermutlich auch EUPL) können mit App-Store-Nutzungsbedingungen kollidieren
  (Präzedenzfall VLC 2011). EUPL-1.2 hat eine Kompatibilitätsliste (GPL, LGPL, MPL …) [W]
  ([EUPL](https://interoperable-europe.ec.europa.eu/collection/eupl/solution/eupl-freeopen-source-software-licence-european-union)). → Juristisch prüfen,
  insbesondere falls Assets/Konfiguration aus PDF-Over (EUPL) übernommen werden.

### 5.6 Auto-Update

| Stack | Lösung |
|---|---|
| Tauri | offizielles `updater`-Plugin, signierte Updates, Desktop [W] |
| Electron | `autoUpdater`/`electron-updater` [E] |
| .NET (Avalonia/Uno) | Velopack (Win/macOS/Linux) [W]/[S]; MSIX App Installer [E] |
| Flutter | Drittpakete (Sparkle/WinSparkle-Wrapper), Linux via Store/Flatpak [E] |
| Qt | Qt Installer Framework / Maintenance Tool [E] |
| Mobile (alle) | ausschließlich über App Store / Play Store [I] |

### 5.7 Krypto-/PAdES-Ökosystem je Sprache (nur Einordnung)

| Sprache | Einschätzung |
|---|---|
| **Rust** | RustCrypto (`cms`, `x509-cert`, `pkcs12`-Umfeld), `ring`/`aws-lc-rs`; PDF: `lopdf`, `pdf-writer`, `pdf_oxide` 0.3.78 (MIT/Apache-2.0, Signatur-Erstellung „PAdES" laut Doku ab 0.3.50, Projekt erst seit 11/2025) [W]/[S]; WASM-fähig → gemeinsamer Kern für Desktop/Mobile/Web [I] |
| **C# (.NET 10)** | Ausgereifteste Basis: OS-gestützte X.509/PKCS#12 auf allen Plattformen, CMS, BouncyCastle .NET, PDFsharp, PAdES-Bibliotheken; Apple ohne Brainpool [W]/[S] |
| **Dart** | `pointycastle`, `pdf_plus` (PAdES B-B/B-T), `nkpdfsign` – jung, kleine Basis [S] |
| **C++** | OpenSSL-CMS, PoDoFo, qpdf, PDFium – mächtig, aber aufwendig [E] |
| **Go** | Standard-Krypto sehr gut, PDF-Signierbibliotheken vorhanden [E] |
| **JS/TS** | pdf.js, PKI.js, `@signpdf`, `pdf-lib` (wenig gepflegt) [E] |

**JCEKS-Keystores (R8)** werden außerhalb von Java von keiner gängigen Bibliothek unterstützt [I] – vermutlich nur PKCS#12 anbieten und
eine Migrationshilfe dokumentieren (offene Frage).

---

## 6. Bewertungsmatrix

### 6.1 Noten je Kriterium (1–5) und gewichtete Summe (0–100)

Gewichte: P 20 · N 20 · K 15 · G 12 · D 10 · X 10 · O 8 · L 5.

| Rang | Kandidat | P | N | K | G | D | X | O | L | **Σ /100** | K.O.? |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | **Tauri 2** | 4 | 4 | 4 | 4,5 | 5 | 4 | 4 | 5 | **84,2** | – |
| (2) | *Variante: Flutter-UI + Rust-Core* | 5 | 4 | 4 | 4 | 3 | 4 | 4 | 5 | *83,0* | – |
| 2 | **Flutter** | 5 | 4 | 3 | 4 | 3 | 5 | 4 | 5 | **82,0** | – |
| 3 | **Avalonia UI 12** | 4 | 4 | 4,5 | 3,5 | 4 | 4 | 4 | 5 | **81,3** | – |
| 4 | Uno Platform 6.6 | 4 | 3,5 | 4,5 | 3 | 4 | 3,5 | 4 | 5 | 77,1 | – |
| 5 | Qt 6.12 / QML | 4,5 | 4,5 | 4 | 3 | 3 | 3 | 4 | 3 | 76,6 | – |
| 6 | Capacitor 8 + Electron | 3,5 | 4 | 3,5 | 2 | 4,5 | 4,5 | 3,5 | 5 | 73,9 | – (Desktop-Größe) |
| 7 | Electron | 2 | 4,5 | 4 | 1 | 5 | 5 | 5 | 5 | 73,4 | **K1** |
| 8 | Wails v3 | 2,5 | 3,5 | 4 | 4,5 | 3,5 | 4 | 3 | 5 | 71,6 | – (Mobile experimentell) |
| 9 | Dioxus 0.7 | 3 | 3 | 4 | 4,5 | 3,5 | 3,5 | 3 | 5 | 70,6 | – |
| 10 | Compose Multiplatform | 3 | 4 | 3,5 | 2 | 3 | 4 | 5 | 5 | 70,3 | **K2** |
| 11 | PWA | 3 | 1 | 3,5 | 5 | 4 | 5 | 4 | 5 | 67,9 | **K3** |
| 12 | .NET MAUI | 2 | 4 | 4,5 | 3 | 3 | 3 | 3 | 5 | 66,5 | **K1** |
| 13 | Slint | 3 | 2,5 | 3,5 | 5 | 3 | 3,5 | 3 | 4 | 66,3 | – |
| 14 | React Native (+Win/mac) | 2 | 3,5 | 3 | 4 | 3 | 4 | 4 | 5 | 66,0 | **K1** |

Alle Noten sind **[I]** (eigene Bewertung auf Basis der in Abschnitt 4/5 belegten Fakten).

### 6.2 Begründung der Kernnoten (Auszug)

- **Tauri P=4:** Desktop reif, Mobile offiziell, aber Plugin-Lücken (Share, Keychain) und weniger Praxisberichte [W]/[S].
- **Tauri G=4,5:** kleinste Installer der Top-3; Abzug für WebKitGTK-Abhängigkeit/AppImage-Größe auf Linux [W].
- **Tauri D=5:** einziger Kandidat der Spitzengruppe, der Desktop- *und* Mobile-Bundling (MSI/NSIS, DMG/App Store, deb/rpm/AppImage/Flatpak-Guide, Play, iOS) plus offiziellen signierten Updater aus einer Toolchain liefert [W] (Electron kann Desktop ebenso gut, aber kein Mobile).
- **Flutter P=5:** alle fünf stabil, beste Mobile-Reife [W]; **K=3:** Dart-PAdES-Bibliotheken jung [S]; **D=3:** Packaging/Update selbst zusammensetzen [W]/[E].
- **Avalonia K=4,5:** .NET-Krypto auf allen Plattformen (PKCS#12 überall), Abzug für Brainpool auf Apple [W]; **P=4:** Mobile laut NuGet „experimental" [W].
- **Qt L=3:** LGPL auf iOS, kommerzielle LTS-Politik [W]/[S]; **X=3:** C++.
- **PWA N=1:** A-Trust/FIDO2/BKU nicht ohne Dritte lösbar (5.1–5.3).

### 6.3 Sensitivitätsanalyse

| Szenario | Gewichte (P/N/K/G/D/X/O/L) | Tauri | Flutter | Avalonia | Sieger |
|---|---|---|---|---|---|
| Basis | 20/20/15/12/10/10/8/5 | **84,2** | 82,0 | 81,3 | Tauri |
| Mobile-first | 30/20/15/6/8/10/8/3 | 82,8 | **84,0** | 81,5 | Flutter |
| Krypto/Integration-first | 15/25/25/10/8/7/6/4 | **83,4** | 78,6 | 82,3 | Tauri (knapp vor Avalonia) |
| Klein & installationsarm | 18/18/12/25/10/8/5/4 | **85,3** | 81,6 | 79,5 | Tauri |

Interpretation [I]: Kein Szenario hebt einen K.O.-Kandidaten oder Uno/Qt in die Top-3. Die Wahl zwischen Tauri und Flutter hängt
im Wesentlichen davon ab, wie stark Mobile-Parität gegenüber Desktop-Größe und Signatur-Kern-Sprache gewichtet wird.

---

## 7. Top-3-Empfehlung mit Risiken

### Platz 1 – Tauri 2 (Rust-Kern + Web-UI, pdf.js)

**Warum [I]:** Kleinste Installation (Größenordnung einstellige MB statt ≈ 50 MB JRE heute, R12), vollständige Desktop-Integrationen
(Dialog, Opener, Deep-Link, Dateizuordnung, Single-Instance, signierter Updater), nativer HTTP ohne CORS (R5), direkter FFI-Zugang für FIDO2
(R7) und lokale BKU (R6), alle Distributionskanäle dokumentiert. Rust ist gleichzeitig eine sehr gute Sprache für den neu zu bauenden
PDF-AS-Kern und lässt sich nach WASM (spätere PWA-Lite) und in Flutter (Plan B) einbetten.

**Risiken:**
1. **Mobile-Reife/Plugin-Lücken** (Share-Sheet, Keychain nur Community) → eigene Swift/Kotlin-Plugins einplanen. *Mitigation:* früher Mobile-Spike.
2. **WebKitGTK unter Linux** (Rendering-/Performance-Unterschiede, pdf.js) [I] → *Mitigation:* PDFium-Rendering im Rust-Kern als Fallback; mittelfristig
   optionale CEF-Runtime aus Tauri 3 [W, Alpha].
3. **Tauri-3-Migration** (Runtime-Split, Breaking Changes) [W] → *Mitigation:* App-Logik im Kern-Crate, dünne Tauri-Schicht.
4. **Zwei Sprachen (TS + Rust)**, Rust-Lernkurve [I].
5. WebView-Versionen auf alten Android-Geräten (System WebView) [I].

### Platz 2 – Flutter (optional mit Rust-Kern über `flutter_rust_bridge`)

**Warum [I]:** Ein UI-Stack für alle fünf Plattformen mit der besten Mobile-Reife, pixelgleiche UI, PDFium-Rendering über `pdfrx`,
Secure Storage auf allen Plattformen inkl. Linux. Mit Rust-Kern entfällt die Dart-Krypto-Schwäche.

**Risiken:**
1. **Governance-Wechsel Desktop → Canonical** (Mai 2026) [S]; Desktop-Roadmap nicht mehr allein bei Google.
2. **Dart-PAdES/CMS-Ökosystem jung** [S] → bei reiner Dart-Lösung hohes Eigenrisiko; mit Rust-Kern zwei Sprachen + Bridge-Komplexität.
3. **Desktop-Packaging/Auto-Update** nicht aus einem Guss (msix, Snap, Drittpakete) [W]/[E].
4. **Breaking Changes** durch Material/Cupertino-Auslagerung (Deprecation im November 2026) [W]; Mindest-OS iOS 15/macOS 12 [W].
5. FIDO2 nur über eigenes FFI (kein Paket mit fremder RP-ID) [W]/[I].

### Platz 3 – Avalonia UI 12 (.NET 10)

**Warum [I]:** Bestes Krypto-/X.509-Fundament (PKCS#12, CMS, OS-Zertifikatsspeicher auf allen Plattformen), reifster Linux-Desktop aller
Kandidaten inkl. Barrierefreiheit (AT-SPI2), XAML-Produktivität, MIT-Kern, Velopack für Desktop-Updates; Windows-FIDO2 über bestehende
.NET-Wrapper.

**Risiken:**
1. **Mobile offiziell „experimental"** (NuGet-Beschreibung) trotz Fortschritten in 12.0 [W].
2. **Größe** (self-contained .NET) näher an der heutigen JRE-Lösung als Tauri [E].
3. **Brainpool nicht auf Apple-Plattformen** in .NET-Krypto [W] → ggf. BouncyCastle als Fallback.
4. Mobile Integrationen (Share, Secure Storage, Deep Links) selbst anbinden [I]; Wayland erst in privater Preview [W].
5. Hersteller mit bezahlten Zusatzprodukten – Abhängigkeit von der Geschäftsstrategie eines Unternehmens [I].

### Empfohlenes Vorgehen (Spikes vor der Festlegung, ca. 2–3 Wochen) [I]

1. **A-Trust-Flow nativ** (POST + HTML-Parsing + Cookies) aus Rust auf allen fünf Plattformen (Test-URL aus `Constants.java:109`).
2. **FIDO2-Assertion mit Origin `https://service.a-trust.at`**: Windows (`webauthn.dll`), Linux/macOS (`libfido2`); Mobile: Machbarkeit klären.
3. **PDF-Vorschau + Positionieren** mit pdf.js in WKWebView, WebView2, WebKitGTK, Android-WebView – Performance bei großen PDFs.
4. **„Öffnen mit"/„Öffnen in"**: Windows-Argumente, macOS/iOS `RunEvent::Opened`, Android-Intents (`content://`-URIs).
5. **CI-Pipeline**: Signierung + Notarisierung (macOS), MSI/MSIX (Windows), AppImage/Flatpak (Linux), TestFlight/Play-Internal.
6. Gleiche Spikes 1–3 als Gegenprobe in Flutter + `flutter_rust_bridge`, um Plan B zu validieren.

---

## 8. Plattformspezifische Einschränkungen (Zusammenfassung)

- **Windows:** WebView2 nötig für WebView-Stacks (ab Win10 1803 vorinstalliert, Installer-Schritt im Tauri-Bundler) [W]; FIDO2 über `webauthn.dll` [W];
  Artifact Signing nur für Organisationen in der EU [W]; Flutter braucht VC++-Runtime-DLLs [W].
- **macOS:** Notarisierung; FIDO2 über AuthenticationServices nur mit Associated Domains/Browser-Entitlement → libfido2-Weg [W]/[I];
  Mindestversionen: Tauri 10.15 [W], Flutter 12 [W], Compose 13 arm64 [W].
- **Linux:** Tauri benötigt `webkit2gtk-4.1` [W]; Flutter GTK 3 [W]; Avalonia Wayland erst Preview (X11/XWayland) [W]; MAUI nicht unterstützt [W];
  FIDO2 via libfido2/udev [I].
- **iOS:** WebAuthn/Passkeys nur für eigene Associated Domains; fremde RP → ASWebAuthenticationSession [W]; Slint nur Rust [W]; Flutter ≥ iOS 15 [W];
  Qt-LGPL-Statik-Problem [S]; keine lokale BKU [I].
- **Android:** Fremde Origin nur für privilegierte Apps (Allowlist, API ≥ 34) [W]; WebView-WebAuthn nur mit Digital Asset Links [W];
  `flutter_secure_storage` minSdk 23 [W]; `androidx.pdf` erst Beta [W].
- **Web/PWA:** CORS (A-Trust), LNA-Prompt für 127.0.0.1 (Chrome ≥ 142), File Handling nur Chromium-Desktop, WebAuthn nur eigene RP-ID [W]/[S].

---

## 9. Offene Fragen

1. Bietet A-Trust eine **offizielle API** für Drittclients statt HTML-Scraping, und wäre A-Trust bereit, Associated Domains/Digital Asset Links bzw.
   Related Origins für einen Klon zu hinterlegen? (entscheidet über FIDO2 auf Mobile/Web)
2. Sendet `service.a-trust.at` CORS-Header? (entscheidet über eine PWA-Variante) – nicht verifiziert.
3. Ist die lokale BKU (Port 3495) für die Zielgruppe noch relevant, und sendet sie CORS-Header?
4. Welche Kurven/Algorithmen haben ID-Austria/A-Trust-Zertifikate und typische Nutzer-PKCS#12-Keystores (Brainpool → .NET/Apple-Einschränkung)?
5. Muss **JCEKS** (`KeystoreConfigurationComposite.java:546`) unterstützt werden, oder reicht PKCS#12 mit Migrationshinweis?
6. Mobile-Umfang: volle Parität oder „Lite" (nur ID Austria/A-Trust, ohne lokale BKU/Keystore/FIDO2)?
7. Ist Store-Vertrieb (App Store, Play, Microsoft Store, Flathub) Pflicht, und welche **Organisation** signiert (Artifact Signing: keine EU-Einzelpersonen)?
8. Mindest-OS-Versionen der Zielgruppe (z. B. Windows 10, macOS 12, ältere Android-Geräte)?
9. Lizenz des Klons und Verträglichkeit von EUPL/Copyleft mit App-Store-Bedingungen (juristisch prüfen).
10. Interna von `webauthn-client-java` (nicht im Quellbaum) – genaue Backends je OS bestätigen.
11. Team-Skills (Rust vs. Dart vs. C#) – beeinflusst X/K stärker als jede Framework-Eigenschaft.
12. Zeitplan Tauri 3 (stable) und ob die CEF-Runtime für Linux produktionsreif wird.

---

## 10. Quellen (Abruf 2026-10-14)

**Tauri:** https://github.com/tauri-apps/tauri/releases · https://v2.tauri.app/start/ · https://v2.tauri.app/start/prerequisites/ ·
https://v2.tauri.app/plugin/ · https://v2.tauri.app/plugin/deep-linking/ · https://v2.tauri.app/plugin/updater/ · https://v2.tauri.app/plugin/stronghold/ ·
https://v2.tauri.app/learn/mobile-file-associations/ · https://v2.tauri.app/distribute/ ·
https://newreleases.io/project/github/tauri-apps/tauri/release/tauri-v3.0.0-alpha.0 ·
https://newreleases.io/project/github/tauri-apps/tauri/release/tauri-bundler-v3.0.0-alpha.0 · https://tech-insider.org/tauri-vs-electron-2026/

**Flutter:** https://flutter.dev/blog/whats-new-in-flutter-3-47 · https://docs.flutter.dev/release/archive ·
https://www.omgubuntu.co.uk/2026/05/flutter-desktop-canonical-maintained · https://docs.flutter.dev/platform-integration/linux/building ·
https://docs.flutter.dev/platform-integration/windows/building · https://pub.dev/packages/pdfrx · https://pub.dev/packages/flutter_secure_storage ·
https://pub.dev/packages/passkeys · https://pub.dev/packages/flutter_web_auth_2 · https://pub.dev/packages/flutter_rust_bridge ·
https://pub.dev/packages/pdf_plus · https://pub.dev/packages/nkpdfsign

**Kotlin/Compose:** https://kotlinlang.org/docs/multiplatform/supported-platforms.html ·
https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html · https://github.com/JetBrains/compose-multiplatform/releases ·
https://blog.jetbrains.com/kotlin/2026/05/compose-multiplatform-1-11-0

**.NET (MAUI, Avalonia, Uno):** https://learn.microsoft.com/en-us/dotnet/maui/supported-platforms ·
https://avaloniaui.net/blog/net-maui-is-coming-to-linux-and-the-browser-powered-by-avalonia · https://avaloniaui.net/blog/maui-avalonia-preview-1 ·
https://theregister.com/2026/03/23/maui_linux_avalonia · https://avaloniaui.net/blog/avalonia-12 · https://www.nuget.org/packages/Avalonia ·
https://docs.avaloniaui.net/docs/concepts/services/storage-provider/ · https://infoq.com/news/2026/08/uno-platform-6-6 · https://platform.uno/pricing/ ·
https://learn.microsoft.com/en-us/dotnet/standard/security/cross-platform-cryptography · https://www.nuget.org/packages/SimpleSign.PAdES/0.6.0 ·
https://docs.velopack.io/

**Qt / Slint / Dioxus:** https://qt.io/blog/qt-6.12-released · https://doc.qt.io/qt-6/qt-releases.html · https://doc-snapshots.qt.io/qtpdf/qtpdf-index.html ·
https://wiki.qt.io/Licensing-talk-about-mobile-platforms · https://github.com/slint-ui/slint/releases ·
https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/ios/ · https://slint.dev/pricing · https://github.com/DioxusLabs/dioxus/releases ·
https://dioxuslabs.com/blog/release-070

**React Native / Capacitor / Electron / Wails:** https://reactnative.dev/docs/releases · https://github.com/microsoft/react-native-macos/releases ·
https://devblogs.microsoft.com/react-native/ · https://ionic.io/blog/announcing-capacitor-8 · https://capawesome.io/blog/announcing-the-capacitor-electron-platform/ ·
https://www.electronjs.org/docs/latest/tutorial/electron-timelines · https://endoflife.date/electron · https://github.com/wailsapp/wails/releases ·
https://wails.io/docs/next/guides/mobile

**Web/PWA & WebAuthn:** https://developer.chrome.com/blog/local-network-access · https://developer.chrome.com/articles/file-handling ·
https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/How_to/Associate_files_with_your_PWA ·
https://developer.chrome.com/blog/passkeys-updates-chrome-129 ·
https://learn.microsoft.com/en-us/windows/security/identity-protection/hello-for-business/webauthn-apis ·
https://developer.apple.com/documentation/authenticationservices/supporting-passkeys · https://passkeys.dev/docs/reference/ios ·
https://developer.apple.com/forums/thread/829169 · https://developer.android.com/identity/sign-in/privileged-apps ·
https://developer.android.com/identity/sign-in/credential-manager-webview ·
https://apps.egiz.gv.at/maven/at/a-sit/webauthn-client-java/0.0.1/webauthn-client-java-0.0.1.pom

**PDF / Distribution / Lizenz:** https://github.com/mozilla/pdf.js/releases · https://crates.io/crates/pdfium-render · https://crates.io/crates/pdf_oxide ·
https://developer.android.com/jetpack/androidx/releases/pdf · https://learn.microsoft.com/azure/trusted-signing/quickstart ·
https://devclass.com/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/ ·
https://interoperable-europe.ec.europa.eu/collection/eupl/solution/eupl-freeopen-source-software-licence-european-union

**Wichtigste Original-Dateien (PDF-Over):** `pdf-over-gui/src/main/java/at/asit/pdfover/gui/bku/MobileBKUConnector.java`,
`…/gui/composites/mobilebku/MobileBKUFido2Composite.java`, `…/gui/bku/LocalBKUConnector.java`, `…/gui/composites/SignaturePanel.java`,
`…/gui/composites/DataSourceSelectComposite.java`, `…/gui/composites/OutputComposite.java`, `…/gui/utils/SWTUtils.java`,
`…/gui/composites/configuration/KeystoreConfigurationComposite.java`, `…/gui/workflow/config/ConfigurationDataInMemory.java`,
`pdf-over-commons/src/main/java/at/asit/pdfover/commons/Constants.java`, `pdf-over-gui/pom.xml`, `pom.xml`, `jre-bundles/`.

# Lizenztexte der Windows-Pakete

`tools/third-party-notices.sh --pdfium win-…` hängt diese Texte als Abschnitt „Windows-Installer und -Laufzeit“ an
THIRD-PARTY-NOTICES an (Lizenzbericht Befunde 1, 7, 8). Sie stammen nicht aus `Cargo.lock` bzw. dem Lockfile der
Oberfläche, sondern aus dem Windows-Teil der Werkzeugkette: NSIS, WiX und `nsis_tauri_utils` nach dem Lizenzbericht
(Analyse der Pakete 0.2.0), die WebView2-Version aus dem Loader, den `webview2-com-sys` (Version in `Cargo.lock`)
mitbringt (Versionsressource von `x64/WebView2Loader.dll`, `CORE_WEBVIEW_TARGET_PRODUCT_VERSION` in `src/bindings.rs`).
Bei einem Wechsel von Tauri-CLI, NSIS, WiX oder `webview2-com-sys` Version und Text hier nachziehen.

| Datei | Bestandteil | Version | Quelle (abgerufen 20.10.2026) |
|---|---|---|---|
| – (Text im Skript) | Microsoft-Visual-C++-Laufzeit (vcruntime), statisch in `pida.exe` (`build.windows.staticVCRuntime: true` in `tauri.conf.json`, Entscheidung 20.10.2026, Backlog Nr. 8) – Visual Studio „Distributable Code“ | Visual Studio Enterprise 2026 des Runners `windows-latest` (Version und MSVC-Toolset je Release im Schritt „MSVC-Version“ des Release-Workflows) | Lizenz: <https://visualstudio.microsoft.com/license-terms/>; Lizenzgrundlage auf den GitHub-Runnern: <https://github.com/actions/runner-images/issues/12945> |
| `Microsoft.Web.WebView2-LICENSE.txt` | WebView2-SDK: `WebView2LoaderStatic.lib` (statisch in `pida.exe`) und die daraus erzeugten Bindings von `webview2-com-sys` 0.39.1 – BSD-3-Clause | 1.0.3800.47 | <https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3800.47/License> (Text gleich wie bei 1.0.3650.58) |
| `NSIS-COPYING.txt` | NSIS-Installerlaufzeit (exehead, Plugins, MUI2-Grafik) in Setup und Uninstaller – zlib/libpng, LZMA-Modul CPL-1.0 (vollständiger Text enthalten) | 3.13 | <https://github.com/kichik/nsis/blob/v313/COPYING> |
| `NSIS-ModernUI2-License.txt` | NSIS Modern UI 2 (`MUI2.nsh`, `Pages/*.nsh`: Seiten und Dialoge von Setup und Uninstaller) – zlib/libpng-Variante, Vermerk bei jeder Weitergabe erhalten | 3.13 | <https://github.com/kichik/nsis/blob/v313/Contrib/Modern%20UI%202/License.txt> |
| `nsis_tauri_utils-LICENSE_MIT.txt` | NSIS-Plugin `nsis_tauri_utils.dll` – Apache-2.0 OR MIT (hier MIT) | 0.5.3 | <https://github.com/tauri-apps/nsis-tauri-utils/blob/nsis_tauri_utils-v0.5.3/LICENSE_MIT> |
| `MS-RL.txt` | WiX-UI-Bibliothek und `uica.dll` (WixUIExtension) im MSI – MS-RL | 3.14.1.8722 | SPDX-Text <https://github.com/spdx/license-list-data/blob/main/text/MS-RL.txt>; Quellcode: <https://github.com/wixtoolset/wix3/releases/tag/wix3141rtm> |

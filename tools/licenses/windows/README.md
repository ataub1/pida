# Lizenztexte der Windows-Pakete

`tools/third-party-notices.sh --pdfium win-…` hängt diese Texte als Abschnitt „Windows-Installer und -Laufzeit“ an
THIRD-PARTY-NOTICES an (Lizenzbericht Befunde 1, 7, 8). Sie stammen nicht aus `Cargo.lock` bzw. dem Lockfile der
Oberfläche, sondern aus dem Windows-Teil der Werkzeugkette; Versionen nach dem Lizenzbericht (Analyse der Pakete 0.2.0).
Bei einem Wechsel von Tauri-CLI, NSIS, WiX oder `webview2-com-sys` Version und Text hier nachziehen.

| Datei | Bestandteil | Version | Quelle (abgerufen 20.10.2026) |
|---|---|---|---|
| `Microsoft.Web.WebView2-LICENSE.txt` | WebView2-SDK: `WebView2LoaderStatic.lib` (statisch in `pida.exe`) und die daraus erzeugten Bindings von `webview2-com-sys` 0.39.1 – BSD-3-Clause | 1.0.3650.58 | <https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3650.58/License> |
| `NSIS-COPYING.txt` | NSIS-Installerlaufzeit (exehead, Plugins, MUI2-Grafik) in Setup und Uninstaller – zlib/libpng, LZMA-Modul CPL-1.0 (vollständiger Text enthalten) | 3.13 | <https://github.com/kichik/nsis/blob/v313/COPYING> |
| `nsis_tauri_utils-LICENSE_MIT.txt` | NSIS-Plugin `nsis_tauri_utils.dll` – Apache-2.0 OR MIT (hier MIT) | 0.5.3 | <https://github.com/tauri-apps/nsis-tauri-utils/blob/nsis_tauri_utils-v0.5.3/LICENSE_MIT> |
| `MS-RL.txt` | WiX-UI-Bibliothek und `uica.dll` (WixUIExtension) im MSI – MS-RL | 3.14.1.8722 | SPDX-Text <https://github.com/spdx/license-list-data/blob/main/text/MS-RL.txt>; Quellcode: <https://github.com/wixtoolset/wix3/releases/tag/wix3141rtm> |

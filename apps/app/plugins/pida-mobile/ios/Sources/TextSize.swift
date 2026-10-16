// PIDA – Systemschriftgröße (Dynamic Type) für die Oberfläche in der WKWebView, EUPL-1.2.
//
// WKWebView übernimmt „Einstellungen › Anzeige & Helligkeit › Textgröße“ bzw. „Bedienungshilfen ›
// Anzeige & Textgröße › Größerer Text“ nicht von selbst (nur Seiten, die `-apple-system-body` verwenden).
// Die Oberfläche bemisst Schrift in `rem` (styles/tokens.css); dieses Modul setzt deshalb die Grundschrift
// (`html { font-size }`) auf 16 px × Faktor von `UIFontMetrics` für `.body` – bei der Standardgröße „Groß“
// genau 16 px, begrenzt auf [TextSize.minScale] … [TextSize.maxScale] (darüber bräche das Layout fester
// Bedienelemente) – und folgt Änderungen zur Laufzeit (`UIContentSizeCategory.didChangeNotification`).
//
// Weg in die Seite: ein `WKUserScript` zu Beginn jedes Ladevorgangs (neu Laden) und `evaluateJavaScript` für
// die geladene Seite. Bei einer Änderung kommt ein weiteres Skript hinzu (das letzte gewinnt) – eigene
// Skripte lassen sich nicht einzeln entfernen, ohne die von Tauri mitzulöschen.

import UIKit
import WebKit

enum TextSize {
  static let minScale: CGFloat = 0.85
  static let maxScale: CGFloat = 2.0
  private static var observer: NSObjectProtocol?

  /// Faktor der Grundschrift für die Kategorie (Standard „Groß“ = 1).
  static func scale(for category: UIContentSizeCategory) -> CGFloat {
    let traits = UITraitCollection(preferredContentSizeCategory: category)
    let value = UIFontMetrics(forTextStyle: .body).scaledValue(for: 100, compatibleWith: traits) / 100
    return min(max(value, minScale), maxScale)
  }

  /// Skript für die Seite (ohne Locale: Dezimalpunkt); zu Beginn des Ladens fehlt `<html>` ggf. noch.
  static func script(scale: CGFloat) -> String {
    let px = String(format: "%.2f", Double(16 * scale))
    return """
      (function () { var set = function () { document.documentElement.style.fontSize = '\(px)px'; };
        if (document.documentElement) { set(); } else { document.addEventListener('readystatechange', set, { once: true }); } })();
      """
  }

  /// Einmalig, sobald Tauri die WebView anlegt (`Plugin.load(webview:)`, Hauptthread).
  static func attach(to webview: WKWebView) {
    apply(to: webview, category: UIApplication.shared.preferredContentSizeCategory)
    if let old = observer {
      NotificationCenter.default.removeObserver(old)
    }
    observer = NotificationCenter.default.addObserver(
      forName: UIContentSizeCategory.didChangeNotification, object: nil, queue: .main
    ) { [weak webview] note in
      guard let webview = webview else { return }
      let category =
        (note.userInfo?[UIContentSizeCategory.newValueUserInfoKey] as? UIContentSizeCategory)
        ?? UIApplication.shared.preferredContentSizeCategory
      apply(to: webview, category: category)
    }
  }

  private static func apply(to webview: WKWebView, category: UIContentSizeCategory) {
    let js = script(scale: scale(for: category))
    webview.configuration.userContentController.addUserScript(
      WKUserScript(source: js, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    webview.evaluateJavaScript(js, completionHandler: nil)
  }
}

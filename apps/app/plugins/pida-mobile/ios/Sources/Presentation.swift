// PIDA – Share-Sheet, Dokument-Picker und QuickLook-Vorschau (iOS), EUPL-1.2.

import QuickLook
import UIKit
import UniformTypeIdentifiers

enum Presentation {
  /// Oberster sichtbarer View-Controller über dem der WebView (für modale Systemdialoge).
  static func presenter(from root: UIViewController?) -> UIViewController? {
    var top = root
    while let next = top?.presentedViewController, !next.isBeingDismissed {
      top = next
    }
    return top
  }

  /// iPad: Popover mittig ohne Pfeil (es gibt keinen nativen Auslöser – die Schaltfläche liegt in der
  /// WebView).
  static func anchor(_ controller: UIViewController, in presenter: UIViewController) {
    guard let popover = controller.popoverPresentationController else { return }
    popover.sourceView = presenter.view
    popover.sourceRect = CGRect(x: presenter.view.bounds.midX, y: presenter.view.bounds.midY, width: 0, height: 0)
    popover.permittedArrowDirections = []
  }

  /// Inhaltstypen der Dateiauswahl (`kind` der Rust-Seite).
  static func contentTypes(kind: String) -> [UTType]? {
    switch kind {
    case "pdf":
      return [.pdf]
    case "pkcs12":
      // com.rsa.pkcs-12 ist systemseitig deklariert (.p12/.pfx); die Endungen decken Dateien ab, die ein
      // File Provider nur als Daten meldet
      var types: [UTType] = []
      for candidate in [UTType("com.rsa.pkcs-12"), UTType(filenameExtension: "p12"), UTType(filenameExtension: "pfx")] {
        if let type = candidate, !types.contains(type) { types.append(type) }
      }
      return types.isEmpty ? [.data] : types
    default:
      return nil
    }
  }
}

/// Delegate eines Dokument-Pickers: meldet genau einmal Auswahl oder Abbruch (auch beim Wegwischen).
final class PickerSession: NSObject, UIDocumentPickerDelegate, UIAdaptivePresentationControllerDelegate {
  private var done = false
  private let onPick: ([URL]) -> Void
  private let onCancel: () -> Void

  init(onPick: @escaping ([URL]) -> Void, onCancel: @escaping () -> Void) {
    self.onPick = onPick
    self.onCancel = onCancel
  }

  private func finish(_ urls: [URL]?) {
    guard !done else { return }
    done = true
    if let urls = urls { onPick(urls) } else { onCancel() }
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    finish(urls)
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    finish(nil)
  }

  func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
    finish(nil)
  }
}

/// Datenquelle und Delegate einer QuickLook-Vorschau mit genau einer Datei.
final class PreviewSession: NSObject, QLPreviewControllerDataSource, QLPreviewControllerDelegate {
  private let url: URL
  private let onDismiss: () -> Void

  init(url: URL, onDismiss: @escaping () -> Void) {
    self.url = url
    self.onDismiss = onDismiss
  }

  func numberOfPreviewItems(in controller: QLPreviewController) -> Int {
    1
  }

  func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
    url as NSURL
  }

  /// Nur ansehen: keine Bearbeitung der signierten Datei (jede Änderung bräche die Signatur).
  func previewController(_ controller: QLPreviewController, editingModeFor previewItem: QLPreviewItem)
    -> QLPreviewItemEditingMode
  {
    .disabled
  }

  func previewControllerDidDismiss(_ controller: QLPreviewController) {
    onDismiss()
  }
}

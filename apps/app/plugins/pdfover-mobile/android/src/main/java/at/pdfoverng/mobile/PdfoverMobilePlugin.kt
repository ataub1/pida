// PDF-Over NG – native Android-Seite des Plugins `pdfover-mobile` (EUPL-1.2).
//
// LEERE HÜLLE (Welle Mobile, Paket M0): Jede Methode lehnt mit dem Code `not-implemented` ab; die
// Rust-Seite fällt dann auf sichere Rückfälle zurück (Secrets nur im RAM, kein Teilen/Exportieren).
// Paket M-Android füllt die Methoden; Vertrag (Argumente, Antworten, Fehlercodes) siehe
// `apps/app/plugins/pdfover-mobile/src/lib.rs` und docs/integration/mobile.md.
//
// Regeln: keine Pfade, Schlüssel oder Werte in Fehlermeldungen oder Logs; Werte des sicheren Speichers
// nur verschlüsselt mit einem Schlüssel aus dem Android Keystore; Kopien eingehender Dokumente
// (content://-URIs über ContentResolver) nur in den übergebenen Zielordner (`destination`).

package at.pdfoverng.mobile

import android.app.Activity
import android.content.Intent
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class KeyArgs {
    lateinit var key: String
}

@InvokeArg
class SetArgs {
    lateinit var key: String
    lateinit var value: String
}

@InvokeArg
class ShareArgs {
    lateinit var path: String
    lateinit var mimeType: String
}

@InvokeArg
class ExportArgs {
    lateinit var path: String
    lateinit var fileName: String
    lateinit var mimeType: String
}

@InvokeArg
class ImportArgs {
    lateinit var kind: String
    var multiple: Boolean = false
    lateinit var destination: String
}

@InvokeArg
class PendingArgs {
    var urls: Array<String> = arrayOf()
    lateinit var destination: String
}

@TauriPlugin
class PdfoverMobilePlugin(private val activity: Activity) : Plugin(activity) {
    private fun notImplemented(invoke: Invoke) {
        invoke.reject("pdfover-mobile: native Seite noch nicht umgesetzt (M-Android)", "not-implemented")
    }

    /** Eingehende Dokumente (Intents VIEW/SEND/SEND_MULTIPLE application/pdf) – M-Android merkt sie vor. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
    }

    /** `{ key }` → `{ value: string | null }` (Android Keystore) */
    @Command
    fun secureStoreGet(invoke: Invoke) = notImplemented(invoke)

    /** `{ key, value }` → `{}` */
    @Command
    fun secureStoreSet(invoke: Invoke) = notImplemented(invoke)

    /** `{ key }` → `{}` (fehlender Eintrag ist kein Fehler) */
    @Command
    fun secureStoreDelete(invoke: Invoke) = notImplemented(invoke)

    /** `{ path, mimeType }` → `{}` (ACTION_SEND mit FileProvider-URI) */
    @Command
    fun shareFile(invoke: Invoke) = notImplemented(invoke)

    /** `{ path, fileName, mimeType }` → `{ saved: bool }` (SAF ACTION_CREATE_DOCUMENT) */
    @Command
    fun exportFile(invoke: Invoke) = notImplemented(invoke)

    /** `{ kind: "pdf" | "pkcs12", multiple, destination }` → `{ files: [{ name, path }] }` (SAF ACTION_OPEN_DOCUMENT) */
    @Command
    fun importFile(invoke: Invoke) = notImplemented(invoke)

    /** `{ urls, destination }` → `{ files: [{ name, path }] }` (Intents, content://-URIs) */
    @Command
    fun takePendingOpenDocuments(invoke: Invoke) = notImplemented(invoke)
}

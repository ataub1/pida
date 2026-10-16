// PIDA – Haupt-Activity der Android-App (EUPL-1.2), Paket M-Android
// (docs/integration/mobile-android.md).
//
// Die WebView, der Rust-Kern und die Plugins hängen an TauriActivity/WryActivity (generiert). Hier nur:
// - eingehende Intents prüfen, bevor tao (RunEvent::Opened) und das Plugin pida-mobile sie sehen
//   ([sanitized]): nur VIEW/SEND/SEND_MULTIPLE mit content://-URIs fremder Apps; file://, data: und
//   URIs der eigenen Provider werden verworfen (sonst könnte ein expliziter Intent eigene Dateien der
//   Sandbox als „Dokument“ einschleusen);
// - nach einer Neuerstellung der Activity (Drehen ohne configChanges, „Aktivitäten nicht behalten“,
//   Neustart aus „Zuletzt verwendet“) den ursprünglichen Intent nicht ein zweites Mal ausliefern;
// - Systemleisten und Tastatur als Abstand des Inhalts ([applySystemBarInsets]); die Flächen der Leisten in
//   den Farben der Oberfläche (oben Kopfzeile, unten Hintergrund) samt passender Symbolfarbe – die
//   Oberfläche meldet sie nach jedem Wechsel Hell/Dunkel über [SystemBarsBridge] (`PidaSystemBars.update`,
//   nur Farben im Format #rrggbb, keine Daten), bis dahin die Ressourcen (values bzw. values-night);
// - Schriftgröße des Systems auch zur Laufzeit (`fontScale` steht in configChanges, die WebView übernimmt
//   sie sonst nur beim Anlegen): [onConfigurationChanged] setzt `textZoom`;
// - Zurück-Taste/-Geste ([BackToBackground]): schließt einen offenen Dialog der Oberfläche (wie Esc),
//   sonst geht die App in den Hintergrund – sie wird **nicht** beendet. Eine beendete Activity beendet
//   unter Tauri auch den Prozess (letztes Fenster zerstört) und damit einen laufenden Signiervorgang;
//   ein Wechsel in die App „ID Austria“ ist kein Abbruch (Plan §3.5);
// - Debug-Builds: Startprüfung über das Intent-Extra `pida.smoke` (setzt PIDA_SMOKE=1 für den
//   Kern, siehe apps/app/src-tauri/src/smoke.rs; die App beendet sich danach selbst).

package at.pida

import android.content.ContentResolver
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.drawable.Drawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.system.Os
import android.util.Log
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import kotlin.math.roundToInt

class MainActivity : TauriActivity() {
  private var webView: WebView? = null
  private val barsBackground = SystemBarsBackground()
  /** Von der Oberfläche gemeldete Farben; `null` = noch keine Meldung (Ressourcen gelten). */
  private var uiBars: BarColors? = null

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    this.webView = webView
    // vor dem ersten Laden der Seite (setWebView ruft dies vor loadUrl), damit die Brücke bereitsteht
    webView.addJavascriptInterface(SystemBarsBridge(), BARS_BRIDGE)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    if (BuildConfig.DEBUG && intent?.getBooleanExtra(EXTRA_SMOKE, false) == true) {
      // vor super.onCreate: dort startet der Rust-Kern
      Os.setenv("PIDA_SMOKE", "1", true)
      Log.i(TAG, "Startprüfung aktiv")
    }
    val relaunched = savedInstanceState != null ||
      ((intent?.flags ?: 0) and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY) != 0
    intent = if (relaunched) Intent(Intent.ACTION_MAIN) else sanitized(intent)
    super.onCreate(savedInstanceState)
    applySystemBarInsets()
    applyBarColors()
    onBackPressedDispatcher.addCallback(this, BackToBackground())
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    // Schriftgröße (Einstellungen › Display) wie beim Anlegen der WebView übernehmen
    webView?.settings?.textZoom = (newConfig.fontScale * 100f).roundToInt()
    // Dunkelmodus des Systems: bis zur Meldung der Oberfläche die Farben der Ressourcen (values-night)
    if (uiBars == null) applyBarColors()
  }

  /** Farben der Systemleisten und Symbolfarbe (helle Symbole auf dunklem Grund). */
  private fun applyBarColors() {
    val night = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
    val colors = uiBars ?: BarColors(
      ContextCompat.getColor(this, R.color.system_bar_top),
      ContextCompat.getColor(this, R.color.window_background),
      night,
    )
    barsBackground.topColor = colors.top
    barsBackground.bottomColor = colors.bottom
    barsBackground.invalidateSelf()
    WindowCompat.getInsetsController(window, window.decorView).apply {
      isAppearanceLightStatusBars = !colors.dark
      isAppearanceLightNavigationBars = !colors.dark
    }
  }

  private data class BarColors(val top: Int, val bottom: Int, val dark: Boolean)

  /** `window.PidaSystemBars.update(top, bottom, dark)` aus der Oberfläche (apps/app/src/app/systemBars.ts). */
  private inner class SystemBarsBridge {
    @JavascriptInterface
    fun update(top: String?, bottom: String?, dark: Boolean) {
      val t = parseColor(top) ?: return
      val b = parseColor(bottom) ?: return
      runOnUiThread {
        uiBars = BarColors(t, b, dark)
        applyBarColors()
      }
    }
  }

  /**
   * Hintergrund des Inhalts (sichtbar nur im Abstand um die WebView): oben die Statusleiste in der Farbe der
   * Kopfzeile, sonst (Navigationsleiste, Display-Ausschnitt im Querformat) die des Hintergrunds.
   */
  private class SystemBarsBackground : Drawable() {
    var top = 0
    var topColor = Color.WHITE
    var bottomColor = Color.WHITE
    private val paint = Paint()

    override fun draw(canvas: Canvas) {
      canvas.drawColor(bottomColor)
      paint.color = topColor
      canvas.drawRect(0f, 0f, bounds.width().toFloat(), top.toFloat(), paint)
    }

    override fun setAlpha(alpha: Int) {}
    override fun setColorFilter(colorFilter: ColorFilter?) {}
    @Deprecated("Deprecated in Java")
    override fun getOpacity(): Int = PixelFormat.OPAQUE
  }

  /** Zurück: offenen Dialog schließen (Esc an die Oberfläche), sonst App in den Hintergrund. */
  private inner class BackToBackground : OnBackPressedCallback(true) {
    override fun handleOnBackPressed() {
      val view = webView
      if (view == null) {
        moveTaskToBack(true)
        return
      }
      view.evaluateJavascript(CLOSE_DIALOG_JS) { closed ->
        if (closed != "true") moveTaskToBack(true)
      }
    }
  }
  /**
   * Android 15 (targetSdk 35) zeichnet immer randlos. Die WebView meldet Systemleisten nicht als
   * `env(safe-area-inset-*)` (nur Display-Ausschnitte) – im Querformat lag die Statusleiste über der
   * Kopfzeile. Deshalb hier: Statusleiste, Navigationsleiste, Ausschnitt und Tastatur als Abstand des
   * Inhalts; [SystemBarsBackground] füllt die Leisten (Farben: [applyBarColors]).
   */
  private fun applySystemBarInsets() {
    val content = findViewById<View>(android.R.id.content)
    content.background = barsBackground
    ViewCompat.setOnApplyWindowInsetsListener(content) { v, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      v.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
      barsBackground.top = bars.top
      barsBackground.invalidateSelf()
      WindowInsetsCompat.CONSUMED
    }
  }

  override fun onNewIntent(intent: Intent) {
    val clean = sanitized(intent)
    setIntent(clean)
    super.onNewIntent(clean)
  }

  /** Nur Dokument-Intents mit ausschließlich zulässigen URIs; alles andere wird zu ACTION_MAIN. */
  private fun sanitized(intent: Intent?): Intent {
    if (intent == null) return Intent(Intent.ACTION_MAIN)
    val action = intent.action
    if (action != Intent.ACTION_VIEW && action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) {
      return intent
    }
    val uris = documentUris(intent)
    if (uris.isEmpty() || !uris.all(::acceptable)) {
      Log.w(TAG, "Intent ohne zulässiges Dokument verworfen")
      return Intent(Intent.ACTION_MAIN)
    }
    return intent
  }

  private fun acceptable(uri: Uri): Boolean {
    val authority = uri.authority ?: return false
    return uri.scheme == ContentResolver.SCHEME_CONTENT && !authority.startsWith(packageName)
  }

  @Suppress("DEPRECATION")
  private fun documentUris(intent: Intent): List<Uri> {
    val out = ArrayList<Uri>()
    intent.data?.let(out::add)
    intent.clipData?.let { clip -> for (i in 0 until clip.itemCount) clip.getItemAt(i).uri?.let(out::add) }
    try {
      if (intent.action == Intent.ACTION_SEND) {
        val stream = if (Build.VERSION.SDK_INT >= 33) {
          intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
          intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
        }
        stream?.let(out::add)
        // Text (EXTRA_TEXT) ist kein Dokument
        if (intent.hasExtra(Intent.EXTRA_TEXT) && stream == null) return emptyList()
      } else if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
        val streams = if (Build.VERSION.SDK_INT >= 33) {
          intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
          intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
        }
        streams?.let(out::addAll)
      }
    } catch (e: RuntimeException) {
      // fremde Parcelables (BadParcelableException) – Intent verwerfen
      Log.w(TAG, "Intent-Extras nicht lesbar: ${e.javaClass.simpleName}")
      return emptyList()
    }
    return out
  }

  private companion object {
    const val TAG = "PIDA"
    const val EXTRA_SMOKE = "pida.smoke"
    const val BARS_BRIDGE = "PidaSystemBars"
    val HEX_COLOR = Regex("^#[0-9a-fA-F]{6}$")

    /** Nur `#rrggbb` (die Tokens der Oberfläche); alles andere wird ignoriert. */
    fun parseColor(value: String?): Int? {
      val v = value?.trim() ?: return null
      return if (HEX_COLOR.matches(v)) Color.parseColor(v) else null
    }

    /** Schließt den obersten Dialog der Oberfläche wie die Esc-Taste (React Aria); `true` = es gab einen. */
    const val CLOSE_DIALOG_JS = """(function () {
      var d = document.querySelectorAll('[role=dialog],[role=alertdialog]');
      if (d.length === 0) return false;
      var t = d[d.length - 1];
      var target = t.contains(document.activeElement) ? document.activeElement : t;
      target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', keyCode: 27, bubbles: true, cancelable: true }));
      return true;
    })()"""
  }
}

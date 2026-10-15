package app.melyxar.web

import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.core.graphics.ColorUtils
import androidx.core.graphics.toColorInt
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.updatePadding
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * The phone's window, as the page sees it: the page is drawn under the bars
 * and the camera, and told here how much room they take, the way an installed
 * app is told by its browser (see the page's `app-window.ts`). Said by the
 * application rather than left to the web view, which only learnt to say it
 * in its recent versions. The keyboard lifts the whole page above it.
 *
 * In return the page gives the colour of its window, and the bars' icons are
 * drawn dark or light from it, as a browser draws them; light over a film,
 * whatever the page is.
 */
class PageWindow(private val activity: ComponentActivity, holder: View, private val page: WebView) {
    @Volatile
    private var edges = Json.encodeToString(Edges(0f, 0f, 0f, 0f))
    private var pageIsLight = false
    private var film = false

    init {
        ViewCompat.setOnApplyWindowInsetsListener(holder) { view, insets ->
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            view.updatePadding(bottom = keyboard)
            heard(insets, keyboard > 0)
            insets
        }
        // Said once, by the page, from what it is told here: the web view
        // saying it again on its own would count every edge twice.
        ViewCompat.setOnApplyWindowInsetsListener(page) { _, _ -> WindowInsetsCompat.CONSUMED }
    }

    /** The keyboard covers the bottom bar, and the page is already above it. */
    private fun heard(insets: WindowInsetsCompat, keyboardShown: Boolean) {
        val room = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
        val density = activity.resources.displayMetrics.density
        val now = Json.encodeToString(
            Edges(
                top = room.top / density,
                bottom = if (keyboardShown) 0f else room.bottom / density,
                left = room.left / density,
                right = room.right / density,
            ),
        )
        if (now != edges) {
            edges = now
            page.evaluateJavascript("window.melyxarEdgesChanged && window.melyxarEdgesChanged()", null)
        }
    }

    @JavascriptInterface
    fun edges(): String = edges

    /** Any frame of the page can call this, so a colour that cannot be read
     *  is left aside rather than trusted. */
    @JavascriptInterface
    fun colour(colour: String) {
        val read = try {
            colour.toColorInt()
        } catch (_: IllegalArgumentException) {
            return
        }
        val light = ColorUtils.calculateLuminance(read) > HALF_LIGHT
        activity.runOnUiThread {
            pageIsLight = light
            showBars()
        }
    }

    fun filmShown(shown: Boolean) {
        film = shown
        showBars()
    }

    private fun showBars() {
        WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
            isAppearanceLightStatusBars = pageIsLight && !film
            isAppearanceLightNavigationBars = pageIsLight && !film
        }
    }

    private companion object {
        /** Lighter than this, the bars' icons are drawn dark. */
        const val HALF_LIGHT = 0.5
    }
}

/** The room taken at each edge, in the page's pixels. */
@Serializable
private data class Edges(val top: Float, val bottom: Float, val left: Float, val right: Float)

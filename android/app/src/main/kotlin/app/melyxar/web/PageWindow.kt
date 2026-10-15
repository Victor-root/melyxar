package app.melyxar.web

import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.core.view.ViewCompat
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
 */
class PageWindow(private val activity: ComponentActivity, holder: View, private val page: WebView) {
    @Volatile
    private var edges = Json.encodeToString(Edges(0f, 0f, 0f, 0f))

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
}

/** The room taken at each edge, in the page's pixels. */
@Serializable
private data class Edges(val top: Float, val bottom: Float, val left: Float, val right: Float)

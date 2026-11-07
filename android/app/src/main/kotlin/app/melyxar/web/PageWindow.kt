package app.melyxar.web

import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsAnimationCompat
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
class PageWindow(private val activity: ComponentActivity, private val holder: View, private val page: WebView) {
    @Volatile
    private var edges = Json.encodeToString(Edges(0f, 0f, 0f, 0f))

    /** Whether the keyboard is on its way, and what the window was told last
     *  while it is. */
    private var keyboardMoving = false
    private var heardWhileMoving: WindowInsetsCompat? = null

    /** Where the foot of the field being typed in stands, in the page's
     *  pixels, as the page said when it was reached. */
    @Volatile
    private var fieldFoot = 0f

    /** How far the page rises with the keyboard for that field to stay above
     *  it, and how high the keyboard comes, in the screen's pixels. */
    private var lift = 0f
    private var keyboardHeight = 0

    /** Where the keyboard stands and where it is heading, in the screen's
     *  pixels: a slide can start from the middle of the one before. */
    private var keyboardNow = 0
    private var keyboardGoal = 0

    init {
        ViewCompat.setOnApplyWindowInsetsListener(holder) { view, insets ->
            // The page gives way once the keyboard has arrived: given way at
            // the start, a black band stood where it was still to come. Going
            // down, the page is whole from the start of the slide, see onStart.
            if (keyboardMoving) {
                heardWhileMoving = insets
            } else {
                settle(view, insets)
            }
            insets
        }
        ViewCompat.setWindowInsetsAnimationCallback(
            holder,
            object : WindowInsetsAnimationCompat.Callback(DISPATCH_MODE_STOP) {
                override fun onPrepare(animation: WindowInsetsAnimationCompat) {
                    if (animation.typeMask and WindowInsetsCompat.Type.ime() != 0) {
                        keyboardMoving = true
                    }
                }

                override fun onStart(
                    animation: WindowInsetsAnimationCompat,
                    bounds: WindowInsetsAnimationCompat.BoundsCompat,
                ): WindowInsetsAnimationCompat.BoundsCompat {
                    if (animation.typeMask and WindowInsetsCompat.Type.ime() == 0) {
                        return bounds
                    }
                    val shown = ViewCompat.getRootWindowInsets(holder)?.isVisible(WindowInsetsCompat.Type.ime()) == true
                    keyboardGoal = if (shown) bounds.upperBound.bottom else 0
                    tellSlide(animation)
                    if (keyboardGoal < holder.paddingBottom) {
                        arrived()
                    }
                    if (shown && holder.paddingBottom == 0) {
                        keyboardHeight = bounds.upperBound.bottom
                        val density = activity.resources.displayMetrics.density
                        val room = holder.height - keyboardHeight
                        lift = ((fieldFoot + FIELD_AIR) * density - room).coerceIn(0f, keyboardHeight.toFloat())
                    }
                    return bounds
                }

                override fun onProgress(
                    insets: WindowInsetsCompat,
                    running: MutableList<WindowInsetsAnimationCompat>,
                ): WindowInsetsCompat {
                    if (running.any { it.typeMask and WindowInsetsCompat.Type.ime() != 0 }) {
                        keyboardNow = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
                        if (lift > 0f && keyboardHeight > 0) {
                            page.translationY = -lift * keyboardNow / keyboardHeight
                        }
                    }
                    return insets
                }

                override fun onEnd(animation: WindowInsetsAnimationCompat) {
                    if (animation.typeMask and WindowInsetsCompat.Type.ime() != 0) {
                        keyboardMoving = false
                        keyboardNow = keyboardGoal
                        val risen = lift
                        lift = 0f
                        if (risen > 0f) {
                            handOver(risen)
                        } else {
                            arrived()
                        }
                    }
                }
            },
        )
        // Said once, by the page, from what it is told here: the web view
        // saying it again on its own would count every edge twice.
        ViewCompat.setOnApplyWindowInsetsListener(page) { _, _ -> WindowInsetsCompat.CONSUMED }
    }

    /** The whole slide at once, for the page to play it in step with the
     *  keyboard however busy it is. */
    private fun tellSlide(animation: WindowInsetsAnimationCompat) {
        val density = activity.resources.displayMetrics.density
        val duration = animation.durationMillis.takeIf { it > 0 } ?: FALLBACK_SLIDE_MILLIS
        val course = (0..COURSE_STEPS).map { step ->
            val time = step.toFloat() / COURSE_STEPS
            animation.interpolator?.getInterpolation(time) ?: time
        }
        val slide = "{from:${keyboardNow / density},to:${keyboardGoal / density},duration:$duration," +
            "course:[${course.joinToString(",")}],startedAt:${System.currentTimeMillis()}}"
        page.evaluateJavascript("window.melyxarKeyboardSlide && window.melyxarKeyboardSlide($slide)", null)
    }

    private fun arrived() {
        heardWhileMoving?.let { settle(holder, it) }
        heardWhileMoving = null
    }

    /**
     * The page has been raised by moving the whole view: the page scrolls by
     * as much itself, and the view goes back in the frame that shows it, so
     * nothing moves. Left to itself, the web view would bring the field to the
     * middle of what is left, a second movement after the keyboard's.
     */
    private fun handOver(risen: Float) {
        val pixels = risen / activity.resources.displayMetrics.density
        page.evaluateJavascript("window.melyxarRaise && window.melyxarRaise($pixels)") {
            page.postVisualStateCallback(0, object : WebView.VisualStateCallback() {
                override fun onComplete(requestId: Long) {
                    page.translationY = 0f
                    arrived()
                }
            })
        }
    }

    private fun settle(view: View, insets: WindowInsetsCompat) {
        val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
        view.updatePadding(bottom = keyboard)
        if (!keyboardMoving) {
            keyboardNow = keyboard
        }
        heard(insets, keyboard > 0)
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
                keyboard = keyboardShown,
            ),
        )
        if (now != edges) {
            edges = now
            page.evaluateJavascript("window.melyxarEdgesChanged && window.melyxarEdgesChanged()", null)
        }
    }

    @JavascriptInterface
    fun edges(): String = edges

    @JavascriptInterface
    fun fieldAt(foot: Float) {
        fieldFoot = foot
    }
}

/** The air kept between a field and the keyboard, in the page's pixels. */
private const val FIELD_AIR = 16f

/** How many steps the page is given of the keyboard's slide. */
private const val COURSE_STEPS = 16

/** How long a slide lasts when the system does not say. */
private const val FALLBACK_SLIDE_MILLIS = 285L

/** The room taken at each edge, in the page's pixels. */
@Serializable
private data class Edges(
    val top: Float,
    val bottom: Float,
    val left: Float,
    val right: Float,
    val keyboard: Boolean = false,
)

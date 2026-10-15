package app.melyxar.web

import android.graphics.Rect
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.PixelCopy
import android.view.View
import androidx.activity.ComponentActivity
import androidx.annotation.RequiresApi
import androidx.core.graphics.createBitmap
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import kotlin.math.pow

/**
 * Whether the icons of the phone's bars are drawn dark or light, from what is
 * drawn under them right now.
 *
 * The page is drawn under both bars, and what is under them is anything: the
 * picture of the banner in whatever colours a film has, a list, a theme
 * somebody wrote. So the strip under each bar is looked at whenever the page
 * draws, at most a few times a second and once more when it settles, and the
 * icons take whichever of dark and light stands out more against it. Over a
 * film they are light, the film being drawn in a layer of its own that is not
 * looked at.
 *
 * From Android 8, the first that lets an application look at its own window;
 * before it, the icons stay light.
 */
class BarShades(private val activity: ComponentActivity, private val page: View) {
    private val beat = Handler(Looper.getMainLooper())
    private var film = false
    private var waiting = false
    private var lookedAt = 0L

    private val look = Runnable {
        waiting = false
        lookedAt = SystemClock.uptimeMillis()
        if (!film && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            lookUnderTheBars()
        }
    }

    init {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            page.viewTreeObserver.addOnDrawListener { drawn() }
        }
    }

    fun filmShown(shown: Boolean) {
        film = shown
        if (shown) {
            show(statusDark = false, navigationDark = false)
        } else {
            drawn()
        }
    }

    /** Looked at once the page has been still a moment, and never more often
     *  than the pace below while it keeps moving. */
    private fun drawn() {
        if (waiting) {
            return
        }
        waiting = true
        val sinceLast = SystemClock.uptimeMillis() - lookedAt
        beat.postDelayed(look, maxOf(SETTLE_MS, LOOK_EVERY_MS - sinceLast))
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun lookUnderTheBars() {
        val window = activity.window.decorView
        val bars = ViewCompat.getRootWindowInsets(window)?.getInsets(WindowInsetsCompat.Type.systemBars()) ?: return
        if (window.width == 0 || window.height == 0) {
            return
        }
        if (bars.top > 0) {
            lookAt(Rect(0, 0, window.width, bars.top)) { dark -> show(statusDark = dark) }
        }
        if (bars.bottom > 0) {
            lookAt(Rect(0, window.height - bars.bottom, window.width, window.height)) { dark ->
                show(navigationDark = dark)
            }
        }
    }

    /** What is drawn in this part of the window, shrunk to a handful of
     *  points: their average is all that is asked of it. */
    @RequiresApi(Build.VERSION_CODES.O)
    private fun lookAt(strip: Rect, seen: (darkIcons: Boolean) -> Unit) {
        val shrunk = createBitmap(LOOK_ACROSS, LOOK_DOWN)
        PixelCopy.request(activity.window, strip, shrunk, { result ->
            if (result == PixelCopy.SUCCESS) {
                val points = IntArray(LOOK_ACROSS * LOOK_DOWN)
                shrunk.getPixels(points, 0, LOOK_ACROSS, 0, 0, LOOK_ACROSS, LOOK_DOWN)
                if (!film) {
                    seen(wantsDarkIcons(points))
                }
            }
            shrunk.recycle()
        }, beat)
    }

    private fun show(statusDark: Boolean? = null, navigationDark: Boolean? = null) {
        WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
            statusDark?.let { isAppearanceLightStatusBars = it }
            navigationDark?.let { isAppearanceLightNavigationBars = it }
        }
    }

    private companion object {
        const val LOOK_EVERY_MS = 250L
        const val SETTLE_MS = 120L
        const val LOOK_ACROSS = 24
        const val LOOK_DOWN = 4
    }
}

/**
 * Whether dark icons stand out more than light ones over these points, as
 * `0xAARRGGBB`: measured as the contrast rules measure it, from the average
 * light of the points, so black and white meet at the same grey they do in
 * those rules.
 */
fun wantsDarkIcons(points: IntArray): Boolean {
    if (points.isEmpty()) {
        return false
    }
    val light = points.sumOf { relativeLight(it) } / points.size
    val againstWhite = 1.05 / (light + 0.05)
    val againstBlack = (light + 0.05) / 0.05
    return againstBlack > againstWhite
}

private fun relativeLight(point: Int): Double =
    0.2126 * linear(point shr 16) + 0.7152 * linear(point shr 8) + 0.0722 * linear(point)

private fun linear(channel: Int): Double {
    val value = (channel and 0xff) / 255.0
    return if (value <= 0.04045) value / 12.92 else ((value + 0.055) / 1.055).pow(2.4)
}

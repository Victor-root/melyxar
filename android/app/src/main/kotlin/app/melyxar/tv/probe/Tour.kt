package app.melyxar.tv.probe

import android.os.SystemClock
import android.util.Log
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * A walk through the home screen played by itself, the remote's keys sent
 * by the probe, once for each way of drawing it: down through its rows and
 * back, then still while the banner turns. Each stretch is logged under the
 * tag "MlxPerf.Tour", so the whole of it is read in one go.
 */
object Tour {
    private const val TAG = "MlxPerf.Tour"
    private const val STEP_MS = 700L
    private const val SETTLE_MS = 2_000L
    /** Longer than the banner stands on one work, so it turns once in it. */
    private const val BANNER_MS = 10_000L
    private const val ROWS = 4
    private const val TO_THE_BAR = 3

    /** Each way of drawing walked through, and the parts it leaves out. */
    private val SETUPS = listOf<Pair<String, Set<Part>>>(
        "everything" to emptySet(),
        "no band glow" to setOf(Part.BandGlow),
        "no fan shadows" to setOf(Part.FanShadows),
        "no band floor" to setOf(Part.BandFloor),
        "no shadows" to setOf(Part.Shadows),
        "no banner fade" to setOf(Part.BannerFade),
        "everything again" to emptySet(),
    )

    var running by mutableStateOf(false)
        private set

    /** Leaves the settings for the home screen and walks it, from its banner. */
    fun start(activity: ComponentActivity) {
        if (running) {
            return
        }
        running = true
        val before = Probe.switches()
        activity.lifecycleScope.launch {
            try {
                Log.i(TAG, "start")
                press(activity, KeyEvent.KEYCODE_BACK)
                delay(SETTLE_MS)
                // From the bar, whatever the remote was left on, down to the banner.
                repeat(TO_THE_BAR) {
                    press(activity, KeyEvent.KEYCODE_DPAD_UP)
                    delay(STEP_MS)
                }
                press(activity, KeyEvent.KEYCODE_DPAD_DOWN)
                SETUPS.forEach { (name, left) ->
                    Probe.leaveOut(left, before)
                    delay(SETTLE_MS)
                    Probe.measure("$name, rows")
                    repeat(ROWS) {
                        press(activity, KeyEvent.KEYCODE_DPAD_DOWN)
                        delay(STEP_MS)
                    }
                    repeat(ROWS) {
                        press(activity, KeyEvent.KEYCODE_DPAD_UP)
                        delay(STEP_MS)
                    }
                    Probe.measured()
                    delay(SETTLE_MS)
                    Probe.measure("$name, banner turning")
                    delay(BANNER_MS)
                    Probe.measured()
                }
            } finally {
                Probe.leaveOut(emptySet(), before)
                running = false
                Log.i(TAG, "done")
            }
        }
    }

    private fun press(activity: ComponentActivity, code: Int) {
        val now = SystemClock.uptimeMillis()
        activity.dispatchKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, code, 0))
        activity.dispatchKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_UP, code, 0))
    }
}

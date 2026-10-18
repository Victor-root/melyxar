package app.melyxar.tv.probe

import android.app.Activity
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.SystemClock
import android.util.Log
import android.view.FrameMetrics
import android.view.KeyEvent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.concurrent.atomic.AtomicInteger

/*
 * Temporary smoothness probe, logged in every build while the television is
 * being tuned. Everything it adds lives in this package or calls `Probe.`,
 * and every line it logs carries a tag starting with "MlxPerf", so it can be
 * found and removed whole.
 */

/** How the cards' shadows are drawn, to compare their cost on the box. */
enum class ShadowTest { Normal, Hard, None }

object Probe {
    private const val TAG = "MlxPerf"
    private const val TAG_FRAME = "MlxPerf.Frame"
    private const val TAG_KEY = "MlxPerf.Key"
    private const val TAG_SHADOW = "MlxPerf.Shadow"
    private const val KEPT = "probe"
    private const val KEPT_SHADOWS = "shadows"
    private const val NS_PER_MS = 1_000_000.0

    private var kept: android.content.SharedPreferences? = null

    var shadows by mutableStateOf(ShadowTest.Normal)
        private set

    private val shadowsMade = AtomicInteger()
    private val shadowsDrawn = AtomicInteger()
    private val litStarted = AtomicInteger()
    private val keys = AtomicInteger()
    private var lastKey = 0L

    private val frames = ArrayList<Long>()
    private val phases = LongArray(Phase.entries.size)
    private var janky = 0

    private enum class Phase(val metric: Int, val label: String) {
        Delay(FrameMetrics.UNKNOWN_DELAY_DURATION, "delay"),
        Input(FrameMetrics.INPUT_HANDLING_DURATION, "input"),
        Animation(FrameMetrics.ANIMATION_DURATION, "anim"),
        Layout(FrameMetrics.LAYOUT_MEASURE_DURATION, "layout"),
        Draw(FrameMetrics.DRAW_DURATION, "draw"),
        Sync(FrameMetrics.SYNC_DURATION, "sync"),
        Issue(FrameMetrics.COMMAND_ISSUE_DURATION, "issue"),
        Swap(FrameMetrics.SWAP_BUFFERS_DURATION, "swap"),
    }

    fun chooseShadows(test: ShadowTest) {
        shadows = test
        kept?.edit()?.putString(KEPT_SHADOWS, test.name)?.apply()
        Log.i(TAG_SHADOW, "shadows now ${test.name}")
    }

    /** Starts logging every frame [activity] draws, with a summary each second. */
    fun watch(activity: Activity) {
        kept = activity.getSharedPreferences(KEPT, Context.MODE_PRIVATE).also { prefs ->
            shadows = prefs.getString(KEPT_SHADOWS, null)?.let { name -> ShadowTest.entries.firstOrNull { it.name == name } } ?: ShadowTest.Normal
        }
        @Suppress("DEPRECATION")
        val refresh = activity.windowManager.defaultDisplay.refreshRate
        val budgetNs = (1e9 / refresh).toLong()
        Log.i(TAG, "watching: ${Build.MANUFACTURER} ${Build.MODEL}, Android ${Build.VERSION.RELEASE}, ${"%.2f".format(refresh)} Hz, budget ${"%.1f".format(budgetNs / NS_PER_MS)} ms, shadows ${shadows.name}")
        val thread = HandlerThread("MlxPerf").apply { start() }
        val handler = Handler(thread.looper)
        activity.window.addOnFrameMetricsAvailableListener({ _, metrics, dropped ->
            frame(metrics, dropped, budgetNs)
        }, handler)
        handler.postDelayed(object : Runnable {
            override fun run() {
                summary()
                handler.postDelayed(this, 1000)
            }
        }, 1000)
    }

    private fun frame(metrics: FrameMetrics, dropped: Int, budgetNs: Long) {
        if (metrics.getMetric(FrameMetrics.FIRST_DRAW_FRAME) == 1L) {
            return
        }
        val total = metrics.getMetric(FrameMetrics.TOTAL_DURATION)
        val deadline = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) metrics.getMetric(FrameMetrics.DEADLINE) else budgetNs
        frames += total
        Phase.entries.forEach { phases[it.ordinal] += metrics.getMetric(it.metric) }
        if (total > deadline) {
            janky++
            val parts = Phase.entries.joinToString(" ") { "${it.label}=${ms(metrics.getMetric(it.metric))}" }
            val gpu = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) " gpu=${ms(metrics.getMetric(FrameMetrics.GPU_DURATION))}" else ""
            Log.w(TAG_FRAME, "slow frame ${ms(total)} ms (deadline ${ms(deadline)}): $parts$gpu dropped=$dropped")
        }
    }

    private fun summary() {
        val made = shadowsMade.getAndSet(0)
        val drawn = shadowsDrawn.getAndSet(0)
        val lit = litStarted.getAndSet(0)
        val pressed = keys.getAndSet(0)
        if (frames.isEmpty()) {
            return
        }
        frames.sort()
        val count = frames.size
        val means = Phase.entries.joinToString(" ") { "${it.label}=${ms(phases[it.ordinal] / count)}" }
        val runtime = Runtime.getRuntime()
        val heap = (runtime.totalMemory() - runtime.freeMemory()) / (1024 * 1024)
        Log.i(
            TAG,
            "1s: $count frames, $janky janky (${janky * 100 / count}%), " +
                "p50=${ms(frames[count / 2])} p90=${ms(frames[count * 9 / 10])} max=${ms(frames[count - 1])} ms | " +
                "mean $means | keys=$pressed lit=$lit | shadows ${shadows.name} made=$made drawn=$drawn | heap=${heap} MB",
        )
        frames.clear()
        phases.fill(0)
        janky = 0
    }

    /** A key the activity is handed, with how long it waited to be. */
    fun key(event: KeyEvent) {
        if (event.action != KeyEvent.ACTION_DOWN) {
            return
        }
        keys.incrementAndGet()
        val now = SystemClock.uptimeMillis()
        val gap = if (lastKey == 0L) 0 else now - lastKey
        lastKey = now
        Log.d(TAG_KEY, "${KeyEvent.keyCodeToString(event.keyCode)} repeat=${event.repeatCount} waited=${now - event.eventTime} ms gap=$gap ms")
    }

    /** A shadow's drawing worked out anew. */
    fun shadowMade() {
        shadowsMade.incrementAndGet()
    }

    /** A shadow drawn. */
    fun shadowDrawn() {
        shadowsDrawn.incrementAndGet()
    }

    /** A piece starting to light or fade. */
    fun litStarted() {
        litStarted.incrementAndGet()
    }

    private fun ms(ns: Long) = "%.1f".format(ns / NS_PER_MS)
}

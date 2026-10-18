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
import androidx.compose.runtime.mutableStateOf
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

/*
 * Temporary smoothness probe, logged in every build while the television is
 * being tuned. Everything it adds lives in this package or calls `Probe.`,
 * and every line it logs carries a tag starting with "MlxPerf", so it can be
 * found and removed whole.
 */

/** A way of drawing the screens the probe switches on and off, to
 *  measure its cost, and whether it is on until it is switched. */
enum class Part(val byDefault: Boolean) {
    Shadows(true),
    BannerFade(true),
    PictureFade(true),
}

object Probe {
    private const val TAG = "MlxPerf"
    private const val TAG_FRAME = "MlxPerf.Frame"
    private const val TAG_KEY = "MlxPerf.Key"
    private const val TAG_PART = "MlxPerf.Part"
    private const val TAG_SCREEN = "MlxPerf.Screen"
    private const val KEPT = "probe"
    private const val NS_PER_MS = 1_000_000.0
    private const val RECENT = 120
    private const val CALM_FRAMES = 10
    private const val FOLLOWED_NS = 5_000_000_000L
    private const val ASKED_WITHIN_NS = 1_000_000_000L

    private var kept: android.content.SharedPreferences? = null

    private val shown = Part.entries.associateWith { mutableStateOf(it.byDefault) }

    private val shadowsMade = AtomicInteger()
    private val shadowsDrawn = AtomicInteger()
    private val shadowsBaked = AtomicInteger()
    private val bakingNs = AtomicLong()
    private val cardsComposed = AtomicInteger()
    private val composingNs = AtomicLong()
    private val litStarted = AtomicInteger()
    private val keys = AtomicInteger()
    private val picturesFaded = AtomicInteger()
    private val picturesFromMemory = AtomicInteger()
    private var lastKey = 0L
    @Volatile
    private var lastKeyNs = 0L
    private var handler: Handler? = null

    private val recent = ArrayDeque<Seen>()
    private val visits = HashMap<String, Int>()
    private var opening: Opening? = null

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

    /** A frame drawn: when it was meant to start, how long it took, and each phase. */
    private class Seen(val start: Long, val total: Long, val late: Boolean, val phases: LongArray)

    /** A screen being shown, followed until its frames come in on time again. */
    private class Opening(val key: String, val visit: Int, val start: Long) {
        var frames = 0
        var slow = 0
        var worst = 0L
        var end = start
        var calm = 0
        val phases = LongArray(Phase.entries.size)
    }

    /** Whether [part] is drawn, read where it draws so it changes at once. */
    fun shows(part: Part) = shown.getValue(part).value

    fun show(part: Part, on: Boolean) {
        shown.getValue(part).value = on
        kept?.edit()?.putBoolean(part.name, on)?.apply()
        Log.i(TAG_PART, "${part.name} ${if (on) "on" else "off"}")
    }

    private fun switched() = Part.entries.joinToString(" ") { "${it.name}=${if (shows(it)) "on" else "off"}" }

    /** Starts logging every frame [activity] draws, with a summary each second. */
    fun watch(activity: Activity) {
        kept = activity.getSharedPreferences(KEPT, Context.MODE_PRIVATE).also { prefs ->
            Part.entries.forEach { part -> shown.getValue(part).value = prefs.getBoolean(part.name, part.byDefault) }
        }
        @Suppress("DEPRECATION")
        val refresh = activity.windowManager.defaultDisplay.refreshRate
        val budgetNs = (1e9 / refresh).toLong()
        Log.i(TAG, "watching: ${Build.MANUFACTURER} ${Build.MODEL}, Android ${Build.VERSION.RELEASE}, ${"%.2f".format(refresh)} Hz, budget ${"%.1f".format(budgetNs / NS_PER_MS)} ms, ${switched()}")
        val thread = HandlerThread("MlxPerf").apply { start() }
        val handler = Handler(thread.looper).also { handler = it }
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
        val each = LongArray(Phase.entries.size) { metrics.getMetric(Phase.entries[it].metric) }
        each.forEachIndexed { at, ns -> phases[at] += ns }
        val start = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) metrics.getMetric(FrameMetrics.INTENDED_VSYNC_TIMESTAMP) else System.nanoTime() - total
        val seen = Seen(start, total, total > deadline, each)
        recent.addLast(seen)
        if (recent.size > RECENT) {
            recent.removeFirst()
        }
        opening?.let { follow(it, seen) }
        if (total > deadline) {
            janky++
            val parts = Phase.entries.joinToString(" ") { "${it.label}=${ms(metrics.getMetric(it.metric))}" }
            val gpu = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) " gpu=${ms(metrics.getMetric(FrameMetrics.GPU_DURATION))}" else ""
            Log.w(TAG_FRAME, "slow frame ${ms(total)} ms (deadline ${ms(deadline)}): $parts$gpu dropped=$dropped")
        }
    }

    /** The screen [key] is shown: how long its frames take to come in on
     *  time again is logged, from the key that asked for it. */
    fun screenShown(key: String) {
        val now = System.nanoTime()
        val asked = lastKeyNs.takeIf { now - it < ASKED_WITHIN_NS } ?: now
        handler?.post {
            opening?.let(::report)
            val visit = (visits[key] ?: 0) + 1
            visits[key] = visit
            val followed = Opening(key, visit, asked)
            opening = followed
            recent.filter { it.start >= asked }.forEach { follow(followed, it) }
        }
    }

    private fun follow(followed: Opening, seen: Seen) {
        if (seen.start < followed.start) {
            return
        }
        followed.frames++
        seen.phases.forEachIndexed { at, ns -> followed.phases[at] += ns }
        followed.worst = maxOf(followed.worst, seen.total)
        if (seen.late) {
            followed.slow++
            followed.calm = 0
            followed.end = seen.start + seen.total
        } else {
            followed.calm++
        }
        if (followed.calm >= CALM_FRAMES || seen.start - followed.start > FOLLOWED_NS) {
            report(followed)
        }
    }

    private fun report(followed: Opening) {
        opening = null
        val sums = Phase.entries.joinToString(" ") { "${it.label}=${ms(followed.phases[it.ordinal])}" }
        Log.i(
            TAG_SCREEN,
            "shown ${followed.key} (visit ${followed.visit}): on time again after ${ms(followed.end - followed.start)} ms, " +
                "${followed.slow} slow of ${followed.frames} frames, worst ${ms(followed.worst)} ms | total $sums",
        )
    }

    private fun summary() {
        val made = shadowsMade.getAndSet(0)
        val drawn = shadowsDrawn.getAndSet(0)
        val baked = shadowsBaked.getAndSet(0)
        val baking = bakingNs.getAndSet(0)
        val composed = cardsComposed.getAndSet(0)
        val composing = composingNs.getAndSet(0)
        val lit = litStarted.getAndSet(0)
        val pressed = keys.getAndSet(0)
        val faded = picturesFaded.getAndSet(0)
        val remembered = picturesFromMemory.getAndSet(0)
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
                "mean $means | keys=$pressed lit=$lit | pictures faded=$faded memory=$remembered | ${switched()} | shadows made=$made baked=$baked in ${ms(baking)} ms drawn=$drawn | cards composed=$composed in ${ms(composing)} ms | heap=${heap} MB",
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
        lastKeyNs = event.eventTime * 1_000_000
        Log.d(TAG_KEY, "${KeyEvent.keyCodeToString(event.keyCode)} repeat=${event.repeatCount} waited=${now - event.eventTime} ms gap=$gap ms")
    }

    /** A shadow's drawing worked out anew. */
    fun shadowMade() {
        shadowsMade.incrementAndGet()
    }

    /** A shadow's mask drawn anew rather than found made, from [began]. */
    fun shadowBaked(began: Long) {
        shadowsBaked.incrementAndGet()
        bakingNs.addAndGet(System.nanoTime() - began)
    }

    /** A card composed, from [began]. */
    fun cardComposed(began: Long) {
        cardsComposed.incrementAndGet()
        composingNs.addAndGet(System.nanoTime() - began)
    }

    /** A shadow drawn. */
    fun shadowDrawn() {
        shadowsDrawn.incrementAndGet()
    }

    /** A picture handed to the screen from [source]: one from the memory
     *  cache shows at once, any other fades in. */
    fun pictureShown(source: String) {
        (if (source == "MEMORY_CACHE") picturesFromMemory else picturesFaded).incrementAndGet()
    }

    /** A piece starting to light or fade. */
    fun litStarted() {
        litStarted.incrementAndGet()
    }

    private fun ms(ns: Long) = "%.1f".format(ns / NS_PER_MS)
}

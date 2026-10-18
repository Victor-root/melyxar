package app.melyxar.tv.probe

import android.os.Looper
import android.os.SystemClock
import android.util.Log

/**
 * Where the main thread spends its time after the remote is pressed: its
 * stack is read every few milliseconds until the keys have been still for a
 * while, and the methods found on it most often are logged, each counted
 * once per reading it was in.
 */
internal object MainSampler {
    private const val TAG = "MlxPerf.Sample"
    private const val EVERY_MS = 5L
    private const val STILL_AFTER_MS = 700L
    private const val SHOWN = 40

    /** Frames found on almost every reading, which say nothing. */
    private val PLUMBING = listOf(
        "android.os.", "android.app.ActivityThread", "java.lang.", "com.android.internal.", "dalvik.",
        "android.view.Choreographer", "android.view.ViewRootImpl", "android.view.View.", "android.view.ViewGroup.",
    )

    @Volatile
    private var until = 0L
    private var keys = 0
    private var thread: Thread? = null

    /** A key pressed: reads on until the keys have been still a while. */
    @Synchronized
    fun pressed() {
        keys++
        until = SystemClock.uptimeMillis() + STILL_AFTER_MS
        if (thread == null) {
            thread = Thread(::sample, "MlxPerf.Sample").apply { start() }
        }
    }

    private fun sample() {
        val main = Looper.getMainLooper().thread
        val counts = HashMap<String, Int>()
        var readings = 0
        var idle = 0
        val began = SystemClock.uptimeMillis()
        while (SystemClock.uptimeMillis() < until) {
            val stack = main.stackTrace
            readings++
            if (stack.firstOrNull()?.methodName == "nativePollOnce") {
                idle++
            } else {
                stack.mapTo(HashSet()) { "${it.className}.${it.methodName}" }
                    .filter { name -> PLUMBING.none(name::startsWith) }
                    .forEach { counts[it] = (counts[it] ?: 0) + 1 }
            }
            Thread.sleep(EVERY_MS)
        }
        val pressed = synchronized(this) {
            thread = null
            keys.also { keys = 0 }
        }
        val busy = readings - idle
        Log.i(TAG, "after $pressed keys, ${SystemClock.uptimeMillis() - began} ms: $readings readings, main busy in $busy")
        counts.entries.sortedByDescending { it.value }.take(SHOWN).forEach { (name, count) ->
            Log.i(TAG, "  ${count * 100 / busy.coerceAtLeast(1)}% ($count) $name")
        }
    }
}

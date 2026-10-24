package app.melyxar.tv.play

import android.os.SystemClock
import app.melyxar.server.Api
import app.melyxar.server.BlockTimed
import okhttp3.Call
import okhttp3.EventListener
import java.io.IOException

/*
 * What the connection to the server carries, as the web player measures it:
 * a small block first, a bigger one for a fast connection, each timed from
 * its first byte to its last. What the films measure afterwards is kept too,
 * so the next film of the evening starts from it without being measured.
 */

private const val FIRST_BLOCK = 256 * 1024
private const val QUICK_S = 0.25
private const val SECOND_BLOCK_S = 0.75
private const val SECOND_BLOCK_LEAST = 1024 * 1024
private const val SECOND_BLOCK_MOST = 16 * 1024 * 1024
private const val PATIENCE_MS = 4_000L
private const val TIMED_AT_LEAST_S = 0.05
private const val KNOWN_FOR_MS = 10 * 60_000L

private var known: Pair<Double, Long>? = null

/** Keeps what a film measured, for the next one. */
fun rememberSpeed(bitsPerSecond: Double) {
    known = bitsPerSecond to SystemClock.elapsedRealtime()
}

private val BlockTimed.bitsPerSecond: Double
    get() = if (flowing >= TIMED_AT_LEAST_S) afterTheFirst * 8 / flowing else bytes * 8 / seconds.coerceAtLeast(0.001)

/** What the connection carries in bits a second, or nothing when it could
 *  not be measured. */
suspend fun connectionSpeed(api: Api): Double? {
    known?.let { (speed, at) -> if (SystemClock.elapsedRealtime() - at < KNOWN_FOR_MS) return speed }
    return try {
        val first = api.timeBlock(FIRST_BLOCK, PATIENCE_MS)
        val speed = if (first.seconds < QUICK_S) {
            val bytes = (first.bitsPerSecond / 8 * SECOND_BLOCK_S).toInt().coerceIn(SECOND_BLOCK_LEAST, SECOND_BLOCK_MOST)
            api.timeBlock(bytes, PATIENCE_MS).bitsPerSecond
        } else {
            first.bitsPerSecond
        }
        rememberSpeed(speed)
        speed
    } catch (_: IOException) {
        // A refusal is one too: either way, nothing is known.
        null
    }
}

/**
 * Times each segment of a rebuilt film from its first byte to its last,
 * which leaves out the server making it: a segment waited on while the server
 * makes it says how fast the server is, not the connection. One for each
 * request, as OkHttp makes them.
 */
class TimedSegments(private val heard: (bytes: Long, flowingMs: Double) -> Unit) : EventListener() {
    private var startedAt = 0L

    override fun responseBodyStart(call: Call) {
        startedAt = System.nanoTime()
    }

    override fun responseBodyEnd(call: Call, byteCount: Long) {
        if ("/segment-" in call.request().url.encodedPath) {
            heard(byteCount, (System.nanoTime() - startedAt) / 1e6)
        }
    }
}

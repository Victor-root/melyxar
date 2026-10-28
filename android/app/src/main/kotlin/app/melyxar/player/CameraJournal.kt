package app.melyxar.player

import android.net.Uri
import android.os.SystemClock
import android.util.Log
import app.melyxar.BuildConfig
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonObjectBuilder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** The tags of the logcat, one for each side of a camera, as the server's
 *  journal has them (`camera_watch`). */
internal const val OPEN = "MelyxarCamOpen"
internal const val DELAY = "MelyxarCamDelay"
internal const val FRAMES = "MelyxarCamFrames"
internal const val NET = "MelyxarCamNet"
internal const val SCREEN = "MelyxarCamScreen"
internal const val ERROR = "MelyxarCamError"

/** How long a crash waits for its fact to reach the server. */
private const val CRASH_TOLD_WITHIN_MS = 2_000L

/** Facts waiting for the server, beyond which the oldest go: a server out of
 *  reach is not a reason to hold the whole evening in memory. */
private const val WAITING_AT_MOST = 200

/**
 * The journal of one camera watched live: every fact goes to the server's
 * journal, under the tags `camera-open`, `camera-delay`, `camera-frames`,
 * `camera-net`, `camera-screen` and `camera-error`, so nothing needs a cable
 * to be read; in the debug build each is also written to the logcat under
 * the matching `MelyxarCam…` tag. The facts are the server's own list: it
 * refuses any other. The account written into the camera's address never
 * leaves: it is taken out of every word sent.
 *
 * [post] hands one fact to the server and returns once it has, off the main
 * thread.
 */
class CameraJournal(private val camera: String, private val screen: String, private val post: (JsonObject) -> Unit) {
    private val started = SystemClock.elapsedRealtime()
    private var secrets: List<String> = emptyList()

    fun since(): Long = SystemClock.elapsedRealtime() - started

    /** The address the camera is opened at, whose account is kept out. */
    fun openingAt(address: String) {
        secrets = Uri.parse(address).let { uri ->
            listOfNotNull(uri.encodedUserInfo, uri.userInfo)
                .flatMap { listOf(it) + it.split(':') }
                .filter { it.length >= 2 }
                .distinct()
                .sortedByDescending { it.length }
        }
        synchronized(open) {
            open.remove(this)
            open.add(this)
        }
        catchCrashes()
    }

    fun closed() {
        synchronized(open) { open.remove(this) }
    }

    fun hidden(text: String?): String {
        var out = text ?: return "null"
        out = out.replace(Regex("(rtsps?://)[^/@\\s]*@"), "$1***@")
        for (secret in secrets) out = out.replace(secret, "***")
        return out
    }

    private fun fact(saw: String, fields: JsonObjectBuilder.() -> Unit): JsonObject = buildJsonObject {
        put("camera", camera)
        put("screen", screen)
        put("at_ms", since())
        put("saw", saw)
        fields()
    }

    /** One fact, under [tag] in the logcat. */
    fun tell(tag: String, saw: String, fields: JsonObjectBuilder.() -> Unit = {}) {
        val told = fact(saw, fields)
        if (BuildConfig.TRACES) {
            Log.i(tag, told.toString())
        }
        sending.execute { tryToPost(told) }
    }

    fun screen(event: String, level: Int? = null) = tell(SCREEN, "screen") {
        put("event", event)
        level?.let { put("level", it) }
    }

    /** Something failed [during] one of the server's named steps. */
    fun failed(during: String, error: Throwable?, code: String? = null) {
        val told = fact("failed") {
            put("during", during)
            code?.let { put("code", hidden(it)) }
            put("causes", hidden(causes(error)))
        }
        if (BuildConfig.TRACES) {
            Log.e(ERROR, told.toString())
            error?.let { Log.e(ERROR, hidden(Log.getStackTraceString(it))) }
        }
        if (during == "crash") {
            // The application goes right after: told on a thread of its own,
            // waited for a moment, rather than queued behind the others.
            Thread { tryToPost(told) }.apply {
                start()
                join(CRASH_TOLD_WITHIN_MS)
            }
        } else {
            sending.execute { tryToPost(told) }
        }
    }

    private fun tryToPost(told: JsonObject) {
        try {
            post(told)
        } catch (failure: Exception) {
            if (BuildConfig.TRACES) {
                Log.w(ERROR, "the server's journal did not take a fact: ${causes(failure)}")
            }
        }
    }

    internal companion object {
        /** The cameras open now, the last opened last. */
        private val open = mutableListOf<CameraJournal>()

        /** The camera opened last, which the exchange with a camera is
         *  told to: one is watched at a time. */
        val current: CameraJournal? get() = synchronized(open) { open.lastOrNull() }

        /** One after the other, in the order told. */
        private val sending = ThreadPoolExecutor(
            1,
            1,
            0L,
            TimeUnit.MILLISECONDS,
            LinkedBlockingQueue(WAITING_AT_MOST),
            ThreadPoolExecutor.DiscardOldestPolicy(),
        )

        fun causes(error: Throwable?): String =
            generateSequence(error) { it.cause }.take(8)
                .joinToString(" <- ") { "${it.javaClass.name}: ${it.message}" }

        private var catching = false

        /** A crash while a camera is open is told before the application
         *  goes, as it would have gone. */
        @Synchronized
        private fun catchCrashes() {
            if (catching) return
            catching = true
            val before = Thread.getDefaultUncaughtExceptionHandler()
            Thread.setDefaultUncaughtExceptionHandler { thread, crash ->
                current?.failed("crash", crash, "thread ${thread.name}")
                before?.uncaughtException(thread, crash)
            }
        }
    }
}

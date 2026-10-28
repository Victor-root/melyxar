package app.melyxar.player

import android.util.Log
import app.melyxar.BuildConfig

/**
 * The journal of a live camera, in the debug build only, under one tag for
 * each side of it so a filter on one tag tells one story:
 *
 * - `MelyxarCamOpen`: how the camera is reached and how long each step to
 *   the first picture takes, the exchange with the camera included.
 * - `MelyxarCamDelay`: how far behind the camera the picture is, every
 *   second, and every change of speed made to catch up.
 * - `MelyxarCamFrames`: pictures received, shown, dropped, and the moments
 *   the picture stands still.
 * - `MelyxarCamNet`: what comes from the camera, reopenings and losses.
 * - `MelyxarCamScreen`: the screen around the picture, its life, its size,
 *   the memory, and the moments the application itself is held.
 * - `MelyxarCamError`: every error, with its whole chain of causes, and a
 *   crash while a camera is open, before the application goes.
 *
 * `adb logcat -s MelyxarCamOpen MelyxarCamDelay MelyxarCamFrames
 * MelyxarCamNet MelyxarCamScreen MelyxarCamError` gives all of it.
 */
internal object CameraLog {
    const val OPEN = "MelyxarCamOpen"
    const val DELAY = "MelyxarCamDelay"
    const val FRAMES = "MelyxarCamFrames"
    const val NET = "MelyxarCamNet"
    const val SCREEN = "MelyxarCamScreen"
    const val ERROR = "MelyxarCamError"

    /** Account names and passwords to keep out of every line, those of the
     *  cameras open now. */
    private val secrets = mutableSetOf<String>()

    /** How many cameras are open, for the crash catcher. */
    @Volatile
    private var open = 0

    @Synchronized
    fun opened(hidden: Collection<String>) {
        secrets += hidden
        open += 1
        catchCrashes()
    }

    @Synchronized
    fun closed(hidden: Collection<String>) {
        open = (open - 1).coerceAtLeast(0)
        if (open == 0) {
            secrets -= hidden.toSet()
        }
    }

    @Synchronized
    fun hidden(text: String?): String {
        var out = text ?: return "null"
        out = out.replace(Regex("(rtsps?://)[^/@\\s]*@"), "$1***@")
        for (secret in secrets.sortedByDescending { it.length }) out = out.replace(secret, "***")
        return out
    }

    fun say(tag: String, line: String) {
        if (BuildConfig.TRACES) {
            Log.i(tag, hidden(line))
        }
    }

    fun error(line: String, error: Throwable?) {
        if (BuildConfig.TRACES) {
            Log.e(ERROR, hidden("$line ${causes(error)}"))
            error?.let { Log.e(ERROR, hidden(Log.getStackTraceString(it))) }
        }
    }

    fun causes(error: Throwable?): String =
        generateSequence(error) { it.cause }.take(8)
            .joinToString(" <- ") { "${it.javaClass.name}: ${it.message}" }

    private var catching = false

    /** A crash while a camera is open is written under the camera's tag
     *  before the application goes, as it would have gone. */
    private fun catchCrashes() {
        if (!BuildConfig.TRACES || catching) return
        catching = true
        val before = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, crash ->
            if (open > 0) {
                error("crash on thread ${thread.name} with $open camera(s) open", crash)
            }
            before?.uncaughtException(thread, crash)
        }
    }
}

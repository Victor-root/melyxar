package app.melyxar.music

import app.melyxar.server.Api
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.IOException

/** How often the server is told the song is still there. */
private const val EVERY_MS = 10_000L

/**
 * What the server is told of the song playing, for the administration to
 * follow it as it follows the films, in the words of the web's player
 * (`reporting.ts`): which song, where it has got to, whether it is paused.
 * Said when a song is loaded, at every change between playing and paused,
 * every few seconds in between, and once more as it is left. A line is held
 * open beside them for as long as the song is: it is how an administrator's
 * stop arrives, which [stop] answers.
 *
 * A word that does not arrive says nothing: the next carries the same news,
 * and the music keeps playing.
 */
internal class MusicReports(
    private val asking: () -> Api?,
    private val scope: CoroutineScope,
    /** Where the song playing has got to, in seconds. */
    private val position: () -> Double,
    private val stop: () -> Unit,
) {
    private var song: String? = null
    private var playing = false
    private var beating: Job? = null
    private var line: Job? = null

    /** The song open now, or none: the one before is said to be left. */
    fun song(id: String?) {
        if (id == song) {
            return
        }
        song?.let { left -> say(left, leaving = true, paused = true) }
        beating?.cancel()
        line?.cancel()
        line = null
        song = id
        val open = id ?: return
        beating = scope.launch {
            tell(open, fresh = true)
            while (true) {
                delay(EVERY_MS)
                tell(open, fresh = false)
            }
        }
    }

    /** Played or paused: the administration sees it at once. */
    fun playing(now: Boolean) {
        if (now == playing) {
            return
        }
        playing = now
        song?.let { open -> say(open, leaving = false, paused = !now) }
    }

    private fun say(open: String, leaving: Boolean, paused: Boolean) {
        val api = asking() ?: return
        val at = position()
        scope.launch {
            try {
                api.stillPlaying(open, at, paused, leaving, fresh = false)
            } catch (_: IOException) {
                // The next word carries the same news.
            }
        }
    }

    private suspend fun tell(open: String, fresh: Boolean) {
        val api = asking() ?: return
        val heard = try {
            api.stillPlaying(open, position(), !playing, leaving = false, fresh = fresh)
        } catch (_: IOException) {
            return
        }
        if (open != song) {
            return
        }
        if (heard.stop) {
            stop()
            return
        }
        if (fresh && line == null) {
            line = scope.launch {
                try {
                    api.playerLine(open).collect { word ->
                        if (word == "stop" && open == song) {
                            stop()
                        }
                    }
                } catch (_: IOException) {
                    // Gone with the network: the reports still carry a stop.
                }
            }
        }
    }
}

package app.melyxar.player

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.HttpDataSource
import androidx.media3.exoplayer.upstream.DefaultLoadErrorHandlingPolicy
import androidx.media3.exoplayer.upstream.LoadErrorHandlingPolicy.LoadErrorInfo
import java.io.IOException

/** How often a server that went away is asked again. */
private const val AGAIN_EVERY_MS = 2_000L

/** How many times it is asked before the film is given up: three minutes,
 *  past a restart of the server or of the machine it runs on. */
private const val TRIES_WHILE_AWAY = 90

/** How many times anything else is tried, as Media3 does on its own. */
private const val TRIES_OTHERWISE = 3

/**
 * A film rides out a server that went away for a moment, as a browser does.
 *
 * Left to Media3, a player gives up after a few seconds without an answer, and
 * a restart of the server stopped the film on the application while a browser,
 * patient for longer, carried on. Unreachable, a gateway with nothing behind
 * it, or a piece of film from a session the restart forgot is asked for again
 * until the server is back and the film's own beat has opened another session
 * where the viewer stands. Anything else fails as soon as it did before.
 */
@OptIn(UnstableApi::class)
class RidesOutARestart : DefaultLoadErrorHandlingPolicy() {
    override fun getRetryDelayMsFor(loadErrorInfo: LoadErrorInfo): Long = when {
        serverAway(loadErrorInfo.exception) ->
            if (loadErrorInfo.errorCount <= TRIES_WHILE_AWAY) AGAIN_EVERY_MS else C.TIME_UNSET
        loadErrorInfo.errorCount > TRIES_OTHERWISE -> C.TIME_UNSET
        else -> super.getRetryDelayMsFor(loadErrorInfo)
    }

    override fun getMinimumLoadableRetryCount(dataType: Int): Int = TRIES_WHILE_AWAY

    private fun serverAway(failure: IOException): Boolean = when (failure) {
        is HttpDataSource.InvalidResponseCodeException -> failure.responseCode in AWAY_ANSWERS
        // Answers about the address or what came back, which no wait changes.
        is HttpDataSource.CleartextNotPermittedException,
        is HttpDataSource.InvalidContentTypeException -> false
        // No answer at all: refused, cut off or too slow.
        is HttpDataSource.HttpDataSourceException -> true
        else -> false
    }

    private companion object {
        /** A session the restart forgot, and a gateway waiting on the server. */
        val AWAY_ANSWERS = setOf(404, 410, 502, 503, 504)
    }
}

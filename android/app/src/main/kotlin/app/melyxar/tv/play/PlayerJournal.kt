package app.melyxar.tv.play

import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.analytics.AnalyticsListener
import app.melyxar.server.Api
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObjectBuilder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.io.IOException

/** How much of a failure's words is sent, the server cutting it again. */
private const val WORDS_SENT = 300

/**
 * What the television's player sees of a film, told to the server's journal
 * as the web's player tells it, under the server's own list of facts: a
 * silence of the sound card, a wait for film that had not arrived, pictures
 * dropped, a jump and what moved it, a film that failed. Without it, a film
 * played on the television left nothing but what the server produced.
 *
 * Each fact names the session the film is rebuilt in, or the film itself
 * when it is played as it lies on the disk.
 */
@OptIn(UnstableApi::class)
class PlayerJournal(
    private val api: Api,
    private val scope: CoroutineScope,
    private val source: String,
    private val session: () -> String?,
    private val shown: () -> Int,
    private val playing: () -> Boolean,
) : AnalyticsListener {
    /** What moves the film next, said by whoever is about to move it;
     *  anything else that moves it moved it by itself. */
    private var moving: String? = null
    private var waitingSince: Long? = null
    private var shownBefore = 0

    /** Says what is about to move the film, among the server's words. */
    fun moving(by: String) {
        moving = by
    }

    /** One fact, sent for whatever is playing. */
    fun tell(saw: String, fields: JsonObjectBuilder.() -> Unit) {
        val fact = buildJsonObject {
            val rebuilt = session()
            if (rebuilt != null) put("session", rebuilt) else put("source", source)
            put("saw", saw)
            fields()
        }
        scope.launch {
            try {
                api.tellTheJournal(fact)
            } catch (_: IOException) {
                // Only the journal goes without it.
            }
        }
    }

    private fun secondOf(eventTime: AnalyticsListener.EventTime) = eventTime.currentPlaybackPositionMs / 1000.0

    override fun onAudioUnderrun(eventTime: AnalyticsListener.EventTime, bufferSize: Int, bufferSizeMs: Long, elapsedSinceLastFeedMs: Long) {
        tell("sound_ran_dry") {
            put("at_second", secondOf(eventTime))
            put("holds_ms", bufferSizeMs)
            put("since_fed_ms", elapsedSinceLastFeedMs)
        }
    }

    override fun onDroppedVideoFrames(eventTime: AnalyticsListener.EventTime, droppedFrames: Int, elapsedMs: Long) {
        val now = shown()
        tell("pictures_were_dropped") {
            put("at_second", secondOf(eventTime))
            put("over_ms", elapsedMs)
            put("pictures_shown", (now - shownBefore).coerceAtLeast(0))
            put("pictures_dropped", droppedFrames)
        }
        shownBefore = now
    }

    override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) {
        when (state) {
            Player.STATE_BUFFERING -> if (waitingSince == null) waitingSince = SystemClock.elapsedRealtime()
            Player.STATE_READY -> waitingSince?.let { since ->
                waitingSince = null
                tell("playback_picked_up_again") {
                    put("at_second", secondOf(eventTime))
                    put("waited_ms", SystemClock.elapsedRealtime() - since)
                }
            }
            else -> waitingSince = null
        }
    }

    override fun onPositionDiscontinuity(
        eventTime: AnalyticsListener.EventTime,
        oldPosition: Player.PositionInfo,
        newPosition: Player.PositionInfo,
        reason: Int,
    ) {
        if (reason != Player.DISCONTINUITY_REASON_SEEK) return
        val by = moving ?: "not_the_page"
        moving = null
        tell("viewer_jumped") {
            put("from_second", oldPosition.positionMs / 1000.0)
            put("to_second", newPosition.positionMs / 1000.0)
            put("was_playing", playing())
            put("moved_by", by)
        }
    }

    override fun onPlayerError(eventTime: AnalyticsListener.EventTime, error: PlaybackException) {
        tell("playback_refused") {
            put("because", "${error.errorCodeName}: ${error.cause?.message ?: error.message}".take(WORDS_SENT))
            put("browser_took_over", false)
        }
    }
}

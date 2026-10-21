package app.melyxar.tv.play

import android.os.SystemClock
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.source.LoadEventInfo
import androidx.media3.exoplayer.source.MediaLoadData
import app.melyxar.BuildConfig

const val START_LOG = "MelyxarStart"

/**
 * How long a film takes to start, step by step, in the debug build's log:
 * each step with the time since the screen opened and since the step before,
 * and every load, decoder and state of the player until the film plays.
 */
@OptIn(UnstableApi::class)
class StartClock : AnalyticsListener {
    private val began = SystemClock.elapsedRealtime()
    private var last = began
    private var playing = false

    fun mark(step: String) {
        if (!BuildConfig.TRACES || playing) {
            return
        }
        val now = SystemClock.elapsedRealtime()
        Log.d(START_LOG, "+${now - began} ms (+${now - last}) $step")
        last = now
    }

    override fun onLoadStarted(
        eventTime: AnalyticsListener.EventTime,
        loadEventInfo: LoadEventInfo,
        mediaLoadData: MediaLoadData,
        retryCount: Int,
    ) = mark("load started: ${loadEventInfo.dataSpec.uri.lastPathSegment} from ${loadEventInfo.dataSpec.position}, kind ${mediaLoadData.dataType}")

    override fun onLoadCompleted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData) =
        mark("load done: ${loadEventInfo.bytesLoaded / 1024} KiB in ${loadEventInfo.loadDurationMs} ms, kind ${mediaLoadData.dataType}, ${held(eventTime)}")

    override fun onVideoDecoderInitialized(
        eventTime: AnalyticsListener.EventTime,
        decoderName: String,
        initializedTimestampMs: Long,
        initializationDurationMs: Long,
    ) = mark("video decoder $decoderName ready in $initializationDurationMs ms")

    override fun onAudioDecoderInitialized(
        eventTime: AnalyticsListener.EventTime,
        decoderName: String,
        initializedTimestampMs: Long,
        initializationDurationMs: Long,
    ) = mark("audio decoder $decoderName ready in $initializationDurationMs ms")

    override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) = mark(
        "player " + when (state) {
            Player.STATE_IDLE -> "idle"
            Player.STATE_BUFFERING -> "buffering"
            Player.STATE_READY -> "ready"
            else -> "ended"
        } + ", ${held(eventTime)}",
    )

    override fun onIsLoadingChanged(eventTime: AnalyticsListener.EventTime, isLoading: Boolean) =
        mark("${if (isLoading) "loading" else "loading no more"}, ${held(eventTime)}")

    /** Where the film is, and how much of it the player holds ahead. */
    private fun held(eventTime: AnalyticsListener.EventTime) =
        "at ${eventTime.currentPlaybackPositionMs} ms, ${eventTime.totalBufferedDurationMs} ms held ahead"

    override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) =
        mark("first picture on screen")

    override fun onIsPlayingChanged(eventTime: AnalyticsListener.EventTime, isPlaying: Boolean) {
        if (isPlaying) {
            mark("playing")
            playing = true
        }
    }
}

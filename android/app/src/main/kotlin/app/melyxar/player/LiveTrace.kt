package app.melyxar.player

import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.Choreographer
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.common.util.Log as MediaLog
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.source.LoadEventInfo
import androidx.media3.exoplayer.source.MediaLoadData
import app.melyxar.BuildConfig
import java.io.IOException
import kotlinx.serialization.json.put

private const val PULSE_MS = 1_000L

/** Seconds a summary covers: one a second would push the opening of a
 *  camera out of the server's journal within the hour. */
private const val SUMMARY_SECONDS = 10

/** A frame of the application later than this is the application held. */
private const val HELD_MS = 100L

/** Lines of the camera's description worth keeping in the logcat: the
 *  streams and their formats. The rest of it says nothing about a picture. */
private val DESCRIBED = listOf("m=", "a=rtpmap", "a=fmtp", "a=framerate", "a=control", "a=range", "b=")

/** Lines of the exchange with the camera never written: the account is in
 *  them. */
private val SECRET = listOf("authorization:", "proxy-authorization:", "www-authenticate:")

/** The steps of RTSP, by the names the server's journal knows them under. */
private val STEPS = mapOf(
    "OPTIONS" to "options",
    "DESCRIBE" to "describe",
    "SETUP" to "setup",
    "PLAY" to "play",
    "PAUSE" to "pause",
    "TEARDOWN" to "teardown",
    "GET_PARAMETER" to "get_parameter",
    "SET_PARAMETER" to "set_parameter",
)

/**
 * What one camera watched live does, told to its [journal]: how long to the
 * first picture, every ten seconds how far behind the camera it is and the
 * pictures received, shown and dropped, the moments it stands still, and
 * every error.
 */
@OptIn(UnstableApi::class)
internal class LiveTrace(
    private val player: ExoPlayer,
    private val journal: CameraJournal,
    private val drift: LiveDrift,
) : AnalyticsListener {
    private val clock = Handler(Looper.getMainLooper())

    private var firstLoad: Long? = null
    private var firstTracks: Long? = null
    private var decoderReady: Long? = null
    private var firstFrame = 0L
    private var frames = 0L

    private var shownBefore = 0
    private var droppedBefore = 0
    private var queuedBefore = 0
    private var stillFor = 0
    private var beats = 0
    private var received = 0
    private var shownSince = 0
    private var droppedSince = 0
    private var mostBehind = 0L

    /** Pictures handed to the decoder and not out of it yet: a television's
     *  decoder can hold seconds of them, unseen by the player's buffer. */
    private var inDecoder = 0
    private var stalledAt = 0L

    private val pulse = object : Runnable {
        override fun run() {
            beat()
            clock.postDelayed(this, PULSE_MS)
        }
    }

    private var lastFrameNanos = 0L
    private val held = object : Choreographer.FrameCallback {
        override fun doFrame(frameTimeNanos: Long) {
            if (lastFrameNanos != 0L) {
                val gap = (frameTimeNanos - lastFrameNanos) / 1_000_000
                if (gap > HELD_MS) journal.tell(SCREEN, "held") { put("ms", gap) }
            }
            lastFrameNanos = frameTimeNanos
            Choreographer.getInstance().postFrameCallback(this)
        }
    }

    init {
        clock.postDelayed(pulse, PULSE_MS)
        Choreographer.getInstance().postFrameCallback(held)
    }

    private fun beat() {
        // Stopped, behind the television's other apps: nothing to tell.
        if (player.playbackState == Player.STATE_IDLE) return
        val video = player.videoDecoderCounters
        val shown = video?.renderedOutputBufferCount ?: 0
        val dropped = video?.droppedBufferCount ?: 0
        val queued = video?.queuedInputBufferCount ?: 0
        if (queued < queuedBefore) {
            // A new decoder, the camera opened again: it counts from nought.
            shownBefore = 0
            queuedBefore = 0
            droppedBefore = 0
        }
        inDecoder = queued - shown - dropped - (video?.skippedOutputBufferCount ?: 0)
        val newlyShown = shown - shownBefore
        val newlyQueued = queued - queuedBefore
        val newlyDropped = dropped - droppedBefore
        shownBefore = shown
        queuedBefore = queued
        droppedBefore = dropped

        beats += 1
        received += newlyQueued
        shownSince += newlyShown
        droppedSince += newlyDropped
        mostBehind = maxOf(mostBehind, player.totalBufferedDuration)
        if (beats == SUMMARY_SECONDS) summarize()

        if (player.playbackState == Player.STATE_READY && player.isPlaying && newlyShown == 0) {
            stillFor += 1
            journal.tell(FRAMES, "stood_still") {
                put("seconds", stillFor)
                put("received", newlyQueued)
            }
        } else {
            stillFor = 0
        }
    }

    private fun summarize() {
        val runtime = Runtime.getRuntime()
        journal.tell(DELAY, "summary") {
            put("over_s", beats)
            put("state", stateName(player.playbackState))
            put("playing", player.isPlaying)
            put("behind_ms", player.totalBufferedDuration)
            put("most_behind_ms", mostBehind)
            put("in_decoder", inDecoder)
            put("speed", player.playbackParameters.speed)
            put("fell_behind_ms", drift.fellBehind())
            put("received", received)
            put("shown", shownSince)
            put("dropped", droppedSince)
            put("memory_mb", (runtime.totalMemory() - runtime.freeMemory()) / 1_048_576)
            put("most_memory_mb", runtime.maxMemory() / 1_048_576)
        }
        beats = 0
        received = 0
        shownSince = 0
        droppedSince = 0
        mostBehind = 0
    }

    private fun stateName(state: Int) = when (state) {
        Player.STATE_BUFFERING -> "buffering"
        Player.STATE_READY -> "ready"
        Player.STATE_ENDED -> "ended"
        else -> "idle"
    }

    private fun kindOf(type: Int) = when (type) {
        C.TRACK_TYPE_VIDEO -> "video"
        C.TRACK_TYPE_AUDIO -> "audio"
        else -> "other"
    }

    override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) {
        when {
            state == Player.STATE_BUFFERING && firstFrame > 0 -> {
                stalledAt = SystemClock.elapsedRealtime()
                journal.tell(FRAMES, "waiting") { put("in_hand_ms", eventTime.totalBufferedDurationMs) }
            }
            state == Player.STATE_READY && stalledAt > 0 -> {
                journal.tell(FRAMES, "moving_again") { put("waited_ms", SystemClock.elapsedRealtime() - stalledAt) }
                stalledAt = 0
            }
        }
    }

    override fun onPlayerError(eventTime: AnalyticsListener.EventTime, error: PlaybackException) {
        journal.failed("player", error, error.errorCodeName)
    }

    override fun onTracksChanged(eventTime: AnalyticsListener.EventTime, tracks: Tracks) {
        if (firstTracks == null && !tracks.isEmpty) {
            firstTracks = journal.since()
        }
        for (group in tracks.groups) {
            for (i in 0 until group.length) {
                val format = group.getTrackFormat(i)
                journal.tell(OPEN, "stream") {
                    put("kind", kindOf(group.type))
                    (format.codecs ?: format.sampleMimeType)?.let { put("codec", it) }
                    if (format.width > 0) put("width", format.width)
                    if (format.height > 0) put("height", format.height)
                    if (format.frameRate > 0) put("fps", format.frameRate)
                    if (format.sampleRate > 0) put("sample_rate", format.sampleRate)
                    if (format.channelCount > 0) put("channels", format.channelCount)
                    put("supported", group.isTrackSupported(i))
                    put("selected", group.isTrackSelected(i))
                }
            }
        }
    }

    override fun onVideoDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
        decoderReady = journal.since()
        journal.tell(OPEN, "decoder") {
            put("kind", "video")
            put("name", decoderName)
            put("ready_ms", initializationDurationMs)
        }
    }

    override fun onAudioDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
        journal.tell(OPEN, "decoder") {
            put("kind", "audio")
            put("name", decoderName)
            put("ready_ms", initializationDurationMs)
        }
    }

    override fun onAudioUnderrun(eventTime: AnalyticsListener.EventTime, bufferSize: Int, bufferSizeMs: Long, elapsedSinceLastFeedMs: Long) {
        journal.tell(FRAMES, "sound_ran_dry") { put("last_fed_ms", elapsedSinceLastFeedMs) }
    }

    override fun onAudioSinkError(eventTime: AnalyticsListener.EventTime, audioSinkError: Exception) {
        journal.failed("audio_sink", audioSinkError)
    }

    override fun onAudioCodecError(eventTime: AnalyticsListener.EventTime, audioCodecError: Exception) {
        journal.failed("audio_codec", audioCodecError)
    }

    override fun onVideoCodecError(eventTime: AnalyticsListener.EventTime, videoCodecError: Exception) {
        journal.failed("video_codec", videoCodecError)
    }

    override fun onVideoSizeChanged(eventTime: AnalyticsListener.EventTime, videoSize: VideoSize) {
        if (videoSize.width > 0) {
            journal.tell(SCREEN, "picture") {
                put("width", videoSize.width)
                put("height", videoSize.height)
            }
        }
    }

    override fun onSurfaceSizeChanged(eventTime: AnalyticsListener.EventTime, width: Int, height: Int) {
        if (width > 0) {
            journal.tell(SCREEN, "drawn") {
                put("width", width)
                put("height", height)
            }
        }
    }

    override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) {
        if (firstFrame != 0L) return
        firstFrame = SystemClock.elapsedRealtime()
        journal.tell(OPEN, "first_picture") {
            firstLoad?.let { put("first_data_ms", it) }
            firstTracks?.let { put("streams_ms", it) }
            decoderReady?.let { put("decoder_ms", it) }
        }
    }

    override fun onVideoFrameProcessingOffset(eventTime: AnalyticsListener.EventTime, totalProcessingOffsetUs: Long, frameCount: Int) {
        frames += frameCount
    }

    override fun onDroppedVideoFrames(eventTime: AnalyticsListener.EventTime, droppedFrames: Int, elapsedMs: Long) {
        journal.tell(FRAMES, "dropped") {
            put("frames", droppedFrames)
            put("within_ms", elapsedMs)
        }
    }

    override fun onLoadStarted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData, retryCount: Int) {
        if (firstLoad == null) firstLoad = journal.since()
    }

    override fun onLoadError(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData, error: IOException, wasCanceled: Boolean) {
        journal.failed("load", error, if (wasCanceled) "canceled" else null)
    }

    override fun onPlayerReleased(eventTime: AnalyticsListener.EventTime) {
        clock.removeCallbacks(pulse)
        Choreographer.getInstance().removeFrameCallback(held)
        journal.tell(OPEN, "let_go") { put("frames", frames) }
        journal.closed()
    }

    companion object {
        /** When each step of the exchange was asked, by its sequence number. */
        private val asked = mutableMapOf<String, Pair<String, Long>>()

        /**
         * Tells each step of the exchange with the camera and its answer:
         * how long it took and the transport the camera agreed to, in its
         * own packets or within the connection. In the debug build the
         * exchange is also written whole to the logcat, the camera's own
         * description of its streams among it, without a line that holds
         * the account.
         */
        fun hearTheExchange() {
            MediaLog.setLogger(object : MediaLog.Logger {
                override fun d(tag: String, message: String, throwable: Throwable?) {
                    if (tag != "RtspClient") return MediaLog.Logger.DEFAULT.d(tag, message, throwable)
                    val journal = CameraJournal.current ?: return
                    val lines = message.lineSequence().map(String::trim).filter(String::isNotEmpty).toList()
                    exchanged(journal, lines)
                    if (BuildConfig.TRACES) {
                        lines.filter { line -> SECRET.none { line.lowercase().startsWith(it) } }
                            .filter { line -> line.length < 2 || line[1] != '=' || DESCRIBED.any(line::startsWith) }
                            .forEach { Log.i(NET, journal.hidden("camera says $it")) }
                    }
                }

                override fun i(tag: String, message: String, throwable: Throwable?) = MediaLog.Logger.DEFAULT.i(tag, message, throwable)

                override fun w(tag: String, message: String, throwable: Throwable?) = MediaLog.Logger.DEFAULT.w(tag, message, throwable)

                override fun e(tag: String, message: String, throwable: Throwable?) {
                    if (tag.startsWith("Rtsp")) CameraJournal.current?.failed("rtsp", throwable ?: IOException(message))
                    MediaLog.Logger.DEFAULT.e(tag, message, throwable)
                }
            })
        }

        private fun exchanged(journal: CameraJournal, lines: List<String>) {
            val first = lines.firstOrNull() ?: return
            val sequence = lines.firstOrNull { it.lowercase().startsWith("cseq:") }?.substringAfter(':')?.trim() ?: return
            if (!first.startsWith("RTSP/")) {
                val step = STEPS[first.substringBefore(' ')] ?: "other"
                synchronized(asked) { asked[sequence] = step to SystemClock.elapsedRealtime() }
                journal.tell(NET, "exchange") { put("step", step) }
                return
            }
            val (step, at) = synchronized(asked) { asked.remove(sequence) } ?: ("other" to null)
            val transport = lines.firstOrNull { it.lowercase().startsWith("transport:") }
            journal.tell(NET, "exchange") {
                put("step", step)
                first.split(' ').getOrNull(1)?.toIntOrNull()?.let { put("status", it) }
                at?.let { put("answer_ms", SystemClock.elapsedRealtime() - it) }
                transport?.let { put("transport", if (it.uppercase().contains("/TCP")) "tcp" else "udp") }
            }
        }
    }
}

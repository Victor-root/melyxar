package app.melyxar.player

import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.Choreographer
import androidx.annotation.OptIn
import androidx.media3.common.Format
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.common.util.Log as MediaLog
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DecoderReuseEvaluation
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.source.LoadEventInfo
import androidx.media3.exoplayer.source.MediaLoadData
import app.melyxar.BuildConfig
import app.melyxar.player.CameraLog.DELAY
import app.melyxar.player.CameraLog.FRAMES
import app.melyxar.player.CameraLog.NET
import app.melyxar.player.CameraLog.OPEN
import app.melyxar.player.CameraLog.SCREEN
import java.io.IOException

private const val PULSE_MS = 1_000L

/** A frame of the application later than this is the application held. */
private const val HELD_MS = 100L

/** Lines of the camera's description worth keeping: the streams and their
 *  formats. The rest of it says nothing about a picture. */
private val DESCRIBED = listOf("m=", "a=rtpmap", "a=fmtp", "a=framerate", "a=control", "a=range", "b=")

/** Lines of the exchange with the camera never written: the account is in
 *  them. */
private val SECRET = listOf("authorization:", "proxy-authorization:", "www-authenticate:")

/**
 * The journal of one camera watched live, in the debug build only (see
 * [CameraLog] for its tags). The account written into the address never
 * reaches it: it is taken out of every line, whatever wrote it.
 */
@OptIn(UnstableApi::class)
internal class LiveTrace(private val player: ExoPlayer, address: String, where: String) : AnalyticsListener {
    private val secrets: List<String> = Uri.parse(address).let { uri ->
        listOfNotNull(uri.encodedUserInfo, uri.userInfo)
            .flatMap { listOf(it) + it.split(':') }
            .filter { it.length >= 2 }
            .distinct()
    }
    private val started = SystemClock.elapsedRealtime()
    private val clock = Handler(Looper.getMainLooper())

    private var firstLoad = 0L
    private var firstTracks = 0L
    private var decoderReady = 0L
    private var firstFrame = 0L
    private var frames = 0L

    /** Where the picture and the clock stood at the first frame, to tell
     *  how much time was lost standing still since. */
    private var positionAtFirst = 0L
    private var shownBefore = 0
    private var droppedBefore = 0
    private var queuedBefore = 0
    private var stillFor = 0
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
                if (gap > HELD_MS) say(SCREEN, "the application was held ${gap}ms between two frames of its own")
            }
            lastFrameNanos = frameTimeNanos
            Choreographer.getInstance().postFrameCallback(this)
        }
    }

    init {
        CameraLog.opened(secrets)
        say(OPEN, "open on $where: $address")
        if (BuildConfig.TRACES) {
            clock.postDelayed(pulse, PULSE_MS)
            Choreographer.getInstance().postFrameCallback(held)
        }
    }

    private fun since() = SystemClock.elapsedRealtime() - started

    fun say(tag: String, line: String) = CameraLog.say(tag, "+${since()}ms $line")

    private fun beat() {
        val video = player.videoDecoderCounters
        val sound = player.audioDecoderCounters
        val shown = video?.renderedOutputBufferCount ?: 0
        val dropped = video?.droppedBufferCount ?: 0
        val queued = video?.queuedInputBufferCount ?: 0
        val newlyShown = shown - shownBefore
        val newlyQueued = queued - queuedBefore
        val newlyDropped = dropped - droppedBefore
        shownBefore = shown
        queuedBefore = queued
        droppedBefore = dropped

        val behind = player.totalBufferedDuration
        // Time since the first frame that the picture did not move: what
        // the camera sent meanwhile is still to be shown, or was lost.
        val lost = if (firstFrame > 0) (SystemClock.elapsedRealtime() - firstFrame) - (player.currentPosition - positionAtFirst) else 0
        say(
            DELAY,
            "behind ${behind}ms (held at most 300 before catching up), speed ${player.playbackParameters.speed}, " +
                "position ${player.currentPosition}ms, lost to standing still since the first frame ${lost}ms, " +
                "state ${stateName(player.playbackState)}",
        )
        say(
            FRAMES,
            "received $newlyQueued shown $newlyShown dropped $newlyDropped in the last second " +
                "(in all received $queued shown $shown dropped $dropped skipped ${video?.skippedOutputBufferCount}), " +
                "sound received ${sound?.queuedInputBufferCount} played ${sound?.renderedOutputBufferCount} dropped ${sound?.droppedBufferCount}",
        )
        if (player.playbackState == Player.STATE_READY && player.isPlaying && newlyShown == 0) {
            stillFor += 1
            say(FRAMES, "the picture stood still for ${stillFor}s while playing, $newlyQueued frames received meanwhile")
        } else {
            stillFor = 0
        }
        val runtime = Runtime.getRuntime()
        say(SCREEN, "memory ${(runtime.totalMemory() - runtime.freeMemory()) / 1_048_576}MB of ${runtime.maxMemory() / 1_048_576}MB")
    }

    private fun stateName(state: Int) = when (state) {
        Player.STATE_IDLE -> "IDLE"
        Player.STATE_BUFFERING -> "BUFFERING"
        Player.STATE_READY -> "READY"
        Player.STATE_ENDED -> "ENDED"
        else -> "$state"
    }

    override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) {
        say(OPEN, "state ${stateName(state)} at ${eventTime.eventPlaybackPositionMs}ms, ${eventTime.totalBufferedDurationMs}ms behind")
        when {
            state == Player.STATE_BUFFERING && firstFrame > 0 -> {
                stalledAt = SystemClock.elapsedRealtime()
                say(FRAMES, "waiting for the camera, ${eventTime.totalBufferedDurationMs}ms in hand")
            }
            state == Player.STATE_READY && stalledAt > 0 -> {
                say(FRAMES, "picture moving again after waiting ${SystemClock.elapsedRealtime() - stalledAt}ms")
                stalledAt = 0
            }
        }
    }

    override fun onIsPlayingChanged(eventTime: AnalyticsListener.EventTime, isPlaying: Boolean) {
        say(OPEN, "playing $isPlaying")
    }

    override fun onPlayWhenReadyChanged(eventTime: AnalyticsListener.EventTime, playWhenReady: Boolean, reason: Int) {
        say(OPEN, "playWhenReady $playWhenReady reason $reason")
    }

    override fun onPlayerError(eventTime: AnalyticsListener.EventTime, error: PlaybackException) {
        CameraLog.error("+${since()}ms player error ${error.errorCodeName} at ${eventTime.eventPlaybackPositionMs}ms", error)
    }

    override fun onTracksChanged(eventTime: AnalyticsListener.EventTime, tracks: Tracks) {
        if (firstTracks == 0L && !tracks.isEmpty) {
            firstTracks = since()
        }
        for (group in tracks.groups) {
            for (i in 0 until group.length) {
                val format = group.getTrackFormat(i)
                say(
                    OPEN,
                    "track ${format.sampleMimeType} ${format.codecs} ${format.width}x${format.height} ${format.frameRate}fps " +
                        "${format.sampleRate}Hz ${format.channelCount}ch bitrate ${format.bitrate} " +
                        "supported=${group.isTrackSupported(i)} selected=${group.isTrackSelected(i)}",
                )
            }
        }
    }

    override fun onVideoDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
        decoderReady = since()
        say(OPEN, "video decoder $decoderName ready in ${initializationDurationMs}ms")
    }

    override fun onVideoInputFormatChanged(eventTime: AnalyticsListener.EventTime, format: Format, decoderReuseEvaluation: DecoderReuseEvaluation?) {
        say(OPEN, "video format ${format.sampleMimeType} ${format.codecs} ${format.width}x${format.height} ${format.frameRate}fps")
    }

    override fun onVideoDecoderReleased(eventTime: AnalyticsListener.EventTime, decoderName: String) {
        say(OPEN, "video decoder $decoderName let go")
    }

    override fun onAudioDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
        say(OPEN, "audio decoder $decoderName ready in ${initializationDurationMs}ms")
    }

    override fun onAudioInputFormatChanged(eventTime: AnalyticsListener.EventTime, format: Format, decoderReuseEvaluation: DecoderReuseEvaluation?) {
        say(OPEN, "audio format ${format.sampleMimeType} ${format.codecs} ${format.sampleRate}Hz ${format.channelCount}ch")
    }

    override fun onAudioPositionAdvancing(eventTime: AnalyticsListener.EventTime, playoutStartSystemTimeMs: Long) {
        say(OPEN, "sound playing")
    }

    override fun onAudioUnderrun(eventTime: AnalyticsListener.EventTime, bufferSize: Int, bufferSizeMs: Long, elapsedSinceLastFeedMs: Long) {
        say(FRAMES, "sound ran dry, last fed ${elapsedSinceLastFeedMs}ms ago")
    }

    override fun onAudioSinkError(eventTime: AnalyticsListener.EventTime, audioSinkError: Exception) {
        CameraLog.error("+${since()}ms audio sink error", audioSinkError)
    }

    override fun onAudioCodecError(eventTime: AnalyticsListener.EventTime, audioCodecError: Exception) {
        CameraLog.error("+${since()}ms audio codec error", audioCodecError)
    }

    override fun onVideoCodecError(eventTime: AnalyticsListener.EventTime, videoCodecError: Exception) {
        CameraLog.error("+${since()}ms video codec error", videoCodecError)
    }

    override fun onVideoSizeChanged(eventTime: AnalyticsListener.EventTime, videoSize: VideoSize) {
        say(SCREEN, "picture ${videoSize.width}x${videoSize.height} pixel ratio ${videoSize.pixelWidthHeightRatio}")
    }

    override fun onSurfaceSizeChanged(eventTime: AnalyticsListener.EventTime, width: Int, height: Int) {
        say(SCREEN, "drawn into ${width}x$height")
    }

    override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) {
        if (firstFrame == 0L) {
            firstFrame = SystemClock.elapsedRealtime()
            positionAtFirst = player.currentPosition
            say(
                OPEN,
                "first picture after ${since()}ms: first data at ${firstLoad}ms, streams known at ${firstTracks}ms, " +
                    "decoder ready at ${decoderReady}ms",
            )
        } else {
            say(OPEN, "first picture again, onto a new surface")
        }
    }

    override fun onVideoFrameProcessingOffset(eventTime: AnalyticsListener.EventTime, totalProcessingOffsetUs: Long, frameCount: Int) {
        frames += frameCount
        if (frameCount > 0) {
            say(FRAMES, "$frameCount frames shown, ${totalProcessingOffsetUs / frameCount / 1_000}ms early on average (negative is late)")
        }
    }

    override fun onDroppedVideoFrames(eventTime: AnalyticsListener.EventTime, droppedFrames: Int, elapsedMs: Long) {
        say(FRAMES, "dropped $droppedFrames frames in ${elapsedMs}ms, too late to show")
    }

    override fun onLoadStarted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData, retryCount: Int) {
        if (firstLoad == 0L) firstLoad = since()
        say(NET, "load started type ${mediaLoadData.dataType} track ${mediaLoadData.trackType} retry $retryCount ${loadEventInfo.uri}")
    }

    override fun onLoadCompleted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData) {
        say(NET, "load completed track ${mediaLoadData.trackType} ${loadEventInfo.bytesLoaded} bytes in ${loadEventInfo.loadDurationMs}ms")
    }

    override fun onLoadCanceled(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData) {
        say(NET, "load canceled track ${mediaLoadData.trackType} after ${loadEventInfo.bytesLoaded} bytes")
    }

    override fun onLoadError(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData, error: IOException, wasCanceled: Boolean) {
        CameraLog.error("+${since()}ms load error track ${mediaLoadData.trackType} canceled=$wasCanceled", error)
    }

    override fun onPlayerReleased(eventTime: AnalyticsListener.EventTime) {
        clock.removeCallbacks(pulse)
        Choreographer.getInstance().removeFrameCallback(held)
        say(OPEN, "let go after ${since()}ms and $frames frames")
        CameraLog.closed(secrets)
    }

    companion object {
        /**
         * Writes the exchange with the camera, its own description of its
         * streams among it, without a line that holds the account. The
         * transport the camera agreed to (over UDP or within the TCP
         * connection) is in it, and so how long each answer took.
         */
        fun hearTheExchange() {
            if (!BuildConfig.TRACES) return
            MediaLog.setLogger(object : MediaLog.Logger {
                override fun d(tag: String, message: String, throwable: Throwable?) {
                    if (tag != "RtspClient") return MediaLog.Logger.DEFAULT.d(tag, message, throwable)
                    message.lineSequence()
                        .filter { line -> line.isNotBlank() && SECRET.none { line.lowercase().startsWith(it) } }
                        .filter { line -> line.length < 2 || line[1] != '=' || DESCRIBED.any(line::startsWith) }
                        .forEach { CameraLog.say(OPEN, "camera says $it") }
                }

                override fun i(tag: String, message: String, throwable: Throwable?) = MediaLog.Logger.DEFAULT.i(tag, message, throwable)

                override fun w(tag: String, message: String, throwable: Throwable?) {
                    if (tag.startsWith("Rtsp")) CameraLog.say(NET, "$tag warns: $message ${CameraLog.causes(throwable)}")
                    MediaLog.Logger.DEFAULT.w(tag, message, throwable)
                }

                override fun e(tag: String, message: String, throwable: Throwable?) {
                    if (tag.startsWith("Rtsp")) CameraLog.error("$tag: $message", throwable)
                    MediaLog.Logger.DEFAULT.e(tag, message, throwable)
                }
            })
        }
    }
}

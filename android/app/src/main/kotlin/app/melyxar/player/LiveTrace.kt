package app.melyxar.player

import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
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
import java.io.IOException

private const val TAG = "MelyxarLive"
private const val PULSE_MS = 2_000L

/**
 * Temporary trace of a live camera, kept in release to find why a picture
 * freezes. The account written into the address never reaches the log: it
 * is taken out of every line, whatever wrote it.
 */
@OptIn(UnstableApi::class)
internal class LiveTrace(private val player: ExoPlayer, address: String) : AnalyticsListener {
    private val secrets: List<String> = Uri.parse(address).let { uri ->
        listOfNotNull(uri.encodedUserInfo, uri.userInfo)
            .flatMap { listOf(it) + it.split(':') }
            .filter { it.length >= 2 }
            .distinct()
            .sortedByDescending { it.length }
    }
    private val started = System.currentTimeMillis()
    private var frames = 0L
    private val clock = Handler(Looper.getMainLooper())
    private val pulse = object : Runnable {
        override fun run() {
            val counters = player.videoDecoderCounters
            val sound = player.audioDecoderCounters
            say(
                "pulse position ${player.currentPosition}ms buffered ${player.totalBufferedDuration}ms " +
                    "rendered ${counters?.renderedOutputBufferCount} dropped ${counters?.droppedBufferCount} " +
                    "skipped ${counters?.skippedOutputBufferCount} queued ${counters?.queuedInputBufferCount} " +
                    "sound queued ${sound?.queuedInputBufferCount} played ${sound?.renderedOutputBufferCount} " +
                    "skipped ${sound?.skippedInputBufferCount}/${sound?.skippedOutputBufferCount} dropped ${sound?.droppedBufferCount}",
            )
            clock.postDelayed(this, PULSE_MS)
        }
    }

    init {
        say("open $address")
        clock.postDelayed(pulse, PULSE_MS)
    }

    private fun hidden(text: String?): String {
        var out = text ?: return "null"
        out = out.replace(Regex("(rtsps?://)[^/@\\s]*@"), "$1***@")
        for (secret in secrets) out = out.replace(secret, "***")
        return out
    }

    fun say(line: String) {
        Log.i(TAG, "+${System.currentTimeMillis() - started}ms ${hidden(line)}")
    }

    private fun causes(error: Throwable?): String =
        generateSequence(error) { it.cause }.take(6)
            .joinToString(" <- ") { "${it.javaClass.simpleName}: ${it.message}" }

    override fun onPlaybackStateChanged(eventTime: AnalyticsListener.EventTime, state: Int) {
        val name = when (state) {
            Player.STATE_IDLE -> "IDLE"
            Player.STATE_BUFFERING -> "BUFFERING"
            Player.STATE_READY -> "READY"
            Player.STATE_ENDED -> "ENDED"
            else -> "$state"
        }
        say("state $name at ${eventTime.eventPlaybackPositionMs}ms buffered ${eventTime.totalBufferedDurationMs}ms")
    }

    override fun onIsPlayingChanged(eventTime: AnalyticsListener.EventTime, isPlaying: Boolean) {
        say("playing $isPlaying")
    }

    override fun onPlayWhenReadyChanged(eventTime: AnalyticsListener.EventTime, playWhenReady: Boolean, reason: Int) {
        say("playWhenReady $playWhenReady reason $reason")
    }

    override fun onPlayerError(eventTime: AnalyticsListener.EventTime, error: PlaybackException) {
        say("error ${error.errorCodeName} ${causes(error)}")
    }

    override fun onTracksChanged(eventTime: AnalyticsListener.EventTime, tracks: Tracks) {
        for (group in tracks.groups) {
            for (i in 0 until group.length) {
                val format = group.getTrackFormat(i)
                say("track ${format.sampleMimeType} ${format.codecs} ${format.width}x${format.height} ${format.frameRate}fps supported=${group.isTrackSupported(i)} selected=${group.isTrackSelected(i)}")
            }
        }
    }

    override fun onVideoDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
        say("video decoder $decoderName in ${initializationDurationMs}ms")
    }

    override fun onVideoInputFormatChanged(eventTime: AnalyticsListener.EventTime, format: Format, decoderReuseEvaluation: DecoderReuseEvaluation?) {
        say("video format ${format.sampleMimeType} ${format.width}x${format.height}")
    }

    override fun onAudioDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
        say("audio decoder $decoderName")
    }

    override fun onAudioInputFormatChanged(eventTime: AnalyticsListener.EventTime, format: Format, decoderReuseEvaluation: DecoderReuseEvaluation?) {
        say("audio format ${format.sampleMimeType} ${format.codecs} ${format.sampleRate}Hz ${format.channelCount}ch")
    }

    override fun onAudioPositionAdvancing(eventTime: AnalyticsListener.EventTime, playoutStartSystemTimeMs: Long) {
        say("sound playing")
    }

    override fun onAudioUnderrun(eventTime: AnalyticsListener.EventTime, bufferSize: Int, bufferSizeMs: Long, elapsedSinceLastFeedMs: Long) {
        say("sound underrun, last fed ${elapsedSinceLastFeedMs}ms ago")
    }

    override fun onAudioSinkError(eventTime: AnalyticsListener.EventTime, audioSinkError: Exception) {
        say("audio sink error ${causes(audioSinkError)}")
    }

    override fun onVideoCodecError(eventTime: AnalyticsListener.EventTime, videoCodecError: Exception) {
        say("video codec error ${causes(videoCodecError)}")
    }

    override fun onVideoSizeChanged(eventTime: AnalyticsListener.EventTime, videoSize: VideoSize) {
        say("video size ${videoSize.width}x${videoSize.height}")
    }

    override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) {
        say("first frame shown")
    }

    override fun onVideoFrameProcessingOffset(eventTime: AnalyticsListener.EventTime, totalProcessingOffsetUs: Long, frameCount: Int) {
        frames += frameCount
        say("frames shown so far $frames")
    }

    override fun onDroppedVideoFrames(eventTime: AnalyticsListener.EventTime, droppedFrames: Int, elapsedMs: Long) {
        say("dropped $droppedFrames frames in ${elapsedMs}ms")
    }

    override fun onLoadStarted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData, retryCount: Int) {
        say("load started type ${mediaLoadData.dataType} track ${mediaLoadData.trackType}")
    }

    override fun onLoadCompleted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData) {
        say("load completed type ${mediaLoadData.dataType} track ${mediaLoadData.trackType} ${loadEventInfo.bytesLoaded} bytes")
    }

    override fun onLoadCanceled(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData) {
        say("load canceled track ${mediaLoadData.trackType}")
    }

    override fun onLoadError(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData, error: IOException, wasCanceled: Boolean) {
        say("load error track ${mediaLoadData.trackType} canceled=$wasCanceled ${causes(error)}")
    }

    companion object {
        /**
         * Lets through, from the camera's own description of its streams,
         * only the lines naming each stream and its format: nothing else of
         * what is said with the camera reaches the log, the account least of
         * all. Every other message of the player is logged as before.
         */
        fun hearDescriptions() {
            MediaLog.setLogger(object : MediaLog.Logger {
                override fun d(tag: String, message: String, throwable: Throwable?) {
                    if (tag != "RtspClient") return MediaLog.Logger.DEFAULT.d(tag, message, throwable)
                    message.lineSequence()
                        .filter { it.startsWith("m=") || it.startsWith("a=rtpmap") || it.startsWith("a=fmtp") }
                        .forEach { Log.i(TAG, "described $it") }
                }

                override fun i(tag: String, message: String, throwable: Throwable?) = MediaLog.Logger.DEFAULT.i(tag, message, throwable)

                override fun w(tag: String, message: String, throwable: Throwable?) = MediaLog.Logger.DEFAULT.w(tag, message, throwable)

                override fun e(tag: String, message: String, throwable: Throwable?) = MediaLog.Logger.DEFAULT.e(tag, message, throwable)
            })
        }
    }

    override fun onPlayerReleased(eventTime: AnalyticsListener.EventTime) {
        clock.removeCallbacks(pulse)
        say("released after $frames frames")
    }
}

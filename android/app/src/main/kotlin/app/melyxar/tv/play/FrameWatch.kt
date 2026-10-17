package app.melyxar.tv.play

import android.content.Context
import android.hardware.display.DisplayManager
import android.media.MediaFormat
import android.util.Log
import android.view.Choreographer
import android.view.Display
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DecoderReuseEvaluation
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.source.LoadEventInfo
import androidx.media3.exoplayer.source.MediaLoadData
import androidx.media3.exoplayer.video.VideoFrameMetadataListener
import java.io.IOException
import java.util.Locale
import java.util.concurrent.atomic.AtomicLong
import kotlin.math.abs
import kotlin.math.roundToLong
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/** The tag the film's pictures are followed under in a debug build. */
const val FRAMES_LOG = "MelyxarFrames"

/** How often what the decoder counted is summed up. */
private const val SUMMED_EVERY_MS = 10_000L

/** How long the thread that draws the screen may go without drawing before
 *  it is said to have been held up: six frames of a screen at sixty. */
private const val HELD_UP_MS = 100L

/** More than the screen's beat, at the slowest screen it can run: the clock
 *  the pictures follow moving by this much between two of them jumped. */
private const val CLOCK_JUMP_NS = 25_000_000L

/** Two pictures further apart than this many steps are a jump, a seek, not
 *  pictures lost. */
private const val JUMP_STEPS = 50

/**
 * For the debug build only: every picture of the film that never reaches the
 * screen, where in the film it was lost and what surrounded it. Each picture
 * handed to the screen is compared with the one before it, so a picture
 * missing between them is named by its time even when the decoder only
 * counts it; each piece of the film is logged as it arrives, with how long
 * it took and how far ahead the player then stood, so a loss can be laid
 * against the joins between pieces; the decoder kept or reset at each new
 * piece, the screen's own thread held up, and every ten seconds the
 * decoder's counts, the tightest margin a picture was handed over with, and
 * what the screen is running at. Nothing here changes how the film plays.
 */
@OptIn(UnstableApi::class)
class FrameWatch(
    private val context: Context,
    private val player: ExoPlayer,
    private val scope: CoroutineScope,
) : VideoFrameMetadataListener, AnalyticsListener, Choreographer.FrameCallback {
    // Read and written on the player's own thread only, where the player
    // itself must not be asked anything.
    private var lastShownUs = C.TIME_UNSET
    private var lastReleaseNs = 0L
    private var stepUs = Long.MAX_VALUE

    // Written on the player's thread, read on the screen's.
    private val tightestNs = AtomicLong(Long.MAX_VALUE)
    private val lostSinceSum = AtomicLong(0)
    private val lateSinceSum = AtomicLong(0)

    private var lastDrawnNs = 0L
    private var watching = false
    private var summing: Job? = null
    private var summed = Counts()
    private var summedSoundSkipped = 0

    fun start() {
        watching = true
        player.setVideoFrameMetadataListener(this)
        player.addAnalyticsListener(this)
        Choreographer.getInstance().postFrameCallback(this)
        summing = scope.launch {
            while (isActive) {
                delay(SUMMED_EVERY_MS)
                sum()
            }
        }
        Log.d(FRAMES_LOG, "watching: ${screen()}")
    }

    fun stop() {
        watching = false
        summing?.cancel()
        Choreographer.getInstance().removeFrameCallback(this)
        player.clearVideoFrameMetadataListener(this)
        player.removeAnalyticsListener(this)
    }

    override fun onVideoFrameAboutToBeRendered(presentationTimeUs: Long, releaseTimeNs: Long, format: Format, mediaFormat: MediaFormat?) {
        val aheadNs = releaseTimeNs - System.nanoTime()
        tightestNs.accumulateAndGet(aheadNs) { a, b -> minOf(a, b) }
        if (aheadNs < 0) {
            lateSinceSum.incrementAndGet()
            Log.d(FRAMES_LOG, "picture at ${seconds(presentationTimeUs)} s handed over ${millis(-aheadNs)} ms late")
        }
        val last = lastShownUs
        val lastRelease = lastReleaseNs
        lastShownUs = presentationTimeUs
        lastReleaseNs = releaseTimeNs
        if (last == C.TIME_UNSET) {
            return
        }
        val gapUs = presentationTimeUs - last
        // How far the clock the pictures follow moved against the film
        // between the two: nothing but the rounding to the screen's beat
        // when it runs evenly, a whole picture or more when it jumped.
        val shiftNs = releaseTimeNs - lastRelease - gapUs * 1000
        // The film's own step between pictures, as the smallest gap seen:
        // the file's frame rate is not always carried to the player.
        if (gapUs in 1_000 until stepUs) {
            stepUs = gapUs
        }
        if (stepUs == Long.MAX_VALUE) {
            return
        }
        val missing = (gapUs.toDouble() / stepUs).roundToLong() - 1
        when {
            gapUs <= 0 || missing > JUMP_STEPS -> Log.d(FRAMES_LOG, "jumped from ${seconds(last)} s to ${seconds(presentationTimeUs)} s")
            abs(shiftNs) > CLOCK_JUMP_NS -> {
                lostSinceSum.addAndGet(missing)
                Log.d(
                    FRAMES_LOG,
                    "the clock moved ${millis(-shiftNs)} ms ahead of the film between ${seconds(last)} s and ${seconds(presentationTimeUs)} s, " +
                        "$missing picture(s) lost, this one handed over ${millis(aheadNs)} ms ahead",
                )
            }
            missing > 0 -> {
                lostSinceSum.addAndGet(missing)
                Log.d(
                    FRAMES_LOG,
                    "$missing picture(s) lost between ${seconds(last)} s and ${seconds(presentationTimeUs)} s, " +
                        "this one handed over ${millis(aheadNs)} ms ahead",
                )
            }
        }
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!watching) {
            return
        }
        val since = (frameTimeNanos - lastDrawnNs) / 1_000_000
        if (lastDrawnNs != 0L && since > HELD_UP_MS && player.isPlaying) {
            Log.d(FRAMES_LOG, "the screen's thread was held up $since ms at ${seconds(player.currentPosition * 1000)} s")
        }
        lastDrawnNs = frameTimeNanos
        Choreographer.getInstance().postFrameCallback(this)
    }

    override fun onLoadCompleted(eventTime: AnalyticsListener.EventTime, loadEventInfo: LoadEventInfo, mediaLoadData: MediaLoadData) {
        if (mediaLoadData.dataType != C.DATA_TYPE_MEDIA) {
            return
        }
        val megabits = loadEventInfo.bytesLoaded * 8 / 1_000_000.0
        val rate = if (loadEventInfo.loadDurationMs > 0) megabits * 1000 / loadEventInfo.loadDurationMs else 0.0
        Log.d(
            FRAMES_LOG,
            "piece ${loadEventInfo.dataSpec.uri.lastPathSegment} (${span(mediaLoadData)}) arrived: " +
                "${String.format(Locale.ROOT, "%.1f", loadEventInfo.bytesLoaded / 1_000_000.0)} MB in ${loadEventInfo.loadDurationMs} ms " +
                "(${String.format(Locale.ROOT, "%.0f", rate)} Mbit/s), ${heldAhead()} s held ahead",
        )
    }

    override fun onLoadError(
        eventTime: AnalyticsListener.EventTime,
        loadEventInfo: LoadEventInfo,
        mediaLoadData: MediaLoadData,
        error: IOException,
        wasCanceled: Boolean,
    ) {
        Log.d(FRAMES_LOG, "piece ${loadEventInfo.dataSpec.uri.lastPathSegment} (${span(mediaLoadData)}) failed: $error")
    }

    override fun onVideoInputFormatChanged(eventTime: AnalyticsListener.EventTime, format: Format, decoderReuseEvaluation: DecoderReuseEvaluation?) {
        val decoder = decoderReuseEvaluation?.let {
            when (it.result) {
                DecoderReuseEvaluation.REUSE_RESULT_NO -> "decoder replaced (reasons ${it.discardReasons})"
                DecoderReuseEvaluation.REUSE_RESULT_YES_WITH_FLUSH -> "decoder flushed"
                DecoderReuseEvaluation.REUSE_RESULT_YES_WITH_RECONFIGURATION -> "decoder reconfigured"
                DecoderReuseEvaluation.REUSE_RESULT_YES_WITHOUT_RECONFIGURATION -> "decoder kept as it is"
                else -> "decoder ${it.result}"
            }
        } ?: "new decoder"
        Log.d(FRAMES_LOG, "picture format at ${seconds(player.currentPosition * 1000)} s: ${format.width}x${format.height} ${format.codecs} at ${format.frameRate} fps, $decoder")
    }

    override fun onAudioUnderrun(eventTime: AnalyticsListener.EventTime, bufferSize: Int, bufferSizeMs: Long, elapsedSinceLastFeedMs: Long) {
        Log.d(FRAMES_LOG, "sound ran dry at ${seconds(eventTime.currentPlaybackPositionMs * 1000)} s, $elapsedSinceLastFeedMs ms since it was last fed")
    }

    override fun onAudioSinkError(eventTime: AnalyticsListener.EventTime, audioSinkError: Exception) {
        Log.d(FRAMES_LOG, "sound trouble at ${seconds(eventTime.currentPlaybackPositionMs * 1000)} s: $audioSinkError")
    }

    override fun onPositionDiscontinuity(
        eventTime: AnalyticsListener.EventTime,
        oldPosition: Player.PositionInfo,
        newPosition: Player.PositionInfo,
        reason: Int,
    ) {
        Log.d(FRAMES_LOG, "position moved from ${seconds(oldPosition.positionMs * 1000)} s to ${seconds(newPosition.positionMs * 1000)} s, reason $reason")
    }

    private fun sum() {
        val counters = player.videoDecoderCounters ?: return
        counters.ensureUpdated()
        val now = Counts(
            shown = counters.renderedOutputBufferCount,
            dropped = counters.droppedBufferCount,
            skipped = counters.skippedOutputBufferCount,
            droppedInput = counters.droppedInputBufferCount,
            droppedToKey = counters.droppedToKeyframeCount,
        )
        val sound = player.audioDecoderCounters?.also { it.ensureUpdated() }
        val soundSkipped = sound?.skippedOutputBufferCount ?: 0
        val tightest = tightestNs.getAndSet(Long.MAX_VALUE)
        Log.d(
            FRAMES_LOG,
            "at ${seconds(player.currentPosition * 1000)} s, last ${SUMMED_EVERY_MS / 1000} s: " +
                "${now.shown - summed.shown} shown, ${now.dropped - summed.dropped} dropped, ${now.skipped - summed.skipped} skipped, " +
                "${now.droppedInput - summed.droppedInput} dropped before decoding, ${now.droppedToKey - summed.droppedToKey} times dropped to a key picture, " +
                "${lostSinceSum.getAndSet(0)} lost between pictures, ${lateSinceSum.getAndSet(0)} handed over late, " +
                "tightest ${if (tightest == Long.MAX_VALUE) "none" else "${millis(tightest)} ms"} ahead, " +
                "most dropped in a row ${counters.maxConsecutiveDroppedBufferCount}, ${heldAhead()} s held ahead, " +
                "${soundSkipped - summedSoundSkipped} pieces of sound skipped, " +
                "speed ${player.playbackParameters.speed}, ${screen()}",
        )
        summed = now
        summedSoundSkipped = soundSkipped
    }

    private fun heldAhead() = String.format(Locale.ROOT, "%.1f", (player.bufferedPosition - player.currentPosition) / 1000.0)

    /** What the screen runs at and what it could run at. */
    private fun screen(): String {
        val display = context.getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY) ?: return "no screen"
        val mode = display.mode
        val modes = display.supportedModes.joinToString { "${it.physicalWidth}x${it.physicalHeight}@${it.refreshRate}" }
        return "screen ${mode.physicalWidth}x${mode.physicalHeight}@${mode.refreshRate} (offers $modes)"
    }

    private fun span(data: MediaLoadData) =
        if (data.mediaStartTimeMs == C.TIME_UNSET) "no time" else "${seconds(data.mediaStartTimeMs * 1000)}-${seconds(data.mediaEndTimeMs * 1000)} s"

    private class Counts(val shown: Int = 0, val dropped: Int = 0, val skipped: Int = 0, val droppedInput: Int = 0, val droppedToKey: Int = 0)
}

private fun seconds(us: Long) = String.format(Locale.ROOT, "%.3f", us / 1_000_000.0)

private fun millis(ns: Long) = String.format(Locale.ROOT, "%.1f", ns / 1_000_000.0)

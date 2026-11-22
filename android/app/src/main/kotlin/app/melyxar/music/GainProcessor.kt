package app.melyxar.music

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * The level of one deck, on the sound itself: what brings every song to the
 * same loudness, above its own when it was made quiet, which a player's
 * volume cannot, and what fades one song into the next.
 *
 * Asked from the main thread and applied on the thread that plays, a frame
 * at a time, so a change is a slope rather than a click. How long that takes
 * is kept for the journal ([taken]): done in time, it is the sound the card
 * plays; done late, it is a silence.
 */
@OptIn(UnstableApi::class)
class GainProcessor : BaseAudioProcessor() {
    /** What was asked last, taken up by the thread that plays. */
    @Volatile
    private var asked: Pair<Float, Double>? = null
    private val ramp = Ramp()

    /** To `level` over `seconds`, or at once. */
    fun aim(level: Float, seconds: Double) {
        asked = level to seconds
    }

    /** The time spent on the thread that plays, in milliseconds: the
     *  longest piece of sound, and all of them together. */
    data class Spent(val longestMs: Double, val busyMs: Double) {
        operator fun plus(other: Spent) = Spent(maxOf(longestMs, other.longestMs), busyMs + other.busyMs)
    }

    @Volatile
    private var longestNs = 0L

    @Volatile
    private var busyNs = 0L

    /** The time spent since the last time it was asked. */
    fun taken(): Spent {
        val spent = Spent(longestNs / 1e6, busyNs / 1e6)
        longestNs = 0
        busyNs = 0
        return spent
    }

    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT && inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT) {
            throw AudioProcessor.UnhandledAudioFormatException(inputAudioFormat)
        }
        return inputAudioFormat
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val started = System.nanoTime()
        level(inputBuffer)
        val spent = System.nanoTime() - started
        busyNs += spent
        if (spent > longestNs) {
            longestNs = spent
        }
    }

    private fun level(inputBuffer: ByteBuffer) {
        asked?.let { (level, seconds) ->
            asked = null
            ramp.aim(level, (seconds * inputAudioFormat.sampleRate).toInt())
        }
        val output = replaceOutputBuffer(inputBuffer.remaining())
        val input = inputBuffer.order(ByteOrder.nativeOrder())
        val channels = inputAudioFormat.channelCount
        if (inputAudioFormat.encoding == C.ENCODING_PCM_16BIT) {
            while (input.remaining() >= 2 * channels) {
                val factor = ramp.next()
                repeat(channels) {
                    val sample = (input.short * factor).toInt().coerceIn(Short.MIN_VALUE.toInt(), Short.MAX_VALUE.toInt())
                    output.putShort(sample.toShort())
                }
            }
        } else {
            while (input.remaining() >= 4 * channels) {
                val factor = ramp.next()
                repeat(channels) { output.putFloat(input.float * factor) }
            }
        }
        output.flip()
    }
}

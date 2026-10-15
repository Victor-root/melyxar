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
 * at a time, so a change is a slope rather than a click.
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

    override fun onConfigure(inputAudioFormat: AudioProcessor.AudioFormat): AudioProcessor.AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT && inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT) {
            throw AudioProcessor.UnhandledAudioFormatException(inputAudioFormat)
        }
        return inputAudioFormat
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
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

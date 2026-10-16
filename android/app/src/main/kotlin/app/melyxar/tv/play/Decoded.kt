package app.melyxar.tv.play

import android.content.Context
import android.os.Handler
import androidx.annotation.OptIn
import androidx.media3.common.Format
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.audio.AudioRendererEventListener
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.MediaCodecAudioRenderer
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.mediacodec.MediaCodecUtil

/**
 * The film's renderers, its sound decoded on the television whenever it has
 * a decoder for it, rather than handed as it is to the receiver behind it:
 * handed on, the television's own volume no longer reaches it, and the
 * remote can only change it through the receiver. Only a sound the
 * television cannot decode is handed on.
 */
@OptIn(UnstableApi::class)
class DecodedSound(context: Context) : DefaultRenderersFactory(context) {
    init {
        setEnableDecoderFallback(true)
    }

    override fun buildAudioRenderers(
        context: Context,
        extensionRendererMode: Int,
        mediaCodecSelector: MediaCodecSelector,
        enableDecoderFallback: Boolean,
        audioSink: AudioSink,
        eventHandler: Handler,
        eventListener: AudioRendererEventListener,
        out: ArrayList<Renderer>,
    ) {
        out += object : MediaCodecAudioRenderer(context, mediaCodecSelector, enableDecoderFallback, eventHandler, eventListener, audioSink) {
            override fun shouldUseBypass(format: Format): Boolean =
                !decodes(mediaCodecSelector, format) && super.shouldUseBypass(format)
        }
    }

    private fun decodes(selector: MediaCodecSelector, format: Format): Boolean {
        val mime = format.sampleMimeType ?: return false
        return try {
            selector.getDecoderInfos(mime, false, false).isNotEmpty()
        } catch (_: MediaCodecUtil.DecoderQueryException) {
            false
        }
    }
}

package app.melyxar.tv.music

import android.graphics.BlurMaskFilter
import android.graphics.PorterDuff
import androidx.compose.foundation.layout.Spacer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.LinearGradientShader
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.unit.IntSize
import app.melyxar.server.Refusal
import app.melyxar.server.SongSpectrum
import app.melyxar.server.spectrum
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import java.io.IOException
import kotlin.math.roundToInt

/** The least time between two drawings of the wave: it only dresses the
 *  player, so thirty a second is plenty. */
private const val FRAME_EVERY_NS = 33_000_000L

/** How much smaller the wave is drawn than it is shown, and how much it is
 *  softened at that size, in its own pixels. */
private const val SHRUNK = 8f
private const val SOFTENED_PX = 1.5f

/** Where the song was when the player last said, and when that was. */
private class Said {
    var position = 0.0
    var at = 0L
}

/**
 * The wave behind the player, as the web's: the levels the server kept for
 * [song], read where the song has got to and drawn as one smooth wave rising
 * from the foot, in the accent, fading towards the top. Nothing runs while
 * the song is paused once the wave has settled, nor when the song has no
 * levels; drawn only when the account wants it.
 */
@Composable
fun SongWave(song: String, modifier: Modifier) {
    val listening = LocalListening.current
    if (!listening.preferences.spectrum) {
        return
    }
    val api = LocalServer.current
    val accent = LocalPalette.current.accent
    val amplitude = listening.preferences.spectrumAmplitude / 100f
    val spectrum by produceState<SongSpectrum?>(null, song) {
        value = null
        value = try {
            api.spectrum(song)
        } catch (_: IOException) {
            null
        } catch (_: Refusal) {
            null
        }
    }
    // The levels the effect reads are the ones this composition saw: the
    // state itself may already hold the next song's, of another size.
    val reading = spectrum
    val wave = remember { WaveLevels() }
    var drawn by remember { mutableIntStateOf(0) }
    val said = remember { Said() }
    LaunchedEffect(listening) {
        snapshotFlow { listening.position }.collect {
            said.position = it
            said.at = System.nanoTime()
        }
    }
    LaunchedEffect(reading, listening.playing) {
        val heard = reading?.takeIf { listening.playing }
        heard?.let { wave.fit(it.bands) }
        var last = 0L
        var going = true
        while (going) {
            going = withFrameNanos { now ->
                if (now - last < FRAME_EVERY_NS) {
                    return@withFrameNanos true
                }
                last = now
                if (heard != null) {
                    levelsAt(heard, momentOf(said.position, (now - said.at) / 1_000_000), wave.targets)
                } else {
                    wave.targets.fill(0f)
                }
                val settled = approach(wave.levels, wave.targets)
                drawn += 1
                heard != null || !settled
            }
        }
    }
    val path = remember { Path() }
    Spacer(
        modifier.drawWithCache {
            // Drawn small and laid large, which softens it as the web's
            // blur does, for next to nothing.
            val small = ImageBitmap(
                (size.width / SHRUNK).roundToInt().coerceAtLeast(1),
                (size.height / SHRUNK).roundToInt().coerceAtLeast(1),
            )
            val canvas = Canvas(small)
            val wide = small.width.toFloat()
            val high = small.height.toFloat()
            val paint = Paint().apply {
                shader = LinearGradientShader(
                    from = Offset(0f, 0f),
                    to = Offset(0f, high),
                    colors = listOf(Color.Transparent, accent.copy(alpha = 0.6f), accent.copy(alpha = 0.6f)),
                    colorStops = listOf(0f, 0.55f, 1f),
                )
                asFrameworkPaint().maskFilter = BlurMaskFilter(SOFTENED_PX, BlurMaskFilter.Blur.NORMAL)
            }
            val whole = IntSize(size.width.roundToInt(), size.height.roundToInt())
            onDrawBehind {
                // Read here, so each step draws the wave again and does
                // nothing more; nothing to draw before the first.
                val levels = wave.levels
                if (drawn == 0 || levels.size < 2) {
                    return@onDrawBehind
                }
                val step = wide / (levels.size - 1)
                fun top(band: Int) = high - levels[band] * amplitude * high
                path.reset()
                path.moveTo(0f, high)
                path.lineTo(0f, top(0))
                for (band in 0 until levels.size - 1) {
                    path.quadraticTo(band * step, top(band), (band + 0.5f) * step, (top(band) + top(band + 1)) / 2)
                }
                path.lineTo(wide, top(levels.size - 1))
                path.lineTo(wide, high)
                path.close()
                canvas.nativeCanvas.drawColor(0, PorterDuff.Mode.CLEAR)
                canvas.drawPath(path, paint)
                drawImage(small, dstSize = whole, filterQuality = FilterQuality.Low)
            }
        },
    )
}

/** The levels of the wave as drawn, and those they glide to; sized again
 *  for a song read in another number of bands, the drawn ones kept where
 *  they fit so the wave glides rather than drops. */
private class WaveLevels {
    var levels = FloatArray(0)
        private set
    var targets = FloatArray(0)
        private set

    fun fit(bands: Int) {
        if (levels.size != bands) {
            levels = levels.copyOf(bands)
            targets = FloatArray(bands)
        }
    }
}

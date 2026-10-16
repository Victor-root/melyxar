package app.melyxar.tv.music

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
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import app.melyxar.server.Refusal
import app.melyxar.server.SongSpectrum
import app.melyxar.server.spectrum
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import java.io.IOException

/** The least time between two drawings of the wave: it only dresses the
 *  player, so thirty a second is plenty. */
private const val FRAME_EVERY_NS = 33_000_000L

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
    val bands = spectrum?.bands ?: 0
    val levels = remember(bands) { FloatArray(bands) }
    var drawn by remember { mutableIntStateOf(0) }
    val said = remember { Said() }
    LaunchedEffect(listening) {
        snapshotFlow { listening.position }.collect {
            said.position = it
            said.at = System.nanoTime()
        }
    }
    LaunchedEffect(spectrum, levels, listening.playing) {
        val wave = spectrum
        val targets = FloatArray(levels.size)
        val reading = listening.playing && wave != null
        var last = 0L
        var going = true
        while (going) {
            going = withFrameNanos { now ->
                if (now - last < FRAME_EVERY_NS) {
                    return@withFrameNanos true
                }
                last = now
                if (wave != null && reading) {
                    levelsAt(wave, momentOf(said.position, (now - said.at) / 1_000_000), targets)
                } else {
                    targets.fill(0f)
                }
                val settled = approach(levels, targets)
                drawn += 1
                reading || !settled
            }
        }
    }
    val path = remember { Path() }
    Spacer(
        modifier.drawWithCache {
            val fill = Brush.verticalGradient(
                0f to Color.Transparent,
                0.55f to accent.copy(alpha = 0.6f),
                1f to accent.copy(alpha = 0.6f),
            )
            onDrawBehind {
                // Read here, so each step draws the wave again and does
                // nothing more; nothing to draw before the first.
                if (drawn == 0 || levels.size < 2) {
                    return@onDrawBehind
                }
                val step = size.width / (levels.size - 1)
                fun top(band: Int) = size.height - levels[band] * amplitude * size.height
                path.reset()
                path.moveTo(0f, size.height)
                path.lineTo(0f, top(0))
                for (band in 0 until levels.size - 1) {
                    path.quadraticTo(band * step, top(band), (band + 0.5f) * step, (top(band) + top(band + 1)) / 2)
                }
                path.lineTo(size.width, top(levels.size - 1))
                path.lineTo(size.width, size.height)
                path.close()
                drawPath(path, fill)
            }
        },
    )
}

package app.melyxar.tv.music

import android.graphics.BlurMaskFilter
import android.graphics.PorterDuff
import androidx.compose.foundation.layout.Spacer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
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
import androidx.compose.ui.graphics.nativePaint
import androidx.compose.ui.unit.IntSize
import app.melyxar.tv.look.LocalPalette
import kotlin.math.roundToInt

/** How much smaller the wave is drawn than it is shown, and how much it is
 *  softened at that size, in its own pixels. */
private const val SHRUNK = 8f
private const val SOFTENED_PX = 1.5f

/**
 * The wave behind the player, as the web's: the levels of the song playing
 * (SongLevels) drawn as one smooth wave rising from the foot, in the accent,
 * fading towards the top; drawn only when the account wants it.
 */
@Composable
fun SongWave(modifier: Modifier) {
    val listening = LocalListening.current
    if (!listening.preferences.spectrum) {
        return
    }
    val levels = LocalSongLevels.current
    levels.Watched()
    val accent = LocalPalette.current.accent
    val amplitude = listening.preferences.spectrumAmplitude / 100f
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
                nativePaint.maskFilter = BlurMaskFilter(SOFTENED_PX, BlurMaskFilter.Blur.NORMAL)
            }
            val whole = IntSize(size.width.roundToInt(), size.height.roundToInt())
            onDrawBehind {
                // Read here, so each step draws the wave again and does
                // nothing more; nothing to draw before the first.
                val heights = levels.levels
                if (levels.drawn == 0 || heights.size < 2) {
                    return@onDrawBehind
                }
                val step = wide / (heights.size - 1)
                fun top(band: Int) = high - heights[band] * amplitude * high
                path.reset()
                path.moveTo(0f, high)
                path.lineTo(0f, top(0))
                for (band in 0 until heights.size - 1) {
                    path.quadraticTo(band * step, top(band), (band + 0.5f) * step, (top(band) + top(band + 1)) / 2)
                }
                path.lineTo(wide, top(heights.size - 1))
                path.lineTo(wide, high)
                path.close()
                canvas.nativeCanvas.drawColor(0, PorterDuff.Mode.CLEAR)
                canvas.drawPath(path, paint)
                drawImage(small, dstSize = whole, filterQuality = FilterQuality.Low)
            }
        },
    )
}

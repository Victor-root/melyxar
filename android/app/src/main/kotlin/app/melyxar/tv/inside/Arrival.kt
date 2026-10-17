package app.melyxar.tv.inside

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.unit.dp
import app.melyxar.tv.door.ServerMark
import app.melyxar.tv.look.LocalPalette

/** How wide the ring is, how thick, and how long it takes to turn once. */
private val RING = 108.dp
private val RING_LINE = 3.dp
private const val TURN_MS = 900

/**
 * What the home page shows while it comes the first time, as the web's: the
 * server's mark, and a ring in the accent turning round it.
 */
@Composable
fun ArrivalLoader(modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val turning = rememberInfiniteTransition(label = "arrival")
    val turn = turning.animateFloat(0f, 360f, infiniteRepeatable(tween(TURN_MS, easing = LinearEasing)), label = "turn")
    Box(
        modifier
            .size(RING)
            .drawBehind {
                val line = RING_LINE.toPx()
                val inside = Size(size.width - line, size.height - line)
                val corner = Offset(line / 2, line / 2)
                drawArc(palette.accent.copy(alpha = 0.18f), 0f, 360f, false, corner, inside, style = Stroke(line))
                // The quarter of a round border its top edge is, turning.
                rotate(turn.value) {
                    drawArc(palette.accentBright, -135f, 90f, false, corner, inside, style = Stroke(line))
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        ServerMark(LocalChrome.current.branding, 52.dp)
    }
}

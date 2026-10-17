package app.melyxar.tv.look

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.unit.dp

/*
 * The bar of a player, drawn as the web's players draw theirs.
 */

/**
 * The line of a bar [line] thick across the middle, as the web's: one fully
 * round tube of [track] that clips what it holds, and in it each of [filled]
 * from the start as far as its share, rounded on the edge that moves alone.
 * Its start follows the tube's own curve, whatever its width, nothing of it
 * poking past.
 */
fun DrawScope.rail(line: Float, track: Color, vararg filled: Pair<Color, Float>) {
    val top = (size.height - line) / 2
    val round = CornerRadius(line / 2)
    val tube = Path().apply { addRoundRect(RoundRect(0f, top, size.width, top + line, round)) }
    clipPath(tube) {
        drawRect(track, Offset(0f, top), Size(size.width, line))
        filled.forEach { (color, share) ->
            val end = size.width * share.coerceIn(0f, 1f)
            if (end > 0f) {
                val fill = RoundRect(0f, top, end, top + line, CornerRadius.Zero, round, round, CornerRadius.Zero)
                drawPath(Path().apply { addRoundRect(fill) }, color)
            }
        }
    }
}

/** The small upright handle of a bar the remote moves along, as the web's
 *  players draw it: at [done] of the way along, grown to its height as the
 *  bar is [lit]. */
fun DrawScope.seekHandle(color: Color, done: Float, lit: Float) {
    if (lit <= 0f) {
        return
    }
    val handle = Size(8.dp.toPx(), 27.dp.toPx() * lit)
    drawRoundRect(
        color.copy(alpha = color.alpha * lit),
        Offset(size.width * done - handle.width / 2, (size.height - handle.height) / 2),
        handle,
        CornerRadius(2.dp.toPx()),
    )
}

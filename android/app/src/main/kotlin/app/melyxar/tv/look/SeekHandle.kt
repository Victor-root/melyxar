package app.melyxar.tv.look

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.dp

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

package app.melyxar.tv.door

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.scale
import app.melyxar.theme.Palette
import app.melyxar.tv.look.LocalPalette

/**
 * The ground of the door, as the web lights it (`door.css`, `.door-stage`):
 * a beam of the accent from above the middle of the screen, and the edges
 * sinking into the ground. Fainter in the light theme.
 */
@Composable
fun StageLight(content: @Composable BoxScope.() -> Unit) {
    val palette = LocalPalette.current
    Box(
        modifier = Modifier
            .fillMaxSize()
            .drawWithCache {
                val force = if (palette.light) 0.45f else 1f
                onDrawBehind {
                    drawRect(palette.ground)
                    beam(palette, force)
                    // The vignette: clear in the middle, the ground at the edges.
                    ellipse(
                        Offset(size.width * 0.5f, size.height * 0.46f),
                        size.width * 1.2f,
                        size.height * 0.95f,
                        0.52f to Color.Transparent,
                        1f to palette.ground.copy(alpha = 0.72f),
                    )
                }
            },
        content = content,
    )
}

private fun DrawScope.beam(palette: Palette, force: Float) {
    ellipse(
        Offset(size.width * 0.5f, -size.height * 0.06f),
        size.width * 0.09f,
        size.height * 0.62f,
        0f to palette.accentBright.copy(alpha = 0.13f * force),
        1f to Color.Transparent,
    )
    ellipse(
        Offset(size.width * 0.5f, -size.height * 0.12f),
        size.width * 0.26f,
        size.height * 0.78f,
        0f to palette.accent.copy(alpha = 0.15f * force),
        0.6f to palette.accentDim.copy(alpha = 0.05f * force),
        1f to Color.Transparent,
    )
}

/** CSS's `radial-gradient(rx ry at x y, ...)`: a circle's gradient drawn
 *  squashed into an ellipse. */
private fun DrawScope.ellipse(center: Offset, radiusX: Float, radiusY: Float, vararg stops: Pair<Float, Color>) {
    scale(1f, radiusY / radiusX, pivot = center) {
        drawRect(
            Brush.radialGradient(*stops, center = center, radius = radiusX),
            topLeft = Offset(0f, center.y - (center.y) * radiusX / radiusY),
            size = size.copy(height = size.height * radiusX / radiusY),
        )
    }
}

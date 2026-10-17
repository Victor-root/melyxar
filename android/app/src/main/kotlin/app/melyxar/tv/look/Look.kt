package app.melyxar.tv.look

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.TweenSpec
import androidx.compose.animation.core.tween
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LocalTextStyle
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.BiasAlignment
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.theme.Inter
import app.melyxar.theme.Palette
import app.melyxar.theme.paletteOf

/*
 * How the television looks: the web interface's tokens (`theme.css`), a CSS
 * pixel of the computer taken for a point of the television's screen, which
 * is 960 points across. Seen from the sofa, that is the computer's interface
 * at twice its size on a 1080p screen, which is what reads well from there.
 */

/** The account's colours, which every piece of the television reads. */
val LocalPalette = staticCompositionLocalOf { paletteOf() }

/** The way everything moves on the web: `--ease`. */
val Ease = CubicBezierEasing(0.22f, 0.61f, 0.36f, 1f)

/** `--quick`, for colours and rims. */
fun <T> quick(): TweenSpec<T> = tween(120, easing = Ease)

/** `--calm`, for sizes and fades. */
fun <T> calm(): TweenSpec<T> = tween(240, easing = Ease)

/** The margin a television keeps clear of its edges, where some screens
 *  cut: close to them, so the screen is used to its sides. */
val Gutter = 24.dp

/** Between two cards of a row, and between two rows. */
val CardGap = 14.dp
val SectionGap = 24.dp

/** Where a band is taken out of a picture taller than the band, as the
 *  web's `--where-a-band-is-cut`: an eighth of the way down from its top,
 *  so that what goes is the floor of the picture rather than its sky. */
val WhereABandIsCut = BiasAlignment(0f, 2 * 0.13f - 1)

/** A card standing up, 2:3, and one lying down, 16:9. */
val PosterWidth = 132.dp
val WideWidth = 230.dp

val CardShape = RoundedCornerShape(10.dp)
val ControlShape = RoundedCornerShape(8.dp)
val PillShape = RoundedCornerShape(percent = 50)

/** How much a chosen card or tile grows. */
const val FOCUS_GROWTH = 1.06f

/** How much smaller than the system's own the television draws everything:
 *  at the system's scale, made for being read across a room, Melyxar's
 *  pages were larger than they need to be. */
private const val DRAWN_AT = 0.85f

/** Everything in [content] drawn at Melyxar's own scale on a television. */
@Composable
fun TvScale(content: @Composable () -> Unit) {
    val system = LocalDensity.current
    CompositionLocalProvider(LocalDensity provides Density(system.density * DRAWN_AT, system.fontScale), content = content)
}

@Composable
fun TvLook(palette: Palette, content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalPalette provides palette,
        LocalTextStyle provides TextStyle(
            fontFamily = Inter,
            fontWeight = FontWeight.Normal,
            fontSize = 15.sp,
            lineHeight = 22.sp,
            color = palette.text,
        ),
        content = content,
    )
}

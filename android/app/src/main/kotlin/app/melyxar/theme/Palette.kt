package app.melyxar.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/**
 * Every colour of a native screen, in one theme and one accent: the tokens of
 * the web interface (`web/src/theme.css`), so a screen drawn here and one
 * drawn by the page are one interface.
 */
@Immutable
data class Palette(
    val light: Boolean,
    val accent: Color,
    val accentBright: Color,
    val accentDim: Color,
    val accentGlow: Color,
    val accentContrast: Color,
    /** The colour the logo is drawn in. */
    val mark: Color,
    val ground: Color,
    val surface: Color,
    val surfaceRaised: Color,
    val surfaceHover: Color,
    val border: Color,
    val borderStrong: Color,
    val text: Color,
    val textSoft: Color,
    val textFaint: Color,
    val ok: Color,
    val attention: Color,
    val trouble: Color,
    val news: Color,
    /** The marks of the rating sites, in their own colours. */
    val imdb: Color = Color(0xFFF5C518),
    val imdbInk: Color = Color(0xFF000000),
    val tmdbFrom: Color = Color(0xFF90CEA1),
    val tmdbTo: Color = Color(0xFF01B4E4),
    val tmdbInk: Color = Color(0xFF0D253F),
)

fun paletteOf(accent: Int = USUAL_ACCENT, light: Boolean = false): Palette {
    val shades = accentShades(accent, light)
    val accentColours = Palette(
        light = light,
        accent = Color(shades.accent),
        accentBright = Color(shades.bright),
        accentDim = Color(shades.dim),
        accentGlow = Color(shades.glow),
        accentContrast = Color(shades.contrast),
        mark = Color(shades.mark),
        ground = Color(0xFF0C0D10),
        surface = Color(0xFF14161B),
        surfaceRaised = Color(0xFF1B1E25),
        surfaceHover = Color(0xFF232730),
        border = Color(0xFF2A2F39),
        borderStrong = Color(0xFF3A4150),
        text = Color(0xFFF2F4F8),
        textSoft = Color(0xFFAAB2C0),
        textFaint = Color(0xFF6F7888),
        ok = Color(0xFF34C77B),
        attention = Color(0xFFF2A93B),
        trouble = Color(0xFFF0555A),
        news = Color(0xFF4C9FFE),
    )
    return if (!light) {
        accentColours
    } else {
        accentColours.copy(
            ground = Color(0xFFF4F5F7),
            surface = Color(0xFFFFFFFF),
            surfaceRaised = Color(0xFFFFFFFF),
            surfaceHover = Color(0xFFECEEF2),
            border = Color(0xFFDFE2E8),
            borderStrong = Color(0xFFC3C8D2),
            text = Color(0xFF14161B),
            textSoft = Color(0xFF4D5462),
            textFaint = Color(0xFF7B8393),
            ok = Color(0xFF17904F),
            attention = Color(0xFFB86E00),
            trouble = Color(0xFFCF2F35),
            news = Color(0xFF1F6FD1),
        )
    }
}

/**
 * Whether an account sees the light theme: its own choice, the server's when
 * it follows the server, and the device's when either follows the device.
 */
fun wantsLight(accountMode: String, serverMode: String, deviceIsDark: Boolean): Boolean {
    val mode = if (accountMode == "server") serverMode else accountMode
    return when (mode) {
        "light" -> true
        "system" -> !deviceIsDark
        else -> false
    }
}

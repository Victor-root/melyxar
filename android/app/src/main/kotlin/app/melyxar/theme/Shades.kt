package app.melyxar.theme

import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt

/*
 * Colours as `0xAARRGGBB`, worked out the way the web interface works them out
 * (`theme.css`, `settings.tsx`, `mark.ts`, `colour.ts`), so a native screen
 * and the page agree on every shade of an account's accent.
 */

/** The accent every account starts with, the red Melyxar was drawn around. */
const val USUAL_ACCENT = 0xFFC81E1E.toInt()

private const val WHITE = 0xFFFFFFFF.toInt()
private const val BLACK = 0xFF000000.toInt()

/** The dark text the web puts on a pale accent. */
private const val INK = 0xFF14161B.toInt()

/** The four shades of an accent, and the colour of the logo drawn in it. */
data class AccentShades(
    val accent: Int,
    val bright: Int,
    val dim: Int,
    val glow: Int,
    /** What is written on the accent. */
    val contrast: Int,
    val mark: Int,
)

/**
 * The shades of an accent. Those of the usual red were measured by hand and
 * are taken as they are; any other is mixed with white and black.
 */
fun accentShades(accent: Int, light: Boolean): AccentShades {
    val glowShare = if (light) 0.22f else 0.35f
    if (accent == USUAL_ACCENT) {
        return AccentShades(
            accent = accent,
            bright = 0xFFE63535.toInt(),
            dim = 0xFF9B1616.toInt(),
            glow = withAlpha(accent, glowShare),
            contrast = WHITE,
            mark = 0xFFE50000.toInt(),
        )
    }
    return AccentShades(
        accent = accent,
        bright = mix(accent, WHITE, 0.82f),
        dim = mix(accent, BLACK, 0.78f),
        glow = withAlpha(accent, glowShare),
        contrast = readableOn(accent),
        mark = vividOf(accent),
    )
}

/** `#rrggbb` as a colour, or nothing when it is not one. */
fun parseHex(written: String): Int? {
    val digits = written.trim().removePrefix("#")
    if (digits.length != 6) {
        return null
    }
    return digits.toLongOrNull(16)?.let { (it or 0xFF000000L).toInt() }
}

/** `color-mix(in srgb, a share, b)`, opaque. */
fun mix(a: Int, b: Int, share: Float): Int {
    fun channel(shift: Int) = ((a shr shift and 0xFF) * share + (b shr shift and 0xFF) * (1 - share)).roundToInt()
    return (0xFF shl 24) or (channel(16) shl 16) or (channel(8) shl 8) or channel(0)
}

/** The colour seen through, at this share. */
fun withAlpha(colour: Int, alpha: Float): Int = ((alpha * 255).roundToInt() shl 24) or (colour and 0xFFFFFF)

/** How light a colour is to the eye, from 0 to 1, as the contrast rules
 *  measure it. */
fun relativeLight(colour: Int): Double =
    0.2126 * linear(colour shr 16) + 0.7152 * linear(colour shr 8) + 0.0722 * linear(colour)

private fun linear(channel: Int): Double {
    val value = (channel and 0xFF) / 255.0
    return if (value <= 0.04045) value / 12.92 else ((value + 0.055) / 1.055).pow(2.4)
}

/** Black or white on this colour, whichever can be read on it. */
fun readableOn(colour: Int): Int = if (relativeLight(colour) > 0.36) INK else WHITE

/** How much more saturated than the accent the logo is drawn, and how
 *  light: the lightness its relief was fitted to. */
private const val MADE_VIVID = 1.35
private const val MARK_LIGHTNESS = 0.45

/** The colour the logo is drawn in for an accent: its hue, made vivid. */
fun vividOf(accent: Int): Int {
    val red = (accent shr 16 and 0xFF) / 255.0
    val green = (accent shr 8 and 0xFF) / 255.0
    val blue = (accent and 0xFF) / 255.0
    val highest = max(red, max(green, blue))
    val lowest = min(red, min(green, blue))
    val spread = highest - lowest
    val lightness = (highest + lowest) / 2
    val saturation = if (spread == 0.0) 0.0 else spread / (1 - abs(2 * lightness - 1))
    val hue = 60 * when {
        spread == 0.0 -> 0.0
        highest == red -> ((green - blue) / spread + 6) % 6
        highest == green -> (blue - red) / spread + 2
        else -> (red - green) / spread + 4
    }
    return fromHsl(hue, min(1.0, saturation * MADE_VIVID), MARK_LIGHTNESS)
}

private fun fromHsl(hue: Double, saturation: Double, lightness: Double): Int {
    val chroma = (1 - abs(2 * lightness - 1)) * saturation
    fun channel(offset: Int): Int {
        val turn = (offset + hue / 30) % 12
        val value = lightness - chroma / 2 * max(-1.0, min(min(turn - 3, 9 - turn), 1.0))
        return (value * 255).roundToInt()
    }
    return (0xFF shl 24) or (channel(0) shl 16) or (channel(8) shl 8) or channel(4)
}

/**
 * One pixel of the logo in a colour: the shade of the relief, a grey picture
 * of the logo's light and shade, laid over the colour in hard light, as much
 * of it as the relief covers. The sums the page and the server make
 * (`web/src/mark.ts`, `crates/app/src/mark.rs`), so all three agree.
 */
fun markPixel(relief: Int, colour: Int): Int {
    val cover = (relief ushr 24) / 255.0
    val shade = (relief shr 16 and 0xFF) / 255.0
    fun channel(shift: Int): Int {
        val under = (colour shr shift and 0xFF) / 255.0
        val mixed = cover * hardLight(under, shade) + (1 - cover) * under
        return (mixed * 255).roundToInt()
    }
    return ((cover * 255).roundToInt() shl 24) or (channel(16) shl 16) or (channel(8) shl 8) or channel(0)
}

/** Below the middle grey the shade darkens the colour, above it lightens it. */
private fun hardLight(under: Double, shade: Double): Double {
    if (shade <= 0.5) {
        return under * 2 * shade
    }
    val lifted = 2 * shade - 1
    return under + lifted - under * lifted
}

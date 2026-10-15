package app.melyxar.theme

import kotlin.math.atan2
import kotlin.math.cbrt
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sin

/*
 * Which painting an account without a picture wears, and the tints it is
 * painted in: the web's sums (`faces.ts`, the `.face-art` tints of
 * `header.css`), so an account wears the same face on the television as on
 * the page.
 */

/** The paintings, in the web's order, which the name's hash picks from. */
enum class FaceDrawing { Planet, Eclipse, Rings, Silk, Horizon, Record, Nebula, Orbits }

/** Degrees round the colour wheel from the accent. The first is the accent
 *  itself; the rest are chosen to sit well beside it rather than evenly. */
private val FACE_TURNS = intArrayOf(0, 170, 205, 40, 285, 325)

data class FaceArt(val drawing: FaceDrawing, val turn: Int)

/** FNV-1a over the name's code points, as `for...of` walks them on the web. */
private fun hashOf(name: String): UInt {
    var hash = 0x811c9dc5u
    name.codePoints().forEach { point ->
        hash = (hash xor point.toUInt()) * 0x01000193u
    }
    return hash
}

fun faceArtOf(name: String): FaceArt {
    val hash = hashOf(name)
    val drawings = FaceDrawing.entries
    return FaceArt(
        drawing = drawings[(hash % drawings.size.toUInt()).toInt()],
        turn = FACE_TURNS[(hash / drawings.size.toUInt() % FACE_TURNS.size.toUInt()).toInt()],
    )
}

/** A colour in OKLCH: lightness from 0 to 1, chroma, hue in degrees. */
data class Oklch(val lightness: Double, val chroma: Double, val hue: Double)

/** Below this chroma a colour has no hue, which a browser then reads as 0. */
private const val ACHROMATIC = 0.000004

/** An opaque colour in OKLCH, through the matrices of CSS Color 4. */
fun oklchOf(colour: Int): Oklch {
    val red = toLinear(colour shr 16)
    val green = toLinear(colour shr 8)
    val blue = toLinear(colour)
    val long = cbrt(0.41222146947076305 * red + 0.5363325372617348 * green + 0.05144599326750221 * blue)
    val middle = cbrt(0.21190349581782522 * red + 0.68069955064523435 * green + 0.10739695353694056 * blue)
    val short = cbrt(0.088302459190056429 * red + 0.2817188391361215 * green + 0.62997870167382219 * blue)
    val lightness = 0.210454268309314 * long + 0.7936177747023054 * middle - 0.0040720430116193 * short
    val a = 1.9779985324311684 * long - 2.4285922420485799 * middle + 0.450593709617411 * short
    val b = 0.0259040424655478 * long + 0.7827717124575296 * middle - 0.8086757549230774 * short
    val chroma = hypot(a, b)
    val hue = if (chroma < ACHROMATIC) 0.0 else Math.toDegrees(atan2(b, a)).let { if (it < 0) it + 360 else it }
    return Oklch(lightness, chroma, hue)
}

/** An OKLCH colour on an sRGB screen, each channel clamped into it as
 *  browsers do. */
fun fromOklch(colour: Oklch): Int {
    val turn = Math.toRadians(colour.hue)
    val a = colour.chroma * cos(turn)
    val b = colour.chroma * sin(turn)
    val long = (colour.lightness + 0.3963377773761749 * a + 0.2158037573099136 * b).pow(3)
    val middle = (colour.lightness - 0.1055613458156586 * a - 0.0638541728258133 * b).pow(3)
    val short = (colour.lightness - 0.0894841775298119 * a - 1.2914855480194092 * b).pow(3)
    val red = 4.0767416360759583 * long - 3.3077115392580616 * middle + 0.23096990318210431 * short
    val green = -1.2684379732850317 * long + 2.6097573492876887 * middle - 0.34131937600265727 * short
    val blue = -0.0041960761386755641 * long - 0.7034186179359363 * middle + 1.7076146940746117 * short
    return (0xFF shl 24) or (toChannel(red) shl 16) or (toChannel(green) shl 8) or toChannel(blue)
}

private fun toLinear(channel: Int): Double {
    val value = (channel and 0xFF) / 255.0
    return if (value <= 0.04045) value / 12.92 else ((value + 0.055) / 1.055).pow(2.4)
}

private fun toChannel(linear: Double): Int {
    val clamped = linear.coerceIn(0.0, 1.0)
    val value = if (clamped > 0.0031308) 1.055 * clamped.pow(1 / 2.4) - 0.055 else 12.92 * clamped
    return (value * 255).roundToInt()
}

/** The tints a painting is made of, as `0xAARRGGBB`. */
data class FaceTints(val night: Int, val deep: Int, val base: Int, val glow: Int, val light: Int)

/**
 * The tints of a painting: the accent turned round the wheel by the account's
 * turn, each keeping a little colour even when the accent has almost none, so
 * a grey accent still gives faces rather than smudges.
 */
fun faceTints(accent: Int, turn: Int): FaceTints {
    val (_, chroma, hue) = oklchOf(accent)
    val turned = hue + turn
    return FaceTints(
        night = fromOklch(Oklch(0.15, 0.03, turned)),
        deep = fromOklch(Oklch(0.34, max(chroma * 0.8, 0.09), turned - 8)),
        base = fromOklch(Oklch(0.6, max(chroma, 0.14), turned)),
        glow = fromOklch(Oklch(0.72, max(chroma, 0.16), turned + 18)),
        light = fromOklch(Oklch(0.93, max(chroma * 0.45, 0.06), turned + 10)),
    )
}

package app.melyxar.tv.look

import android.graphics.Bitmap
import android.graphics.BlurMaskFilter
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.RectF
import android.os.Build
import android.util.LruCache
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.ModifierNodeElement
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.platform.InspectorInfo
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.core.graphics.withSave
import app.melyxar.theme.Palette
import app.melyxar.tv.probe.Probe
import app.melyxar.tv.probe.ShadowTest
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.roundToInt

/*
 * The web's box-shadow, value for value: a CSS pixel is a point of the
 * television. Each shadow is blurred once into a mask that every piece of
 * the same size shares, and drawn from it after that: blurring and cutting
 * it live on every frame cost the box's graphics card most of its time.
 */

/** One shadow of CSS's `box-shadow: x y blur spread color`. */
@Immutable
data class BoxShadow(val color: Color, val blur: Dp, val spread: Dp = 0.dp, val x: Dp = 0.dp, val y: Dp = 0.dp)

/** The ink of the light theme's shadows, `rgb(20, 22, 27)`. */
private val LightInk = Color(0xFF14161B)

private val LightCard = BoxShadow(LightInk.copy(alpha = 0.14f), blur = 14.dp, y = 4.dp)
private val DarkCard = BoxShadow(Color.Black.copy(alpha = 0.35f), blur = 18.dp, y = 6.dp)
private val LightRaised = BoxShadow(LightInk.copy(alpha = 0.18f), blur = 32.dp, y = 12.dp)
private val DarkRaised = BoxShadow(Color.Black.copy(alpha = 0.45f), blur = 40.dp, y = 14.dp)
private val LightTile = arrayOf(BoxShadow(LightInk.copy(alpha = 0.2f), blur = 3.dp, y = 1.dp))
private val DarkTile = arrayOf(
    BoxShadow(Color.White.copy(alpha = 0.06f), blur = 0.dp, spread = 1.dp),
    BoxShadow(Color.Black.copy(alpha = 0.45f), blur = 3.dp, y = 1.dp),
)

/** `--shadow-card`, in each theme. */
val Palette.shadowCard: BoxShadow
    get() = if (light) LightCard else DarkCard

/** `--shadow-raised`, in each theme. */
val Palette.shadowRaised: BoxShadow
    get() = if (light) LightRaised else DarkRaised

/** `--shadow-tile`, in each theme: what a card rests on. */
val Palette.shadowTile: Array<BoxShadow>
    get() = if (light) LightTile else DarkTile

/** Shown whole, the same every time it is passed. */
internal val Whole: () -> Float = { 1f }

/**
 * [shadows] round a piece of [shape], as strong as [shown] says, drawn
 * outside the piece only, as CSS draws them. Below Android 9 the graphics
 * card cannot blur, and they are left out rather than drawn hard.
 *
 * What is worked out for them is kept from one composition to the next
 * while the shape, the shadows and the piece's size stay the same, so a
 * card composed again does not make its shadows anew.
 */
fun Modifier.boxShadow(shape: Shape, vararg shadows: BoxShadow, shown: () -> Float = Whole): Modifier {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
        return this
    }
    return this then BoxShadowElement(shape, shadows.asList(), shown)
}

private data class BoxShadowElement(
    val shape: Shape,
    val shadows: List<BoxShadow>,
    val shown: () -> Float,
) : ModifierNodeElement<BoxShadowNode>() {
    override fun create() = BoxShadowNode(shape, shadows, shown)

    override fun update(node: BoxShadowNode) = node.update(shape, shadows, shown)

    override fun InspectorInfo.inspectableProperties() {
        name = "boxShadow"
    }
}

private class BoxShadowNode(
    private var shape: Shape,
    private var shadows: List<BoxShadow>,
    private var shown: () -> Float,
) : Modifier.Node(), DrawModifierNode {
    private val made = MadeFor()
    private var piece = android.graphics.Path()
    private var pieceCorner = 0f
    private var pieceSize = Size.Zero
    private var layers = emptyList<DrawnShadow>()

    fun update(shape: Shape, shadows: List<BoxShadow>, shown: () -> Float) {
        if (shape != this.shape || shadows != this.shadows) {
            this.shape = shape
            this.shadows = shadows
            made.forget()
        }
        this.shown = shown
        invalidateDraw()
    }

    override fun ContentDrawScope.draw() {
        if (made.isStale(this)) {
            make()
        }
        val strength = shown()
        val test = Probe.shadows
        if (strength > 0f && test != ShadowTest.None) {
            Probe.shadowDrawn()
            drawIntoCanvas { canvas ->
                val native = canvas.nativeCanvas
                if (test == ShadowTest.Baked) {
                    layers.forEach { layer ->
                        val baked = layer.baked ?: BakedShadows.of(pieceSize, pieceCorner, layer).also { layer.baked = it }
                        layer.bakedPaint.alpha = (layer.alpha * strength * 255).roundToInt()
                        native.drawBitmap(baked.mask, baked.left, baked.top, layer.bakedPaint)
                    }
                } else {
                    native.withSave {
                        clipOutPath(piece)
                        layers.forEach { layer ->
                            layer.paint.alpha = (layer.alpha * strength * 255).roundToInt()
                            drawRoundRect(layer.box, layer.corner, layer.corner, layer.paint)
                        }
                    }
                }
            }
        }
        drawContent()
    }

    private fun DrawScope.make() {
        Probe.shadowMade()
        val outline = shape.createOutline(size, layoutDirection, this)
        val corner = (outline as? Outline.Rounded)?.roundRect?.topLeftCornerRadius?.x ?: 0f
        pieceCorner = corner
        pieceSize = size
        piece = android.graphics.Path().apply {
            addRoundRect(0f, 0f, size.width, size.height, corner, corner, android.graphics.Path.Direction.CW)
        }
        layers = shadows.map { shadow ->
            val spread = shadow.spread.toPx()
            val x = shadow.x.toPx()
            val y = shadow.y.toPx()
            val radius = blurOf(shadow.blur.toPx())
            DrawnShadow(
                box = RectF(x - spread, y - spread, size.width + x + spread, size.height + y + spread),
                corner = max(0f, corner + spread),
                alpha = shadow.color.alpha,
                radius = radius,
                paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                    color = shadow.color.toArgb()
                    radius?.let { maskFilter = BlurMaskFilter(it, BlurMaskFilter.Blur.NORMAL) }
                },
                bakedPaint = android.graphics.Paint(android.graphics.Paint.FILTER_BITMAP_FLAG).apply {
                    color = shadow.color.toArgb()
                },
            )
        }
    }
}

private class DrawnShadow(
    val box: RectF,
    val corner: Float,
    val alpha: Float,
    val radius: Float?,
    val paint: android.graphics.Paint,
    val bakedPaint: android.graphics.Paint,
) {
    var baked: BakedShadow? = null
}

/** A shadow drawn once into a mask of its own, its blur and the cut of the
 *  piece out of it included, and where it sits from the piece's corner. */
private class BakedShadow(val mask: Bitmap, val left: Float, val top: Float)

/** What a baked shadow is made from: two pieces of the same size wearing
 *  the same shadow share its mask, whatever its colour. */
private data class BakeKey(
    val width: Float,
    val height: Float,
    val pieceCorner: Float,
    val box: RectF,
    val corner: Float,
    val radius: Float?,
)

/**
 * The shadows' masks, each made once on the thread that composes and drawn
 * after that as a plain picture, coloured by its paint: the graphics card
 * then neither blurs nor cuts anything while the screen moves, and a piece
 * growing as the remote comes to it only stretches its picture.
 */
private object BakedShadows {
    private const val KEPT_BYTES = 16 * 1024 * 1024

    private val kept = object : LruCache<BakeKey, BakedShadow>(KEPT_BYTES) {
        override fun sizeOf(key: BakeKey, value: BakedShadow) = value.mask.allocationByteCount
    }

    fun of(pieceSize: Size, pieceCorner: Float, layer: DrawnShadow): BakedShadow {
        val key = BakeKey(pieceSize.width, pieceSize.height, pieceCorner, layer.box, layer.corner, layer.radius)
        return kept.get(key) ?: bake(key).also { kept.put(key, it) }
    }

    private fun bake(key: BakeKey): BakedShadow {
        Probe.shadowBaked()
        // How far Android's blur reaches past the box: three deviations.
        val reach = key.radius?.let { ceil(3 * (0.57735f * it + 0.5f)).toInt() } ?: 0
        val left = floor(key.box.left).toInt() - reach - 1
        val top = floor(key.box.top).toInt() - reach - 1
        val right = ceil(key.box.right).toInt() + reach + 1
        val bottom = ceil(key.box.bottom).toInt() + reach + 1
        val mask = Bitmap.createBitmap(right - left, bottom - top, Bitmap.Config.ALPHA_8)
        android.graphics.Canvas(mask).apply {
            translate(-left.toFloat(), -top.toFloat())
            drawRoundRect(key.box, key.corner, key.corner, android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                key.radius?.let { maskFilter = BlurMaskFilter(it, BlurMaskFilter.Blur.NORMAL) }
            })
            drawRoundRect(0f, 0f, key.width, key.height, key.pieceCorner, key.pieceCorner, android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                xfermode = PorterDuffXfermode(PorterDuff.Mode.CLEAR)
            })
        }
        return BakedShadow(mask, left.toFloat(), top.toFloat())
    }
}

/** The size, density and direction what a piece's drawing works out was
 *  made for, to make it again only once one of them moves. */
internal class MadeFor {
    private var size = Size.Unspecified
    private var density = 0f
    private var direction: LayoutDirection? = null

    /** What was made no longer holds, whatever the piece is drawn at. */
    fun forget() {
        direction = null
    }

    /** Whether what was made is not for [scope], taking it as made for it. */
    fun isStale(scope: DrawScope): Boolean {
        if (scope.size == size && scope.density == density && scope.layoutDirection == direction) {
            return false
        }
        size = scope.size
        density = scope.density
        direction = scope.layoutDirection
        return true
    }
}

/**
 * The radius Android's blur takes for a CSS blur of [cssBlur] pixels, or
 * nothing for one too small to show. CSS blurs by a deviation of half its
 * radius; Android turns a radius r into a deviation of 0.57735 r + 0.5.
 */
internal fun blurOf(cssBlur: Float): Float? {
    val deviation = cssBlur / 2
    return if (deviation <= 0.5f) null else (deviation - 0.5f) / 0.57735f
}

/** The web's halo of a card on hover, `--shadow-raised` under it. */
fun Modifier.halo(shape: Shape, palette: Palette, lit: () -> Float) =
    boxShadow(shape, palette.shadowRaised, accentAir(palette.accent), accentBloom(palette.accent), shown = lit)

/** The two blooms of the accent round a lit card or tile:
 *  `0 0 8px accent 55%` and `0 0 26px 2px accent 30%`. */
fun accentAir(accent: Color) = BoxShadow(accent.copy(alpha = 0.55f), blur = 8.dp)

fun accentBloom(accent: Color) = BoxShadow(accent.copy(alpha = 0.30f), blur = 26.dp, spread = 2.dp)

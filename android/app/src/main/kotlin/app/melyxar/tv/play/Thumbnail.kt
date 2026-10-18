package app.melyxar.tv.play

import android.content.Context
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import app.melyxar.server.Thumbnails
import app.melyxar.tv.look.LocalPictures
import app.melyxar.tv.look.LocalServer
import coil3.ImageLoader
import coil3.request.CachePolicy
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.size.Size
import coil3.toBitmap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlin.math.roundToInt

/** Where the thumbnail of a moment sits: on which sheet, in which column
 *  and row of it. */
data class Spot(val sheet: Int, val column: Int, val row: Int)

/** The thumbnail covering [seconds] of the film, never past the last one: a
 *  sheet is filled to its end with black, and a black square is worse than
 *  none. Nothing when the film was not read for them. */
fun Thumbnails.spotAt(seconds: Double): Spot? {
    val perSheet = columns * rows
    if (everySeconds <= 0 || perSheet <= 0 || counted <= 0) {
        return null
    }
    val index = (seconds.coerceAtLeast(0.0) / everySeconds).toInt().coerceAtMost(counted - 1)
    val onItsSheet = index % perSheet
    return Spot(sheet = index / perSheet, column = onItsSheet % columns, row = onItsSheet / columns)
}

/** How tall a thumbnail is [across] wide, the film's own shape kept. */
fun Thumbnails.heightAt(across: Dp): Dp = if (width > 0) across * height / width else 0.dp

/**
 * The thumbnail covering [seconds], cut out of its sheet and drawn [across]
 * wide. Nothing until its sheet is in: an empty box where a picture should
 * be says the player is broken, and it is not.
 */
@Composable
fun Thumbnail(thumbnails: Thumbnails, seconds: Double, across: Dp, modifier: Modifier = Modifier) {
    val spot = thumbnails.spotAt(seconds) ?: return
    val sheet by rememberSheet(thumbnails, spot.sheet)
    Canvas(modifier.size(across, thumbnails.heightAt(across))) {
        val drawn = sheet ?: return@Canvas
        // The sheet as it came, which the loader shrinks when it is wider
        // than it takes: cut in its own pixels, or the cut slides into the
        // next picture down.
        val kept = drawn.width.toFloat() / (thumbnails.columns * thumbnails.width)
        val keptDown = drawn.height.toFloat() / (thumbnails.rows * thumbnails.height)
        drawImage(
            drawn,
            srcOffset = IntOffset((spot.column * thumbnails.width * kept).roundToInt(), (spot.row * thumbnails.height * keptDown).roundToInt()),
            srcSize = IntSize((thumbnails.width * kept).roundToInt(), (thumbnails.height * keptDown).roundToInt()),
            dstSize = IntSize(size.width.roundToInt(), size.height.roundToInt()),
        )
    }
}

/** How many sheets are kept decoded: those of a stretch of chapters on the
 *  screen, with the one the bar's picture is cut from. */
private const val SHEETS_KEPT = 3

/**
 * The sheets of thumbnails last fetched, each decoded once for every
 * picture cut from it, so going back along the bar draws at once. They stay
 * out of the posters' memory: a whole sheet weighs as much as a hundred
 * posters, and would push them out of it.
 */
private class Sheets(private val pictures: ImageLoader, private val context: Context, private val scope: CoroutineScope) {
    private val kept = object : LinkedHashMap<String, Deferred<ImageBitmap?>>(SHEETS_KEPT, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Deferred<ImageBitmap?>>) = size > SHEETS_KEPT
    }

    /** The sheet at [address], fetched unless it is kept or on its way. */
    fun sheet(address: String): Deferred<ImageBitmap?> = kept.getOrPut(address) {
        scope.async {
            // Whole, since a thumbnail is cut out of it by its pixels.
            val request = ImageRequest.Builder(context)
                .data(address)
                .size(Size.ORIGINAL)
                .memoryCachePolicy(CachePolicy.DISABLED)
                .build()
            val decoded = (pictures.execute(request) as? SuccessResult)?.image?.toBitmap()?.asImageBitmap()
            // One that did not come is asked again the next time.
            if (decoded == null) kept.remove(address)
            decoded
        }
    }
}

private val LocalSheets = staticCompositionLocalOf<Sheets> { error("no sheets") }

/** [content] with the sheets its thumbnails are cut from kept, for as long
 *  as it is shown and no longer. */
@Composable
fun KeepingSheets(content: @Composable () -> Unit) {
    val pictures = LocalPictures.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    CompositionLocalProvider(LocalSheets provides remember(pictures) { Sheets(pictures, context, scope) }, content = content)
}

/** One sheet of thumbnails, fetched as the posters are. */
@Composable
private fun rememberSheet(thumbnails: Thumbnails, sheet: Int): State<ImageBitmap?> {
    val api = LocalServer.current
    val sheets = LocalSheets.current
    return produceState(null, thumbnails, sheet) {
        // The sheet before stays out: its pictures are not this one's.
        value = null
        value = sheets.sheet(api.address("${thumbnails.url}/$sheet.jpg")).await()
    }
}

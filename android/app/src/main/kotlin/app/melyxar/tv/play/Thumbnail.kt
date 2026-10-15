package app.melyxar.tv.play

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
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
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.size.Size
import coil3.toBitmap
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
        drawImage(
            drawn,
            srcOffset = IntOffset(spot.column * thumbnails.width, spot.row * thumbnails.height),
            srcSize = IntSize(thumbnails.width, thumbnails.height),
            dstSize = IntSize(size.width.roundToInt(), size.height.roundToInt()),
        )
    }
}

/** One sheet of thumbnails, fetched as the posters are and kept by the same
 *  memory, so going back along the bar draws at once. */
@Composable
private fun rememberSheet(thumbnails: Thumbnails, sheet: Int): State<ImageBitmap?> {
    val api = LocalServer.current
    val pictures = LocalPictures.current
    val context = LocalContext.current
    return produceState(null, thumbnails, sheet) {
        // Whole, since a thumbnail is cut out of it by its pixels.
        val request = ImageRequest.Builder(context)
            .data(api.address("${thumbnails.url}/$sheet.jpg"))
            .size(Size.ORIGINAL)
            .build()
        value = (pictures.execute(request) as? SuccessResult)?.image?.toBitmap()?.asImageBitmap()
    }
}

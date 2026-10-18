package app.melyxar.tv.play

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import app.melyxar.server.Api
import app.melyxar.server.Thumbnails
import app.melyxar.tv.look.LocalPictures
import app.melyxar.tv.look.LocalServer
import coil3.ImageLoader
import coil3.request.CachePolicy
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.request.bitmapConfig
import coil3.size.Size
import coil3.toBitmap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
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
 * wide. The one before stays until it is in: nothing at all where a picture
 * should be says the player is broken, and it is not.
 */
@Composable
fun Thumbnail(thumbnails: Thumbnails, seconds: Double, across: Dp, modifier: Modifier = Modifier) {
    val spot = thumbnails.spotAt(seconds) ?: return
    val tile by rememberTile(thumbnails, spot)
    Canvas(modifier.size(across, thumbnails.heightAt(across))) {
        val drawn = tile ?: return@Canvas
        drawImage(drawn, dstSize = IntSize(size.width.roundToInt(), size.height.roundToInt()))
    }
}

/** The thumbnails of [moments] fetched as the screen showing them opens, so
 *  they are in by the time the remote reaches them. */
@Composable
fun FetchingThumbnails(thumbnails: Thumbnails, moments: List<Double>) {
    val api = LocalServer.current
    val sheets = LocalSheets.current
    LaunchedEffect(sheets, thumbnails, moments) {
        moments.mapNotNull { thumbnails.spotAt(it) }.distinct().forEach { sheets.tile(api.sheetOf(thumbnails, it.sheet), it, thumbnails) }
    }
}

/** Every sheet of [thumbnails] brought onto the device, not decoded: going
 *  anywhere on the bar then costs only the reading of one. */
@Composable
fun FetchingSheets(thumbnails: Thumbnails) {
    val api = LocalServer.current
    val sheets = LocalSheets.current
    LaunchedEffect(sheets, thumbnails) {
        repeat(thumbnails.sheetsCounted()) { sheets.bringOver(api.sheetOf(thumbnails, it)) }
    }
}

/** How many sheets the thumbnails take. */
fun Thumbnails.sheetsCounted(): Int = (columns * rows).takeIf { it > 0 }?.let { (counted + it - 1) / it } ?: 0

private fun Api.sheetOf(thumbnails: Thumbnails, sheet: Int): String = address("${thumbnails.url}/$sheet.jpg")

/** How many sheets are kept decoded: the one the bar's picture is cut from,
 *  and the one it was cut from before, as the bar goes back and forth. */
private const val SHEETS_KEPT = 2

/** How many thumbnails are kept cut out: a page's row of chapters and the
 *  player's, with the bar's last ones. */
private const val TILES_KEPT = 48

/** How wide a sheet brought over ahead is read, the reading being only the
 *  way it lands on the device. */
private const val BROUGHT_OVER_PX = 64

/** A map keeping only the [most] last used of its entries. */
private fun <K, V> lastUsed(most: Int) = object : LinkedHashMap<K, V>(most, 0.75f, true) {
    override fun removeEldestEntry(eldest: MutableMap.MutableEntry<K, V>) = size > most
}

/**
 * The thumbnails last shown, each cut once out of its sheet. Only the little
 * pictures go to the graphics card: a whole sheet weighs as much as a
 * hundred posters, and sending one there held the screen still. The sheets
 * stay out of the posters' memory too, and are decoded one at a time, a
 * row of chapters touching most of them at once.
 */
private class Sheets(private val pictures: ImageLoader, private val context: Context, private val scope: CoroutineScope) {
    private val sheets = lastUsed<String, Deferred<Bitmap?>>(SHEETS_KEPT)
    private val tiles = lastUsed<Pair<String, Spot>, ImageBitmap>(TILES_KEPT)
    private val cutting = Mutex()

    /**
     * The thumbnail at [spot] of the sheet at [address], cut unless it is
     * kept. Waiting its turn only while it is still wanted: the bar running
     * past a hundred thumbnails asks for each, and cutting those it has
     * already left kept the one it stopped on waiting behind them.
     */
    suspend fun tile(address: String, spot: Spot, thumbnails: Thumbnails): ImageBitmap? = tiles[address to spot] ?: cutting.withLock {
        tiles[address to spot] ?: sheet(address).await()
            ?.let { sheet -> withContext(Dispatchers.Default) { cutOut(sheet, spot, thumbnails) } }
            ?.also { tiles[address to spot] = it }
    }

    /** The sheet at [address] onto the device, as small as it can be read
     *  to get there: what is kept on the device is the file itself. */
    suspend fun bringOver(address: String) {
        pictures.execute(
            ImageRequest.Builder(context)
                .data(address)
                .size(BROUGHT_OVER_PX)
                .allowHardware(false)
                .memoryCachePolicy(CachePolicy.DISABLED)
                .build(),
        )
    }

    /** The sheet at [address], fetched unless it is kept or on its way. */
    private fun sheet(address: String): Deferred<Bitmap?> = sheets.getOrPut(address) {
        scope.async {
            // Whole, since a thumbnail is cut out of it by its pixels, and
            // kept in memory rather than on the graphics card, which only
            // ever gets the pieces.
            val request = ImageRequest.Builder(context)
                .data(address)
                .size(Size.ORIGINAL)
                .allowHardware(false)
                .bitmapConfig(Bitmap.Config.RGB_565)
                .memoryCachePolicy(CachePolicy.DISABLED)
                .build()
            val decoded = (pictures.execute(request) as? SuccessResult)?.image?.toBitmap()
            if (decoded == null) sheets.remove(address)
            decoded
        }
    }
}

/** The thumbnail at [spot], cut out of [sheet] in its own pixels, which the
 *  loader shrinks when it is wider than it takes: or the cut slides into the
 *  next picture down. */
private fun cutOut(sheet: Bitmap, spot: Spot, thumbnails: Thumbnails): ImageBitmap {
    val kept = sheet.width.toFloat() / (thumbnails.columns * thumbnails.width)
    val keptDown = sheet.height.toFloat() / (thumbnails.rows * thumbnails.height)
    val left = (spot.column * thumbnails.width * kept).roundToInt()
    val top = (spot.row * thumbnails.height * keptDown).roundToInt()
    val wide = (thumbnails.width * kept).roundToInt().coerceAtMost(sheet.width - left)
    val high = (thumbnails.height * keptDown).roundToInt().coerceAtMost(sheet.height - top)
    return Bitmap.createBitmap(sheet, left, top, wide, high).asImageBitmap()
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

/** One thumbnail, fetched as the posters are. */
@Composable
private fun rememberTile(thumbnails: Thumbnails, spot: Spot): State<ImageBitmap?> {
    val api = LocalServer.current
    val sheets = LocalSheets.current
    return produceState(null, thumbnails, spot) {
        sheets.tile(api.sheetOf(thumbnails, spot.sheet), spot, thumbnails)?.let { value = it }
    }
}

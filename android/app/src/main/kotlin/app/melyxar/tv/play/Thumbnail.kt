package app.melyxar.tv.play

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapRegionDecoder
import android.graphics.Rect
import android.os.Build
import android.os.SystemClock
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
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
import app.melyxar.tv.probe.Probe
import coil3.ImageLoader
import coil3.request.CachePolicy
import coil3.request.ImageRequest
import coil3.request.allowHardware
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.withContext
import java.io.IOException
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
 *  they are in by the time the remote reaches them: their sheets all asked
 *  for at once, then each thumbnail cut. */
@Composable
fun FetchingThumbnails(thumbnails: Thumbnails, moments: List<Double>) {
    val api = LocalServer.current
    val sheets = LocalSheets.current
    LaunchedEffect(sheets, thumbnails, moments) {
        val spots = moments.mapNotNull { thumbnails.spotAt(it) }.distinct()
        spots.map { it.sheet }.distinct().forEach { sheets.open(api.sheetOf(thumbnails, it)) }
        spots.forEach { sheets.tile(api.sheetOf(thumbnails, it.sheet), it, thumbnails) }
    }
}

/** Every sheet of [thumbnails] made ready to be cut from: going anywhere on
 *  the bar then costs only the cutting of one thumbnail. */
@Composable
fun FetchingSheets(thumbnails: Thumbnails) {
    val api = LocalServer.current
    val sheets = LocalSheets.current
    LaunchedEffect(sheets, thumbnails) {
        repeat(thumbnails.sheetsCounted()) { sheets.open(api.sheetOf(thumbnails, it)) }
    }
}

/** How many sheets the thumbnails take. */
fun Thumbnails.sheetsCounted(): Int = (columns * rows).takeIf { it > 0 }?.let { (counted + it - 1) / it } ?: 0

private fun Api.sheetOf(thumbnails: Thumbnails, sheet: Int): String = address("${thumbnails.url}/$sheet.jpg")

/** How many thumbnails are kept cut out: a page's row of chapters and the
 *  player's, with the bar's last ones. */
private const val TILES_KEPT = 48

/** How wide a sheet brought onto the device is read, the reading being only
 *  the way it lands there. */
private const val BROUGHT_OVER_PX = 64

/** A map keeping only the [most] last used of its entries. */
private fun <K, V> lastUsed(most: Int) = object : LinkedHashMap<K, V>(most, 0.75f, true) {
    override fun removeEldestEntry(eldest: MutableMap.MutableEntry<K, V>) = size > most
}

/**
 * The sheets the thumbnails are cut from, and the thumbnails last shown.
 *
 * A sheet is kept as its file, a reader of it ready to draw out any one
 * piece: drawing out the whole sheet to cut one thumbnail held the bar's
 * picture for as long as a sheet took to decode each time the bar crossed
 * into another, where drawing out a piece takes a few milliseconds. Every
 * sheet of a film is kept so, a few megabytes in all, and only the little
 * pictures ever go to the graphics card.
 */
private class Sheets(private val pictures: ImageLoader, private val context: Context, private val scope: CoroutineScope) {
    private val readers = HashMap<String, Deferred<BitmapRegionDecoder?>>()
    private val tiles = lastUsed<Pair<String, Spot>, ImageBitmap>(TILES_KEPT)

    /** The thumbnail at [spot] of the sheet at [address], cut unless it is
     *  kept, once that sheet is in. */
    suspend fun tile(address: String, spot: Spot, thumbnails: Thumbnails): ImageBitmap? = tiles[address to spot] ?: run {
        val reader = open(address).await() ?: return null
        val began = SystemClock.uptimeMillis()
        withContext(Dispatchers.IO) { cutOut(reader, spot, thumbnails) }?.also {
            tiles[address to spot] = it
            Probe.thumbnailCut(SystemClock.uptimeMillis() - began)
        }
    }

    /** The sheet at [address] made ready to be cut from, unless it is or is
     *  on its way: brought onto the device, then its file read. */
    fun open(address: String): Deferred<BitmapRegionDecoder?> = readers.getOrPut(address) {
        scope.async {
            val began = SystemClock.uptimeMillis()
            val bytes = bytesOf(address)
            val reader = bytes?.let { withContext(Dispatchers.IO) { readerOf(it) } }
            Probe.sheetOpened(address.substringAfterLast('/'), bytes?.size, SystemClock.uptimeMillis() - began)
            // One that did not come is asked for again the next time.
            if (reader == null) readers.remove(address)
            reader
        }
    }

    /** The sheet's file, from the pictures' store on the device, brought
     *  there first unless it already is. */
    private suspend fun bytesOf(address: String): ByteArray? {
        pictures.execute(
            ImageRequest.Builder(context)
                .data(address)
                .diskCacheKey(address)
                .size(BROUGHT_OVER_PX)
                .allowHardware(false)
                .memoryCachePolicy(CachePolicy.DISABLED)
                .build(),
        )
        val store = pictures.diskCache ?: return null
        return withContext(Dispatchers.IO) {
            store.openSnapshot(address)?.use { kept -> store.fileSystem.read(kept.data) { readByteArray() } }
        }
    }

    /** Lets go of the sheets' readers, the screen showing them gone. */
    @OptIn(ExperimentalCoroutinesApi::class)
    fun close() {
        readers.values.forEach { reader -> if (reader.isCompleted) reader.getCompleted()?.recycle() else reader.cancel() }
        readers.clear()
    }
}

private fun readerOf(bytes: ByteArray): BitmapRegionDecoder? = try {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        BitmapRegionDecoder.newInstance(bytes, 0, bytes.size)
    } else {
        @Suppress("DEPRECATION")
        BitmapRegionDecoder.newInstance(bytes, 0, bytes.size, false)
    }
} catch (_: IOException) {
    null
}

/** The thumbnail at [spot], drawn out of its sheet alone, in the sheet's own
 *  pixels, which may be fewer than the thumbnails say: or the cut slides
 *  into the next picture down. Nothing once the sheet is let go of. */
private fun cutOut(sheet: BitmapRegionDecoder, spot: Spot, thumbnails: Thumbnails): ImageBitmap? {
    val kept = sheet.width.toFloat() / (thumbnails.columns * thumbnails.width)
    val keptDown = sheet.height.toFloat() / (thumbnails.rows * thumbnails.height)
    val left = (spot.column * thumbnails.width * kept).roundToInt()
    val top = (spot.row * thumbnails.height * keptDown).roundToInt()
    val right = (left + (thumbnails.width * kept).roundToInt()).coerceAtMost(sheet.width)
    val bottom = (top + (thumbnails.height * keptDown).roundToInt()).coerceAtMost(sheet.height)
    val drawn = try {
        sheet.decodeRegion(Rect(left, top, right, bottom), BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.RGB_565 })
    } catch (_: IllegalStateException) {
        null
    } ?: return null
    // Handed to the graphics card now, rather than in the frame that shows it.
    drawn.prepareToDraw()
    return drawn.asImageBitmap()
}

private val LocalSheets = staticCompositionLocalOf<Sheets> { error("no sheets") }

/** [content] with the sheets its thumbnails are cut from kept, for as long
 *  as it is shown and no longer. */
@Composable
fun KeepingSheets(content: @Composable () -> Unit) {
    val pictures = LocalPictures.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val sheets = remember(pictures) { Sheets(pictures, context, scope) }
    DisposableEffect(sheets) { onDispose { sheets.close() } }
    CompositionLocalProvider(LocalSheets provides sheets, content = content)
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

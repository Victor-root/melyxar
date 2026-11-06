package app.melyxar.tv.look

import android.content.Context
import android.os.SystemClock
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import app.melyxar.server.Api
import app.melyxar.server.Picture
import app.melyxar.server.fitting
import app.melyxar.tv.probe.Part
import app.melyxar.tv.probe.PictureProbe
import app.melyxar.tv.probe.Probe
import app.melyxar.tv.probe.probed
import coil3.ImageLoader
import coil3.bitmapFactoryMaxParallelism
import coil3.compose.AsyncImage
import coil3.memory.MemoryCache
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.ImageRequest
import coil3.request.crossfade
import kotlin.math.roundToInt
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope

/** The share of the app's memory the pictures already drawn are kept in:
 *  the home screen's alone fill the usual fifth. */
private const val PICTURES_KEPT = 0.35

/** The most pictures decoded at once, what the picture loader takes alone. */
private const val MOST_AT_ONCE = 4

/** How many pictures are decoded at once: as many as leaves two cores to
 *  the screen, whose first drawing otherwise waits behind a row of them. */
private fun picturesAtOnce() = (Runtime.getRuntime().availableProcessors() - 2).coerceIn(1, MOST_AT_ONCE)

/** The server the screens are drawn from, and the pictures fetched from it
 *  with its session. */
val LocalServer = staticCompositionLocalOf<Api> { error("no server") }
val LocalPictures = staticCompositionLocalOf<ImageLoader> { error("no pictures") }

/** What fetches a server's pictures, with the session it is asked with. */
@Composable
fun rememberPictures(api: Api): ImageLoader {
    val context = LocalContext.current
    return remember(api) {
        ImageLoader.Builder(context)
            .components { add(OkHttpNetworkFetcherFactory(callFactory = { api.http })) }
            .eventListener(PictureProbe)
            .memoryCache { MemoryCache.Builder().maxSizePercent(context, PICTURES_KEPT).build() }
            .bitmapFactoryMaxParallelism(picturesAtOnce())
            .build()
    }
}

/** A work's pictures, and how wide they are drawn. */
data class Drawn(val pictures: List<Picture>, val across: Dp)

/** The share of the screen's pixels a picture is fetched for, drawn
 *  [Lighter]: often the server's next size down, on a 1080p screen a poster
 *  200 pixels across rather than 400, a quarter of the pixels to fetch,
 *  decode and hand to the graphics card, and softer under the eye. */
private const val LIGHTER_PIXELS = 2f / 3f

/** How many pixels across a picture drawn [across] wide is fetched for. */
private fun Density.pixelsFor(across: Dp): Int = (across.toPx() * if (Lighter.on) LIGHTER_PIXELS else 1f).roundToInt()

/** Where the picture of [pictures] drawn [pixels] wide is fetched from: the
 *  smallest the server made that has a pixel for every pixel of the screen
 *  it covers. */
private fun Api.addressOf(pictures: List<Picture>, pixels: Int): String? = pictures.fitting(pixels)?.let { address(it.url) }

/**
 * Fetches [drawn] into memory before the screen draws it, the picture
 * ready for the graphics card: drawn then, it is there at once, with no
 * fade and nothing left to decode or hand to the graphics card.
 */
@Composable
fun rememberFetchAhead(): suspend (List<Drawn>) -> Unit {
    val api = LocalServer.current
    val pictures = LocalPictures.current
    val context = LocalContext.current
    val density = LocalDensity.current
    return remember<suspend (List<Drawn>) -> Unit>(api, pictures, context, density) {
        { drawn -> if (Probe.shows(Part.PicturesAhead)) pictures.fetchAhead(context, api, density, drawn) }
    }
}

private suspend fun ImageLoader.fetchAhead(context: Context, api: Api, density: Density, drawn: List<Drawn>) {
    val began = SystemClock.uptimeMillis()
    val addresses = drawn.mapNotNull { api.addressOf(it.pictures, with(density) { pixelsFor(it.across) }) }.distinct()
    coroutineScope {
        addresses.map { async { execute(ImageRequest.Builder(context).data(it).build()) } }.awaitAll()
    }
    Probe.fetchedAhead(addresses.size, SystemClock.uptimeMillis() - began, memoryCache)
}

/**
 * One of a work's pictures at the size it is drawn, [across] wide, faded in
 * unless it was already in memory.
 */
@Composable
fun ServerPicture(
    pictures: List<Picture>,
    across: Dp,
    modifier: Modifier = Modifier,
    contentScale: ContentScale = ContentScale.Crop,
    alignment: Alignment = Alignment.Center,
) {
    val api = LocalServer.current
    val context = LocalContext.current
    val pixels = with(LocalDensity.current) { pixelsFor(across) }
    val address = remember(api, pictures, pixels) { api.addressOf(pictures, pixels) } ?: return
    val fades = Probe.shows(Part.PictureFade) && !Lighter.on
    AsyncImage(
        model = remember(address, fades) {
            ImageRequest.Builder(context).data(address).crossfade(if (fades) 240 else 0).build()
        },
        imageLoader = LocalPictures.current,
        contentDescription = null,
        contentScale = contentScale,
        alignment = alignment,
        modifier = modifier.probed(Part.Pictures),
    )
}

package app.melyxar.tv.look

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import app.melyxar.server.Api
import app.melyxar.server.Picture
import app.melyxar.server.fitting
import coil3.ImageLoader
import coil3.compose.AsyncImage
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import coil3.request.ImageRequest
import coil3.request.crossfade

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
            .build()
    }
}

/**
 * One of a work's pictures at the size it is drawn, [across] wide: the
 * smallest the server made that has a pixel for every pixel of the screen it
 * covers, faded in.
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
    val pixels = with(LocalDensity.current) { across.roundToPx() }
    val chosen = remember(pictures, pixels) { pictures.fitting(pixels) } ?: return
    AsyncImage(
        model = remember(chosen) {
            ImageRequest.Builder(context).data(api.address(chosen.url)).crossfade(240).build()
        },
        imageLoader = LocalPictures.current,
        contentDescription = null,
        contentScale = contentScale,
        alignment = alignment,
        modifier = modifier,
    )
}

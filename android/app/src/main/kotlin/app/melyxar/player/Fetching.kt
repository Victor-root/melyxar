package app.melyxar.player

import android.webkit.CookieManager
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource

/** Above the server's own patience with a piece of film or a song it is
 *  still making, as the page's own player is. */
private const val PATIENCE_MS = 40_000

/**
 * Everything the application's players fetch from the server, fetched as the
 * page fetches it: signed in with the page's cookie, under the page's name.
 */
@OptIn(UnstableApi::class)
fun serverFetching(userAgent: String): DataSource.Factory = ResolvingDataSource.Factory(
    DefaultHttpDataSource.Factory()
        .setUserAgent(userAgent)
        .setConnectTimeoutMs(PATIENCE_MS)
        .setReadTimeoutMs(PATIENCE_MS),
) { asked ->
    CookieManager.getInstance().getCookie(asked.uri.toString())
        ?.let { asked.withAdditionalHeaders(mapOf("Cookie" to it)) }
        ?: asked
}

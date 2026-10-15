package app.melyxar.server

import android.os.Build
import app.melyxar.BuildConfig
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

/** How long a server is given to be reached at all. */
private const val REACH_WITHIN_S = 6L

/** Above the server's own patience with a piece of film it is still making,
 *  as the page's player is. */
private const val PATIENCE_S = 40L

/**
 * The one client every server is asked through, its connections shared by
 * the calls, the pictures and the films. It says it is Melyxar on a
 * television, which is what the server's list of devices shows.
 */
val network: OkHttpClient by lazy {
    val said = "MelyxarTV/${BuildConfig.VERSION_NAME} (Linux; Android ${Build.VERSION.RELEASE}; ${Build.MODEL}) SmartTV"
    OkHttpClient.Builder()
        .connectTimeout(REACH_WITHIN_S, TimeUnit.SECONDS)
        .readTimeout(PATIENCE_S, TimeUnit.SECONDS)
        .addInterceptor { chain -> chain.proceed(chain.request().newBuilder().header("User-Agent", said).build()) }
        .build()
}

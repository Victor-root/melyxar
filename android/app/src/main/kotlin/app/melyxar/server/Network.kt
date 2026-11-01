package app.melyxar.server

import android.content.Context
import android.os.Build
import android.webkit.CookieManager
import android.webkit.WebSettings
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

/** What the phone application calls itself, its page and its players alike:
 *  the web view's own name, and Melyxar's after it, which is how the server
 *  tells the application from a browser. */
fun appUserAgent(context: Context): String = "${WebSettings.getDefaultUserAgent(context)} MelyxarApp/${BuildConfig.VERSION_NAME}"

/**
 * The client the phone application asks its server through away from the
 * page, as the page: under its name, signed in with its cookie, read at each
 * call, so a session the page opens or closes counts at once.
 */
fun pageClient(context: Context): OkHttpClient {
    val said = appUserAgent(context)
    return network.newBuilder()
        .addInterceptor { chain ->
            val asked = chain.request().newBuilder().header("User-Agent", said)
            CookieManager.getInstance().getCookie(chain.request().url.toString())?.let { asked.header("Cookie", it) }
            chain.proceed(asked.build())
        }
        .build()
}

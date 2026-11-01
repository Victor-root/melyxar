package app.melyxar.server

import android.content.Context
import android.webkit.CookieManager

/**
 * The server the phone application opened last, asked as the account its
 * page is signed in as: for what asks it away from the page, the player of
 * music and the covers a car reads. Read again at each ask, so a session the
 * page opens or closes, or another server opened, counts at once.
 */
object PageSession {
    @Volatile
    private var asked: Api? = null

    /** Nothing before anybody has signed in on that server. */
    fun api(context: Context): Api? {
        val origin = KnownServers(context).current()?.origin ?: return null
        val cookie = CookieManager.getInstance().getCookie(origin) ?: return null
        if (cookie.split(';').none { it.trim().startsWith("$SESSION_COOKIE=") }) {
            return null
        }
        asked?.takeIf { it.origin.value == origin }?.let { return it }
        return Api(ServerOrigin(origin), pageClient(context.applicationContext), token = null).also { asked = it }
    }
}

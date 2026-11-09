package app.melyxar.web

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.webkit.WebViewCompat

/**
 * The engine the device draws web pages with, which draws the interface.
 *
 * It is a part of the device rather than of this application, brought up to
 * date by the store on most devices and never on some: a phone whose system
 * was replaced, or whose store is gone, can keep one years old. The
 * interface is written for the engines of today, and an old one draws it
 * broken, the sizes worked out to nothing or to the whole screen, with
 * nothing to say why. So its version is read before the interface is shown.
 */
object WebEngine {
    /** The oldest that draws the interface: the first to mix two colours in
     *  a style sheet, which every colour of the interface is made with. */
    const val OLDEST_DRAWING_IT = 111

    /** The engine's major version, or nothing when the device has none or
     *  does not say. */
    fun majorVersion(context: Context): Int? =
        WebViewCompat.getCurrentWebViewPackage(context)?.versionName?.let(::majorOf)

    fun majorOf(versionName: String): Int? = versionName.substringBefore('.').toIntOrNull()

    fun isTooOld(major: Int?) = major != null && major < OLDEST_DRAWING_IT

    /** Its page in the store, where it is brought up to date. */
    fun storePage(): Intent = Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=$GOOGLE_ENGINE"))

    /** The same page on the web, for a device whose store does not take the
     *  first. */
    fun webStorePage(): Intent =
        Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=$GOOGLE_ENGINE"))

    private const val GOOGLE_ENGINE = "com.google.android.webview"
}

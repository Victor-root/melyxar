package app.melyxar.web

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.webkit.JavascriptInterface
import androidx.core.content.edit
import app.melyxar.theme.OFFERED_ACCENTS
import app.melyxar.theme.USUAL_ACCENT
import app.melyxar.theme.nearestOffered
import app.melyxar.theme.parseHex

/** The grounds the logo can sit on in the launcher, by the name the page
 *  knows them under: the ground of the TV's banner, or nothing at all. The
 *  one nothing sits on first, since it is read first when two are on. */
private val GROUNDS = listOf("clear", "full")

/** The icon shown until one is chosen, the one the manifest leaves on. */
private const val USUAL_GROUND = "full"
private val USUAL_TINT = OFFERED_ACCENTS.first()

private const val KEPT = "launcher_icon"
private const val FOLLOWS = "follows"

/**
 * The icon of the launcher, as the page's `app-icon.ts` reads and chooses
 * it. Each ground in each offered accent is an alias of the door in the
 * manifest: the chosen one is turned on and the others off, which the
 * launcher follows by itself, closing the application on many phones: the
 * page asks first and changes it only when told to. An icon that follows the
 * account wears the offered accent nearest to the account's own; one that
 * does not wears the usual red.
 */
class LauncherIcons(private val context: Context) {
    private val kept = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE)

    private fun of(ground: String, tint: String) = ComponentName(
        context.packageName,
        "app.melyxar.connect.Icon${ground.replaceFirstChar(Char::uppercaseChar)}${tint.removePrefix("#").uppercase()}",
    )

    private fun isOn(ground: String, tint: String): Boolean =
        when (context.packageManager.getComponentEnabledSetting(of(ground, tint))) {
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED -> true
            PackageManager.COMPONENT_ENABLED_STATE_DEFAULT -> ground == USUAL_GROUND && tint == USUAL_TINT
            else -> false
        }

    private fun shown(): Pair<String, String> = GROUNDS.firstNotNullOfOrNull { ground ->
        OFFERED_ACCENTS.firstOrNull { isOn(ground, it) }?.let { ground to it }
    } ?: (USUAL_GROUND to USUAL_TINT)

    private fun tintFor(follows: Boolean, accent: String): String =
        if (follows) nearestOffered(parseHex(accent) ?: USUAL_ACCENT) else USUAL_TINT

    @JavascriptInterface
    fun ground(): String = shown().first

    @JavascriptInterface
    fun follows(): Boolean = kept.getBoolean(FOLLOWS, false)

    /** Whether applying these would show another icon than the one shown. */
    @JavascriptInterface
    fun differs(ground: String, follows: Boolean, accent: String): Boolean =
        ground in GROUNDS && shown() != ground to tintFor(follows, accent)

    /** Keeps whether the logo follows the account, and shows the icon
     *  these make when it is not the one shown. */
    @JavascriptInterface
    fun apply(ground: String, follows: Boolean, accent: String) {
        if (ground !in GROUNDS) {
            return
        }
        kept.edit { putBoolean(FOLLOWS, follows) }
        if (!differs(ground, follows, accent)) {
            return
        }
        showOnly(ground to tintFor(follows, accent))
    }

    /**
     * Leaves one icon on when several are. The white and the dark grounds
     * were dropped for the banner's: a phone that had chosen the logo alone
     * finds the new usual icon on beside it, since nothing ever turned that
     * one off, and keeps the logo alone.
     */
    fun settle() {
        val on = GROUNDS.sumOf { ground -> OFFERED_ACCENTS.count { isOn(ground, it) } }
        if (on > 1) {
            showOnly(shown())
        }
    }

    private fun showOnly(icon: Pair<String, String>) {
        GROUNDS.forEach { each ->
            OFFERED_ACCENTS.forEach { offered ->
                context.packageManager.setComponentEnabledSetting(
                    of(each, offered),
                    if (each to offered == icon) {
                        PackageManager.COMPONENT_ENABLED_STATE_ENABLED
                    } else {
                        PackageManager.COMPONENT_ENABLED_STATE_DISABLED
                    },
                    PackageManager.DONT_KILL_APP,
                )
            }
        }
    }
}

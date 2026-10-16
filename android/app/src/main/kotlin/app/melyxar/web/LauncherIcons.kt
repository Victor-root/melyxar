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
 *  knows them under: white, the dark ground, or nothing at all. */
private val GROUNDS = listOf("light", "dark", "clear")

/** The icon shown until one is chosen, the one the manifest leaves on. */
private const val USUAL_GROUND = "light"
private val USUAL_TINT = OFFERED_ACCENTS.first()

private const val KEPT = "launcher_icon"
private const val FOLLOWS = "follows"
private const val ACCENT = "accent"

/**
 * The icon of the launcher, as the page's `app-icon.ts` reads and chooses
 * it. Each ground in each offered accent is an alias of the door in the
 * manifest: the chosen one is turned on and the others off, which the
 * launcher follows by itself. An icon that follows the account wears the
 * offered accent nearest to the account's own; one that does not wears the
 * usual red.
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

    private fun tint(): String =
        if (follows()) nearestOffered(parseHex(kept.getString(ACCENT, null).orEmpty()) ?: USUAL_ACCENT) else USUAL_TINT

    private fun show(ground: String, tint: String) {
        if (shown() == ground to tint) {
            return
        }
        GROUNDS.forEach { each ->
            OFFERED_ACCENTS.forEach { offered ->
                val on = each == ground && offered == tint
                context.packageManager.setComponentEnabledSetting(
                    of(each, offered),
                    if (on) PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                    PackageManager.DONT_KILL_APP,
                )
            }
        }
    }

    @JavascriptInterface
    fun ground(): String = shown().first

    @JavascriptInterface
    fun follows(): Boolean = kept.getBoolean(FOLLOWS, false)

    @JavascriptInterface
    fun choose(ground: String, follows: Boolean) {
        if (ground !in GROUNDS) {
            return
        }
        kept.edit { putBoolean(FOLLOWS, follows) }
        show(ground, tint())
    }

    /** The account's accent, given whenever it is known or changes. */
    @JavascriptInterface
    fun wear(accent: String) {
        kept.edit { putString(ACCENT, accent) }
        show(ground(), tint())
    }
}

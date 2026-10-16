package app.melyxar.web

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.webkit.JavascriptInterface

/** The icons the phone's launcher can show for Melyxar, each an alias of the
 *  door in the manifest, by the name the page knows it under: the logo on
 *  white, on the dark ground, or alone with nothing behind it. */
private val ICONS = mapOf(
    "light" to "app.melyxar.connect.IconLight",
    "dark" to "app.melyxar.connect.IconDark",
    "clear" to "app.melyxar.connect.IconClear",
)

/** The icon shown until one is chosen, the one the manifest leaves on. */
private const val USUAL = "light"

/**
 * The icon of the launcher, as the page's `app-icon.ts` reads and chooses
 * it: the chosen alias is turned on and the others off, which the launcher
 * follows by itself. Nothing but the names above can be chosen.
 */
class LauncherIcons(private val context: Context) {
    private fun of(alias: String) = ComponentName(context.packageName, alias)

    @JavascriptInterface
    fun current(): String = ICONS.entries.firstOrNull { (_, alias) ->
        when (context.packageManager.getComponentEnabledSetting(of(alias))) {
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED -> true
            PackageManager.COMPONENT_ENABLED_STATE_DEFAULT -> alias == ICONS[USUAL]
            else -> false
        }
    }?.key ?: USUAL

    @JavascriptInterface
    fun choose(name: String) {
        if (name !in ICONS || name == current()) {
            return
        }
        ICONS.forEach { (each, alias) ->
            val state = if (each == name) PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DISABLED
            context.packageManager.setComponentEnabledSetting(of(alias), state, PackageManager.DONT_KILL_APP)
        }
    }
}

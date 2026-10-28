package app.melyxar.connect

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import app.melyxar.theme.OFFERED_ACCENTS
import app.melyxar.theme.nearestOffered
import app.melyxar.theme.parseHex
import app.melyxar.theme.USUAL_ACCENT

/** The tint the logo wears: the offered accent nearest to the account's when
 *  it follows the account, the usual red when it does not. */
fun doorTint(follows: Boolean, accent: String): String =
    if (follows) nearestOffered(parseHex(accent) ?: USUAL_ACCENT) else OFFERED_ACCENTS.first()

/**
 * The aliases of the door the manifest declares for one launcher, by their
 * class names, [usual] the one it leaves on. The chosen one is turned on and
 * the others off, which the launcher follows by itself, closing the
 * application on many devices.
 */
class DoorAliases(private val context: Context, private val names: List<String>, private val usual: String) {
    private fun of(name: String) = ComponentName(context.packageName, "app.melyxar.connect.$name")

    fun isOn(name: String): Boolean = when (context.packageManager.getComponentEnabledSetting(of(name))) {
        PackageManager.COMPONENT_ENABLED_STATE_ENABLED -> true
        PackageManager.COMPONENT_ENABLED_STATE_DEFAULT -> name == usual
        else -> false
    }

    /** The first of the aliases on, in the order given, or the usual one. */
    fun shown(): String = names.firstOrNull(::isOn) ?: usual

    fun showOnly(shown: String) {
        names.forEach { name ->
            context.packageManager.setComponentEnabledSetting(
                of(name),
                if (name == shown) PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                PackageManager.DONT_KILL_APP,
            )
        }
    }

    /** Leaves one alias on when several are. */
    fun settle() {
        if (names.count(::isOn) > 1) {
            showOnly(shown())
        }
    }
}

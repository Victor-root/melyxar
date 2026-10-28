package app.melyxar.tv.door

import android.content.Context
import androidx.core.content.edit
import app.melyxar.connect.DoorAliases
import app.melyxar.connect.doorTint
import app.melyxar.theme.OFFERED_ACCENTS

private const val KEPT = "television"
private const val FOLLOWS = "banner_follows"

private fun aliasOf(tint: String) = "Banner${tint.removePrefix("#").uppercase()}"

/**
 * The banner the television's launcher shows, as the phone's icon: one alias
 * of the door for each offered accent, the usual red unless the banner
 * follows the account, when it wears the offered accent nearest to the
 * account's own. Changing it can close the application, so it changes only
 * when chosen in the settings, never on its own when the accent does.
 */
class Banners(context: Context) {
    private val kept = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE)
    private val aliases = DoorAliases(context, OFFERED_ACCENTS.map(::aliasOf), aliasOf(OFFERED_ACCENTS.first()))

    fun follows(): Boolean = kept.getBoolean(FOLLOWS, false)

    /** Whether choosing these would show another banner than the one shown. */
    fun differs(follows: Boolean, accent: String): Boolean = aliases.shown() != aliasOf(doorTint(follows, accent))

    fun choose(follows: Boolean, accent: String) {
        kept.edit { putBoolean(FOLLOWS, follows) }
        if (differs(follows, accent)) {
            aliases.showOnly(aliasOf(doorTint(follows, accent)))
        }
    }
}

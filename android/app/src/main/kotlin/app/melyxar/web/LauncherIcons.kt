package app.melyxar.web

import android.content.Context
import android.webkit.JavascriptInterface
import androidx.core.content.edit
import app.melyxar.connect.DoorAliases
import app.melyxar.connect.doorTint
import app.melyxar.theme.OFFERED_ACCENTS

/** The grounds the logo can sit on in the launcher, by the name the page
 *  knows them under: the ground of the TV's banner, or nothing at all. The
 *  one nothing sits on first, since it is read first when two are on. */
private val GROUNDS = listOf("clear", "full")

/** The icon shown until one is chosen, the one the manifest leaves on. */
private const val USUAL_GROUND = "full"

private const val KEPT = "launcher_icon"
private const val FOLLOWS = "follows"

private fun aliasOf(ground: String, tint: String) =
    "Icon${ground.replaceFirstChar(Char::uppercaseChar)}${tint.removePrefix("#").uppercase()}"

/**
 * The icon of the launcher, as the page's `app-icon.ts` reads and chooses
 * it. Each ground in each offered accent is an alias of the door in the
 * manifest: the page asks first and changes it only when told to, since
 * changing it closes the application on many phones. An icon that follows
 * the account wears the offered accent nearest to the account's own; one
 * that does not wears the usual red.
 */
class LauncherIcons(context: Context) {
    private val kept = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE)
    private val icons = GROUNDS.flatMap { ground -> OFFERED_ACCENTS.map { ground to it } }
    private val aliases = DoorAliases(
        context,
        icons.map { (ground, tint) -> aliasOf(ground, tint) },
        aliasOf(USUAL_GROUND, OFFERED_ACCENTS.first()),
    )

    private fun shown(): Pair<String, String> = aliases.shown().let { alias -> icons.first { aliasOf(it.first, it.second) == alias } }

    @JavascriptInterface
    fun ground(): String = shown().first

    @JavascriptInterface
    fun follows(): Boolean = kept.getBoolean(FOLLOWS, false)

    /** Whether applying these would show another icon than the one shown. */
    @JavascriptInterface
    fun differs(ground: String, follows: Boolean, accent: String): Boolean =
        ground in GROUNDS && shown() != ground to doorTint(follows, accent)

    /** Keeps whether the logo follows the account, and shows the icon
     *  these make when it is not the one shown. */
    @JavascriptInterface
    fun apply(ground: String, follows: Boolean, accent: String) {
        if (ground !in GROUNDS) {
            return
        }
        kept.edit { putBoolean(FOLLOWS, follows) }
        if (differs(ground, follows, accent)) {
            aliases.showOnly(aliasOf(ground, doorTint(follows, accent)))
        }
    }

    /**
     * Leaves one icon on when several are. The white and the dark grounds
     * were dropped for the banner's: a phone that had chosen the logo alone
     * finds the new usual icon on beside it, since nothing ever turned that
     * one off, and keeps the logo alone.
     */
    fun settle() = aliases.settle()
}

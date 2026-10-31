package app.melyxar.web

import android.content.Context
import android.content.Intent
import android.webkit.JavascriptInterface
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import app.melyxar.R
import app.melyxar.connect.ConnectActivity
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

/** One shortcut as the page offers it (web/src/app-shortcuts.ts). */
@Serializable
private data class Offered(val id: String, val label: String, val path: String)

/** The picture of each shortcut the page may offer; any other is refused. */
private val PICTURES = mapOf(
    "music" to R.drawable.shortcut_music,
    "dashboard" to R.drawable.shortcut_dashboard,
    "settings" to R.drawable.shortcut_settings,
    "link" to R.drawable.shortcut_scan,
)

private val reading = Json { ignoreUnknownKeys = true }

/**
 * What a long press on the application's icon offers, as the page says it:
 * the page knows the account's language, its rights and its libraries. Each
 * opens a page of the server the application is on, and only of it: a path
 * that is not one of its own is refused.
 */
class Shortcuts(private val context: Context) {
    @JavascriptInterface
    fun offer(said: String) {
        val offered = try {
            reading.decodeFromString(ListSerializer(Offered.serializer()), said)
        } catch (_: SerializationException) {
            return
        }
        val shortcuts = offered
            .filter { it.id in PICTURES && it.path.startsWith("/") && !it.path.startsWith("//") }
            .mapIndexed { rank, shortcut ->
                ShortcutInfoCompat.Builder(context, shortcut.id)
                    .setShortLabel(shortcut.label)
                    .setIcon(IconCompat.createWithResource(context, PICTURES.getValue(shortcut.id)))
                    .setIntent(
                        Intent(context, ConnectActivity::class.java)
                            .setAction(OPEN_A_PAGE)
                            .putExtra(WebActivity.PATH, shortcut.path),
                    )
                    .setRank(rank)
                    .build()
            }
        ShortcutManagerCompat.setDynamicShortcuts(context, shortcuts)
    }

    companion object {
        /** What a shortcut asks of the application: a page of its server. */
        const val OPEN_A_PAGE = "app.melyxar.OPEN_A_PAGE"
    }
}

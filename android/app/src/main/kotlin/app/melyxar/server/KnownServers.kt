package app.melyxar.server

import android.content.Context
import androidx.core.content.edit
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

/** A server this device has been to, under the name it gave itself. */
@Serializable
data class KnownServer(val origin: String, val name: String)

/**
 * The servers this device knows, the one last opened first, kept on the
 * device. Opening the application goes straight back to that one.
 */
class KnownServers(context: Context) {
    private val kept = context.getSharedPreferences("servers", Context.MODE_PRIVATE)

    fun all(): List<KnownServer> {
        val written = kept.getString(LIST, null) ?: return emptyList()
        return try {
            Json.decodeFromString<List<KnownServer>>(written)
        } catch (_: SerializationException) {
            emptyList()
        }
    }

    /** The server to open on, when there is one. */
    fun current(): KnownServer? = all().firstOrNull()

    /** Puts a server first, as the one to open on from now. */
    fun open(server: KnownServer) = save(listOf(server) + all().filter { it.origin != server.origin })

    fun forget(origin: String) = save(all().filter { it.origin != origin })

    private fun save(servers: List<KnownServer>) = kept.edit { putString(LIST, Json.encodeToString(servers)) }

    private companion object {
        const val LIST = "list"
    }
}

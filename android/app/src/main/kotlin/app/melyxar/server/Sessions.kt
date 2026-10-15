package app.melyxar.server

import android.content.Context
import androidx.core.content.edit
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import java.util.UUID

/** An account signed in on this device, by the name it signs in with. */
@Serializable
data class KeptSession(val name: String, val token: String)

/**
 * The accounts signed in on this device, server by server, each keeping its
 * own session: choosing a profile again opens it without its password. Kept
 * on the device only, as a browser keeps its cookie.
 */
class Sessions(context: Context) {
    private val kept = context.getSharedPreferences("sessions", Context.MODE_PRIVATE)

    /** What this device calls itself to every server, made once: signing in
     *  again from it replaces the account's session here rather than adding
     *  one. */
    val client: String
        get() = kept.getString(CLIENT, null) ?: UUID.randomUUID().toString().also { made ->
            kept.edit { putString(CLIENT, made) }
        }

    fun of(origin: ServerOrigin): List<KeptSession> {
        val written = kept.getString(origin.value, null) ?: return emptyList()
        return try {
            Json.decodeFromString<List<KeptSession>>(written)
        } catch (_: SerializationException) {
            emptyList()
        }
    }

    fun keep(origin: ServerOrigin, session: KeptSession) =
        save(origin, of(origin).filter { it.name != session.name } + session)

    fun forget(origin: ServerOrigin, name: String) = save(origin, of(origin).filter { it.name != name })

    private fun save(origin: ServerOrigin, sessions: List<KeptSession>) =
        kept.edit { putString(origin.value, Json.encodeToString(sessions)) }

    private companion object {
        const val CLIENT = "client"
    }
}

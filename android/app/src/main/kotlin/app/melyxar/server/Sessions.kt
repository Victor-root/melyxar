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

/** Whom this device opens on as it starts, at one server's door. */
sealed interface Opening {
    /** "Who is watching?", every time. */
    data object Ask : Opening

    /** The profile last opened here. */
    data object Last : Opening

    /** Always the same profile. */
    data class Always(val name: String) : Opening
}

/** The profile to open on as the device starts, [last] being the one last
 *  opened here; none to ask. */
fun Opening.who(last: String?): String? = when (this) {
    Opening.Ask -> null
    Opening.Last -> last
    is Opening.Always -> name
}

/** How an opening is written down, and read back; anything unreadable asks. */
fun Opening.written(): String = when (this) {
    Opening.Ask -> "ask"
    Opening.Last -> "last"
    is Opening.Always -> "always:$name"
}

fun openingWritten(written: String?): Opening = when {
    written == "last" -> Opening.Last
    written?.startsWith("always:") == true -> Opening.Always(written.removePrefix("always:"))
    else -> Opening.Ask
}

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

    /** Keeps [session], the profile last opened here from now on. */
    fun keep(origin: ServerOrigin, session: KeptSession) {
        save(origin, of(origin).filter { it.name != session.name } + session)
        kept.edit { putString("$LAST${origin.value}", session.name) }
    }

    fun last(origin: ServerOrigin): String? = kept.getString("$LAST${origin.value}", null)

    fun opening(origin: ServerOrigin): Opening = openingWritten(kept.getString("$OPENING${origin.value}", null))

    fun openOn(origin: ServerOrigin, opening: Opening) =
        kept.edit { putString("$OPENING${origin.value}", opening.written()) }

    fun forget(origin: ServerOrigin, name: String) = save(origin, of(origin).filter { it.name != name })

    private fun save(origin: ServerOrigin, sessions: List<KeptSession>) =
        kept.edit { putString(origin.value, Json.encodeToString(sessions)) }

    private companion object {
        const val CLIENT = "client"
        const val LAST = "last:"
        const val OPENING = "opening:"
    }
}

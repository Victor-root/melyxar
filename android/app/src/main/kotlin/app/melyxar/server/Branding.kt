package app.melyxar.server

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.SerializationException
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URI

/** What a server says of itself before anybody signs in: the address every
 *  sign in screen draws from, open to all. */
@Serializable
data class Branding(
    @SerialName("server_name") val serverName: String,
    /** Where the administrator's own logo is, or nothing for Melyxar's. */
    val logo: String? = null,
    @SerialName("setup_complete") val setupComplete: Boolean = true,
)

/** How long a server is given to answer before it is said to be out of reach. */
private const val ANSWER_WITHIN_MS = 6_000

private val reading = Json { ignoreUnknownKeys = true }

/** Reads a server's answer about itself. */
fun brandingOf(body: String): Branding = reading.decodeFromString(Branding.serializer(), body)

/**
 * Asks a server what it is. Nothing back when it does not answer as a Melyxar
 * server does: out of reach, or something else at that address.
 */
suspend fun askBranding(origin: ServerOrigin): Branding? = withContext(Dispatchers.IO) {
    val connection = URI("$origin/api/v1/public/branding").toURL().openConnection() as HttpURLConnection
    try {
        connection.connectTimeout = ANSWER_WITHIN_MS
        connection.readTimeout = ANSWER_WITHIN_MS
        connection.setRequestProperty("Accept", "application/json")
        if (connection.responseCode != HttpURLConnection.HTTP_OK) {
            return@withContext null
        }
        val body = connection.inputStream.bufferedReader().use { it.readText() }
        brandingOf(body)
    } catch (_: IOException) {
        null
    } catch (_: SerializationException) {
        null
    } finally {
        connection.disconnect()
    }
}

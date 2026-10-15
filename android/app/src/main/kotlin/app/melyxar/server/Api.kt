package app.melyxar.server

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerializationException
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources
import java.io.IOException
import java.util.concurrent.TimeUnit

/** The server said no, or did not answer as a Melyxar server does. */
class Refusal(
    val status: Int,
    val code: String?,
    /** How long the server asks to wait before trying again, when it does. */
    val waitSeconds: Int? = null,
) : IOException("$status $code") {
    /** The session this was asked with is over. */
    val signedOut get() = status == 401
}

/** The server read with the names it writes, snake case, and whatever else
 *  it has started writing since left aside. */
@OptIn(ExperimentalSerializationApi::class)
val serverJson = Json {
    ignoreUnknownKeys = true
    explicitNulls = false
    coerceInputValues = true
    namingStrategy = JsonNamingStrategy.SnakeCase
}

/** The cookie the server keeps a session in. */
private const val SESSION_COOKIE = "melyxar_session"

private val JSON = "application/json".toMediaType()

/** What a television calls its browser in the server's lists of devices. */
private const val DEVICE_NAME = "Melyxar"

/** Below this many seconds in, a film is only said to be on, not where. */
private const val WORTH_KEEPING_S = 5.0

/** What the player asks of the subtitles. */
sealed interface TrackChoice {
    /** Whatever the account's way with subtitles gives. */
    data object Unsaid : TrackChoice

    data object Off : TrackChoice

    /** A subtitle of pictures, painted into the film by the server. */
    data class Painted(val id: String) : TrackChoice
}

/** What a player asks of a film: what the device plays, within the limits
 *  chosen, the tracks chosen, and what to do with its HDR for this film
 *  alone, the account's own choice when nothing is said. */
data class Asked(
    val profile: JsonElement,
    val audio: String?,
    val subtitle: TrackChoice,
    /** automatic, always_convert or never_convert. */
    val wideGamut: String?,
)

@Serializable
private data class SignIn(val name: String, val password: String, val remember: Boolean, val client: String)

@Serializable
private data class Failure(val code: String? = null, val details: Details? = null)

@Serializable
private data class Details(val seconds: Int? = null)

/**
 * One server, asked as one account, or as nobody before anybody has signed
 * in. Every picture and every film is fetched through [http] too, so all of
 * them carry the same session.
 */
class Api(val origin: ServerOrigin, private val base: OkHttpClient, val token: String?) {
    private val host = origin.value.toHttpUrl().host
    private val port = origin.value.toHttpUrl().port

    val http: OkHttpClient = base.newBuilder()
        .addInterceptor { chain ->
            val asked = chain.request()
            val ours = token != null && asked.url.host == host && asked.url.port == port
            chain.proceed(if (ours) asked.newBuilder().header("Cookie", "$SESSION_COOKIE=$token").build() else asked)
        }
        .build()

    /** Where a path of the server's is, written whole. */
    fun address(path: String): String = if ("://" in path) path else origin.value + path

    fun signedInAs(token: String?) = Api(origin, base, token)

    suspend fun names(): DoorNames = get("/api/v1/public/names", DoorNames.serializer())

    suspend fun branding(): Branding = get("/api/v1/public/branding", Branding.serializer())

    /** Signs in, and gives back the account with its session; a wrong name
     *  or password is a [Refusal] of 401. */
    suspend fun signIn(name: String, password: String, client: String): Pair<Account, String> =
        withContext(Dispatchers.IO) {
            val body = serverJson.encodeToString(SignIn.serializer(), SignIn(name, password, remember = true, client))
            http.newCall(request("/api/v1/session").post(body.asJson()).build()).execute().use { answer ->
                val account = read(answer, Account.serializer())
                val session = answer.headers("Set-Cookie")
                    .firstNotNullOfOrNull { cookie ->
                        cookie.substringBefore(';').takeIf { it.startsWith("$SESSION_COOKIE=") }?.substringAfter('=')
                    }
                    ?: throw Refusal(answer.code, "no_session")
                account to session
            }
        }

    suspend fun me(): Account = get("/api/v1/me", Account.serializer())

    /** Names this device in the account's list of devices. */
    suspend fun nameTheDevice() = send("PUT", "/api/v1/me/browser", buildJsonObject { put("browser", DEVICE_NAME) })

    suspend fun signOut() = send("DELETE", "/api/v1/session", null)

    suspend fun preferences(): Preferences = get("/api/v1/preferences", Preferences.serializer())

    suspend fun home(): Home = get("/api/v1/home", Home.serializer())

    suspend fun libraries(): List<Library> = get("/api/v1/libraries", ListSerializer(Library.serializer()))

    suspend fun person(id: String): Person = get("/api/v1/people/$id", Person.serializer())

    suspend fun filters(library: String): Filters = get("/api/v1/libraries/$library/filters", Filters.serializer())

    /** A page of the grid [browse] asks for, from after the card [after]. */
    suspend fun works(browse: Browse, after: String?, limit: Int): CardPage {
        val url = address("/api/v1/works").toHttpUrl().newBuilder()
            .apply {
                browse.library?.let { addQueryParameter("library", it) }
                browse.kind?.let { addQueryParameter("kind", it) }
                addQueryParameter("order", browse.order)
                if (browse.descending) addQueryParameter("descending", "true")
                browse.genre?.let { addQueryParameter("genre", it) }
                browse.decade?.let { addQueryParameter("decade", it.toString()) }
                browse.initial?.let { addQueryParameter("initial", it) }
                browse.search?.let { addQueryParameter("search", it) }
                if (browse.favourites) addQueryParameter("favourites", "true")
                if (browse.watchLater) addQueryParameter("watch_later", "true")
                after?.let { addQueryParameter("after", it) }
            }
            .addQueryParameter("limit", limit.toString())
            .build()
        return get(url.toString(), CardPage.serializer())
    }

    suspend fun work(id: String): Work = get("/api/v1/works/$id", Work.serializer())

    /** watched, favourite or watch-later, set on or off; the answer is what
     *  the server holds now. */
    suspend fun mark(work: String, mark: String, on: Boolean): Boolean {
        val field = mark.replace('-', '_')
        val answer = send("PUT", "/api/v1/works/$work/$mark", buildJsonObject { put(field, on) })
        return answer?.get(field)?.jsonPrimitive?.booleanOrNull == true
    }

    /**
     * How the server will hand this file over to a device that plays what
     * [profile] says, with these tracks, said by a player about to show it.
     */
    suspend fun plan(source: String, asked: Asked): Plan =
        post("/api/v1/playback/$source/plan", watched(asked), Plan.serializer())

    /** Opens a film the server rebuilds, starting at [from] seconds. */
    suspend fun openStream(source: String, asked: Asked, from: Double): StreamSession =
        post(
            "/api/v1/playback/$source/session",
            JsonObject(watched(asked) + ("start_at_seconds" to JsonPrimitive(from))),
            StreamSession.serializer(),
        )

    /** Tells the server a rebuilt film is still watched; false when it has
     *  gone, and a new one has to be opened. */
    suspend fun keepStream(id: String): Boolean = try {
        send("POST", "/api/v1/stream/$id", JsonObject(emptyMap()))
        true
    } catch (refused: Refusal) {
        if (refused.status !in 400..499) throw refused
        false
    }

    suspend fun closeStream(id: String) {
        send("DELETE", "/api/v1/stream/$id", null)
    }

    /** Where the film is and whether it stands still. Below a few seconds
     *  only that the player is there: a position worth keeping starts later. */
    suspend fun report(work: String, seconds: Double, paused: Boolean, leaving: Boolean): Heard {
        val path = if (seconds >= WORTH_KEEPING_S) "/api/v1/playback/progress" else "/api/v1/playback/watching"
        val body = buildJsonObject {
            put("work_id", work)
            put("position_seconds", if (seconds > 0) JsonPrimitive(seconds) else JsonNull)
            put("paused", paused)
            put("leaving", leaving)
        }
        return post(path, body, Heard.serializer())
    }

    /** Remembers the tracks chosen, so the next time starts the same way. */
    suspend fun rememberTracks(work: String, source: String, audio: String?, subtitle: String?) {
        send(
            "POST",
            "/api/v1/playback/tracks",
            buildJsonObject {
                put("work_id", work)
                put("source_id", source)
                audio?.let { put("audio_track_id", it) }
                subtitle?.let { put("subtitle_track_id", it) }
            },
        )
    }

    private fun watched(asked: Asked): JsonObject =
        buildJsonObject {
            put("profile", asked.profile)
            put("watching", true)
            asked.audio?.let { put("audio_track_id", it) }
            when (asked.subtitle) {
                TrackChoice.Off -> put("subtitles_off", true)
                is TrackChoice.Painted -> put("subtitle_track_id", asked.subtitle.id)
                TrackChoice.Unsaid -> Unit
            }
            asked.wideGamut?.let { put("wide_gamut", it) }
        }

    private suspend fun <T> post(path: String, body: JsonObject, answer: DeserializationStrategy<T>): T =
        withContext(Dispatchers.IO) {
            http.newCall(request(path).post(body.toString().asJson()).build()).execute().use { read(it, answer) }
        }

    suspend fun <T> get(path: String, answer: DeserializationStrategy<T>): T = withContext(Dispatchers.IO) {
        http.newCall(request(path).get().build()).execute().use { read(it, answer) }
    }

    /** Sends a change, and gives back the server's answer as it wrote it. */
    /**
     * What the server's live line says, word by word, its opening said as
     * "open"; over when the line closes, and failing when it breaks.
     */
    fun live(): Flow<String> = callbackFlow {
        val listening = object : EventSourceListener() {
            override fun onOpen(eventSource: EventSource, response: Response) {
                trySend("open")
            }

            override fun onEvent(eventSource: EventSource, id: String?, type: String?, data: String) {
                type?.let { trySend(it) }
            }

            override fun onClosed(eventSource: EventSource) {
                close()
            }

            override fun onFailure(eventSource: EventSource, t: Throwable?, response: Response?) {
                close(response?.code?.takeIf { it >= 400 }?.let { Refusal(it, null) } ?: t ?: IOException("the live line broke"))
            }
        }
        // Held open for as long as it is followed: nothing on it may be
        // read as late.
        val line = EventSources.createFactory(http.newBuilder().readTimeout(0, TimeUnit.SECONDS).build())
            .newEventSource(request("/api/v1/system/live").header("Accept", "text/event-stream").build(), listening)
        awaitClose { line.cancel() }
    }

    /** A file the server hands over as it is, the words of a subtitle. */
    suspend fun file(path: String): ByteArray = withContext(Dispatchers.IO) {
        http.newCall(Request.Builder().url(address(path)).get().build()).execute().use { answer ->
            if (!answer.isSuccessful) throw Refusal(answer.code, null)
            answer.body.bytes()
        }
    }

    suspend fun send(method: String, path: String, body: JsonObject?): JsonObject? = withContext(Dispatchers.IO) {
        val sent = body?.toString()?.asJson()
        http.newCall(request(path).method(method, sent).build()).execute().use { answer ->
            read(answer, JsonObject.serializer())
        }
    }

    private fun request(path: String) = Request.Builder().url(address(path)).header("Accept", "application/json")

    private fun String.asJson(): RequestBody = toRequestBody(JSON)

    private fun <T> read(answer: Response, reading: DeserializationStrategy<T>): T {
        val text = answer.body.string()
        if (!answer.isSuccessful) {
            val failure = try {
                serverJson.decodeFromString(Failure.serializer(), text)
            } catch (_: SerializationException) {
                null
            }
            throw Refusal(answer.code, failure?.code, failure?.details?.seconds)
        }
        return try {
            serverJson.decodeFromString(reading, text)
        } catch (failure: SerializationException) {
            throw Refusal(answer.code, "unreadable: ${failure.message}")
        }
    }
}

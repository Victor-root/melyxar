package app.melyxar.tv.inside

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.staticCompositionLocalOf
import app.melyxar.music.Listening
import app.melyxar.music.Queue
import app.melyxar.server.Api
import app.melyxar.server.LiveWord
import app.melyxar.server.Refusal
import app.melyxar.server.Song
import app.melyxar.server.pageJson
import app.melyxar.server.serverJson
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.merge
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonObjectBuilder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import java.io.IOException

/** How long what moved is gathered before it is said: a volume changed is
 *  said once it settles. */
private const val GATHERED_FOR_MS = 400L

/** How many orders wait to be done before the oldest is dropped: a remote
 *  pressed again is the answer to a lost one. */
private const val WAITING_ORDERS = 16

/**
 * One thing another device of the account asks of this television, as the
 * live line says it (see the decision in docs/architecture/README.md):
 * [act] says what, and the rest what it carries.
 */
@Serializable
data class Order(
    @SerialName("do") val act: String,
    val seconds: Double? = null,
    val level: Double? = null,
    val muted: Boolean? = null,
    val track: String? = null,
    val index: Int? = null,
    val work: String? = null,
    val source: String? = null,
    val songs: JsonArray? = null,
    val at: Int? = null,
    val shuffle: Boolean? = null,
    val fromSeconds: Double? = null,
)

/**
 * This television as a device the account's remotes steer: the orders its
 * live line brings, handed to what plays, and how loud it plays and the
 * queue of its music, said to the server for the remotes to show.
 */
class Steered(private val api: Api, private val listening: Listening) {
    private val heard = MutableSharedFlow<Order>(extraBufferCapacity = WAITING_ORDERS)

    /** Every order, as it comes. */
    val orders: SharedFlow<Order> = heard

    /** How loud the film on the screen plays, and whether it is silent,
     *  while there is one: said in place of the music's. */
    var filmLoudness by mutableStateOf<Float?>(null)
    var filmMuted by mutableStateOf(false)

    /** Moved on whenever the line opens, the server forgetting what a device
     *  said once it can no longer be reached. */
    private val opened = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    /** A word of the live line. */
    fun heard(word: LiveWord) {
        when (word.name) {
            "remote" -> try {
                heard.tryEmit(serverJson.decodeFromString(Order.serializer(), word.data))
            } catch (_: SerializationException) {
                // An order of a newer server, left undone rather than taken
                // for something else.
            } catch (_: IllegalArgumentException) {
                // The same, in a shape this cannot read at all.
            }
            "open" -> opened.tryEmit(Unit)
        }
    }

    /** Does what an order asks of the music; one about a film or starting a
     *  film is left to them. */
    fun obeyMusic(order: Order) {
        when (order.act) {
            "toggle" -> listening.toggle()
            "pause" -> listening.pause()
            "resume" -> listening.resume()
            "stop" -> listening.stop()
            "next" -> listening.next()
            "previous" -> listening.previous()
            "seek" -> order.seconds?.let(listening::seek)
            "step" -> order.seconds?.let { listening.seek((listening.position + it).coerceAtLeast(0.0)) }
            "volume" -> order.level?.let { listening.changeVolume(it.toFloat()) }
            "muted" -> order.muted?.let(listening::mute)
            "shuffle" -> listening.shuffle()
            "repeat" -> listening.repeat()
            "jump" -> order.index?.let(listening::jumpTo)
            "play_songs" -> {
                val songs = order.songs?.let { decodedSongs(it) } ?: return
                listening.play(songs, order.at ?: 0, order.shuffle == true, order.fromSeconds ?: 0.0)
            }
        }
    }

    private fun decodedSongs(songs: JsonArray): List<Song>? = try {
        serverJson.decodeFromJsonElement(ListSerializer(Song.serializer()), songs)
    } catch (_: SerializationException) {
        null
    } catch (_: IllegalArgumentException) {
        null
    }

    /** Says how loud this television plays and the queue of its music,
     *  gathered a moment after it last moved, and again whenever the line
     *  opens, for as long as it is followed. */
    @OptIn(FlowPreview::class)
    suspend fun follow() {
        merge(snapshotFlow { told() }, toldOnOpening())
            .debounce(GATHERED_FOR_MS)
            .collect { told ->
                try {
                    api.send("PUT", "/api/v1/remote/told", told)
                } catch (_: Refusal) {
                    // Refused while the line is closed, which its next
                    // opening makes up for.
                } catch (_: IOException) {
                    // The same.
                }
            }
    }

    /** What is said again each time the line opens. */
    private fun toldOnOpening() = flow {
        opened.collect { emit(told()) }
    }

    private fun told(): JsonObject {
        val film = filmLoudness
        val queue = listening.queue
        return buildJsonObject {
            put("volume", film ?: listening.volume)
            put("muted", if (film != null) filmMuted else listening.muted)
            if (queue.order.isEmpty()) {
                put("queue", null as String?)
            } else {
                putJsonObject("queue") { inPlayedOrder(queue) }
            }
        }
    }

    private fun JsonObjectBuilder.inPlayedOrder(queue: Queue) {
        put("songs", pageJson.encodeToJsonElement(ListSerializer(Song.serializer()), queue.order.map { queue.songs[it] }))
        put("at", queue.at)
        put("shuffle", queue.shuffle)
        put("repeat", queue.repeat.name.lowercase())
    }
}

/** The television as the account's remotes steer it. */
val LocalSteered = staticCompositionLocalOf<Steered> { error("not steered") }

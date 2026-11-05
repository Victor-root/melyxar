package app.melyxar.web

import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.compose.runtime.snapshotFlow
import androidx.lifecycle.lifecycleScope
import app.melyxar.music.Listening
import app.melyxar.music.PhoneMusic
import app.melyxar.music.Queue
import app.melyxar.server.Song
import app.melyxar.server.serverJson
import kotlinx.coroutines.launch
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerializationException
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject

/**
 * The page's player of music and the application's, which holds the music:
 * the page follows it as a tab of a browser follows the one that holds the
 * sound (see the page's `app-music.ts` and `tabs.ts`). Every press of the
 * page is sent here and done by the player; the queue and where the song has
 * got to go back to the page as they change, and each song counted as
 * listened to.
 *
 * Called by the page off the main thread, each request is handed to it.
 */
class MusicBridge(
    private val activity: ComponentActivity,
    private val page: WebView,
    private val listening: Listening,
    /** Told each time the music is asked to play, on the main thread. */
    private val onPlay: () -> Unit,
) {
    /** The songs the page was last handed, not handed again while the
     *  queue only moves along them. */
    private var songsSent: List<Song>? = null

    init {
        activity.lifecycleScope.launch {
            snapshotFlow { listening.queue }.collect(::sendQueue)
        }
        activity.lifecycleScope.launch {
            snapshotFlow { Beat(listening.playing, listening.waiting, listening.position, listening.length, listening.inHand, listening.volume, listening.muted) }
                .collect(::sendBeat)
        }
        activity.lifecycleScope.launch {
            listening.counted.collect { song -> call("melyxarMusicListened", Json.encodeToString(song)) }
        }
    }

    /** The page's player is in place, signed in as [account]: everything is
     *  said to it afresh. */
    @JavascriptInterface
    fun hello(account: String) = activity.runOnUiThread {
        PhoneMusic.heldBy(account)
        songsSent = null
        sendQueue(listening.queue)
        sendBeat(Beat(listening.playing, listening.waiting, listening.position, listening.length, listening.inHand, listening.volume, listening.muted))
    }

    /** One of the page's commands, by its name (`tabs.ts`), with what it
     *  asks for written as a list. */
    @JavascriptInterface
    fun command(name: String, said: String) {
        val asked = try {
            READ.decodeFromString(JsonArray.serializer(), said)
        } catch (_: SerializationException) {
            return
        }
        activity.runOnUiThread { obey(name, asked) }
    }

    /** The account's choices for its music changed in the page. */
    @JavascriptInterface
    fun preferencesChanged() = activity.runOnUiThread {
        activity.lifecycleScope.launch { listening.readPreferences() }
    }

    private fun obey(name: String, asked: JsonArray) {
        fun number(at: Int) = asked[at].jsonPrimitive
        fun songs(at: Int) = READ.decodeFromJsonElement(ListSerializer(Song.serializer()), asked[at])
        try {
            when (name) {
                "play" -> {
                    listening.play(
                        songs(0),
                        number(1).int,
                        asked.getOrNull(2)?.jsonPrimitive?.booleanOrNull == true,
                        asked.getOrNull(3)?.jsonPrimitive?.doubleOrNull ?: 0.0,
                    )
                    onPlay()
                }
                "toggle" -> {
                    listening.toggle()
                    onPlay()
                }
                "pause" -> listening.pause()
                "resume" -> {
                    listening.resume()
                    onPlay()
                }
                "stop" -> listening.stop()
                "next" -> listening.next()
                "previous" -> listening.previous()
                "seek" -> listening.seek(number(0).double)
                "setVolume" -> listening.changeVolume(number(0).double.toFloat())
                "setMuted" -> listening.mute(number(0).booleanOrNull == true)
                "toggleShuffle" -> listening.shuffle()
                "cycleRepeat" -> listening.repeat()
                "jump" -> listening.jumpTo(number(0).int)
                "playNext" -> listening.playNext(songs(0))
                "playLast" -> listening.playLast(songs(0))
                "remove" -> listening.remove(number(0).int)
                "move" -> listening.move(number(0).int, number(1).int)
            }
        } catch (_: IllegalArgumentException) {
            // Not what the command takes: a page of another version, left
            // unanswered rather than taken for something else.
        } catch (_: IndexOutOfBoundsException) {
            // Short of what the command takes, the same.
        }
    }

    private fun sendQueue(queue: Queue) {
        val songs = queue.songs.takeUnless { it === songsSent }
        songsSent = queue.songs
        val said = buildJsonObject {
            songs?.let { put("songs", PAGE.encodeToJsonElement(ListSerializer(Song.serializer()), it)) }
            put("order", JsonArray(queue.order.map { JsonPrimitive(it) }))
            put("at", queue.at)
            put("shuffle", queue.shuffle)
            put("repeat", queue.repeat.name.lowercase())
        }
        call("melyxarMusicQueue", said.toString())
    }

    private fun sendBeat(beat: Beat) {
        val said = buildJsonObject {
            put("playing", beat.playing)
            put("waiting", beat.waiting)
            put("position", beat.position)
            put("length", beat.length ?: 0.0)
            put("loaded", beat.inHand)
            putJsonObject("loudness") {
                put("volume", beat.volume)
                put("muted", beat.muted)
            }
        }
        call("melyxarMusicBeat", said.toString())
    }

    private fun call(function: String, argument: String) {
        page.evaluateJavascript("window.$function && window.$function($argument)", null)
    }

    /** What the page shows of where the music has got to, told each time
     *  any of it changes. */
    private data class Beat(
        val playing: Boolean,
        val waiting: Boolean,
        val position: Double,
        val length: Double?,
        val inHand: Double,
        val volume: Float,
        val muted: Boolean,
    )

    private companion object {
        val READ: Json = serverJson

        /** The songs written as the page reads them, every field there, an
         *  empty one as null. */
        @OptIn(ExperimentalSerializationApi::class)
        val PAGE = Json(serverJson) {
            explicitNulls = true
            encodeDefaults = true
        }
    }
}

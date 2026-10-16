package app.melyxar.tv.music

import android.content.ComponentName
import android.content.Context
import androidx.annotation.OptIn
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.melyxar.music.DeckTold
import app.melyxar.music.Described
import app.melyxar.music.MusicEngine
import app.melyxar.music.MusicService
import app.melyxar.player.measuredMusicForms
import app.melyxar.server.Api
import app.melyxar.server.MusicPreferences
import app.melyxar.server.Refusal
import app.melyxar.server.Slice
import app.melyxar.server.Song
import app.melyxar.server.changeMusicPreferences
import app.melyxar.server.fitting
import app.melyxar.server.libraryQueue
import app.melyxar.server.listenedTo
import app.melyxar.server.musicPreferences
import app.melyxar.server.songs
import app.melyxar.server.soundOf
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import okhttp3.Call
import java.io.IOException
import kotlin.random.Random

/** How long before the end of a song the next one is made ready, so it is
 *  there when its moment comes even on a slow way in. */
private const val PREPARE_AHEAD_SECONDS = 20.0

/** How wide the cover handed to the system's card is, in pixels. */
private const val CARD_COVER_ACROSS = 512

/**
 * The television's player of music, as the web's: the queue and its rules,
 * the engine's two decks, one playing and the next song made ready in the
 * other, each song brought to the level of the others, one fading into the
 * next when the account asks for it, and every song listened to counted.
 * Asked by the system's card and the remote's keys as by its own buttons.
 */
@Stable
@OptIn(UnstableApi::class)
class Listening(private val context: Context, private val api: Api, private val scope: CoroutineScope) : MusicEngine.Listener {
    /** The engine and the service that keeps the music going, made at the
     *  first song: an account that never listens to music costs nothing. */
    private var made: MusicEngine? = null
    private var service: ListenableFuture<MediaController>? = null

    private val engine: MusicEngine
        get() = made ?: MusicEngine.of(context) {
            // The songs and the covers fetched signed in as the account
            // listening, whichever it is by then: the engine outlives it.
            OkHttpDataSource.Factory(Call.Factory { request -> checkNotNull(signedIn) { "nobody listening" }.http.newCall(request) })
        }.also { engine ->
            engine.listener = this
            made = engine
            service = MediaController.Builder(context, SessionToken(context, ComponentName(context, MusicService::class.java))).buildAsync()
        }

    /** What the device plays, as the server reads it. */
    private val forms by lazy { measuredMusicForms() }

    var queue by mutableStateOf(Queue())
        private set

    var playing by mutableStateOf(false)
        private set

    /** Where the song playing is, and how long it lasts, in seconds. */
    var position by mutableDoubleStateOf(0.0)
        private set
    var length by mutableStateOf<Double?>(null)
        private set

    var preferences by mutableStateOf(MusicPreferences())

    /** How many times music was started anew, from songs, an album or a
     *  library pressed, which the screens follow to the song's page. */
    var begun by mutableIntStateOf(0)
        private set

    /** The deck the song playing is on. */
    private var deck = 0

    /** Where the deck's sound was asked from: a song made into another form
     *  on its way starts there, and its own clock starts at nought. */
    private var offset = 0.0

    /** The song made ready on the other deck, and the queue it plays in. */
    private var prepared: Queue? = null

    /** Whether the song playing was counted as listened to. */
    private var counted = false

    /** Whether the deck knows how long what it plays lasts: the file itself
     *  does, a song made into another form on its way does not, and is only
     *  moved in by being asked again from elsewhere. */
    private var measured = false

    /** Whether the queue played to its end, so play starts it over. */
    private var over = false

    init {
        signedIn = api
    }

    /** Reads again how the account listens. */
    suspend fun readPreferences() {
        try {
            preferences = api.musicPreferences()
        } catch (_: Refusal) {
            // Listened to as before.
        } catch (_: IOException) {
            // Nor with the network gone.
        }
    }

    /** Changes how the account listens: shown at once, and as it was again
     *  if the server refuses. [sent] names what changes as the server does. */
    fun changePreferences(shown: MusicPreferences, sent: JsonObject) {
        val before = preferences
        preferences = shown
        scope.launch {
            preferences = try {
                api.changeMusicPreferences(sent)
            } catch (_: Refusal) {
                before
            } catch (_: IOException) {
                before
            }
        }
    }

    /** Plays [songs] from the one at [first], shuffled or not. */
    fun play(songs: List<Song>, first: Int = 0, shuffle: Boolean = false) {
        if (songs.isEmpty()) {
            return
        }
        start(playing(songs, first, shuffle, queue.repeat))
        begun += 1
    }

    /** Plays a whole library, in the order of its albums or shuffled. */
    suspend fun playLibrary(library: String, shuffle: Boolean) {
        val offset = if (shuffle) shuffleOffset(api.songs(library, "title", false, Slice(0, 1)).total, Random.nextDouble()) else 0
        val songs = api.libraryQueue(library, offset).items
        if (songs.isNotEmpty()) {
            play(songs, if (shuffle) Random.nextInt(songs.size) else 0, shuffle)
        }
    }

    fun toggle() {
        when {
            queue.current == null -> Unit
            over -> start(queue.jumpTo(0))
            playing -> engine.pause(deck)
            else -> engine.play(deck)
        }
    }

    fun next() {
        queue.forward()?.let(::start)
    }

    fun previous() {
        val back = queue.backward(position)
        if (back.restart) seek(0.0) else start(back.queue)
    }

    /** Takes the song playing to [seconds]. */
    fun seek(seconds: Double) {
        val song = queue.current ?: return
        val to = seconds.coerceAtLeast(0.0).let { at -> length?.let(at::coerceAtMost) ?: at }
        forgetThePrepared()
        if (measured) engine.seek(deck, to) else startSong(song, from = to)
    }

    /** Back or on within the song, by the account's own steps. */
    fun skip(on: Boolean) = seek(position + if (on) preferences.skipOnSeconds else -preferences.skipBackSeconds)

    fun shuffle() {
        forgetThePrepared()
        queue = queue.withShuffle(!queue.shuffle)
        describe()
    }

    fun repeat() {
        forgetThePrepared()
        queue = queue.copy(repeat = queue.repeat.next())
        describe()
    }

    /** Plays the song at [place] of the order. */
    fun jumpTo(place: Int) = start(queue.jumpTo(place))

    /** Everything stopped, the queue emptied. */
    fun stop() {
        made?.stopAll()
        queue = Queue()
        playing = false
        position = 0.0
        length = null
        prepared = null
    }

    /** Everything let go of, the television leaving this account. */
    fun leave() {
        stop()
        made?.listener = null
        service?.let(MediaController::releaseFuture)
        if (signedIn === api) {
            signedIn = null
        }
    }

    private fun start(next: Queue) {
        val song = next.current ?: return
        // Whatever the other deck holds goes, a song still fading out of it
        // included.
        engine.cancelHandOver()
        engine.clear(1 - deck)
        prepared = null
        queue = next
        startSong(song, from = 0.0)
    }

    private fun startSong(song: Song, from: Double) {
        over = false
        measured = false
        offset = from
        counted = counted && from > 0
        position = from
        length = song.seconds?.toDouble()
        engine.level(deck, levelOf(song, preferences.volumeMode), 0.0)
        engine.load(deck, api.soundOf(song.id, forms, from), andPlay = true)
        describe()
    }

    /** The other deck let go of, and whatever it was made ready for. */
    private fun forgetThePrepared() {
        if (prepared != null) {
            engine.cancelHandOver()
            engine.clear(1 - deck)
            prepared = null
        }
    }

    override fun told(told: DeckTold) {
        if (told.deck != deck) {
            heardFromTheOther(told)
            return
        }
        measured = told.duration != null
        position = offset + told.position
        length = told.duration?.let { offset + it } ?: queue.current?.seconds?.toDouble()
        playing = told.playing
        countTheListen()
        when {
            "ended" in told.events -> afterTheEnd()
            "error" in told.events -> next()
            else -> prepareTheNext(told)
        }
    }

    /** The deck made ready took over, the song on it now the one playing;
     *  or the one it took over from has faded out to its end. */
    private fun heardFromTheOther(told: DeckTold) {
        val next = prepared
        if ("ended" in told.events) {
            engine.clear(told.deck)
        }
        if ("handedover" in told.events && next != null) {
            deck = told.deck
            prepared = null
            queue = next
            offset = 0.0
            counted = false
            measured = told.duration != null
            position = told.position
            length = told.duration ?: next.current?.seconds?.toDouble()
            describe()
        }
    }

    private fun afterTheEnd() {
        val next = queue.afterTheEnd()
        when {
            next == null -> {
                playing = false
                over = true
            }
            // The same song again, under repeat one.
            next === queue -> {
                seek(0.0)
                engine.play(deck)
            }
            else -> start(next)
        }
    }

    /** Near the end of a song, the next made ready on the other deck and
     *  started at its moment, faded in when the account asks for it. */
    private fun prepareTheNext(told: DeckTold) {
        val duration = told.duration ?: return
        val next = queue.afterTheEnd()?.takeUnless { it === queue } ?: return
        val song = next.current ?: return
        if (prepared != null || duration - told.position > PREPARE_AHEAD_SECONDS) {
            return
        }
        prepared = next
        val other = 1 - deck
        val fade = fadeBetween(preferences.crossfadeSeconds.toDouble(), duration, song.seconds?.toDouble())
        engine.load(other, api.soundOf(song.id, forms), andPlay = false)
        engine.handOverAt(deck, other, duration - fade, fade, levelOf(song, preferences.volumeMode))
    }

    private fun countTheListen() {
        val song = queue.current ?: return
        if (counted || !countsAsListened(position, length)) {
            return
        }
        counted = true
        scope.launch {
            try {
                api.listenedTo(song.id)
            } catch (_: Refusal) {
                // Counted or not, the song plays on.
            } catch (_: IOException) {
                // Lost with the network: one count less.
            }
        }
    }

    /** The song playing, as the system's card shows it. */
    private fun describe() {
        val song = queue.current
        made?.describe(
            song?.let {
                Described(
                    title = it.title,
                    artist = it.artists.joinToString(", ") { artist -> artist.name },
                    album = it.album?.name.orEmpty(),
                    artwork = it.cover.fitting(CARD_COVER_ACROSS)?.url?.let(api::address),
                    seconds = it.seconds?.toDouble(),
                    canStepOn = queue.canStepOn,
                )
            },
        )
    }

    /** A press on the system's card or a remote's media key. */
    override fun asked(asked: String, seconds: Double) {
        when (asked) {
            "play" -> engine.play(deck)
            "pause" -> engine.pause(deck)
            "next" -> next()
            "previous" -> previous()
            "seek" -> seek(seconds)
            "stop" -> stop()
        }
    }

    private companion object {
        /** The account listening, whose session the engine fetches with. */
        @Volatile
        var signedIn: Api? = null
    }
}

val LocalListening = staticCompositionLocalOf<Listening> { error("no music") }

package app.melyxar.music

import android.content.ComponentName
import android.content.Context
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.melyxar.player.measuredMusicForms
import app.melyxar.server.Api
import app.melyxar.server.MusicPreferences
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
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
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

/** How often where the queue has got to is kept, while it plays. */
private const val KEEP_PLACE_EVERY_MS = 5_000L

/**
 * The application's player of music, as the web's: the queue and its rules,
 * the engine's two decks, one playing and the next song made ready in the
 * other, each song brought to the level of the others, one fading into the
 * next when the account asks for it, every song listened to counted, and
 * the server told what plays. Asked by the system's card, a headset, a car
 * and the remote's keys as by the buttons of the screens that show it: the
 * television's own, and on a phone the page, which follows it.
 *
 * On a phone it also keeps its queue and its loudness for the next time
 * ([keeping]), and hands the cards covers the car can read ([artworkOf]).
 */
@Stable
@OptIn(UnstableApi::class)
class Listening(
    private val context: Context,
    /** The server, asked as the account listening; nothing when nobody is. */
    private val asking: () -> Api?,
    private val scope: CoroutineScope,
    private val keeping: KeptMusic? = null,
    /** Where a card shows a cover from, given the server's address of it. */
    private val artworkOf: (String) -> String = { it },
) : MusicEngine.Listener {
    /** The engine and the service that keeps the music going, made at the
     *  first song: an account that never listens to music costs nothing. */
    private var made: MusicEngine? = null
    private var service: ListenableFuture<MediaController>? = null

    private val engine: MusicEngine
        get() = made ?: MusicEngine.of(context) {
            // The songs and the covers fetched signed in as the account
            // listening, whichever it is by then: the engine outlives it.
            OkHttpDataSource.Factory(Call.Factory { request -> checkNotNull(leading?.invoke()) { "nobody listening" }.http.newCall(request) })
        }.also { engine ->
            engine.listener = this
            made = engine
            service = MediaController.Builder(context, SessionToken(context, ComponentName(context, MusicService::class.java))).buildAsync()
            sound(engine)
        }

    /** The engine, made now if it was not: for the service a car wakes
     *  with nothing played yet. */
    fun engineNow(): MusicEngine = engine

    /** What the device plays, as the server reads it: asked of its
     *  decoders away from the thread that draws the screen, which asking
     *  them all would freeze, the first time a song is played. */
    private var forms: String? = null
    private val measuring = scope.async(Dispatchers.Default, start = CoroutineStart.LAZY) { measuredMusicForms() }

    /** The song waiting for [forms] to be known before it is loaded, and
     *  whether it starts playing once it is, which play and pause change
     *  meanwhile. */
    private var untilFormsKnown: Job? = null
    private var startsPlaying = true

    private var held by mutableStateOf(Queue())

    /** The queue. Kept for the next time as it changes, when it is kept. */
    var queue: Queue
        get() = held
        private set(next) {
            val was = held
            held = next
            val kept = keeping ?: return
            if (next.songs != was.songs || next.order != was.order || next.shuffle != was.shuffle || next.repeat != was.repeat) {
                scope.launch(WRITING) { kept.keepQueue(next.takeIf { it.current != null }) }
            }
            if (next.at != was.at) {
                keepThePlace()
            }
        }

    var playing by mutableStateOf(false)
        private set

    /** Asked to play, and the sound not there yet. */
    var waiting by mutableStateOf(false)
        private set

    /** Where the song playing is, how long it lasts, and how far its sound
     *  is in hand, in seconds from its start. */
    var position by mutableDoubleStateOf(0.0)
        private set
    var length by mutableStateOf<Double?>(null)
        private set
    var inHand by mutableDoubleStateOf(0.0)
        private set

    /** How loud it plays, under the device's own volume. */
    var volume by mutableFloatStateOf(1f)
        private set
    var muted by mutableStateOf(false)
        private set

    var preferences by mutableStateOf(MusicPreferences())

    private val heard = MutableSharedFlow<String>(extraBufferCapacity = 8)

    /** Each song as it is counted as listened to, for what shows the
     *  listening to read it again. */
    val counted: SharedFlow<String> = heard

    /** The deck the song playing is on. */
    private var deck = 0

    /** Where the deck's sound was asked from: a song made into another form
     *  on its way starts there, and its own clock starts at nought. */
    private var offset = 0.0

    /** Whether the song playing was given a deck: a queue taken up from the
     *  last time is not, until it is played, from [resumeAt]. */
    private var loaded = false
    private var resumeAt = 0.0

    /** The song made ready on the other deck, and the queue it plays in. */
    private var prepared: Queue? = null

    /** Whether the song playing was counted as listened to. */
    private var counting = false

    /** Whether the deck knows how long what it plays lasts: the file itself
     *  does, a song made into another form on its way does not, and is only
     *  moved in by being asked again from elsewhere. */
    private var measured = false

    /** Whether the queue played to its end, so play starts it over. */
    private var over = false

    private var placeKeptAt = 0L

    private val reports = MusicReports(asking, scope, { position }, ::stop)

    init {
        leading = asking
        keeping?.loudness()?.let { kept ->
            volume = kept.volume
            muted = kept.muted
        }
    }

    /** Reads again how the account listens. */
    suspend fun readPreferences() {
        val api = asking() ?: return
        try {
            preferences = api.musicPreferences()
        } catch (_: IOException) {
            // Listened to as before, refused or with the network gone.
        }
    }

    /** Changes how the account listens: shown at once, and as it was again
     *  if the server refuses. [sent] names what changes as the server does. */
    fun changePreferences(shown: MusicPreferences, sent: JsonObject) {
        val api = asking() ?: return
        val before = preferences
        preferences = shown
        scope.launch {
            preferences = try {
                api.changeMusicPreferences(sent)
            } catch (_: IOException) {
                before
            }
        }
    }

    /** The queue kept from the last time, taken up paused where it was,
     *  unless something plays already. */
    fun takeUp(kept: KeptQueue) {
        val taken = Queue(kept.songs, kept.order, kept.at, kept.shuffle, kept.repeat)
        val song = taken.current
        if (queue.current != null || song == null) {
            return
        }
        queue = taken
        loaded = false
        resumeAt = kept.position
        position = kept.position
        length = song.seconds?.toDouble()
        describe()
    }

    /** Plays [songs] from the one at [first], shuffled or not, [from]
     *  seconds into it. */
    fun play(songs: List<Song>, first: Int = 0, shuffle: Boolean = false, from: Double = 0.0) {
        if (songs.isEmpty()) {
            return
        }
        start(playing(songs, first, shuffle, queue.repeat), from)
    }

    /** Plays a whole library, in the order of its albums or shuffled. */
    suspend fun playLibrary(library: String, shuffle: Boolean) {
        val api = asking() ?: return
        val offset = if (shuffle) shuffleOffset(api.songs(library, "title", false, Slice(0, 1)).total, Random.nextDouble()) else 0
        val songs = api.libraryQueue(library, offset).items
        if (songs.isNotEmpty()) {
            play(songs, if (shuffle) Random.nextInt(songs.size) else 0, shuffle)
        }
    }

    fun toggle() {
        if (playing || (untilFormsKnown?.isActive == true && startsPlaying)) pause() else resume()
    }

    fun pause() {
        if (untilFormsKnown?.isActive == true) {
            startsPlaying = false
        } else {
            made?.pause(deck)
        }
        keepThePlace()
    }

    fun resume() {
        val song = queue.current ?: return
        when {
            untilFormsKnown?.isActive == true -> startsPlaying = true
            over -> start(queue.jumpTo(0))
            !loaded -> startSong(song, from = resumeAt)
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
        if (!loaded) {
            resumeAt = to
            position = to
            return
        }
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

    fun repeat() = repeat(queue.repeat.next())

    fun repeat(mode: Repeat) {
        forgetThePrepared()
        queue = queue.copy(repeat = mode)
        describe()
    }

    /** Puts [songs] right after the song playing. */
    fun playNext(songs: List<Song>) = queued(queue.withNext(songs))

    /** Puts [songs] at the end of the queue. */
    fun playLast(songs: List<Song>) = queued(queue.withLast(songs))

    /** Takes a song still to come out of the queue, by its place in the order. */
    fun remove(place: Int) = queued(queue.without(place))

    /** Puts the song at one place of the order at another. */
    fun move(from: Int, to: Int) = queued(queue.moved(from, to))

    /** The queue changed around the song playing: played at once when
     *  nothing was, the song playing left alone otherwise. */
    private fun queued(next: Queue) {
        if (queue.current == null) {
            start(next)
            return
        }
        forgetThePrepared()
        queue = next
        describe()
    }

    /** Plays the song at [place] of the order. */
    fun jumpTo(place: Int) = start(queue.jumpTo(place))

    fun changeVolume(level: Float) {
        volume = level.coerceIn(0f, 1f)
        if (level > 0f) {
            muted = false
        }
        loudnessChanged()
    }

    fun mute(silent: Boolean) {
        muted = silent
        loudnessChanged()
    }

    private fun loudnessChanged() {
        made?.let(::sound)
        keeping?.keepLoudness(Loudness(volume, muted))
    }

    /** Both decks at the loudness chosen. */
    private fun sound(engine: MusicEngine) {
        (0..1).forEach { engine.setSound(it, volume, muted) }
    }

    /** Everything stopped, the queue emptied. */
    fun stop() {
        untilFormsKnown?.cancel()
        made?.stopAll()
        queue = Queue()
        playing = false
        waiting = false
        position = 0.0
        length = null
        inHand = 0.0
        prepared = null
        loaded = false
        over = false
        describe()
    }

    /** Everything let go of, nobody listening here any more. */
    fun leave() {
        stop()
        made?.let { engine ->
            if (engine.listener === this) {
                engine.listener = null
            }
        }
        service?.let(MediaController::releaseFuture)
        if (leading === asking) {
            leading = null
        }
    }

    private fun start(next: Queue, from: Double = 0.0) {
        val song = next.current ?: return
        // Whatever the other deck holds goes, a song still fading out of it
        // included.
        engine.cancelHandOver()
        engine.clear(1 - deck)
        prepared = null
        queue = next
        startSong(song, from)
    }

    private fun startSong(song: Song, from: Double) {
        val api = asking() ?: return
        over = false
        measured = false
        loaded = true
        offset = from
        counting = counting && from > 0
        position = from
        inHand = from
        length = song.seconds?.toDouble()
        engine.level(deck, levelOf(song, preferences.volumeMode), 0.0)
        untilFormsKnown?.cancel()
        val known = forms
        if (known != null) {
            engine.load(deck, api.soundOf(song.id, known, from), andPlay = true)
        } else {
            startsPlaying = true
            waiting = true
            untilFormsKnown = scope.launch {
                val measured = measuring.await()
                forms = measured
                engine.load(deck, api.soundOf(song.id, measured, from), andPlay = startsPlaying)
            }
        }
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
        inHand = offset + told.buffered
        playing = told.playing
        waiting = told.playing && !told.advancing
        reports.playing(playing)
        countTheListen()
        if (playing && SystemClock.elapsedRealtime() - placeKeptAt > KEEP_PLACE_EVERY_MS) {
            keepThePlace()
        }
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
            counting = false
            measured = told.duration != null
            position = told.position
            inHand = told.buffered
            length = told.duration ?: next.current?.seconds?.toDouble()
            describe()
        }
    }

    private fun afterTheEnd() {
        val next = queue.afterTheEnd()
        when {
            next == null && preferences.closeWhenDone -> stop()
            next == null -> {
                playing = false
                waiting = false
                over = true
                // The last second is rounded away by the clock: a song that
                // has ended shows its whole length played.
                length?.let { position = it }
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
        val known = forms ?: return
        val duration = told.duration ?: return
        val next = queue.afterTheEnd()?.takeUnless { it === queue } ?: return
        val song = next.current ?: return
        val api = asking() ?: return
        if (prepared != null || duration - told.position > PREPARE_AHEAD_SECONDS) {
            return
        }
        prepared = next
        val other = 1 - deck
        val fade = fadeBetween(preferences.crossfadeSeconds.toDouble(), duration, song.seconds?.toDouble())
        engine.load(other, api.soundOf(song.id, known), andPlay = false)
        engine.handOverAt(deck, other, duration - fade, fade, levelOf(song, preferences.volumeMode))
    }

    private fun countTheListen() {
        val song = queue.current ?: return
        if (counting || !countsAsListened(position, length)) {
            return
        }
        counting = true
        val api = asking() ?: return
        scope.launch {
            try {
                api.listenedTo(song.id)
                heard.emit(song.id)
            } catch (_: IOException) {
                // Refused or lost with the network: one count less, and the
                // song plays on.
            }
        }
    }

    private fun keepThePlace() {
        val kept = keeping ?: return
        placeKeptAt = SystemClock.elapsedRealtime()
        kept.keepPlace(queue.at, position)
    }

    /** The queue as the system's card shows it, a queue taken up with
     *  nothing played yet included, and the song open said to the server. */
    private fun describe() {
        val now = queue
        reports.song(now.current?.id)
        if (now.current == null) {
            made?.describe(null)
            return
        }
        val api = asking()
        engine.describe(
            Card(
                songs = now.order.map { place -> describedOf(now.songs[place], api) },
                at = now.at,
                canStepOn = now.canStepOn,
                shuffle = now.shuffle,
                repeat = now.repeat,
            ),
        )
    }

    private fun describedOf(song: Song, api: Api?) = Described(
        id = song.id,
        title = song.title,
        artist = song.artists.joinToString(", ") { artist -> artist.name },
        album = song.album?.name.orEmpty(),
        artwork = api?.let { song.cover.fitting(CARD_COVER_ACROSS)?.url?.let(it::address)?.let(artworkOf) },
        seconds = song.seconds?.toDouble(),
    )

    /** A press on the system's card, a headset, a car or a remote's media key. */
    override fun asked(pressed: Pressed) {
        when (pressed) {
            Pressed.Play -> resume()
            Pressed.Pause -> pause()
            Pressed.Next -> next()
            Pressed.Previous -> previous()
            Pressed.Stop -> stop()
            is Pressed.Seek -> seek(pressed.seconds)
            is Pressed.Jump -> jumpTo(pressed.place)
            is Pressed.Shuffled -> if (pressed.on != queue.shuffle) shuffle()
            is Pressed.Repeated -> repeat(pressed.mode)
        }
    }

    override fun positionMs(): Long =
        if (!loaded) (resumeAt * 1000).toLong() else (offset * 1000).toLong() + (made?.livePositionMs() ?: 0L)

    private companion object {
        /** How the player the engine fetches with asks its server, as the
         *  account listening. */
        @Volatile
        var leading: (() -> Api?)? = null

        /** What is kept is written one thing after the other, never two
         *  writes racing to leave the older last. */
        val WRITING = Dispatchers.IO.limitedParallelism(1)
    }
}

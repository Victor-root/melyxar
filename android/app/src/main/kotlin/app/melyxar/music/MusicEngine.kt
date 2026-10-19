package app.melyxar.music

import android.annotation.SuppressLint
import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.annotation.OptIn
import android.media.AudioManager
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.audio.AudioFocusRequestCompat
import androidx.media3.common.audio.AudioManagerCompat
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.exoplayer.source.ProgressiveMediaSource
import kotlinx.serialization.Serializable

/**
 * The sound of the page's player of music: its two decks, each a player of
 * its own, the one playing and the one the next song is made ready in (see
 * the page's `deck.ts` and `app-music.ts`).
 *
 * The page decides everything: what each deck plays, when, and what comes
 * next. What is done here on its own is only what cannot wait for a page in
 * the background: the next deck started right at the moment it was asked
 * for, the levels and fades on the sound, and the sound given up when
 * something else needs it, a call, a film, another application.
 *
 * One for the whole application, outliving any screen, so the service that
 * keeps it playing can hold it too.
 */
@OptIn(UnstableApi::class)
class MusicEngine private constructor(
    private val context: Context,
    /** How the songs, and the covers on the card, are fetched. */
    val fetching: DataSource.Factory,
) {
    /** What the engine says, to whoever leads it: the page, or the
     *  television's own player. */
    interface Listener {
        fun told(told: DeckTold)
        fun asked(asked: String, seconds: Double)
    }

    var listener: Listener? = null

    /** The card of the lock screen, held here and drawn by the service. Made
     *  once the engine is whole, as it reads it. */
    val face by lazy { SessionFace(this) }

    private val main = Handler(Looper.getMainLooper())
    private val sources = ProgressiveMediaSource.Factory(fetching)
    private val sound = AudioAttributes.Builder()
        .setUsage(C.USAGE_MEDIA)
        .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
        .build()

    // Made after everything they read as they are made.
    private val decks = List(2) { Deck(it) }

    /** The deck the card speaks of: the last one started. */
    var live = 0
        private set

    /** A deck to start when the other reaches a moment of its own clock. */
    private class HandOver(val from: Int, val to: Int, val atMs: Long, val fadeSeconds: Double, val level: Float)

    private var handOver: HandOver? = null
    private val checkHandOver = Runnable { lookAtTheHandOver() }

    private val tell = object : Runnable {
        override fun run() {
            decks.forEach { it.tell() }
            lookAtTheHandOver()
            if (decks.any { it.holds }) {
                main.postDelayed(this, TELL_EVERY_MS)
            }
        }
    }

    private inner class Deck(val index: Int) {
        val gain = GainProcessor()
        val player: ExoPlayer = ExoPlayer.Builder(context, renderersWith(gain))
            // Both decks sound at once during a fade, so the focus is held
            // by the engine for the two together rather than by each.
            .setAudioAttributes(sound, false)
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
            .apply { addListener(Heard()) }
        var holds = false
        var volume = 1f
        var muted = false
        private var metadataSaid = false
        private var error: String? = null
        val events = mutableListOf<String>()

        fun load(address: String, andPlay: Boolean) {
            SoundTrace.say("deck $index load andPlay=$andPlay")
            holds = true
            metadataSaid = false
            error = null
            player.setMediaSource(sources.createMediaSource(MediaItem.fromUri(address)))
            player.playWhenReady = andPlay
            player.prepare()
            if (andPlay) {
                started(this)
            }
            tellNow()
        }

        fun clear() {
            holds = false
            player.stop()
            player.clearMediaItems()
        }

        fun applyVolume() {
            player.volume = if (muted) 0f else volume * ducked
        }

        fun tell() {
            if (!holds && events.isEmpty()) {
                return
            }
            val told = DeckTold(
                deck = index,
                position = player.currentPosition / 1000.0,
                duration = player.duration.takeIf { it != C.TIME_UNSET }?.let { it / 1000.0 },
                buffered = player.bufferedPosition / 1000.0,
                playing = player.playWhenReady && player.playbackState != Player.STATE_ENDED,
                advancing = player.isPlaying,
                error = error,
                events = events.toList(),
            )
            events.clear()
            listener?.told(told)
        }

        private inner class Heard : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                SoundTrace.say("deck $index state=$state")
                when (state) {
                    Player.STATE_READY -> if (!metadataSaid) {
                        metadataSaid = true
                        events += listOf("loadedmetadata", "canplay")
                    }
                    Player.STATE_BUFFERING -> if (player.playWhenReady) events += "waiting"
                    Player.STATE_ENDED -> events += listOf("pause", "ended")
                    // Only ever stopped by the page, which knows it already.
                    Player.STATE_IDLE -> Unit
                }
                changed()
            }

            override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
                SoundTrace.say("deck $index playWhenReady=$playWhenReady reason=$reason")
                events += if (playWhenReady) "play" else "pause"
                changed()
            }

            override fun onIsPlayingChanged(isPlaying: Boolean) {
                SoundTrace.say("deck $index isPlaying=$isPlaying")
                if (isPlaying) {
                    SoundTrace.lookAround(context, "deck $index sounds")
                }
                if (isPlaying) {
                    events += "playing"
                }
                changed()
            }

            override fun onPlayerError(failure: PlaybackException) {
                SoundTrace.say("deck $index error ${failure.errorCodeName}")
                error = "${failure.errorCodeName}: ${failure.message}"
                events += "error"
                changed()
            }
        }
    }

    /** Each deck plays through its own gain. */
    private fun renderersWith(gain: GainProcessor) = object : DefaultRenderersFactory(context) {
        override fun buildAudioSink(
            context: Context,
            enableFloatOutput: Boolean,
            enableAudioTrackPlaybackParams: Boolean,
        ): AudioSink = DefaultAudioSink.Builder(context)
            .setAudioProcessors(arrayOf(gain))
            .build()
    }

    private fun changed() {
        tellNow()
        face.changed()
    }

    private fun tellNow() {
        main.removeCallbacks(tell)
        main.post(tell)
    }

    fun load(deck: Int, address: String, andPlay: Boolean) = decks[deck].load(address, andPlay)

    fun clear(deck: Int) {
        decks[deck].clear()
        if (handOver?.to == deck || handOver?.from == deck) {
            cancelHandOver()
        }
        if (decks.none { it.holds }) {
            letGoOfTheFocus()
        }
        changed()
    }

    fun play(deck: Int) {
        SoundTrace.say("play deck $deck")
        val playing = decks[deck]
        if (!playing.holds) {
            return
        }
        playing.player.play()
        started(playing)
    }

    fun pause(deck: Int) = decks[deck].player.pause()

    fun seek(deck: Int, seconds: Double) = decks[deck].player.seekTo((seconds * 1000).toLong())

    fun setSound(deck: Int, volume: Float, muted: Boolean) {
        decks[deck].volume = volume
        decks[deck].muted = muted
        decks[deck].applyVolume()
    }

    fun level(deck: Int, level: Float, seconds: Double) = decks[deck].gain.aim(level, seconds)

    fun handOverAt(from: Int, to: Int, atSeconds: Double, fadeSeconds: Double, level: Float) {
        handOver = HandOver(from, to, (atSeconds * 1000).toLong(), fadeSeconds, level)
        lookAtTheHandOver()
    }

    fun cancelHandOver() {
        handOver = null
        main.removeCallbacks(checkHandOver)
    }

    /**
     * The hand over done when its moment has come, or looked at again in
     * time for it: at every word to the page, and, in its last second, at
     * the very moment it is due, which the page in the background could not
     * be counted on for.
     */
    private fun lookAtTheHandOver() {
        val due = handOver ?: return
        val from = decks[due.from].player
        main.removeCallbacks(checkHandOver)
        if (!from.isPlaying) {
            return
        }
        val leftMs = due.atMs - from.currentPosition
        when {
            leftMs <= ON_TIME_MS -> handOver(due)
            leftMs < TELL_EVERY_MS * 2 -> main.postDelayed(checkHandOver, leftMs)
        }
    }

    private fun handOver(due: HandOver) {
        handOver = null
        val from = decks[due.from]
        val to = decks[due.to]
        if (due.fadeSeconds > 0) {
            from.gain.aim(0f, due.fadeSeconds)
            to.gain.aim(0f, 0.0)
        }
        to.gain.aim(due.level, due.fadeSeconds)
        // Said first, so the page takes this deck for the one playing before
        // it hears it start.
        to.events += "handedover"
        to.player.play()
        started(to)
        changed()
    }

    /** A deck asked to sound: the one the card speaks of, with the focus. */
    private fun started(deck: Deck) {
        SoundTrace.lookAround(context, "deck ${deck.index} started")
        live = deck.index
        holdTheFocus()
        face.changed()
    }

    /** The song the card shows, as the page describes it. */
    fun describe(described: Described?) = face.describe(described)

    /** Where the deck the card speaks of has got to. */
    fun livePositionMs(): Long = decks[live].player.currentPosition

    fun liveIsPlaying(): Boolean = decks[live].player.playWhenReady

    fun liveIsWaiting(): Boolean = decks[live].player.playbackState == Player.STATE_BUFFERING

    /** A press on the card, a headset or a watch, for the page to answer. */
    fun asked(asked: String, seconds: Double = 0.0) {
        listener?.asked(asked, seconds)
    }

    /** Everything let go of, the page that led it being gone. */
    fun stopAll() {
        cancelHandOver()
        decks.forEach { it.clear() }
        letGoOfTheFocus()
        face.describe(null)
    }

    // -------------------------------------------------------------------
    // The sound shared with the rest of the phone.
    // -------------------------------------------------------------------

    private val audio = AudioManagerCompat.getAudioManager(context)
    private var focusHeld = false

    /** Lowered for a moment while something else says a few words. */
    private var ducked = 1f

    /** Paused by somebody else, and to be played again when they are done. */
    private var pausedForAMoment = false

    private val focus = AudioFocusRequestCompat.Builder(AudioManagerCompat.AUDIOFOCUS_GAIN)
        .setAudioAttributes(sound)
        .setOnAudioFocusChangeListener({ change -> focusChanged(change) }, main)
        .build()

    private fun holdTheFocus() {
        if (focusHeld) {
            return
        }
        focusHeld = AudioManagerCompat.requestAudioFocus(audio, focus) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        SoundTrace.say("focus asked granted=$focusHeld")
    }

    private fun letGoOfTheFocus() {
        if (focusHeld) {
            AudioManagerCompat.abandonAudioFocusRequest(audio, focus)
        }
        focusHeld = false
        pausedForAMoment = false
    }

    private fun focusChanged(change: Int) {
        SoundTrace.say("focus changed $change")
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS -> {
                focusHeld = false
                pausedForAMoment = false
                decks.forEach { it.player.pause() }
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
                pausedForAMoment = decks[live].player.playWhenReady
                decks.forEach { it.player.pause() }
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                ducked = DUCKED
                decks.forEach { it.applyVolume() }
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                ducked = 1f
                decks.forEach { it.applyVolume() }
                if (pausedForAMoment) {
                    pausedForAMoment = false
                    decks[live].player.play()
                }
            }
        }
    }

    companion object {
        private const val TELL_EVERY_MS = 250L

        /** Close enough to its moment for a hand over to be done now. */
        private const val ON_TIME_MS = 8L

        /** How loud the music stays under somebody else's few words. */
        private const val DUCKED = 0.2f

        // It holds the application's own context, which lives as long as the
        // process does, never a screen's.
        @SuppressLint("StaticFieldLeak")
        @Volatile
        private var made: MusicEngine? = null

        /** The engine, made the first time it is asked for. */
        fun of(context: Context, fetching: () -> DataSource.Factory): MusicEngine =
            made ?: synchronized(this) {
                made ?: MusicEngine(context.applicationContext, fetching()).also { made = it }
            }

        /** The engine if it was made, for the service, which never makes it. */
        fun ifMade(): MusicEngine? = made
    }
}

/** What the engine says of one deck, word for word what the page reads. */
@Serializable
data class DeckTold(
    val deck: Int,
    val position: Double,
    val duration: Double?,
    val buffered: Double,
    val playing: Boolean,
    val advancing: Boolean,
    val error: String?,
    val events: List<String>,
)

/** The song the card shows, as the page describes it. */
@Serializable
data class Described(
    val title: String,
    val artist: String,
    val album: String,
    val artwork: String?,
    val seconds: Double?,
    val canStepOn: Boolean,
)

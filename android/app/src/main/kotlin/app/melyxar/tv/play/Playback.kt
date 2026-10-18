package app.melyxar.tv.play

import android.content.Context
import android.util.Log
import androidx.annotation.OptIn
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Timeline
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.util.EventLogger
import app.melyxar.BuildConfig
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import app.melyxar.player.ClientProfile
import app.melyxar.player.measuredProfile
import app.melyxar.server.Api
import app.melyxar.server.Asked
import app.melyxar.server.Plan
import app.melyxar.server.Refusal
import app.melyxar.server.StreamSession
import app.melyxar.server.Track
import app.melyxar.server.TrackChoice
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import java.io.IOException

/** How often the server is told where the film is, as the web tells it. */
private const val REPORT_EVERY_MS = 10_000L

/** How often the server is asked how far a rebuilt film has got, while it
 *  is on its way, as the web player asks. */
private const val PREPARATION_EVERY_MS = 250L

/** How often a film the server rebuilds is said to be still watched. */
private const val STILL_HERE_EVERY_MS = 30_000L

/** The speeds a film can be played at, as the web's player offers them. */
val SPEEDS = listOf(0.75f, 1f, 1.25f, 1.5f, 1.75f, 2f)

private val sound = AudioAttributes.Builder()
    .setUsage(C.USAGE_MEDIA)
    .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
    .build()

/** What the device plays, measured once for each way its sound may go: its
 *  decoders and its screen do not change while it runs. What the sound goes
 *  through is read by Media3 at each film. Asking every decoder what it reads takes long enough to freeze
 *  the screen, so it is asked away from the thread that draws it. */
private val measured = HashMap<Boolean, JsonObject>()

/** What the device plays, held to [quality] as the web's page holds it. */
private suspend fun profileOf(context: Context, quality: Quality): JsonObject {
    val passedOn = soundPassedOn(context)
    val device = measured[passedOn]
        ?: withContext(Dispatchers.Default) {
            Json.encodeToJsonElement(ClientProfile.serializer(), measuredProfile(context, sound, passedOn)).jsonObject
        }.also { measured[passedOn] = it }
    val ceiling = listOfNotNull(
        quality.height?.let { "max_height" to JsonPrimitive(it) },
        quality.bitrate?.let { "max_bitrate" to JsonPrimitive(it) },
    )
    return JsonObject(device + ceiling)
}

/**
 * One film played on the television: asked of the server, played by Media3
 * from the file itself or from what the server rebuilds, and where it stands
 * told to the server as the web player tells it.
 */
@OptIn(UnstableApi::class)
class Playback(
    private val context: Context,
    private val api: Api,
    private val work: String,
    private val source: String,
    /** Where to start, or nothing for where the server has it stopped. */
    private val from: Double?,
    private val scope: CoroutineScope,
    /** An administrator asked the film to stop. */
    private val onStopped: () -> Unit,
    /** The server holds where the film was left, once it is left. */
    private val onLeft: () -> Unit,
) {
    val player: ExoPlayer = ExoPlayer.Builder(context, DecodedSound(context))
        .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(context, OkHttpDataSource.Factory(api.http))))
        .setAudioAttributes(sound, true)
        .setHandleAudioBecomingNoisy(true)
        .build()

    var plan by mutableStateOf<Plan?>(null)
        private set

    /** The audio track playing, by the server's name for it. */
    var audio by mutableStateOf<String?>(null)
        private set

    /** The subtitle shown, by the server's name for it, or nothing. */
    var subtitle by mutableStateOf<String?>(null)
        private set

    var failed by mutableStateOf(false)
        private set

    private val keptQuality = KeptQuality(context)

    /** What the picture is held to, as this television was left. */
    var quality by mutableStateOf(keptQuality.read())
        private set

    /** What to do with the film's HDR for this film alone: automatic,
     *  always_convert or never_convert, or the account's own choice. */
    var wideGamut by mutableStateOf<String?>(null)
        private set

    var speed by mutableFloatStateOf(1f)
        private set

    /** The subtitle of words shown, laid out in time; empty when none is. */
    var words by mutableStateOf<List<Stretch>>(emptyList())
        private set

    /** How many seconds later the words come than the file says, earlier
     *  below nought, as the web's subtitle sync. */
    var wordsOffset by mutableFloatStateOf(0f)
        private set

    /** Where the words shown were read from. */
    private var wordsFrom: String? = null
    private var reading: Job? = null

    /** How far a film the server rebuilds is on its way, from nought to a
     *  hundred, while it is; nothing for a file played as it is, or once
     *  the film plays. */
    var loading by mutableStateOf<Float?>(null)
        private set
    private var stage = LoadingStage.Opening
    private var counting: Job? = null

    private var stream: StreamSession? = null

    /** The session the server makes the film in, while it does. */
    val streamId: String? get() = stream?.id

    /** The decoders this device took for the picture and the sound, for the
     *  diagnostics; no decoder of sound for one sent as it is to the sound
     *  system. */
    var videoDecoder by mutableStateOf<String?>(null)
        private set
    var audioDecoder by mutableStateOf<String?>(null)
        private set
    private var reporting: Job? = null

    /** How long the film takes to start, for the debug build's log. */
    private val clock = StartClock()
    private var keeping: Job? = null

    init {
        // The words are read and drawn by Melyxar, which can move them.
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .build()
        if (BuildConfig.DEBUG) {
            // Every format, decoder and output of sound the player takes.
            player.addAnalyticsListener(EventLogger(SOUND_LOG))
            player.addAnalyticsListener(clock)
        }
        player.addAnalyticsListener(object : AnalyticsListener {
            override fun onVideoDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
                videoDecoder = decoderName
            }

            override fun onAudioDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) {
                audioDecoder = decoderName
            }

            override fun onAudioDecoderReleased(eventTime: AnalyticsListener.EventTime, decoderName: String) {
                audioDecoder = null
            }
        })
        player.addListener(object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                scope.launch { report(leaving = false) }
            }

            override fun onPlayerError(error: PlaybackException) {
                failed = true
            }

            override fun onTimelineChanged(timeline: Timeline, reason: Int) {
                if (!timeline.isEmpty) reached(LoadingStage.ManifestParsed)
            }

            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_READY || state == Player.STATE_ENDED) reached(LoadingStage.Done)
                // Waiting again in the middle of a film the server rebuilds,
                // after a jump or a stall: counted again from what it says.
                if (state == Player.STATE_BUFFERING && stage == LoadingStage.Done) {
                    stream?.let {
                        stage = LoadingStage.ManifestParsed
                        countTheWait(it.id, afresh = true)
                    }
                }
            }
        })
        scope.launch {
            try {
                clock.mark("screen open, measuring the device")
                val asking = asked(TrackChoice.Unsaid)
                clock.mark("device measured, asking the plan")
                val first = api.plan(source, asking)
                clock.mark("plan received: ${first.method}, quality held to $quality, because ${first.reasons}")
                audio = first.chosenAudioId
                subtitle = first.chosenSubtitleId
                open(first, from ?: first.resumeFromSeconds ?: 0.0)
            } catch (_: IOException) {
                failed = true
            }
        }
    }

    /** Where the film is, in seconds. */
    val seconds: Double get() = player.currentPosition / 1000.0

    /** How long the film lasts, as the server knows it, the player's own
     *  count of a film still being rebuilt being only what is made so far. */
    val length: Double?
        get() = plan?.durationMinutes?.let { it * 60 }
            ?: player.duration.takeIf { it != C.TIME_UNSET }?.let { it / 1000.0 }

    private val painted: Track? get() = plan?.subtitles?.firstOrNull { it.id == subtitle && it.burnsIn }

    /** The subtitle the server paints into the film, if any. */
    private val paintedChoice: TrackChoice get() = painted?.let { TrackChoice.Painted(it.id) } ?: TrackChoice.Unsaid

    private suspend fun asked(subtitle: TrackChoice) = Asked(profileOf(context, quality), audio, subtitle, wideGamut)

    /** Plays a plan from [at] seconds: the file itself, or a film rebuilt
     *  from there. */
    private suspend fun open(plan: Plan, at: Double) {
        this.plan = plan
        if (BuildConfig.DEBUG) {
            Log.d(SOUND_LOG, "plan: ${plan.method}, audio ${plan.audio.firstOrNull { it.id == plan.chosenAudioId }?.codec}")
        }
        closeTheStream()
        val address = if (plan.method == "direct_play") {
            api.address(plan.url)
        } else {
            stage = LoadingStage.Opening
            loading = 0f
            clock.mark("opening a stream on the server")
            api.openStream(source, asked(paintedChoice), at).also {
                clock.mark("stream opened")
                stream = it
                reached(LoadingStage.SessionOpened)
                countTheWait(it.id, afresh = false)
            }.let { api.address(it.playlistUrl) }
        }
        readTheWords()
        val item = MediaItem.Builder()
            .setUri(address)
            .apply { if (plan.method != "direct_play") setMimeType(MimeTypes.APPLICATION_M3U8) }
            .build()
        clock.mark("film handed to the player at $at s")
        player.setMediaItem(item, (at * 1000).toLong())
        player.prepare()
        player.play()
        keepTheStream()
        reportEveryNowAndThen()
    }

    /** Plays it again from where it is, as the server now has it for these
     *  tracks. */
    private fun again(subtitleChoice: TrackChoice) {
        val at = seconds
        scope.launch {
            try {
                open(api.plan(source, asked(subtitleChoice)), at)
            } catch (_: IOException) {
                failed = true
            }
        }
    }

    fun chooseAudio(track: Track) {
        if (track.id == audio) {
            return
        }
        audio = track.id
        remember()
        again(paintedChoice)
    }

    /** Holds the picture to [chosen] from here, kept for the films after. */
    fun chooseQuality(chosen: Quality) {
        if (chosen == quality) {
            return
        }
        quality = chosen
        keptQuality.keep(chosen)
        again(paintedChoice)
    }

    fun chooseWideGamut(choice: String?) {
        if (choice == wideGamut) {
            return
        }
        wideGamut = choice
        again(paintedChoice)
    }

    fun chooseSpeed(chosen: Float) {
        speed = chosen
        player.setPlaybackSpeed(chosen)
    }

    /** A subtitle of words is only shown or hidden; one of pictures, or
     *  leaving one, has the server paint the film again. */
    fun chooseSubtitle(track: Track?) {
        val wasPainted = painted != null
        subtitle = track?.id
        remember()
        when {
            track?.burnsIn == true -> again(TrackChoice.Painted(track.id))
            wasPainted -> again(if (track == null) TrackChoice.Off else TrackChoice.Unsaid)
            else -> readTheWords()
        }
    }

    fun chooseWordsOffset(seconds: Float) {
        wordsOffset = seconds
    }

    private fun remember() {
        scope.launch {
            try {
                api.rememberTracks(work, source, audio, subtitle)
            } catch (_: IOException) {
                // Only the next film's first choice: not worth a word.
            }
        }
    }

    /** Reads the subtitle of words chosen, unless it is the one shown: a
     *  subtitle of pictures is the server's to paint. */
    private fun readTheWords() {
        val from = plan?.subtitles?.firstOrNull { it.id == subtitle && !it.burnsIn }?.url
        if (from == wordsFrom) {
            return
        }
        wordsFrom = from
        reading?.cancel()
        words = emptyList()
        from ?: return
        reading = scope.launch {
            words = try {
                val file = api.file(from)
                withContext(Dispatchers.Default) { wordsOf(file) }
            } catch (_: IOException) {
                // The film plays on without them, as it would with none.
                emptyList()
            }
        }
    }

    private fun keepTheStream() {
        keeping?.cancel()
        val kept = stream ?: return
        keeping = scope.launch {
            while (isActive) {
                delay(STILL_HERE_EVERY_MS)
                val alive = try {
                    api.keepStream(kept.id)
                } catch (_: IOException) {
                    // One that did not arrive says nothing: the next carries
                    // the same news.
                    true
                }
                if (!alive) {
                    // Swept away while paused, or the server started again:
                    // another from where the viewer stands.
                    stream = null
                    plan?.let { open(it, seconds) }
                    return@launch
                }
            }
        }
    }

    private fun reportEveryNowAndThen() {
        reporting?.cancel()
        reporting = scope.launch {
            while (isActive) {
                delay(REPORT_EVERY_MS)
                report(leaving = false)
            }
        }
    }

    private suspend fun report(leaving: Boolean) {
        if (plan == null) {
            return
        }
        try {
            val heard = api.report(work, seconds, paused = !player.isPlaying, leaving = leaving)
            if (heard.stop && !leaving) {
                onStopped()
            }
        } catch (_: Refusal) {
            // A position refused is one older than the server already has.
        } catch (_: IOException) {
            // The next report carries the same news.
        }
    }

    /** A later stage of the wait reached, never an earlier one. */
    private fun reached(next: LoadingStage) {
        if (next > stage) stage = next
        if (stage == LoadingStage.Done) {
            counting?.cancel()
            loading = null
        }
    }

    /**
     * Reads how far the server has got with the first piece, and moves the
     * number shown toward it, until the film plays. Waiting [afresh] in the
     * middle of a film, the server tells of the piece the film was playing
     * from until it starts its tool again where the film went, counting that
     * piece from nothing: no number is shown before, rather than a wrong one.
     */
    private fun countTheWait(id: String, afresh: Boolean) {
        counting?.cancel()
        counting = scope.launch {
            var written = 0f
            var heard = !afresh
            while (isActive && stage != LoadingStage.Done) {
                try {
                    val seen = api.preparation(id)
                    heard = heard || seen.firstWritten < 1f
                    if (heard) {
                        written = maxOf(written, seen.firstWritten)
                        reached(if (seen.firstWritten >= 1f) LoadingStage.Produced else if (seen.step != "starting") LoadingStage.Producing else stage)
                    }
                } catch (_: IOException) {
                    // The next look carries the same news.
                } catch (_: Refusal) {
                    // Nor is a refused look worth troubling the viewer with.
                }
                if (heard && stage != LoadingStage.Done) {
                    loading = glideToward(loading ?: stage.floor, percentOf(stage, written))
                }
                delay(PREPARATION_EVERY_MS)
            }
        }
    }

    private suspend fun closeTheStream() {
        keeping?.cancel()
        counting?.cancel()
        val closing = stream ?: return
        stream = null
        try {
            api.closeStream(closing.id)
        } catch (_: IOException) {
            // Swept away by the server on its own soon after.
        }
    }

    /**
     * The viewer leaves the film: where it was is said one last time, and
     * what the server was rebuilding for it stops. Done whatever becomes of
     * the screen, which is going away.
     */
    @kotlin.OptIn(kotlinx.coroutines.DelicateCoroutinesApi::class)
    fun leave() {
        reporting?.cancel()
        counting?.cancel()
        keeping?.cancel()
        val at = seconds
        val paused = !player.isPlaying
        player.release()
        val closing = stream
        GlobalScope.launch(Dispatchers.IO + NonCancellable) {
            try {
                if (plan != null) {
                    api.report(work, at, paused, leaving = true)
                    withContext(Dispatchers.Main) { onLeft() }
                }
                closing?.let { api.closeStream(it.id) }
            } catch (_: IOException) {
                // The server notices on its own that nobody is watching.
            }
        }
    }

    fun withStep(seconds: Int) = goTo(this.seconds + seconds)

    /** Takes the film to [seconds], never before its start nor past its end. */
    fun goTo(seconds: Double) {
        val to = seconds.coerceAtLeast(0.0)
        player.seekTo((length?.let { to.coerceAtMost(it) } ?: to).times(1000).toLong())
    }

    fun toggle() {
        if (player.isPlaying) {
            player.pause()
        } else {
            if (player.playbackState == Player.STATE_ENDED) {
                player.seekTo(0)
            }
            player.play()
        }
    }
}

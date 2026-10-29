package app.melyxar.tv.play

import android.content.Context
import android.os.SystemClock
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
import app.melyxar.player.RidesOutARestart
import app.melyxar.player.boundedLoading
import app.melyxar.player.lightEffects
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
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import java.io.IOException
import kotlin.math.roundToLong

/** How often the server is told where the film is, as the web tells it. */
private const val REPORT_EVERY_MS = 10_000L

/** How often the server is asked how far a rebuilt film has got, while it
 *  is on its way, as the web player asks. */
private const val PREPARATION_EVERY_MS = 250L

/** How often a film the server rebuilds is said to be still watched. */
private const val STILL_HERE_EVERY_MS = 30_000L

/** How often a playing film is looked at for standing still at a hole in
 *  its sound. */
private const val LOOKED_FOR_A_HOLE_EVERY_MS = 500L

/** A segment smaller than this arrives too fast to be timed fairly. */
private const val TIMED_FROM_BYTES = 64 * 1024

/** A wait this soon after the film opened or jumped is the film arriving,
 *  not the connection failing it. */
private const val OPENING_MS = 5_000L
private const val AFTER_A_JUMP_MS = 4_000L

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
    /** What the account holds the picture to, as it last chose. */
    startingQuality: Quality,
    /** Keeps a quality chosen with the account, for every device. */
    private val keepQuality: (Quality) -> Unit,
    /** An administrator asked the film to stop. */
    private val onStopped: () -> Unit,
    /** The server holds where the film was left, once it is left. */
    private val onLeft: () -> Unit,
) {
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

    /** What the picture is held to, as the account last chose. */
    var quality by mutableStateOf(startingQuality)
        private set

    /** The rung the automatic quality stands on, while it is the one chosen. */
    var adaptiveRung by mutableStateOf<Quality?>(null)
        private set
    private var course: Course? = null
    private var measuredSpeed: Speed? = null

    /** When the film opened last started playing, and when it was last
     *  moved, to tell a stall from the film arriving. */
    private var playingSince: Long? = null
    private var jumpedAt = 0L

    /** What the film is fetched with, each segment timed for the automatic
     *  quality. */
    private val timedHttp = api.http.newBuilder()
        .eventListenerFactory { TimedSegments { bytes, flowingMs -> scope.launch { heardSegment(bytes, flowingMs) } } }
        .build()

    /** What the server is asked to hold the picture to: the viewer's rung, or
     *  the one the automatic quality stands on. */
    private val held: Quality
        get() = if (quality == ADAPTIVE) adaptiveRung ?: ADAPTIVE_RUNGS[UNMEASURED_RUNG] else quality

    private val keptLight = KeptLight(context)

    /** How bright and contrasted the picture is shown, as this television
     *  was left, while it is shown in standard range. */
    var brightness by mutableFloatStateOf(keptLight.brightness())
        private set
    var contrast by mutableFloatStateOf(keptLight.contrast())
        private set

    /** Whether the picture is shown in standard range, the only one lit
     *  here: a film shown in HDR is the screen's to light. */
    val lightsThePicture: Boolean get() = plan?.wideGamut?.converted != false

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
    private var hopping: Job? = null

    /**
     * What plays the film. Made anew, where the film is, when the picture has
     * to start or stop going through Media3's effects, which light it: Media3
     * settles that once, as a player first shows a film, and holds to it for
     * as long as that player lives, so lighting chosen on a player that took
     * the plain way showed only on the next film.
     */
    var player: ExoPlayer by mutableStateOf(newPlayer())
        private set

    /** Whether [player] passes the picture through Media3's effects. */
    private var throughEffects = false

    init {
        scope.launch {
            try {
                if (quality == ADAPTIVE) measureTheConnection()
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

    private fun newPlayer(): ExoPlayer {
        val player = ExoPlayer.Builder(context, DecodedSound(context))
            .setMediaSourceFactory(
                DefaultMediaSourceFactory(DefaultDataSource.Factory(context, OkHttpDataSource.Factory(timedHttp)))
                    .setLoadErrorHandlingPolicy(RidesOutARestart()),
            )
            .setAudioAttributes(sound, true)
            .setHandleAudioBecomingNoisy(true)
            .setLoadControl(boundedLoading())
            .build()
        // The words are read and drawn by Melyxar, which can move them.
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .build()
        if (BuildConfig.TRACES) {
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

            override fun onPositionDiscontinuity(oldPosition: Player.PositionInfo, newPosition: Player.PositionInfo, reason: Int) {
                if (reason == Player.DISCONTINUITY_REASON_SEEK) jumpedAt = SystemClock.elapsedRealtime()
            }

            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_READY && playingSince == null) playingSince = SystemClock.elapsedRealtime()
                if (state == Player.STATE_BUFFERING) stalled()
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
        return player
    }

    /** Where the film is, in seconds. */
    val seconds: Double get() = player.currentPosition / 1000.0

    /** How long the film lasts, as the server knows it, the player's own
     *  count of a film still being rebuilt being only what is made so far. */
    val length: Double?
        get() = plan?.durationSeconds
            ?: player.duration.takeIf { it != C.TIME_UNSET }?.let { it / 1000.0 }

    private val painted: Track? get() = plan?.subtitles?.firstOrNull { it.id == subtitle && it.burnsIn }

    /** The subtitle the server paints into the film, if any. */
    private val paintedChoice: TrackChoice get() = painted?.let { TrackChoice.Painted(it.id) } ?: TrackChoice.Unsaid

    private suspend fun asked(subtitle: TrackChoice) = Asked(profileOf(context, held), audio, subtitle, wideGamut)

    /** Plays a plan from [at] seconds: the file itself, or a film rebuilt
     *  from there, paused unless [going]. */
    private suspend fun open(plan: Plan, at: Double, going: Boolean = true) {
        this.plan = plan
        if (BuildConfig.TRACES) {
            Log.d(SOUND_LOG, "plan: ${plan.method}, audio ${plan.audio.firstOrNull { it.id == plan.chosenAudioId }?.codec}")
        }
        closeTheStream()
        playingSince = null
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
        drawPicture(carryOn = false)
        player.setMediaItem(item, (at * 1000).toLong())
        player.prepare()
        player.playWhenReady = going
        keepTheStream()
        goOnPastHoles()
        reportEveryNowAndThen()
    }

    /** Plays it again from where it is, as the server now has it for these
     *  tracks. */
    private fun again(subtitleChoice: TrackChoice, going: Boolean = true) {
        val at = seconds
        scope.launch {
            try {
                open(api.plan(source, asked(subtitleChoice)), at, going)
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
        keepQuality(chosen)
        if (chosen == ADAPTIVE) {
            scope.launch {
                measureTheConnection()
                again(paintedChoice)
            }
        } else {
            course = null
            adaptiveRung = null
            again(paintedChoice)
        }
    }

    /** Measures the connection before the film is asked for at the rung it
     *  carries: asking first and correcting after would open it twice. */
    private suspend fun measureTheConnection() {
        val speed = connectionSpeed(api)
        // Another rung chosen while it was measured.
        if (quality != ADAPTIVE) return
        measuredSpeed = speed?.let { Speed(it, it) }
        val first = speed?.let(::rungFor) ?: UNMEASURED_RUNG
        course = Course(first, SystemClock.elapsedRealtime())
        adaptiveRung = ADAPTIVE_RUNGS[first]
        tellOfTheRung("measured", null, first)
    }

    private fun heardSegment(bytes: Long, flowingMs: Double) {
        val current = course ?: return
        if (bytes < TIMED_FROM_BYTES || flowingMs <= 0) return
        val seconds = flowingMs / 1000
        val speed = measuredSpeed.withMeasure(bytes * 8 / seconds, seconds)
        measuredSpeed = speed
        course = current.copy(measured = current.measured + 1)
        consider()
    }

    /** The film stopped to wait for what has not arrived, other than while it
     *  opens or right after a jump. */
    private fun stalled() {
        val current = course ?: return
        val since = playingSince ?: return
        val now = SystemClock.elapsedRealtime()
        if (!player.playWhenReady || now - since < OPENING_MS || now - jumpedAt < AFTER_A_JUMP_MS) return
        course = current.copy(stalls = current.stalls + now)
        consider()
    }

    /** Moves the rung when it has to, opening the film again where it is. */
    private fun consider() {
        val current = course ?: return
        val now = SystemClock.elapsedRealtime()
        val film = plan?.film?.let { Film(it.picture?.bitrate ?: it.overallBitrate, it.picture?.height) }
        val heldAhead = (player.bufferedPosition - player.currentPosition) / 1000.0
        val move = nextMove(current, measuredSpeed?.believed, heldAhead, now, film) ?: return
        course = current.afterMoving(move, now)
        adaptiveRung = ADAPTIVE_RUNGS[move.rung]
        tellOfTheRung(move.because.word, current.rung, move.rung)
        again(paintedChoice, going = player.playWhenReady)
    }

    /** Writes into the journal the rung taken, and why, as the web player does. */
    private fun tellOfTheRung(why: String, from: Int?, to: Int) {
        val taken = ADAPTIVE_RUNGS[to]
        val fact = buildJsonObject {
            put("source", source)
            put("saw", "quality_adapted")
            put("why", why)
            measuredSpeed?.let { put("measured_kbps", (it.believed / 1000).roundToLong()) }
            from?.let { ADAPTIVE_RUNGS[it].bitrate }?.let { put("from_kbps", it / 1000) }
            taken.bitrate?.let { put("to_kbps", it / 1000) }
            taken.height?.let { put("to_height", it) }
        }
        scope.launch {
            try {
                api.tellTheJournal(fact)
            } catch (_: IOException) {
                // Only the journal goes without it.
            }
        }
    }

    fun chooseWideGamut(choice: String?) {
        if (choice == wideGamut) {
            return
        }
        wideGamut = choice
        again(paintedChoice)
    }

    fun chooseLight(brightness: Float, contrast: Float) {
        this.brightness = brightness
        this.contrast = contrast
        keptLight.keep(brightness, contrast)
        drawPicture(carryOn = true)
    }

    /**
     * The picture lit as chosen while it is shown in standard range, at once
     * on a player already passing it through the effects. Otherwise on a new
     * player, [carryOn] taking the film over to it where it is; a player
     * passing through the effects is also let go of for a picture shown in
     * HDR, which the effects would draw wrong.
     */
    private fun drawPicture(carryOn: Boolean) {
        val lighting = lightsThePicture && (brightness != 1f || contrast != 1f)
        when {
            // Nothing shown yet: this one takes whichever way is set now.
            player.currentMediaItem == null -> {
                if (lighting) player.setVideoEffects(lightEffects(brightness, contrast))
                throughEffects = lighting
            }
            lighting && !throughEffects || !lightsThePicture && throughEffects -> anotherPlayer(lighting, carryOn)
            throughEffects -> player.setVideoEffects(lightEffects(brightness, contrast))
        }
    }

    private fun anotherPlayer(lighting: Boolean, carryOn: Boolean) {
        val before = player
        val item = before.currentMediaItem.takeIf { carryOn }
        val at = before.currentPosition
        val going = before.playWhenReady
        val speed = before.playbackParameters.speed
        // Let go of first, so the two never hold a film ahead at once.
        before.release()
        val next = newPlayer()
        if (lighting) next.setVideoEffects(lightEffects(brightness, contrast))
        throughEffects = lighting
        next.setPlaybackSpeed(speed)
        item?.let {
            next.setMediaItem(it, at)
            next.playWhenReady = going
            next.prepare()
        }
        player = next
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
                    // another from where the viewer stands, asked apart from
                    // this beat, which opening a film stops.
                    stream = null
                    again(paintedChoice, going = player.playWhenReady)
                    return@launch
                }
            }
        }
    }

    /** A film standing still at a hole in its own sound goes on from where
     *  the sound comes back (see SoundHole.kt). Asked once for each place it
     *  stands at, so a hole the server finds no end to costs one question. */
    private fun goOnPastHoles() {
        hopping?.cancel()
        val kept = stream
        hopping = scope.launch {
            var watch: StillWatch? = null
            var asked: Long? = null
            while (isActive) {
                delay(LOOKED_FOR_A_HOLE_EVERY_MS)
                if (!player.playWhenReady) {
                    watch = null
                    continue
                }
                val at = player.currentPosition
                val seen = watchStill(watch, at, SystemClock.elapsedRealtime(), player.bufferedPosition - at)
                watch = seen.watch
                if (!seen.stuck || asked == at) continue
                asked = at
                val back = try {
                    if (kept != null) api.soundAfter(kept.id, at / 1000.0) else api.soundAfterInTheFile(source, audio, at / 1000.0)
                } catch (_: IOException) {
                    null
                } catch (_: Refusal) {
                    null
                }
                if (BuildConfig.TRACES) {
                    Log.d(SOUND_LOG, "stood still at $at ms, the sound comes back at $back s")
                }
                if (back != null && player.currentPosition == at) {
                    player.seekTo((back * 1000).toLong())
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
        hopping?.cancel()
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
        hopping?.cancel()
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
        if (player.isPlaying) pause() else play()
    }

    fun play() {
        if (player.playbackState == Player.STATE_ENDED) {
            player.seekTo(0)
        }
        player.play()
    }

    fun pause() = player.pause()
}

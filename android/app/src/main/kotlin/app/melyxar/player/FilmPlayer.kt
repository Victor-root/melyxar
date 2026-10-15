package app.melyxar.player

import android.app.PictureInPictureParams
import android.content.pm.ActivityInfo
import android.graphics.Color
import android.graphics.Rect
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Rational
import android.view.View
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.annotation.OptIn
import androidx.annotation.RequiresApi
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.effect.ScaleAndRotateTransformation
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.hls.HlsMediaSource
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.ProgressiveMediaSource
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * The film, played by Media3 beneath the server's page, for the page's player.
 *
 * The page keeps its own controls, words and reports, and asks this player for
 * what a browser's video element would have done (see the page's
 * `app-player.ts`, which this answers word for word). What happens comes back
 * to the page a few times a second, and at once whenever something happens.
 *
 * One film at a time: each is numbered by the page, and a word about any
 * other number is one about a film already gone.
 */
@OptIn(UnstableApi::class)
class FilmPlayer(
    private val activity: ComponentActivity,
    private val page: WebView,
    /** The page's own colour, put back behind it once no film is under it. */
    private val ground: Int,
    /** Whether an address is the server's: nothing else is played. */
    private val isTheServers: (String) -> Boolean,
    /** Said when a film comes under the page and when it goes. */
    private val onShown: (Boolean) -> Unit,
) {
    private val sound = AudioAttributes.Builder()
        .setUsage(C.USAGE_MEDIA)
        .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
        .build()

    /** Measured once: what the device decodes does not change while it runs. */
    private val measured by lazy { Json.encodeToString(measuredProfile(activity, sound)) }

    /** Everything is fetched as the page fetches it, signed in with its
     *  cookie and named as it is. */
    private val fetching: DataSource.Factory = ResolvingDataSource.Factory(
        DefaultHttpDataSource.Factory()
            .setUserAgent(page.settings.userAgentString)
            .setConnectTimeoutMs(SEGMENT_PATIENCE_MS)
            .setReadTimeoutMs(SEGMENT_PATIENCE_MS),
    ) { asked ->
        CookieManager.getInstance().getCookie(asked.uri.toString())
            ?.let { asked.withAdditionalHeaders(mapOf("Cookie" to it)) }
            ?: asked
    }

    val picture = PlayerView(activity).apply {
        useController = false
        resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
        setShutterBackgroundColor(Color.BLACK)
        // The page draws the words.
        subtitleView?.visibility = View.GONE
        visibility = View.GONE
    }

    private var made: ExoPlayer? = null
    private val player: ExoPlayer
        get() = made ?: makePlayer().also {
            made = it
            picture.player = it
        }

    private val beat = Handler(Looper.getMainLooper())

    /** The film playing now, by the page's number for it. */
    private var current: Int? = null
    private var volume = 1f
    private var muted = false
    private var turn = 0
    private var filled = false
    private var metadataSaid = false
    private var seeking = false
    private var error: String? = null
    private val events = mutableListOf<String>()

    private val tell = object : Runnable {
        override fun run() {
            val id = current ?: return
            page.evaluateJavascript("window.melyxarPlayerTold && window.melyxarPlayerTold(${Json.encodeToString(toldOf(id))})", null)
            events.clear()
            beat.postDelayed(this, TELL_EVERY_MS)
        }
    }

    private fun makePlayer() = ExoPlayer.Builder(activity, DefaultRenderersFactory(activity).setEnableDecoderFallback(true))
        .setAudioAttributes(sound, true)
        .setHandleAudioBecomingNoisy(true)
        .build()
        .apply {
            trackSelectionParameters = trackSelectionParameters.buildUpon()
                .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
                .build()
            addListener(Heard())
        }

    private inner class Heard : Player.Listener {
        override fun onPlaybackStateChanged(state: Int) {
            when (state) {
                Player.STATE_READY -> {
                    if (!metadataSaid) {
                        metadataSaid = true
                        events += "loadedmetadata"
                    }
                    settleSeek()
                }
                Player.STATE_BUFFERING -> if (player.playWhenReady) events += "waiting"
                Player.STATE_ENDED -> {
                    settleSeek()
                    events += listOf("pause", "ended")
                }
                // Only ever stopped by the page, which knows it already.
                Player.STATE_IDLE -> Unit
            }
            tellNow()
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            events += if (playWhenReady) "play" else "pause"
            tellNow()
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (isPlaying) {
                events += "playing"
            }
            picture.keepScreenOn = isPlaying
            shapeTheWindow()
            tellNow()
        }

        override fun onRenderedFirstFrame() {
            settleSeek()
            tellNow()
        }

        override fun onVideoSizeChanged(videoSize: VideoSize) {
            shapeTheWindow()
        }

        override fun onPlayerError(failure: PlaybackException) {
            error = "${failure.errorCodeName}: ${failure.message}"
            events += "error"
            tellNow()
        }
    }

    private fun settleSeek() {
        if (seeking) {
            seeking = false
            events += "seeked"
        }
    }

    private fun tellNow() {
        beat.removeCallbacks(tell)
        beat.post(tell)
    }

    private fun toldOf(id: Int): Told {
        val counters = player.videoDecoderCounters?.also { it.ensureUpdated() }
        val size = player.videoSize
        return Told(
            id = id,
            position = player.currentPosition / 1000.0,
            duration = player.duration.takeIf { it != C.TIME_UNSET }?.let { it / 1000.0 },
            buffered = player.bufferedPosition / 1000.0,
            playing = player.playWhenReady && player.playbackState != Player.STATE_ENDED,
            advancing = player.isPlaying,
            ended = player.playbackState == Player.STATE_ENDED,
            seeking = seeking,
            width = (size.width * size.pixelWidthHeightRatio).toInt(),
            height = size.height,
            volume = volume,
            muted = muted,
            shown = counters?.renderedOutputBufferCount ?: 0,
            dropped = counters?.droppedBufferCount ?: 0,
            error = error,
            events = events.toList(),
        )
    }

    /** Runs what the page asked on the thread the player lives on, for the
     *  film it named only. */
    private fun forFilm(id: Int, request: () -> Unit) = activity.runOnUiThread {
        if (id == current) {
            request()
        }
    }

    @JavascriptInterface
    fun open(id: Int, address: String, settings: String) {
        if (!isTheServers(address)) {
            return
        }
        val asked = READ.decodeFromString<OpenSettings>(settings)
        activity.runOnUiThread {
            current = id
            metadataSaid = false
            seeking = false
            error = null
            events.clear()
            volume = asked.volume
            muted = asked.muted
            player.volume = if (muted) 0f else volume
            player.setPlaybackSpeed(asked.rate)
            player.repeatMode = if (asked.loop) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
            shape(asked.fit, asked.turn)
            player.setMediaSource(sourceOf(address), (asked.start * 1000).toLong())
            player.playWhenReady = asked.autoplay
            player.prepare()
            picture.visibility = View.VISIBLE
            page.setBackgroundColor(Color.TRANSPARENT)
            onShown(true)
            tellNow()
        }
    }

    /** A playlist the server rebuilds, or the file itself. */
    private fun sourceOf(address: String): MediaSource {
        val item = MediaItem.fromUri(address)
        return if (address.substringBefore('?').endsWith(".m3u8")) {
            HlsMediaSource.Factory(fetching).createMediaSource(item)
        } else {
            ProgressiveMediaSource.Factory(fetching).createMediaSource(item)
        }
    }

    /** From the beginning again once it has ended, as a browser does. */
    @JavascriptInterface
    fun play(id: Int) = forFilm(id) {
        if (player.playbackState == Player.STATE_ENDED) {
            player.seekTo(0)
        }
        player.play()
    }

    @JavascriptInterface
    fun pause(id: Int) = forFilm(id) { player.pause() }

    @JavascriptInterface
    fun seek(id: Int, seconds: Double) = forFilm(id) {
        seeking = true
        player.seekTo((seconds * 1000).toLong())
    }

    @JavascriptInterface
    fun setSound(id: Int, volume: Double, muted: Boolean) = forFilm(id) {
        this.volume = volume.toFloat()
        this.muted = muted
        player.volume = if (muted) 0f else this.volume
    }

    @JavascriptInterface
    fun setRate(id: Int, rate: Double) = forFilm(id) { player.setPlaybackSpeed(rate.toFloat()) }

    @JavascriptInterface
    fun setLoop(id: Int, loop: Boolean) = forFilm(id) {
        player.repeatMode = if (loop) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
    }

    @JavascriptInterface
    fun setShape(id: Int, fit: String, turn: Int) = forFilm(id) { shape(fit, turn) }

    @JavascriptInterface
    fun close(id: Int) = forFilm(id) { closeFilm() }

    @JavascriptInterface
    fun profile(): String = measured

    @JavascriptInterface
    fun pictureInPicture() = activity.runOnUiThread { intoTheWindow() }

    @JavascriptInterface
    fun fillScreen(filled: Boolean) = activity.runOnUiThread { fill(filled) }

    private fun shape(fit: String, turn: Int) {
        picture.resizeMode = when (fit) {
            "cover" -> AspectRatioFrameLayout.RESIZE_MODE_ZOOM
            "stretch" -> AspectRatioFrameLayout.RESIZE_MODE_FILL
            else -> AspectRatioFrameLayout.RESIZE_MODE_FIT
        }
        if (turn != this.turn) {
            this.turn = turn
            // Media3 turns the other way round from the page.
            player.setVideoEffects(
                if (turn == 0) {
                    emptyList()
                } else {
                    listOf(ScaleAndRotateTransformation.Builder().setRotationDegrees((360 - turn).toFloat()).build())
                },
            )
        }
    }

    private fun closeFilm() {
        current = null
        beat.removeCallbacks(tell)
        player.stop()
        player.clearMediaItems()
        picture.visibility = View.GONE
        page.setBackgroundColor(ground)
        onShown(false)
        fill(false)
        shapeTheWindow()
    }

    /** The whole screen, the phone's bars put away, and turned on its side
     *  for a picture wider than tall, the way every phone shows a film. */
    private fun fill(filled: Boolean) {
        if (filled == this.filled) {
            return
        }
        this.filled = filled
        val bars = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        if (filled) {
            bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            bars.hide(WindowInsetsCompat.Type.systemBars())
            val size = player.videoSize
            if (size.width * size.pixelWidthHeightRatio > size.height) {
                activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            }
        } else {
            bars.show(WindowInsetsCompat.Type.systemBars())
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }
    }

    /** The shape of the window a film goes to, and from Android 12 whether
     *  leaving the screen sends it there by itself: while it plays. */
    private fun shapeTheWindow() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            activity.setPictureInPictureParams(windowShape())
        }
    }

    @RequiresApi(Build.VERSION_CODES.O)
    private fun windowShape(): PictureInPictureParams {
        val size = made?.videoSize
        val ratio = if (size != null && size.width > 0 && size.height > 0) {
            Rational((size.width * size.pixelWidthHeightRatio).toInt(), size.height)
                .coerceIn(Rational(100, 239), Rational(239, 100))
        } else {
            Rational(16, 9)
        }
        // Where the picture is now, so the system grows the window out of it.
        val shown = Rect().also { picture.getGlobalVisibleRect(it) }
        val shape = PictureInPictureParams.Builder().setAspectRatio(ratio).setSourceRectHint(shown)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            shape.setAutoEnterEnabled(current != null && made?.isPlaying == true)
        }
        return shape.build()
    }

    private fun intoTheWindow() {
        if (current != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            activity.enterPictureInPictureMode(windowShape())
        }
    }

    /** Before Android 12, leaving the screen while a film plays is caught by
     *  the screen and handed here, to put it in a window of its own. */
    fun leaving() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S && made?.isPlaying == true) {
            intoTheWindow()
        }
    }

    /** The page itself is loaded again: whatever film it had is gone with it. */
    fun pageLeft() {
        if (current != null) {
            closeFilm()
        }
    }

    /** In a window of its own, the film is shown alone, as a browser shows
     *  it: the controls are far too small to use there. */
    fun windowed(inWindow: Boolean) {
        page.visibility = if (inWindow) View.INVISIBLE else View.VISIBLE
    }

    /** The screen is no longer seen at all. */
    fun stopped() {
        made?.pause()
    }

    fun release() {
        beat.removeCallbacks(tell)
        made?.release()
        made = null
    }

    private companion object {
        const val TELL_EVERY_MS = 250L

        /** Above the server's own patience with a piece of film it is
         *  rebuilding, as the page's own player is. */
        const val SEGMENT_PATIENCE_MS = 40_000

        val READ = Json { ignoreUnknownKeys = true }
    }
}

/** What the page sets when it opens a film. */
@Serializable
private data class OpenSettings(
    val start: Double,
    val autoplay: Boolean,
    val volume: Float,
    val muted: Boolean,
    val rate: Float,
    val loop: Boolean,
    val fit: String,
    val turn: Int,
)

/** What the page is told of the film. */
@Serializable
private data class Told(
    val id: Int,
    val position: Double,
    val duration: Double?,
    val buffered: Double,
    val playing: Boolean,
    val advancing: Boolean,
    val ended: Boolean,
    val seeking: Boolean,
    val width: Int,
    val height: Int,
    val volume: Float,
    val muted: Boolean,
    val shown: Int,
    val dropped: Int,
    val error: String?,
    val events: List<String>,
)

/** Kept within what the system takes for a window's shape. */
private fun Rational.coerceIn(least: Rational, most: Rational): Rational = when {
    this < least -> least
    this > most -> most
    else -> this
}

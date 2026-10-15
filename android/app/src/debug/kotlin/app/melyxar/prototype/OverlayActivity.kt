package app.melyxar.prototype

import android.annotation.SuppressLint
import android.app.PictureInPictureParams
import android.content.res.Configuration
import android.graphics.Rect
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Rational
import android.view.View
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.OptIn
import androidx.annotation.RequiresApi
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import androidx.webkit.WebViewAssetLoader
import app.melyxar.BuildConfig
import org.json.JSONObject

/**
 * The prototype the Android application is built on: a video drawn by Media3,
 * and over it, in a see-through web view, controls written as the interface's
 * own are. It answers one question before anything else is built: whether a
 * page laid over a native picture stays smooth, in full screen and in a
 * window of its own, on the phone in hand.
 *
 * The video is any file picked on the phone, so nothing here depends on a
 * server. The page measures its own frames and is told the frames the player
 * dropped.
 */
@OptIn(UnstableApi::class)
class OverlayActivity : ComponentActivity() {
    private lateinit var player: ExoPlayer
    private lateinit var picture: PlayerView
    private lateinit var controls: WebView
    private val beat = Handler(Looper.getMainLooper())
    private var dropped = 0

    private val choose = registerForActivityResult(ActivityResultContracts.OpenDocument()) { picked: Uri? ->
        picked?.let {
            player.setMediaItem(MediaItem.fromUri(it))
            player.prepare()
            player.play()
        }
    }

    /** What the page is told of the player, four times a second. */
    private val tell = object : Runnable {
        override fun run() {
            val state = JSONObject()
                .put("position", player.currentPosition)
                .put("duration", player.duration.coerceAtLeast(0))
                .put("buffered", player.bufferedPosition)
                .put("playing", player.isPlaying)
                .put("dropped", dropped)
            controls.evaluateJavascript("window.fromNative && window.fromNative($state)", null)
            beat.postDelayed(this, TELL_EVERY_MS)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        WindowCompat.getInsetsController(window, window.decorView).apply {
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            hide(WindowInsetsCompat.Type.systemBars())
        }

        player = ExoPlayer.Builder(this).build().apply {
            addAnalyticsListener(object : AnalyticsListener {
                override fun onDroppedVideoFrames(time: AnalyticsListener.EventTime, frames: Int, elapsedMs: Long) {
                    dropped += frames
                }
            })
            addListener(object : Player.Listener {
                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        setPictureInPictureParams(windowShape())
                    }
                }
            })
        }
        picture = PlayerView(this).apply {
            useController = false
            resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
            setShutterBackgroundColor(Color.BLACK)
            player = this@OverlayActivity.player
        }
        controls = controlsPage()

        val holder = FrameLayout(this).apply { setBackgroundColor(Color.BLACK) }
        holder.addView(picture, FrameLayout.LayoutParams(MATCH, MATCH))
        holder.addView(controls, FrameLayout.LayoutParams(MATCH, MATCH))
        setContentView(holder)
        controls.loadUrl("https://${WebViewAssetLoader.DEFAULT_DOMAIN}/assets/prototype/overlay.html")
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun controlsPage(): WebView {
        val assets = WebViewAssetLoader.Builder()
            .addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(this))
            .build()
        return WebView(this).apply {
            WebView.setWebContentsDebuggingEnabled(BuildConfig.DEBUG)
            // See-through: the picture under it shows wherever the page
            // paints nothing.
            setBackgroundColor(Color.TRANSPARENT)
            settings.javaScriptEnabled = true
            settings.allowFileAccess = false
            addJavascriptInterface(Bridge(), "Native")
            webViewClient = ControlsClient(assets)
        }
    }

    // The check counts the call to the parent's constructor that a Kotlin
    // class makes as a bare WebViewClient made without that method: this one
    // has it, below.
    @SuppressLint("MissingOnRenderProcessGone")
    private inner class ControlsClient(private val assets: WebViewAssetLoader) : WebViewClient() {
        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
            assets.shouldInterceptRequest(request.url)

        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            recreate()
            return true
        }
    }

    /** What the page asks of the player. Called off the main thread, so each
     *  is handed to it. */
    private inner class Bridge {
        @JavascriptInterface
        fun chooseVideo() = runOnUiThread { choose.launch(arrayOf("video/*")) }

        @JavascriptInterface
        fun play() = runOnUiThread { player.play() }

        @JavascriptInterface
        fun pause() = runOnUiThread { player.pause() }

        @JavascriptInterface
        fun seekTo(positionMs: Double) = runOnUiThread { player.seekTo(positionMs.toLong()) }

        @JavascriptInterface
        fun resetDropped() = runOnUiThread { dropped = 0 }

        @JavascriptInterface
        fun pictureInPicture() = runOnUiThread { enterWindow() }
    }

    /** The shape of the window the film goes to, and from Android 12 whether
     *  leaving the screen sends it there by itself: while it plays. */
    @RequiresApi(Build.VERSION_CODES.O)
    private fun windowShape(): PictureInPictureParams {
        val size = player.videoSize
        val ratio = if (size.width > 0 && size.height > 0) {
            Rational(size.width, size.height).coerceIn(Rational(100, 239), Rational(239, 100))
        } else {
            Rational(16, 9)
        }
        // Where the picture is now, so the system grows the window out of it.
        val shown = Rect().also { picture.getGlobalVisibleRect(it) }
        val shape = PictureInPictureParams.Builder().setAspectRatio(ratio).setSourceRectHint(shown)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            shape.setAutoEnterEnabled(player.isPlaying)
        }
        return shape.build()
    }

    private fun enterWindow() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            enterPictureInPictureMode(windowShape())
        }
    }

    /** Before Android 12, leaving the screen while the film plays is caught
     *  here to put it in a window of its own. */
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S && player.isPlaying) {
            enterWindow()
        }
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        controls.visibility = if (isInPictureInPictureMode) View.GONE else View.VISIBLE
    }

    override fun onStart() {
        super.onStart()
        beat.post(tell)
    }

    override fun onStop() {
        super.onStop()
        beat.removeCallbacks(tell)
        player.pause()
    }

    override fun onDestroy() {
        controls.destroy()
        player.release()
        super.onDestroy()
    }

    private companion object {
        const val MATCH = ViewGroup.LayoutParams.MATCH_PARENT
        const val TELL_EVERY_MS = 250L
    }
}

/** Kept within what the system takes for a window's shape. */
private fun Rational.coerceIn(least: Rational, most: Rational): Rational = when {
    this < least -> least
    this > most -> most
    else -> this
}

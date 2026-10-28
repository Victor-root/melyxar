package app.melyxar.web

import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.graphics.Color
import android.os.Bundle
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.widget.FrameLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.annotation.OptIn
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import app.melyxar.R
import app.melyxar.player.CameraJournal
import app.melyxar.player.livePlayer
import app.melyxar.server.network
import kotlinx.serialization.json.JsonObject
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

/**
 * A camera watched live on the phone, on a screen of its own over the page,
 * which the back key closes. The camera is opened by the phone itself at the
 * address the page handed over, account written in; nothing passes through
 * the server. Let go as soon as the screen is no longer seen, and opened
 * again on now when it comes back.
 */
@OptIn(UnstableApi::class)
class LiveActivity : ComponentActivity() {
    private lateinit var picture: PlayerView
    private lateinit var said: TextView
    private var player: ExoPlayer? = null
    private lateinit var journal: CameraJournal

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        journal = CameraJournal(intent.getStringExtra(NAME).orEmpty(), "phone", toTheServer())
        journal.screen("made")
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        WindowCompat.getInsetsController(window, window.decorView).apply {
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            hide(WindowInsetsCompat.Type.systemBars())
        }
        picture = PlayerView(this).apply {
            useController = false
            resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
            setShutterBackgroundColor(Color.BLACK)
            keepScreenOn = true
        }
        said = TextView(this).apply {
            text = getString(R.string.camera_live_name, intent.getStringExtra(NAME).orEmpty())
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            setShadowLayer(6f, 0f, 1f, Color.BLACK)
        }
        val padding = (24 * resources.displayMetrics.density).toInt()
        setContentView(
            FrameLayout(this).apply {
                setBackgroundColor(Color.BLACK)
                addView(picture, FrameLayout.LayoutParams(MATCH, MATCH))
                addView(
                    said,
                    FrameLayout.LayoutParams(WRAP, WRAP, Gravity.TOP or Gravity.START).apply {
                        setMargins(padding, padding, padding, padding)
                    },
                )
            },
        )
    }

    override fun onStart() {
        super.onStart()
        val address = intent.getStringExtra(ADDRESS) ?: return finish()
        journal.screen("seen")
        player = livePlayer(this, address, journal).also { opened ->
            opened.addListener(object : Player.Listener {
                override fun onPlayerError(error: PlaybackException) {
                    journal.screen("cannot_play")
                    said.text = getString(R.string.camera_cannot_play)
                    said.visibility = View.VISIBLE
                }
            })
            picture.player = opened
        }
    }

    override fun onStop() {
        journal.screen("not_seen")
        picture.player = null
        player?.release()
        player = null
        super.onStop()
    }

    override fun onDestroy() {
        journal.screen("closed")
        super.onDestroy()
    }

    override fun onTrimMemory(level: Int) {
        journal.screen("memory_asked", level)
        super.onTrimMemory(level)
    }

    /** Hands a fact to the server's journal, signed in as the page is, under
     *  the page's own name. */
    private fun toTheServer(): (JsonObject) -> Unit {
        val origin = intent.getStringExtra(ORIGIN).orEmpty()
        val agent = intent.getStringExtra(AGENT).orEmpty()
        return { fact ->
            val url = "$origin/api/v1/system/journal/camera"
            val asked = Request.Builder()
                .url(url)
                .header("User-Agent", agent)
                .post(fact.toString().toRequestBody("application/json".toMediaType()))
            CookieManager.getInstance().getCookie(url)?.let { asked.header("Cookie", it) }
            network.newCall(asked.build()).execute().close()
        }
    }

    private companion object {
        const val ADDRESS = "address"
        const val NAME = "name"
        const val ORIGIN = "origin"
        const val AGENT = "agent"
        const val MATCH = FrameLayout.LayoutParams.MATCH_PARENT
        const val WRAP = FrameLayout.LayoutParams.WRAP_CONTENT
    }

    /** What the page asks of the phone's camera screen (see the page's
     *  `app-camera.ts`). */
    class Bridge(private val context: Context, private val origin: () -> String, private val agent: String) {
        @JavascriptInterface
        fun watch(address: String, name: String) {
            if (!address.startsWith("rtsp://") && !address.startsWith("rtsps://")) {
                return
            }
            context.startActivity(
                Intent(context, LiveActivity::class.java)
                    .putExtra(ADDRESS, address)
                    .putExtra(NAME, name)
                    .putExtra(ORIGIN, origin())
                    .putExtra(AGENT, agent),
            )
        }
    }
}

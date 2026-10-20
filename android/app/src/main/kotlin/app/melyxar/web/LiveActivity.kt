package app.melyxar.web

import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.graphics.Color
import android.os.Bundle
import android.util.TypedValue
import android.view.Gravity
import android.view.View
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
import app.melyxar.player.livePlayer

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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
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
        player = livePlayer(this, address).also { opened ->
            opened.addListener(object : Player.Listener {
                override fun onPlayerError(error: PlaybackException) {
                    said.text = getString(R.string.camera_cannot_play)
                    said.visibility = View.VISIBLE
                }
            })
            picture.player = opened
        }
    }

    override fun onStop() {
        picture.player = null
        player?.release()
        player = null
        super.onStop()
    }

    private companion object {
        const val ADDRESS = "address"
        const val NAME = "name"
        const val MATCH = FrameLayout.LayoutParams.MATCH_PARENT
        const val WRAP = FrameLayout.LayoutParams.WRAP_CONTENT
    }

    /** What the page asks of the phone's camera screen (see the page's
     *  `app-camera.ts`). */
    class Bridge(private val context: Context) {
        @JavascriptInterface
        fun watch(address: String, name: String) {
            if (!address.startsWith("rtsp://") && !address.startsWith("rtsps://")) {
                return
            }
            context.startActivity(
                Intent(context, LiveActivity::class.java)
                    .putExtra(ADDRESS, address)
                    .putExtra(NAME, name),
            )
        }
    }
}

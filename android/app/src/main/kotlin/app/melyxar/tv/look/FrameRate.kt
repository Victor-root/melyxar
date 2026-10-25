package app.melyxar.tv.look

import android.content.Context
import android.content.SharedPreferences
import android.os.Handler
import android.os.HandlerThread
import android.view.FrameMetrics
import android.view.Window
import androidx.activity.compose.LocalActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.edit
import app.melyxar.R
import kotlinx.coroutines.delay
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

/**
 * Whether this television shows, in a corner, how many frames its screens
 * draw each second and how long the slowest of them took: chosen in its
 * settings alone and kept on it, for seeing where it stutters.
 */
object FrameRate {
    private const val KEPT = "television"
    private const val SHOWN = "frame_rate_shown"

    private var kept: SharedPreferences? = null

    var on by mutableStateOf(false)
        private set

    fun keep(context: Context) {
        kept = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE).also { on = it.getBoolean(SHOWN, false) }
    }

    fun choose(shown: Boolean) {
        on = shown
        kept?.edit { putBoolean(SHOWN, shown) }
    }
}

/** How often the corner says what it counted. */
private const val SAID_EVERY_MS = 1_000L

/**
 * The frames drawn over the last second and the slowest of them, read from
 * the window as each frame is finished, away from the thread that draws. A
 * screen standing still draws nothing, and says so: the one frame a second it
 * then shows is this corner drawing its own number.
 */
@Composable
fun FrameRateMeter(modifier: Modifier) {
    if (!FrameRate.on) return
    val window = LocalActivity.current?.window ?: return
    val frames = remember { AtomicInteger() }
    val slowestNs = remember { AtomicLong() }
    var said by remember { mutableStateOf<Pair<Int, Long>?>(null) }

    DisposableEffect(window) {
        val thread = HandlerThread("frame-rate").apply { start() }
        val listener = Window.OnFrameMetricsAvailableListener { _, metrics, _ ->
            frames.incrementAndGet()
            slowestNs.accumulateAndGet(metrics.getMetric(FrameMetrics.TOTAL_DURATION), ::maxOf)
        }
        window.addOnFrameMetricsAvailableListener(listener, Handler(thread.looper))
        onDispose {
            window.removeOnFrameMetricsAvailableListener(listener)
            thread.quitSafely()
        }
    }
    LaunchedEffect(Unit) {
        while (true) {
            delay(SAID_EVERY_MS)
            said = frames.getAndSet(0) to slowestNs.getAndSet(0) / 1_000_000
        }
    }

    val (perSecond, slowestMs) = said ?: return
    Text(
        stringResource(R.string.tv_frame_rate_meter, perSecond, slowestMs),
        fontSize = 15.sp,
        fontFamily = FontFamily.Monospace,
        color = Color.White,
        modifier = modifier
            .padding(12.dp)
            .background(Color.Black.copy(alpha = 0.6f), RoundedCornerShape(6.dp))
            .padding(horizontal = 10.dp, vertical = 4.dp),
    )
}

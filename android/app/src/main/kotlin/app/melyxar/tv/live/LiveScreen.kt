package app.melyxar.tv.live

import android.view.View
import android.view.ViewGroup
import androidx.annotation.OptIn
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import app.melyxar.R
import app.melyxar.player.livePlayer
import app.melyxar.server.Camera
import app.melyxar.tv.inside.NoBarPinned
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import java.io.IOException
import kotlinx.coroutines.delay

/** How long the camera's name stays over the picture once a key is let go. */
private const val SHOWN_FOR_MS = 3_000L

/**
 * A camera watched live, on the whole screen. The address, its account
 * written in, is asked of the server as the screen opens and held only while
 * it is open; the picture comes from the camera itself. Any key shows the
 * camera's name a moment, and the back key closes it.
 */
@OptIn(UnstableApi::class)
@Composable
fun LiveScreen(camera: Camera) {
    val palette = LocalPalette.current
    val api = LocalServer.current
    NoBarPinned()
    // Asked here rather than through the answers kept for the screens: it
    // holds a password, and is let go with the screen.
    var address by remember { mutableStateOf<String?>(null) }
    var failed by remember { mutableStateOf(false) }
    var playing by remember { mutableStateOf(false) }
    var nudges by remember { mutableIntStateOf(0) }
    var shown by remember { mutableStateOf(true) }
    LaunchedEffect(camera.id) {
        address = try {
            api.liveAddress(camera.id)
        } catch (_: IOException) {
            failed = true
            null
        }
    }
    LaunchedEffect(nudges, playing) {
        shown = true
        if (playing) {
            delay(SHOWN_FOR_MS)
            shown = false
        }
    }

    val focus = remember { FocusRequester() }
    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .focusRequester(focus)
            .onPreviewKeyEvent { event ->
                if (event.type == KeyEventType.KeyDown) nudges += 1
                false
            }
            .focusable(),
    ) {
        address?.let { opened ->
            LivePicture(opened, onPlaying = { playing = it }, onFailed = { failed = true })
        }
        if (failed) {
            Text(
                stringResource(R.string.tv_camera_cannot_play),
                fontSize = 18.sp,
                fontWeight = FontWeight.Medium,
                color = palette.text,
                textAlign = TextAlign.Center,
                modifier = Modifier.align(Alignment.Center).padding(horizontal = Gutter),
            )
        }
        AnimatedVisibility(shown || !playing, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.fillMaxWidth()) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.75f), Color.Transparent)))
                    .padding(start = Gutter, end = Gutter, top = 28.dp, bottom = 40.dp),
            ) {
                Glyph(Icons.Camera, palette.accentBright, 24.dp)
                Text(
                    camera.name,
                    fontSize = 22.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = palette.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Text(stringResource(R.string.tv_camera_live), fontSize = 15.sp, color = palette.accentBright)
            }
        }
    }
    LaunchedEffect(Unit) { focus.requestFocus() }
}

/** The camera's own picture, opened at [address] and let go with the screen. */
@OptIn(UnstableApi::class)
@Composable
private fun LivePicture(address: String, onPlaying: (Boolean) -> Unit, onFailed: () -> Unit) {
    val context = LocalContext.current
    val player: ExoPlayer = remember(address) { livePlayer(context, address) }
    DisposableEffect(player) {
        val heard = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) = onPlaying(isPlaying)

            override fun onPlayerError(error: PlaybackException) = onFailed()
        }
        player.addListener(heard)
        onDispose {
            player.removeListener(heard)
            player.release()
        }
    }
    // A live picture is not left playing behind the television's other apps,
    // and comes back to now rather than to where it was left.
    LifecycleEventEffect(Lifecycle.Event.ON_STOP) { player.stop() }
    LifecycleEventEffect(Lifecycle.Event.ON_START) {
        player.prepare()
        player.play()
    }
    AndroidView(
        factory = { made ->
            PlayerView(made).apply {
                useController = false
                resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                setShutterBackgroundColor(android.graphics.Color.BLACK)
                isFocusable = false
                descendantFocusability = ViewGroup.FOCUS_BLOCK_DESCENDANTS
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
                this.player = player
            }
        },
        update = { view -> view.keepScreenOn = true },
        modifier = Modifier.fillMaxSize(),
    )
}

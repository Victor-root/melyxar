package app.melyxar.tv.trailer

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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import app.melyxar.R
import app.melyxar.tv.clock
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.NoBarPinned
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.rail
import kotlinx.coroutines.delay

/** How far one press to the side moves the trailer, in milliseconds. */
private const val STEP_MS = 10_000L

/** How long what is said over the trailer stays once the remote stops. */
private const val SHOWN_FOR_MS = 3_000L

/** How often the bar follows the trailer while it is shown. */
private const val FOLLOWED_EVERY_MS = 250L

/** The keys the trailer answers, down and up both taken so nothing behind
 *  it hears them: the keys of music among them, which would move a song. */
private val PAUSING = setOf(Key.DirectionCenter, Key.Enter, Key.NumPadEnter, Key.MediaPlayPause, Key.MediaPlay, Key.MediaPause)
private val BACKWARD = setOf(Key.DirectionLeft, Key.MediaRewind)
private val FORWARD = setOf(Key.DirectionRight, Key.MediaFastForward)
private val SHOWING = setOf(Key.DirectionUp, Key.DirectionDown, Key.Menu, Key.Info)

private val sound = AudioAttributes.Builder()
    .setUsage(C.USAGE_MEDIA)
    .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
    .build()

/**
 * A trailer sitting next to its film, on the whole screen, as the web plays
 * it: played at once, the middle button pausing it, the arrows to the side
 * moving it ten seconds, any other press saying where it is. The back key
 * closes it, and so does its end.
 */
@OptIn(UnstableApi::class)
@Composable
fun TrailerScreen(title: String, url: String) {
    val context = LocalContext.current
    val api = LocalServer.current
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    NoBarPinned()
    val player = remember(url) {
        ExoPlayer.Builder(context)
            .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(context, OkHttpDataSource.Factory(api.http))))
            .setAudioAttributes(sound, true)
            .setHandleAudioBecomingNoisy(true)
            .build()
            .apply {
                setMediaItem(MediaItem.fromUri(api.address(url)))
                prepare()
                playWhenReady = true
            }
    }
    var playing by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf(false) }
    var position by remember { mutableDoubleStateOf(0.0) }
    var length by remember { mutableDoubleStateOf(0.0) }
    var shown by remember { mutableStateOf(true) }
    var nudges by remember { mutableIntStateOf(0) }
    val leave by rememberUpdatedState(chrome.back)
    DisposableEffect(player) {
        val heard = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                playing = isPlaying
            }

            override fun onPlayerError(error: PlaybackException) {
                failed = true
            }

            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED) leave()
            }
        }
        player.addListener(heard)
        onDispose {
            player.removeListener(heard)
            player.release()
        }
    }
    // The television going elsewhere leaves the trailer where it was.
    LifecycleEventEffect(Lifecycle.Event.ON_STOP) { player.pause() }
    LaunchedEffect(shown, nudges, playing) {
        if (shown && playing) {
            delay(SHOWN_FOR_MS)
            shown = false
        }
    }
    LaunchedEffect(shown) {
        while (shown) {
            position = player.currentPosition / 1000.0
            length = player.duration.takeIf { it != C.TIME_UNSET }?.let { it / 1000.0 } ?: 0.0
            delay(FOLLOWED_EVERY_MS)
        }
    }

    fun nudge() {
        nudges += 1
        shown = true
    }

    val focus = remember { FocusRequester() }
    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .focusRequester(focus)
            .onPreviewKeyEvent { event ->
                val key = event.key
                if (key !in PAUSING && key !in BACKWARD && key !in FORWARD && key !in SHOWING) {
                    return@onPreviewKeyEvent false
                }
                if (event.type == KeyEventType.KeyDown && !failed) {
                    when (key) {
                        in PAUSING -> if (player.isPlaying) player.pause() else player.play()
                        in BACKWARD -> player.seekTo((player.currentPosition - STEP_MS).coerceAtLeast(0))
                        in FORWARD -> player.seekTo(player.currentPosition + STEP_MS)
                    }
                    nudge()
                }
                true
            }
            .focusable(),
    ) {
        AndroidView(
            factory = { made ->
                PlayerView(made).apply {
                    useController = false
                    resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                    setShutterBackgroundColor(android.graphics.Color.BLACK)
                    // Nothing of the view is chosen with the remote.
                    isFocusable = false
                    descendantFocusability = ViewGroup.FOCUS_BLOCK_DESCENDANTS
                    importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
                    this.player = player
                }
            },
            update = { view -> view.keepScreenOn = true },
            modifier = Modifier.fillMaxSize(),
        )
        if (failed) {
            Text(
                stringResource(R.string.tv_trailer_cannot_play),
                fontSize = 18.sp,
                fontWeight = FontWeight.Medium,
                color = palette.text,
                modifier = Modifier.align(Alignment.Center),
            )
        }
        AnimatedVisibility(shown || !playing, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.fillMaxSize()) {
            Box(Modifier.fillMaxSize()) {
                Text(
                    stringResource(R.string.tv_trailer_of, title),
                    fontSize = 22.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = palette.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.75f), Color.Transparent)))
                        .padding(start = Gutter, end = Gutter, top = 28.dp, bottom = 40.dp),
                )
                Column(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.8f))))
                        .padding(start = Gutter, end = Gutter, top = 40.dp, bottom = 32.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    val done = if (length > 0) (position / length).toFloat() else 0f
                    Spacer(Modifier.fillMaxWidth().height(10.dp).drawBehind { rail(size.height, palette.text.copy(alpha = 0.2f), palette.accent to done) })
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Glyph(if (playing) Icons.Pause else Icons.Play, palette.text, 22.dp)
                        Text(
                            if (length > 0) "${clock(position)} / ${clock(length)}" else clock(position),
                            fontSize = 15.sp,
                            color = palette.textSoft,
                        )
                    }
                }
            }
        }
    }
    LaunchedEffect(Unit) { focus.requestFocus() }
}

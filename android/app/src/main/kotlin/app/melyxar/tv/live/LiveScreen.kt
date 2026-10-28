package app.melyxar.tv.live

import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import app.melyxar.R
import app.melyxar.player.CameraJournal
import app.melyxar.player.OPEN
import app.melyxar.player.livePlayer
import app.melyxar.server.Camera
import app.melyxar.tv.inside.NoBarPinned
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.play.PlayerButton
import app.melyxar.tv.play.Who
import java.io.IOException
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.put

/** How long the controls stay up once the remote stops, as the film's. */
private const val CONTROLS_FOR_MS = 4_000L

/** The keys that bring the controls up, or come to them once up. */
private val CALLING_KEYS = setOf(
    Key.DirectionUp, Key.DirectionDown, Key.DirectionLeft, Key.DirectionRight,
    Key.DirectionCenter, Key.Enter, Key.NumPadEnter, Key.Menu, Key.Info,
)

/**
 * A camera watched live, on the whole screen, with the film player's look:
 * any key brings up the camera's name and a row of what can be done with it,
 * the back key takes them away and then closes the camera. The address, its
 * account written in, is asked of the server as the screen opens and held
 * only while it is open; the picture comes from the camera itself.
 *
 * The picture can be moved into the black bands around it, as this screen
 * needs, and is kept where it was put for this camera on this device.
 */
@OptIn(UnstableApi::class)
@Composable
fun LiveScreen(camera: Camera) {
    val palette = LocalPalette.current
    val api = LocalServer.current
    val context = LocalContext.current
    NoBarPinned()
    val journal = remember(camera.id) {
        CameraJournal(camera.name, "television") { fact -> runBlocking { api.tellTheCameraJournal(fact) } }
    }
    DisposableEffect(journal) {
        journal.screen("made")
        onDispose { journal.screen("closed") }
    }
    // Asked here rather than through the answers kept for the screens: it
    // holds a password, and is let go with the screen.
    var address by remember { mutableStateOf<String?>(null) }
    var failed by remember { mutableStateOf(false) }
    var playing by remember { mutableStateOf(false) }
    /** Counted up to open the camera again from the start: its address is
     *  asked again, and the picture made anew. */
    var tries by remember { mutableIntStateOf(0) }
    LaunchedEffect(camera.id, tries) {
        address = null
        failed = false
        val asked = SystemClock.elapsedRealtime()
        address = try {
            api.liveAddress(camera.id).also {
                journal.tell(OPEN, "opened") { put("server_answer_ms", SystemClock.elapsedRealtime() - asked) }
            }
        } catch (refused: IOException) {
            journal.failed("server", refused)
            failed = true
            null
        }
    }

    val kept = remember(context) { KeptPlacement(context) }
    var placement by remember(camera.id) { mutableStateOf(kept.of(camera.id)) }
    /** Where the picture was before it began to be moved, put back by the
     *  back key; none while it is not being moved. */
    var before by remember { mutableStateOf<Placement?>(null) }
    val moving = before != null
    var screen by remember { mutableStateOf(IntSize.Zero) }
    var picture by remember { mutableStateOf(VideoSize.UNKNOWN) }

    var shown by remember { mutableStateOf(true) }
    var nudges by remember { mutableIntStateOf(0) }
    val quiet = remember { FocusRequester() }
    var quietHeld by remember { mutableStateOf(false) }
    val first = remember { FocusRequester() }
    var named by remember { mutableStateOf<String?>(null) }
    /** Whether the remote goes back to the move button once the controls
     *  are up again. */
    var placed by remember { mutableStateOf(false) }

    // Away a moment after the remote stops, once the picture plays.
    LaunchedEffect(shown, playing, moving) {
        if (shown && playing && !moving) {
            snapshotFlow { nudges }.collectLatest {
                delay(CONTROLS_FOR_MS)
                shown = false
            }
        }
    }
    // With the controls away the remote rests on the screen itself; it comes
    // to the buttons only once an arrow asks for it, and back to the move
    // button once the picture is placed.
    LaunchedEffect(shown, moving) {
        if (!shown || moving) {
            quiet.requestFocus()
        } else if (placed) {
            placed = false
            withFrameNanos {}
            first.requestFocus()
        }
    }

    fun startMoving() {
        before = placement
        shown = false
    }

    fun stopMoving(keep: Boolean) {
        if (keep) kept.keep(camera.id, placement) else before?.let { placement = it }
        before = null
        placed = true
        shown = true
        nudges += 1
    }

    // Before the screens' own: moving is undone first, then the controls go.
    BackHandler(enabled = moving || shown) {
        if (moving) stopMoving(keep = false) else shown = false
    }

    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .onSizeChanged { screen = it }
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                if (moving) {
                    when (event.key) {
                        Key.DirectionUp -> placement = placement.moved(0, -1)
                        Key.DirectionDown -> placement = placement.moved(0, 1)
                        Key.DirectionLeft -> placement = placement.moved(-1, 0)
                        Key.DirectionRight -> placement = placement.moved(1, 0)
                        Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> stopMoving(keep = true)
                        else -> return@onPreviewKeyEvent false
                    }
                    return@onPreviewKeyEvent true
                }
                nudges += 1
                if (!quietHeld || event.key !in CALLING_KEYS) return@onPreviewKeyEvent false
                if (shown) first.requestFocus() else shown = true
                true
            },
    ) {
        Box(Modifier.fillMaxSize().focusRequester(quiet).onFocusChanged { quietHeld = it.isFocused }.focusable())
        address?.let { opened ->
            LivePicture(
                opened,
                journal,
                onPlaying = { playing = it },
                onFailed = {
                    journal.screen("cannot_play")
                    failed = true
                },
                onPicture = { picture = it },
                modifier = Modifier.fillMaxSize().offset {
                    val room = roomAround(screen.width, screen.height, picture.width, picture.height, picture.pixelWidthHeightRatio)
                    val (x, y) = placement.offset(room)
                    IntOffset(x, y)
                },
            )
        }
        if (failed) {
            Text(
                stringResource(R.string.camera_cannot_play),
                fontSize = 18.sp,
                fontWeight = FontWeight.Medium,
                color = palette.text,
                textAlign = TextAlign.Center,
                modifier = Modifier.align(Alignment.Center).padding(horizontal = Gutter),
            )
        }
        AnimatedVisibility(shown && !moving, enter = fadeIn(), exit = fadeOut()) {
            Box(Modifier.fillMaxSize()) {
                Heading(camera)
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.82f))))
                        .padding(start = Gutter, end = Gutter, top = 90.dp, bottom = 24.dp),
                ) {
                    LiveButton(Icons.Move, stringResource(R.string.camera_move), ::startMoving, { named = it }, Modifier.focusRequester(first))
                    if (!placement.centred) {
                        LiveButton(Icons.Recentre, stringResource(R.string.camera_recentre), {
                            placement = Placement()
                            kept.keep(camera.id, placement)
                            first.requestFocus()
                        }, { named = it })
                    }
                    LiveButton(Icons.Restart, stringResource(R.string.camera_reconnect), {
                        journal.screen("reconnect")
                        tries += 1
                    }, { named = it })
                    named?.let {
                        Text(it, fontSize = 15.sp, color = palette.textSoft, maxLines = 1, modifier = Modifier.padding(start = 14.dp))
                    }
                }
            }
        }
        if (moving) {
            MovingHint(Modifier.align(Alignment.BottomCenter).padding(bottom = 40.dp))
        }
    }
    LaunchedEffect(Unit) { quiet.requestFocus() }
}

/** The camera's name and that it is live at the top left, the time and who
 *  watches at the top right, as the film player's heading. */
@Composable
private fun Heading(camera: Camera) {
    val palette = LocalPalette.current
    Row(
        verticalAlignment = Alignment.Top,
        modifier = Modifier
            .fillMaxWidth()
            .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.72f), Color.Transparent)))
            .padding(start = Gutter, end = Gutter, top = 24.dp, bottom = 56.dp),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.weight(1f),
        ) {
            Glyph(Icons.SecurityCamera, palette.accentBright, 24.dp)
            Text(
                camera.name,
                fontSize = 22.sp,
                fontWeight = FontWeight.SemiBold,
                color = palette.text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(stringResource(R.string.camera_live), fontSize = 15.sp, color = palette.accentBright)
        }
        Who()
    }
}

/** A button of the row, saying what it does while the remote is on it. */
@Composable
private fun LiveButton(icon: ImageVector, name: String, onClick: () -> Unit, onNamed: (String?) -> Unit, modifier: Modifier = Modifier) {
    PlayerButton(icon, onClick, modifier.onFocusChanged { if (it.isFocused) onNamed(name) })
}

/** What the arrows and the keys do while the picture is being moved. */
@Composable
private fun MovingHint(modifier: Modifier) {
    val palette = LocalPalette.current
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        modifier = modifier
            .widthIn(max = 640.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(palette.surfaceRaised.copy(alpha = 0.92f))
            .padding(horizontal = 22.dp, vertical = 16.dp),
    ) {
        Glyph(Icons.Move, palette.accentBright, 28.dp)
        Text(stringResource(R.string.camera_moving), fontSize = 16.sp, lineHeight = 22.sp, color = palette.text)
    }
}

/** The camera's own picture, opened at [address] and let go with the screen. */
@OptIn(UnstableApi::class)
@Composable
private fun LivePicture(
    address: String,
    journal: CameraJournal,
    onPlaying: (Boolean) -> Unit,
    onFailed: () -> Unit,
    onPicture: (VideoSize) -> Unit,
    modifier: Modifier,
) {
    val context = LocalContext.current
    val player: ExoPlayer = remember(address) { livePlayer(context, address, journal) }
    DisposableEffect(player) {
        val heard = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) = onPlaying(isPlaying)

            override fun onPlayerError(error: PlaybackException) = onFailed()

            override fun onVideoSizeChanged(videoSize: VideoSize) = onPicture(videoSize)
        }
        player.addListener(heard)
        onDispose {
            player.removeListener(heard)
            player.release()
            onPlaying(false)
        }
    }
    // A live picture is not left playing behind the television's other apps,
    // and comes back to now rather than to where it was left.
    LifecycleEventEffect(Lifecycle.Event.ON_STOP) {
        journal.screen("not_seen")
        player.stop()
    }
    LifecycleEventEffect(Lifecycle.Event.ON_START) {
        journal.screen("seen")
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
        modifier = modifier,
    )
}

package app.melyxar.tv.play

import android.view.View
import androidx.activity.compose.BackHandler
import androidx.annotation.OptIn
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import app.melyxar.R
import app.melyxar.server.NextEpisode
import app.melyxar.server.Preferences
import app.melyxar.tv.inside.LocalAccountPreferences
import app.melyxar.tv.inside.LocalMarks
import app.melyxar.tv.inside.Play
import app.melyxar.tv.inside.ready
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.rememberLoaded
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import kotlinx.coroutines.delay

/** How long the controls stay up once the remote stops. */
private const val CONTROLS_FOR_MS = 4_000L

/** The keys of the remote the player answers; every other one, its volume
 *  first, is left to the television. */
private val FILM_KEYS = setOf(
    Key.DirectionCenter, Key.Enter, Key.NumPadEnter, Key.Back,
    Key.DirectionLeft, Key.DirectionRight, Key.DirectionUp, Key.DirectionDown,
    Key.Menu, Key.Info,
)

/** What takes the foot of the film in place of the controls. */
private val FOOT_TAKERS = setOf(Menu.Chapters, Menu.Info, Menu.Cast)

/** The steps back and on of the arrows and the buttons, as the account
 *  chose them. */
data class Steps(val back: Int, val on: Int)

/**
 * A film on the whole screen. The remote's arrows step through it and its
 * middle button pauses it with the controls away; any key brings them up,
 * Melyxar's own, as the web player draws them.
 */
@OptIn(UnstableApi::class)
@Composable
fun PlayerScreen(play: Play, onDone: () -> Unit, onNext: (Play) -> Unit) {
    val context = LocalContext.current
    val api = LocalServer.current
    val scope = rememberCoroutineScope()
    val marks = LocalMarks.current
    val chosen = LocalAccountPreferences.current.now ?: Preferences()
    val steps = Steps(chosen.stepBackSeconds, chosen.stepOnSeconds)
    val source = play.source
    val work = rememberLoaded("work:${play.work}") { work(play.work) }.value.ready
    val playback = remember(play) {
        source?.let { Playback(context, api, play.work, it, play.from, scope, onStopped = onDone, onLeft = marks::rowsHaveMoved) }
    }
    DisposableEffect(playback) { onDispose { playback?.leave() } }
    if (playback == null) {
        CannotPlay(onDone)
        return
    }

    // Away until the film is on its way, so nothing invites a press while
    // it comes.
    var shown by remember { mutableStateOf(false) }
    var nudges by remember { mutableIntStateOf(0) }
    var menu by remember { mutableStateOf<Menu?>(null) }
    var left by remember { mutableStateOf<Menu?>(null) }
    var playing by remember { mutableStateOf(false) }
    var ended by remember { mutableStateOf(false) }
    var waiting by remember { mutableStateOf(true) }
    val quiet = remember { FocusRequester() }
    var quietHeld by remember { mutableStateOf(false) }
    val memory = rememberFocusMemory()
    val upNextFirst = remember { FocusRequester() }
    val next = work?.carryOnWith?.takeIf { it.sourceId != null }
    var declined by remember { mutableStateOf(false) }
    val inCredits by rememberInCredits(playback)
    val announced = next?.takeIf { !declined && (ended || inCredits) }

    fun playNext(episode: NextEpisode) = onNext(Play(episode.id, episode.sourceId, null))

    DisposableEffect(playback) {
        val heard = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                playing = isPlaying
            }

            override fun onPlaybackStateChanged(state: Int) {
                ended = state == Player.STATE_ENDED
                waiting = state == Player.STATE_BUFFERING || state == Player.STATE_IDLE
            }
        }
        playback.player.addListener(heard)
        onDispose { playback.player.removeListener(heard) }
    }
    // Over with no next episode to announce: back to where it came from.
    LaunchedEffect(ended, declined) {
        if (ended && (next == null || declined)) onDone()
    }
    // Away a moment after the remote stops, while the film plays.
    LaunchedEffect(shown, nudges, playing, menu) {
        if (shown && playing && menu == null) {
            delay(CONTROLS_FOR_MS)
            shown = false
        }
    }
    // The controls come up with nothing chosen on them: the remote is
    // only put on them once an arrow asks for it.
    LaunchedEffect(shown, announced != null) {
        when {
            announced != null -> upNextFirst.requestFocus()
            !shown -> quiet.requestFocus()
        }
    }

    fun nudge() {
        nudges += 1
        shown = true
    }

    /** Opens a list over the film, or closes the one open. */
    fun openMenu(next: Menu?) {
        left = menu
        menu = next
    }

    // Once a list or a sheet closes, the controls are there again, and the
    // remote goes back to the button that opened it.
    LaunchedEffect(menu) {
        if (menu == null && left != null) {
            shown = true
            withFrameNanos {}
            memory.restore()
        }
    }

    // An open list of tracks takes the key first, and closes.
    BackHandler(onBack = onDone)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) {
                    return@onPreviewKeyEvent false
                }
                when (event.key) {
                    Key.MediaPlayPause, Key.MediaPlay, Key.MediaPause -> {
                        playback.toggle()
                        nudge()
                        true
                    }
                    Key.MediaFastForward -> {
                        playback.withStep(steps.on)
                        nudge()
                        true
                    }
                    Key.MediaRewind -> {
                        playback.withStep(-steps.back)
                        nudge()
                        true
                    }
                    // The television's own: its volume, and anything else
                    // the film has no use for.
                    !in FILM_KEYS -> false
                    // The controls up with nothing chosen: any arrow comes to
                    // them. What the film is opens from the buttons only.
                    else -> if (shown && quietHeld && menu == null && announced == null) {
                        when (event.key) {
                            Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> playback.toggle()
                            Key.Back -> return@onPreviewKeyEvent false
                            else -> memory.requester(PLAY).requestFocus()
                        }
                        nudge()
                        true
                    } else if (!shown && announced == null) {
                        // The controls away, the arrows and the middle button
                        // act on the film itself.
                        when (event.key) {
                            Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> playback.toggle()
                            Key.DirectionLeft -> playback.withStep(-steps.back)
                            Key.DirectionRight -> playback.withStep(steps.on)
                            Key.Back -> return@onPreviewKeyEvent false
                        }
                        nudge()
                        true
                    } else {
                        nudges += 1
                        false
                    }
                }
            },
    ) {
        Picture(playback)
        WordsOverFilm(playback, raised = shown, modifier = Modifier.fillMaxSize())
        Box(Modifier.fillMaxSize().focusRequester(quiet).onFocusChanged { quietHeld = it.isFocused }.focusable())
        if (waiting && !playback.failed) {
            Spinner(Modifier.align(Alignment.Center))
        }
        // The chapters and the sheets take the foot of the film the controls
        // stand in.
        AnimatedVisibility(shown && menu !in FOOT_TAKERS, enter = fadeIn(), exit = fadeOut()) {
            Controls(
                playback = playback,
                work = work,
                playing = playing,
                steps = steps,
                memory = memory,
                onMenu = ::openMenu,
                onNext = next?.let { { playNext(it) } },
            )
        }
        // The credits of an episode with another after it are the next
        // episode's announcement rather than something to skip.
        Skip(
            playback,
            creditsToo = next == null || declined,
            modifier = Modifier.align(Alignment.BottomEnd).padding(end = Gutter, bottom = 120.dp),
        )
        announced?.let {
            UpNext(
                it,
                first = upNextFirst,
                onPlay = { playNext(it) },
                onCancel = { declined = true },
                modifier = Modifier.align(Alignment.BottomEnd).padding(end = Gutter, bottom = 120.dp),
            )
        }
        when (val open = menu) {
            null -> Unit
            Menu.Info -> work?.let { InfoSheet(it, onClose = { openMenu(null) }, Modifier.align(Alignment.BottomCenter)) }
            Menu.Cast -> work?.let { CastSheet(it, onClose = { openMenu(null) }, Modifier.align(Alignment.BottomCenter)) }
            else -> PlayerMenu(open, left, playback, onMenu = ::openMenu, Modifier.align(Alignment.BottomEnd))
        }
        if (playback.failed) {
            CannotPlay(onDone)
        }
    }
}

/** The film, drawn by Media3. */
@OptIn(UnstableApi::class)
@Composable
private fun Picture(playback: Playback) {
    AndroidView(
        factory = { context ->
            PlayerView(context).apply {
                useController = false
                resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                setShutterBackgroundColor(android.graphics.Color.BLACK)
                setKeepContentOnPlayerReset(true)
                // Nothing of the view is chosen with the remote.
                isFocusable = false
                descendantFocusability = android.view.ViewGroup.FOCUS_BLOCK_DESCENDANTS
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
                player = playback.player
            }
        },
        update = { view ->
            view.player = playback.player
            view.keepScreenOn = true
        },
        modifier = Modifier.fillMaxSize(),
    )
}

/** A skip of the recap, the opening or the credits, while the film is in
 *  one. */
@Composable
private fun Skip(playback: Playback, creditsToo: Boolean, modifier: Modifier) {
    var at by remember { mutableDoubleStateOf(0.0) }
    LaunchedEffect(playback) {
        while (true) {
            at = playback.seconds
            delay(500)
        }
    }
    val segment = playback.plan?.segments?.firstOrNull { at >= it.fromSecond && at < it.toSecond - 1 } ?: return
    val words = when (segment.kind) {
        "recap" -> R.string.tv_skip_recap
        "intro" -> R.string.tv_skip_intro
        "outro" -> if (creditsToo) R.string.tv_skip_outro else return
        else -> return
    }
    AccentButton(
        stringResource(words),
        onClick = { playback.player.seekTo((segment.toSecond * 1000).toLong()) },
        modifier = modifier,
        icon = null,
    )
}

@Composable
private fun CannotPlay(onDone: () -> Unit) {
    val palette = LocalPalette.current
    val back = remember { FocusRequester() }
    Column(
        modifier = Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(Color.Black, palette.ground))),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterVertically),
    ) {
        Text(stringResource(R.string.tv_cannot_play), fontSize = 18.sp, fontWeight = FontWeight.Medium, color = palette.text)
        AccentButton(stringResource(R.string.tv_back), onDone, Modifier.focusRequester(back), icon = null)
    }
    LaunchedEffect(Unit) { back.requestFocus() }
}

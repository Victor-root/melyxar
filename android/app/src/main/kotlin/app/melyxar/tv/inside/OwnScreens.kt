package app.melyxar.tv.inside

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
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
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Work
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalPictures
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.RoundMark
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import kotlinx.coroutines.delay

/** How long the viewer's controls stay up once the remote stops. */
private const val CONTROLS_FOR_MS = 4_000L

/** How long each photo of a slideshow is shown. */
private const val SLIDE_FOR_MS = 6_000L

/** How long one photo takes to give way to the next. */
private const val CROSSFADE_MS = 600

/**
 * A folder of one's own, as the web shows it: what it holds in the grid of
 * wide cards its library uses, its folders saying how much they hold.
 */
@Composable
fun FolderPage(work: Work, memory: FocusMemory, first: FocusRequester) {
    val chrome = LocalChrome.current
    val notes = work.children.associate { child ->
        child.card.id to child.takeIf { it.card.kind == "folder" }?.let {
            pluralStringResource(R.plurals.tv_item_count, it.childCount, it.childCount)
        }
    }
    CardGrid(
        cards = work.children.map { it.card },
        lying = true,
        empty = stringResource(R.string.tv_folder_empty),
        memory = memory,
        grid = rememberLazyGridState(),
        onOpen = { chrome.go(Place.Opened(it.id)) },
        firstCard = first,
        noteOf = { notes[it.id] },
    ) {
        // The grid keeps the page's margin already.
        TopBar(Modifier.padding(bottom = 22.dp), inset = Gutter) {
            BarTitle(Icons.Camera, work.title, work.children.size)
        }
    }
}

/**
 * The photos of a folder, one at a time on the whole screen, from [startAt]:
 * the arrows step through the folder and a slideshow runs through it on its
 * own. The steps stay on this one screen, so the back key leads to the
 * folder however many photos were looked at, as on the web.
 */
@Composable
fun PhotoViewer(startAt: String) {
    val palette = LocalPalette.current
    var showing by rememberSaveable { mutableStateOf(startAt) }
    val photo = rememberLoaded("work:$showing") { work(showing) }.value.ready?.takeIf { it.id == showing }
    var running by remember { mutableStateOf(false) }
    var shown by remember { mutableStateOf(true) }
    var nudges by remember { mutableIntStateOf(0) }
    val quiet = remember { FocusRequester() }
    val controls = remember { FocusRequester() }
    val request = rememberPhotoRequest()

    fun step(to: String?) {
        to?.let { showing = it }
    }

    fun nudge() {
        nudges += 1
        shown = true
    }

    // The neighbours fetched ahead, so a step shows the next photo at once.
    val pictures = LocalPictures.current
    LaunchedEffect(photo) {
        listOfNotNull(photo?.previousPhoto, photo?.nextPhoto).forEach { pictures.enqueue(request(it)) }
    }
    // The slideshow moves on once the photo shown has been seen a while,
    // and stops at the end of the folder.
    LaunchedEffect(running, photo) {
        if (!running || photo == null) {
            return@LaunchedEffect
        }
        delay(SLIDE_FOR_MS)
        if (photo.nextPhoto != null) step(photo.nextPhoto) else running = false
    }
    LaunchedEffect(shown, nudges, running) {
        if (shown) {
            delay(CONTROLS_FOR_MS)
            shown = false
        }
    }
    LaunchedEffect(shown) {
        if (shown) controls.requestFocus() else quiet.requestFocus()
    }
    // With the controls up, the back key only takes them away.
    BackHandler(enabled = shown) { shown = false }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) {
                    return@onPreviewKeyEvent false
                }
                when {
                    event.key == Key.MediaPlayPause || event.key == Key.MediaPlay || event.key == Key.MediaPause -> {
                        running = !running
                        nudge()
                        true
                    }
                    shown -> {
                        nudges += 1
                        false
                    }
                    // The controls away, the arrows step through the folder.
                    event.key == Key.DirectionLeft -> {
                        step(photo?.previousPhoto)
                        true
                    }
                    event.key == Key.DirectionRight -> {
                        step(photo?.nextPhoto)
                        true
                    }
                    event.key == Key.Back -> false
                    else -> {
                        nudge()
                        true
                    }
                }
            },
    ) {
        Crossfade(showing, animationSpec = tween(CROSSFADE_MS), label = "photo") { id ->
            AsyncImage(
                model = request(id),
                imageLoader = pictures,
                contentDescription = photo?.title,
                contentScale = ContentScale.Fit,
                modifier = Modifier.fillMaxSize(),
            )
        }
        if (photo?.versions?.isNotEmpty() == true && photo.versions.all { it.missing }) {
            Text(stringResource(R.string.tv_not_on_disk), fontSize = 16.sp, color = palette.textSoft, modifier = Modifier.align(Alignment.Center))
        }
        Box(Modifier.fillMaxSize().focusRequester(quiet).focusable())
        AnimatedVisibility(shown, enter = fadeIn(), exit = fadeOut()) {
            PhotoControls(photo, running, controls, onStep = ::step, onSlideshow = { running = !running })
        }
    }
}

/** The photo's name above it, and the steps and the slideshow below. */
@Composable
private fun PhotoControls(photo: Work?, running: Boolean, first: FocusRequester, onStep: (String?) -> Unit, onSlideshow: () -> Unit) {
    val palette = LocalPalette.current
    Box(Modifier.fillMaxSize()) {
        Text(
            photo?.title.orEmpty(),
            fontSize = 22.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .fillMaxWidth()
                .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.7f), Color.Transparent)))
                .padding(start = Gutter, end = Gutter, top = 24.dp, bottom = 48.dp),
        )
        Row(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.7f))))
                .padding(top = 60.dp, bottom = 28.dp)
                .focusRestorer(first),
            horizontalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterHorizontally),
        ) {
            photo?.previousPhoto?.let {
                RoundMark(Icons.ChevronLeft, label = stringResource(R.string.tv_previous_photo), on = false, onClick = { onStep(it) })
            }
            RoundMark(
                if (running) Icons.Pause else Icons.Play,
                label = stringResource(R.string.tv_slideshow),
                on = running,
                onClick = onSlideshow,
                modifier = Modifier.focusRequester(first),
            )
            photo?.nextPhoto?.let {
                RoundMark(Icons.ChevronRight, label = stringResource(R.string.tv_next_photo), on = false, onClick = { onStep(it) })
            }
        }
    }
}

/** How a photo is asked for: at the size of the screen, the same for the
 *  one shown and those fetched ahead, so they are one and the same. */
@Composable
private fun rememberPhotoRequest(): (String) -> ImageRequest {
    val api = LocalServer.current
    val context = LocalContext.current
    val screen = LocalWindowInfo.current.containerSize
    return remember(api, screen) {
        { id ->
            ImageRequest.Builder(context)
                .data(api.address("/api/v1/works/$id/photo"))
                .size(screen.width, screen.height)
                .build()
        }
    }
}

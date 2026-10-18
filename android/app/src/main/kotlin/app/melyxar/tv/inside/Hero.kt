package app.melyxar.tv.inside

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.HeroMore
import app.melyxar.server.WithCard
import app.melyxar.tv.fileBadges
import app.melyxar.tv.howFarIn
import app.melyxar.tv.lengthOfAPlay
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Badge
import app.melyxar.tv.look.Drawn
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.WhereABandIsCut
import app.melyxar.tv.look.PillShape
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.calm
import app.melyxar.tv.look.rememberFetchAhead
import app.melyxar.tv.probe.Part
import app.melyxar.tv.probe.Probe
import app.melyxar.tv.probe.probed
import kotlinx.coroutines.delay

/** The height of the banner, under which the first row shows. */
val HeroHeight = 340.dp

/** How far the banner's picture runs on under the row below it, fading
 *  away behind the tiles of the libraries rather than ending above them. */
private val UNDER_THE_ROW = 120.dp

/** How long each work of the banner stands before the next. */
private const val EACH_STANDS_FOR_MS = 9_000L

/** The crossfade from one work of the banner to the next. */
private const val CROSSFADE_MS = 700

/** How wide a work's picture and its title's logo are drawn in the banner. */
private val BACKDROP_ACROSS = 960.dp
private val LOGO_ACROSS = 300.dp

/** What the banner draws of [work]: its picture, and its title's logo. */
fun bannerDrawnOf(work: WithCard<HeroMore>): List<Drawn> =
    listOf(Drawn(backdropOf(work), BACKDROP_ACROSS), Drawn(work.more.logo, LOGO_ACROSS))

private fun backdropOf(work: WithCard<HeroMore>) = work.more.backdrop.ifEmpty { work.card.wide }

/**
 * The banner of the home screen, as the web's: a few works one after the
 * other, each over its own picture faded into the page. Left and right on
 * its buttons step through them; on their own they turn every nine seconds.
 */
@Composable
fun Hero(
    items: List<WithCard<HeroMore>>,
    onPlay: (WithCard<HeroMore>) -> Unit,
    onOpen: (String) -> Unit,
    first: FocusRequester,
    modifier: Modifier = Modifier,
) {
    var shown by remember { mutableIntStateOf(0) }
    var turns by remember { mutableIntStateOf(0) }
    var holdsTheRemote by remember { mutableStateOf(false) }
    /** Whether the remote was in the banner when it turned: a button the
     *  next work has no use for takes the remote away with it. */
    var keepTheRemote by remember { mutableStateOf(false) }
    val at = shown.coerceIn(0, items.lastIndex.coerceAtLeast(0))
    fun step(by: Int) {
        keepTheRemote = holdsTheRemote
        shown = (at + by + items.size) % items.size
        turns += 1
    }
    LaunchedEffect(items.size, turns) {
        if (items.size > 1) {
            delay(EACH_STANDS_FOR_MS)
            step(1)
        }
    }
    // The next work's pictures, while this one stands, so it comes in whole.
    val fetchAhead = rememberFetchAhead()
    LaunchedEffect(items, at) {
        if (items.size > 1) {
            fetchAhead(bannerDrawnOf(items[(at + 1) % items.size]))
        }
    }
    LaunchedEffect(at) {
        withFrameNanos {}
        if (keepTheRemote && !holdsTheRemote) {
            first.requestFocus()
        }
    }
    Box(modifier.fillMaxWidth().height(HeroHeight).onFocusChanged { holdsTheRemote = it.hasFocus }) {
        AnimatedContent(
            targetState = at,
            transitionSpec = { fadeIn(tween(CROSSFADE_MS)) togetherWith fadeOut(tween(CROSSFADE_MS)) },
            label = "hero",
            modifier = Modifier.fillMaxWidth().runsOnUnderTheRow().probed(Part.Banner).then(if (Probe.shows(Part.BannerFade)) Modifier.fadesIntoThePage() else Modifier),
        ) { index ->
            items.getOrNull(index)?.let { Backdrop(it) }
        }
        items.getOrNull(at)?.let { work ->
            Words(
                work = work,
                onPlay = { onPlay(work) },
                onOpen = { onOpen(work.card.id) },
                onStep = { by -> if (items.size > 1) step(by) },
                first = first,
                modifier = Modifier.align(Alignment.CenterStart).padding(start = Gutter, top = 70.dp),
            )
        }
        if (items.size > 1) {
            Dots(items.size, at, Modifier.align(Alignment.BottomEnd).padding(end = Gutter, bottom = 24.dp))
        }
    }
}

/** Does [action] when [key] is pressed on this piece, rather than moving
 *  the remote away. */
private fun Modifier.onKey(key: Key, action: () -> Unit) = onPreviewKeyEvent { event ->
    if (event.type == KeyEventType.KeyDown && event.key == key) {
        action()
        true
    } else {
        false
    }
}

@Composable
private fun Backdrop(work: WithCard<HeroMore>) {
    val palette = LocalPalette.current
    Box(Modifier.fillMaxSize().drawWithCache {
        // Shaded enough on the left to read the words over, never so much
        // that the picture looks pushed aside: a solid edge reads as a hole.
        val below = Brush.verticalGradient(0.58f to Color.Transparent, 0.98f to palette.ground)
        val words = Brush.horizontalGradient(
            0f to palette.ground.copy(alpha = 0.8f),
            0.18f to palette.ground.copy(alpha = 0.6f),
            0.38f to palette.ground.copy(alpha = 0.3f),
            0.56f to palette.ground.copy(alpha = 0.08f),
            0.7f to Color.Transparent,
        )
        onDrawWithContent {
            drawContent()
            drawRect(words)
            drawRect(below)
        }
    }) {
        ServerPicture(
            backdropOf(work),
            BACKDROP_ACROSS,
            Modifier.fillMaxSize(),
            alignment = WhereABandIsCut,
        )
    }
}

/**
 * The picture taller than the banner by [UNDER_THE_ROW], hanging below it
 * while the banner keeps its own height for what comes after: the row under
 * it is drawn later and so over it.
 */
private fun Modifier.runsOnUnderTheRow() = layout { measurable, constraints ->
    val height = constraints.maxHeight + UNDER_THE_ROW.roundToPx()
    val picture = measurable.measure(constraints.copy(minHeight = height, maxHeight = height))
    layout(picture.width, constraints.maxHeight) { picture.place(0, 0) }
}

/**
 * The web's `--hero-fade`: the banner, its picture and its shades together,
 * made clear towards its foot, so the page's own ground shows through it
 * rather than a band of plain colour. Its foot is under the row below.
 */
private fun Modifier.fadesIntoThePage() = this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithCache {
        val clear = Brush.verticalGradient(
            0.66f to Color.Black,
            0.78f to Color.Black.copy(alpha = 0.88f),
            0.87f to Color.Black.copy(alpha = 0.56f),
            0.94f to Color.Black.copy(alpha = 0.24f),
            1f to Color.Transparent,
        )
        onDrawWithContent {
            drawContent()
            drawRect(clear, blendMode = BlendMode.DstIn)
        }
    }

@Composable
private fun Words(
    work: WithCard<HeroMore>,
    onPlay: () -> Unit,
    onOpen: () -> Unit,
    /** Left on the first button steps back a work, right on the last one
     *  steps on. */
    onStep: (Int) -> Unit,
    first: FocusRequester,
    modifier: Modifier,
) {
    val palette = LocalPalette.current
    val card = work.card
    val more = work.more
    Column(modifier.widthIn(max = 480.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        more.tagline?.let {
            Text(
                it.uppercase(),
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.18.em,
                color = palette.textSoft,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        val name = more.seriesTitle ?: card.title
        if (more.logo.isNotEmpty()) {
            ServerPicture(
                more.logo,
                LOGO_ACROSS,
                Modifier.heightIn(max = 56.dp).widthIn(max = LOGO_ACROSS),
                contentScale = ContentScale.Fit,
                alignment = Alignment.CenterStart,
            )
        } else {
            Text(
                name,
                fontSize = 42.sp,
                lineHeight = 44.sp,
                fontWeight = FontWeight.Bold,
                letterSpacing = (-0.02).em,
                color = palette.text,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Facts(work)
        // What room the banner has left once everything else is in, so a
        // long tagline or a second line of badges shortens the overview
        // rather than squeezing the buttons out.
        more.overview?.let {
            Text(
                it,
                fontSize = 14.5.sp,
                lineHeight = 22.sp,
                color = palette.textSoft,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
        }
        val playable = card.source != null && card.kind != "series"
        val resume = LocalMarks.current.whereabouts(card).resume
        val started = resume != null
        // One accent button whatever the work, so the remote stays on it
        // as the banner turns: it plays what can be played, and opens the
        // rest.
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.padding(top = 4.dp).focusRestorer(first)) {
            val back = Modifier.focusRequester(first).onKey(Key.DirectionLeft) { onStep(-1) }
            AccentButton(
                text = stringResource(
                    when {
                        !playable -> R.string.tv_open
                        started -> R.string.tv_resume
                        else -> R.string.tv_play
                    },
                ),
                onClick = if (playable) onPlay else onOpen,
                modifier = if (playable) back else back.onKey(Key.DirectionRight) { onStep(1) },
                icon = if (playable) Icons.Play else Icons.Info,
            )
            if (playable) {
                PlainButton(
                    stringResource(R.string.tv_open),
                    onOpen,
                    Modifier.onKey(Key.DirectionRight) { onStep(1) },
                    icon = Icons.Info,
                )
            }
        }
        resume?.let { at -> Progress(at.toDouble(), lengthOfAPlay(card)) }
    }
}

/** The line of facts under the title, and the badges of the file. */
@Composable
private fun Facts(work: WithCard<HeroMore>) {
    val palette = LocalPalette.current
    val card = work.card
    val more = work.more
    val facts = listOfNotNull(
        whichEpisode(more.seasonNumber, more.episodeNumber, card.title),
        card.year?.toString(),
        card.runtimeMinutes?.takeIf { it > 0 }?.let { howLong(it) },
    ) + more.genres.take(2)
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        Text(facts.joinToString(" · "), fontSize = 13.5.sp, color = palette.textSoft, maxLines = 1)
        fileBadges(more.width, more.height, more.hdr, more.sound).forEach { Badge(it) }
    }
}

/** How far in, as a bar and in hours and minutes. */
@Composable
fun Progress(seconds: Double, length: Long?, width: Dp = 220.dp) {
    val palette = LocalPalette.current
    val done = howFarIn(seconds, length) ?: return
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.width(width).height(4.dp).clip(PillShape).background(palette.text.copy(alpha = 0.22f))) {
            Box(Modifier.fillMaxHeight().fillMaxWidth(done).clip(PillShape).background(palette.accent))
        }
        length?.let {
            Text(
                stringResource(R.string.tv_progress, howLong((seconds / 60).toInt()), howLong((it / 60).toInt())),
                fontSize = 12.5.sp,
                color = palette.textFaint,
            )
        }
    }
}

@Composable
private fun Dots(count: Int, at: Int, modifier: Modifier) {
    val palette = LocalPalette.current
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        repeat(count) { index ->
            val width by animateDpAsState(if (index == at) 22.dp else 8.dp, calm(), label = "dot")
            Box(
                Modifier
                    .size(width, 8.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(if (index == at) palette.accent else palette.text.copy(alpha = 0.35f)),
            )
        }
    }
}

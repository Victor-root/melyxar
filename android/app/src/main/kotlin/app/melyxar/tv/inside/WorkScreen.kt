package app.melyxar.tv.inside

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Child
import app.melyxar.server.Credit
import app.melyxar.server.Refusal
import app.melyxar.server.Work
import app.melyxar.tv.fileBadges
import app.melyxar.tv.lengthOfAPlay
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Badge
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.LocalServer
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.PosterWidth
import app.melyxar.tv.look.RoundMark
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim
import app.melyxar.tv.look.shadowRaised
import java.io.IOException
import java.util.Locale
import kotlinx.coroutines.launch

/** What an account has said of a work, as the page shows it: changed at
 *  once on a press, and put back if the server refuses. */
private data class Marks(val watched: Boolean, val favourite: Boolean, val watchLater: Boolean)

/**
 * A work's page, as the web's: its picture behind everything, its poster,
 * its name and facts, how to play it, and below what it holds and what is
 * near it.
 */
@Composable
fun WorkScreen(id: String, comebacks: Int) {
    var tries by remember { mutableIntStateOf(0) }
    val loaded = rememberLoaded("work:$id", comebacks, tries) { work(id) }.value
    val work = loaded.ready
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    Box(Modifier.fillMaxSize()) {
        when {
            work != null -> Page(work, memory, first)
            loaded is Loaded.Failed -> Trouble(onRetry = { tries += 1 })
            else -> TopBar()
        }
    }
    RestoreFocus(memory, first, ready = work != null)
}

@Composable
private fun Page(work: Work, memory: FocusMemory, first: FocusRequester) {
    val chrome = LocalChrome.current
    val list = rememberLazyListState()
    Backdrop(work)
    ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
        LazyColumn(state = list, modifier = Modifier.fillMaxSize()) {
            item("top") {
                Column(Modifier.head(head, "top")) {
                    TopBar()
                    Top(work, first)
                }
            }
            if (work.kind == "series") {
                item("seasons") { Seasons(work, memory) }
            }
            if (work.kind == "season") {
                section("episodes", work.children) { Episodes(stringResource(R.string.tv_episodes), work.children, memory) }
            }
            section("cast", work.cast) {
                CardRow(stringResource(R.string.tv_cast), Icons.People, work.cast, key = { it.personId + it.role }) { credit ->
                    Person(credit, Modifier.remembered(memory, "cast/${credit.personId}"))
                }
            }
            work.saga?.let { saga ->
                section("saga", saga.cards) {
                    CardRow(saga.name ?: stringResource(R.string.tv_saga), Icons.Film, saga.cards, key = { it.id }) { card ->
                        WorkCard(card, onClick = { chrome.go(Place.Opened(card.id)) }, modifier = Modifier.remembered(memory, "saga/${card.id}"))
                    }
                }
            }
            work.alike?.let { alike ->
                section("alike", alike.cards) {
                    CardRow(
                        stringResource(R.string.tv_alike, alike.genre.orEmpty()),
                        Icons.Film,
                        alike.cards,
                        key = { it.id },
                    ) { card ->
                        WorkCard(card, onClick = { chrome.go(Place.Opened(card.id)) }, modifier = Modifier.remembered(memory, "alike/${card.id}"))
                    }
                }
            }
        }
    }
}

/** The work's picture behind the page, faded into it on the left, where the
 *  words are, and towards the foot. */
@Composable
private fun Backdrop(work: Work) {
    val palette = LocalPalette.current
    Box(Modifier.fillMaxSize().drawWithCache {
        val words = Brush.horizontalGradient(
            0.18f to palette.ground,
            0.55f to palette.ground.copy(alpha = 0.7f),
            1f to palette.ground.copy(alpha = 0.4f),
        )
        val foot = Brush.verticalGradient(0.4f to Color.Transparent, 1f to palette.ground)
        onDrawWithContent {
            drawContent()
            drawRect(words)
            drawRect(foot)
        }
    }) {
        ServerPicture(work.backdrop, 960.dp, Modifier.fillMaxSize(), alignment = Alignment.TopCenter)
    }
}

@Composable
private fun Top(work: Work, first: FocusRequester) {
    val palette = LocalPalette.current
    val lying = work.kind == "episode"
    Row(
        modifier = Modifier.padding(start = Gutter, end = Gutter, top = 30.dp, bottom = 34.dp),
        horizontalArrangement = Arrangement.spacedBy(36.dp),
    ) {
        val across = if (lying) 300.dp else 190.dp
        Box(
            Modifier
                .width(across)
                .boxShadow(CardShape, palette.shadowRaised)
                .clip(CardShape)
                .background(palette.surfaceRaised),
        ) {
            ServerPicture(
                if (lying) work.backdrop.ifEmpty { work.poster } else work.poster,
                across,
                Modifier.fillMaxWidth().aspectRatio(if (lying) 16f / 9 else 2f / 3),
            )
        }
        Column(verticalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.weight(1f)) {
            if (work.logo.isNotEmpty()) {
                ServerPicture(
                    work.logo,
                    360.dp,
                    Modifier.heightIn(max = 90.dp).widthIn(max = 360.dp),
                    contentScale = ContentScale.Fit,
                    alignment = Alignment.CenterStart,
                )
            } else {
                Text(
                    work.title,
                    fontSize = 40.sp,
                    lineHeight = 42.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = (-0.02).em,
                    color = palette.text,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            work.tagline?.let {
                Text(it, fontSize = 16.sp, fontStyle = FontStyle.Italic, color = palette.textSoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Facts(work)
            Actions(work, first)
            work.card?.resumeFromSeconds?.takeIf { work.kind != "series" }?.let { at ->
                Progress(at.toDouble(), work.card.let(::lengthOfAPlay), width = 300.dp)
            }
            work.overview?.let {
                Text(it, fontSize = 15.5.sp, lineHeight = 25.sp, color = palette.textSoft, maxLines = 5, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** Ratings, dates, length, age and genres, then what the file holds. */
@Composable
private fun Facts(work: Work) {
    val palette = LocalPalette.current
    val version = work.versions.firstOrNull()
    val picture = version?.video?.firstOrNull()
    val sound = version?.audio?.let { tracks -> tracks.firstOrNull { it.isDefault } ?: tracks.firstOrNull() }
    val seasons = work.children.size.takeIf { work.kind == "series" && it > 0 }
    val facts = listOfNotNull(
        work.year?.toString(),
        seasons?.let { pluralStringResource(R.plurals.tv_season_count, it, it) },
        work.runtimeMinutes?.takeIf { it > 0 && work.kind != "series" }?.let { howLong(it) },
    )
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        work.rating?.let { RatingMark("TMDB", it, Brush.horizontalGradient(listOf(palette.tmdbFrom, palette.tmdbTo)), palette.tmdbInk) }
        work.ratings.firstOrNull { it.source == "imdb" }?.let {
            RatingMark("IMDb", it.value, Brush.linearGradient(listOf(palette.imdb, palette.imdb)), palette.imdbInk)
        }
        if (facts.isNotEmpty()) {
            Text(facts.joinToString(" · "), fontSize = 15.sp, color = palette.textSoft)
        }
        work.ageRating?.let { Badge(it) }
        if (work.genres.isNotEmpty()) {
            Text(work.genres.joinToString(", "), fontSize = 15.sp, color = palette.textSoft)
        }
        fileBadges(picture?.width, picture?.height, picture?.hdr, sound?.profile ?: sound?.codec).forEach { Badge(it) }
    }
}

@Composable
private fun RatingMark(site: String, value: Double, fill: Brush, ink: Color) {
    val palette = LocalPalette.current
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            site,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
            color = ink,
            modifier = Modifier.clip(RoundedCornerShape(3.dp)).background(fill).padding(horizontal = 4.dp),
        )
        Text(
            String.format(Locale.ROOT, "%.1f / 10", value),
            fontSize = 15.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
        )
    }
}

/** How to play the work, and the marks an account puts on it. */
@Composable
private fun Actions(work: Work, first: FocusRequester) {
    val chrome = LocalChrome.current
    val api = LocalServer.current
    val signedOut = LocalSignedOut.current
    val scope = rememberCoroutineScope()
    val card = work.card
    var marks by remember(card) {
        mutableStateOf(Marks(card?.seen == "watched", card?.favourite == true, card?.watchLater == true))
    }

    fun mark(mark: String, to: Marks) {
        val before = marks
        marks = to
        scope.launch {
            try {
                val on = when (mark) {
                    "watched" -> to.watched
                    "favourite" -> to.favourite
                    else -> to.watchLater
                }
                api.mark(work.id, mark, on)
            } catch (refused: Refusal) {
                marks = before
                if (refused.signedOut) signedOut()
            } catch (_: IOException) {
                marks = before
            }
        }
    }

    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(top = 6.dp).focusRestorer(first),
    ) {
        val next = work.carryOnWith
        val source = work.versions.firstOrNull()?.id ?: card?.source
        when {
            work.kind == "series" || work.kind == "season" -> next?.let { episode ->
                val started = card?.seen == "in_progress"
                AccentButton(
                    stringResource(
                        if (started) R.string.tv_carry_on_at else R.string.tv_start_series,
                        episode.season ?: 1,
                        episode.episode ?: 1,
                    ),
                    onClick = { chrome.play(Play(episode.id, episode.sourceId, null)) },
                    modifier = Modifier.focusRequester(first),
                )
            }
            source != null -> {
                val resumeAt = card?.resumeFromSeconds
                AccentButton(
                    stringResource(if (resumeAt != null) R.string.tv_resume else R.string.tv_play),
                    onClick = { chrome.play(Play(work.id, source, resumeAt?.toDouble() ?: 0.0)) },
                    modifier = Modifier.focusRequester(first),
                )
                if (resumeAt != null) {
                    PlainButton(
                        stringResource(R.string.tv_from_start),
                        onClick = { chrome.play(Play(work.id, source, 0.0)) },
                        icon = Icons.Restart,
                    )
                }
            }
        }
        if (card?.watchedMarks != false) {
            RoundMark(
                Icons.Tick,
                label = stringResource(R.string.tv_mark_watched),
                on = marks.watched,
                // Watched takes a work off the list to watch later, as the
                // server does.
                onClick = { mark("watched", marks.copy(watched = !marks.watched, watchLater = marks.watchLater && marks.watched)) },
                modifier = if (next == null && source == null) Modifier.focusRequester(first) else Modifier,
            )
        }
        RoundMark(
            if (marks.favourite) Icons.HeartFull else Icons.Heart,
            label = stringResource(R.string.tv_favourites),
            on = marks.favourite,
            onClick = { mark("favourite", marks.copy(favourite = !marks.favourite)) },
        )
        RoundMark(
            Icons.Clock,
            label = stringResource(R.string.tv_watch_later),
            on = marks.watchLater,
            onClick = { mark("watch-later", marks.copy(watchLater = !marks.watchLater)) },
        )
    }
}

/** The seasons of a series as tabs, and the episodes of the one chosen. */
@Composable
private fun Seasons(work: Work, memory: FocusMemory) {
    if (work.children.isEmpty()) {
        return
    }
    val playing = work.carryOnWith?.season
    var chosen by remember(work.id) {
        mutableStateOf(work.children.firstOrNull { it.number == playing }?.card?.id ?: work.children.first().card.id)
    }
    // Coming into the tabs lands on the one left, or on the season shown.
    val shown = remember { FocusRequester() }
    Column {
        Row(
            modifier = Modifier.padding(start = Gutter, bottom = 4.dp).focusRestorer(shown),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            work.children.forEach { season ->
                SeasonTab(
                    name = season.title ?: season.number?.let { number ->
                        if (number == 0) stringResource(R.string.tv_specials) else stringResource(R.string.tv_season, number)
                    } ?: "",
                    active = season.card.id == chosen,
                    onChoose = { chosen = season.card.id },
                    modifier = Modifier
                        .remembered(memory, "season/${season.card.id}")
                        .then(if (season.card.id == chosen) Modifier.focusRequester(shown) else Modifier),
                )
            }
        }
        val episodes = rememberLoaded("work:$chosen", chosen) { work(chosen) }.value.ready?.children.orEmpty()
        if (episodes.isNotEmpty()) {
            Episodes(null, episodes, memory)
        }
    }
}

@Composable
private fun SeasonTab(name: String, active: Boolean, onChoose: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val shape = RoundedCornerShape(percent = 50)
    Text(
        name,
        fontSize = 15.sp,
        fontWeight = FontWeight.SemiBold,
        color = when {
            active -> palette.accentContrast
            lit > 0.5f -> palette.accentBright
            else -> palette.textSoft
        },
        modifier = modifier
            .grows({ lit }, 1.05f)
            .clip(shape)
            .background(if (active) palette.accent else palette.surfaceRaised.copy(alpha = 0.74f))
            .rim(shape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onChoose)
            .padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/** A season's episodes lying down, each played at once from where it was
 *  left. */
@Composable
private fun Episodes(title: String?, episodes: List<Child>, memory: FocusMemory) {
    val chrome = LocalChrome.current
    CardRow(title, null, episodes, key = { it.card.id }) { episode ->
        val card = episode.card
        WorkCard(
            card = card,
            lying = true,
            title = listOfNotNull(episode.number?.let { "$it." }, episode.title ?: card.title).joinToString(" "),
            note = card.runtimeMinutes?.takeIf { it > 0 }?.let { howLong(it) },
            onClick = { chrome.play(Play(card.id, card.source, card.resumeFromSeconds?.toDouble())) },
            modifier = Modifier.remembered(memory, "episode/${card.id}"),
        )
    }
}

/** Somebody of the cast: their photo, their name and their part. */
@Composable
private fun Person(credit: Credit, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Column(modifier.width(PosterWidth * 0.8f).focusable(interactionSource = interaction)) {
        Box(
            Modifier
                .size(PosterWidth * 0.8f, PosterWidth * 1.2f)
                .grows({ lit })
                .clip(CardShape)
                .background(palette.surfaceRaised)
                .rim(CardShape, palette.accentBright, 1.5.dp) { lit },
        ) {
            ServerPicture(credit.photo, PosterWidth * 0.8f, Modifier.fillMaxSize())
        }
        Text(credit.name, fontSize = 13.sp, fontWeight = FontWeight.Medium, color = palette.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
        credit.character?.let {
            Text(it, fontSize = 12.sp, color = palette.textFaint, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

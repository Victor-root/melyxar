package app.melyxar.tv.inside

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
import app.melyxar.server.Work
import app.melyxar.tv.fileBadges
import app.melyxar.tv.lengthOfAPlay
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Badge
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.PosterWidth
import app.melyxar.tv.look.RoundMark
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rememberRowScrolling
import app.melyxar.tv.look.rim
import app.melyxar.tv.look.shadowRaised
import java.util.Locale

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
            // What somebody filmed or photographed is not shown as a film.
            work?.kind == "photo" -> PhotoViewer(work.id)
            work?.kind == "folder" -> FolderPage(work, memory, first)
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
                    CastMember(credit, Modifier.remembered(memory, "cast/${credit.personId}")) { chrome.go(Place.Person(credit.personId)) }
                }
            }
            // A series and a season are nothing but what they hold: neither
            // is played.
            val source = work.versions.firstOrNull()?.takeIf { !it.missing }?.id
            if (source != null && work.kind != "series" && work.kind != "season" && work.children.isEmpty()) {
                item("chapters") { WorkChapters(source, memory) { at -> chrome.play(Play(work.id, source, at)) } }
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
    val lying = work.kind == "episode" || work.kind == "video"
    Row(
        modifier = Modifier.padding(start = Gutter, end = Gutter, top = 24.dp, bottom = 30.dp),
        horizontalArrangement = Arrangement.spacedBy(32.dp),
    ) {
        val across = if (lying) 280.dp else 170.dp
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
        Column(verticalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.weight(1f)) {
            Text(
                work.title,
                fontSize = 30.sp,
                fontWeight = FontWeight.Bold,
                letterSpacing = (-0.02).em,
                color = palette.text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            work.tagline?.let {
                Text(it, fontSize = 16.sp, fontStyle = FontStyle.Italic, color = palette.textSoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Facts(work)
            Actions(work, first)
            work.card?.takeIf { work.kind != "series" }?.let { card ->
                LocalMarks.current.whereabouts(card).resume?.let { at ->
                    Progress(at.toDouble(), lengthOfAPlay(card), width = 300.dp)
                }
            }
            // Always whole: there is room for it once the title is a line.
            work.overview?.let { Text(it, fontSize = 15.sp, lineHeight = 23.sp, color = palette.textSoft) }
        }
    }
}

/** Ratings, dates, length, age and genres, then what the file holds. */
@Composable
fun Facts(work: Work) {
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
    val marks = LocalMarks.current
    val card = work.card
    val whereabouts = card?.let(marks::whereabouts)

    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(top = 6.dp).focusRestorer(first),
    ) {
        val next = work.carryOnWith
        val source = work.versions.firstOrNull()?.id ?: card?.source
        when {
            work.kind == "series" || work.kind == "season" -> next?.let { episode ->
                val started = whereabouts?.seen == "in_progress"
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
                val resumeAt = whereabouts?.resume
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
        // Marks belong to a work the server sent a card for.
        card ?: return@Row
        if (card.watchedMarks) {
            val watched = whereabouts?.seen == "watched"
            RoundMark(
                Icons.Tick,
                label = stringResource(R.string.tv_mark_watched),
                on = watched,
                onClick = { marks.setWatched(card, !watched) },
                modifier = if (next == null && source == null) Modifier.focusRequester(first) else Modifier,
            )
        }
        val favourite = marks.favourite(card)
        RoundMark(
            if (favourite) Icons.HeartFull else Icons.Heart,
            label = stringResource(R.string.tv_favourites),
            on = favourite,
            onClick = { marks.setFavourite(card, !favourite) },
        )
        val later = marks.watchLater(card)
        RoundMark(
            Icons.Clock,
            label = stringResource(R.string.tv_watch_later),
            on = later,
            onClick = { marks.setWatchLater(card, !later) },
        )
    }
}

/** The seasons of a series as tabs, and the episodes of the one chosen. */
@OptIn(ExperimentalFoundationApi::class)
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
        // Scrolled rather than squeezed: a tab squeezed past the edge wraps
        // its name a letter a line and makes the row as tall as that.
        CompositionLocalProvider(LocalBringIntoViewSpec provides rememberRowScrolling()) {
            Row(
                modifier = Modifier
                    .focusRestorer(shown)
                    .horizontalScroll(rememberScrollState())
                    .padding(start = Gutter, end = Gutter, bottom = 4.dp),
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
            lit > 0.5f -> palette.text
            else -> palette.textSoft
        },
        modifier = modifier
            .grows({ lit }, 1.05f)
            .clip(shape)
            .background(if (active) palette.accent else palette.surfaceRaised.copy(alpha = 0.74f))
            .rim(shape, if (active) palette.accentContrast else palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onChoose)
            .padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/** A season's episodes lying down, each played at once from where it was
 *  left. */
@Composable
private fun Episodes(title: String?, episodes: List<Child>, memory: FocusMemory) {
    val chrome = LocalChrome.current
    val marks = LocalMarks.current
    CardRow(title, null, episodes, key = { it.card.id }) { episode ->
        val card = episode.card
        WorkCard(
            card = card,
            lying = true,
            title = listOfNotNull(episode.number?.let { "$it." }, episode.title ?: card.title).joinToString(" "),
            note = card.runtimeMinutes?.takeIf { it > 0 }?.let { howLong(it) },
            onClick = { chrome.play(Play(card.id, card.source, marks.whereabouts(card).resume?.toDouble())) },
            modifier = Modifier.remembered(memory, "episode/${card.id}"),
        )
    }
}

/** Somebody of the cast: their photo, their name and their part; pressed,
 *  [onOpen]. */
@Composable
fun CastMember(credit: Credit, modifier: Modifier, onOpen: () -> Unit) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Column(
        modifier
            .width(PosterWidth * 0.8f)
            .clickable(interaction, indication = null, onClick = onOpen),
    ) {
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

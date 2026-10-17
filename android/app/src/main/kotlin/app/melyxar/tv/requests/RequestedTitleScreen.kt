package app.melyxar.tv.requests

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Picture
import app.melyxar.server.TitlePage
import app.melyxar.server.titlePage
import app.melyxar.tv.inside.CardRow
import app.melyxar.tv.inside.Face
import app.melyxar.tv.inside.FocusMemory
import app.melyxar.tv.inside.Loaded
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.Place
import app.melyxar.tv.inside.RatingMark
import app.melyxar.tv.inside.RestoreFocus
import app.melyxar.tv.inside.RowHead
import app.melyxar.tv.inside.ScrollingPage
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.Trouble
import app.melyxar.tv.inside.WorkBackdrop
import app.melyxar.tv.inside.head
import app.melyxar.tv.inside.howLong
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.ready
import app.melyxar.tv.inside.rememberLoaded
import app.melyxar.tv.inside.remembered
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Badge
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.shadowRaised
import app.melyxar.tv.readableDay
import java.util.Locale

/** How many of the cast are shown, as on the web. */
private const val FACES_SHOWN = 18

/**
 * The whole page of a title the server may not have, read as the page of
 * one it has: its backdrop and poster, what it is and what it is about, and
 * beside it asking for it, or opening it once it is here; then for a series
 * its seasons, and who is in it.
 */
@Composable
fun RequestedTitleScreen(catalogue: String, tmdbId: String) {
    var tries by remember { mutableIntStateOf(0) }
    val language = requestLanguage()
    val loaded = rememberLoaded("request-title:$language:$catalogue:$tmdbId", tries) { titlePage(catalogue, tmdbId, language) }.value
    val page = loaded.ready
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    Box(Modifier.fillMaxSize()) {
        when {
            page != null -> TitleView(page, memory, first)
            loaded is Loaded.Failed -> Trouble(onRetry = { tries += 1 })
            else -> TopBar()
        }
    }
    RestoreFocus(memory, first, ready = page != null)
}

@Composable
private fun TitleView(page: TitlePage, memory: FocusMemory, first: FocusRequester) {
    val palette = LocalPalette.current
    val list = rememberLazyListState()
    var asking by remember { mutableStateOf(false) }
    WorkBackdrop(listOfNotNull(page.backdrop).map(::Picture))
    ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
        LazyColumn(state = list, contentPadding = PaddingValues(bottom = 40.dp), modifier = Modifier.fillMaxSize()) {
            item("top") {
                Column(Modifier.head(head, "top")) {
                    TopBar()
                    Top(page, memory, first, onAsk = { asking = true })
                }
            }
            if (page.seasons.isNotEmpty()) {
                item("seasons") {
                    Column(Modifier.padding(bottom = 30.dp)) {
                        RowHead(stringResource(R.string.tv_requests_seasons), Icons.Series)
                        FlowRow(
                            modifier = Modifier.padding(horizontal = Gutter),
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            page.seasons.forEach { season ->
                                val episodes = pluralStringResource(R.plurals.tv_requests_episodes, season.episodes, season.episodes)
                                StatePill(
                                    listOfNotNull(
                                        stringResource(R.string.tv_season, season.number),
                                        episodes,
                                        stringResource(R.string.tv_requests_season_here).takeIf { season.held },
                                    ).joinToString(" · "),
                                    if (season.held) palette.ok else palette.textSoft,
                                )
                            }
                        }
                    }
                }
            }
            if (page.cast.isNotEmpty()) {
                item("cast") {
                    CardRow(stringResource(R.string.tv_requests_cast), Icons.People, page.cast.take(FACES_SHOWN).withIndex().toList(), key = { "${it.index}" }) { (at, person) ->
                        Face(person.name, person.character, listOfNotNull(person.photo).map(::Picture), Modifier.remembered(memory, "cast/$at"), onOpen = {})
                    }
                }
            }
        }
    }
    if (asking) {
        AskSheet(page.found, onClose = { asking = false })
    }
}

/** The poster, then the title, what it is, where it stands here and what it
 *  is about. */
@Composable
private fun Top(page: TitlePage, memory: FocusMemory, first: FocusRequester, onAsk: () -> Unit) {
    val palette = LocalPalette.current
    val found = page.found
    Row(
        modifier = Modifier.padding(start = Gutter, end = Gutter, top = 24.dp, bottom = 30.dp),
        horizontalArrangement = Arrangement.spacedBy(32.dp),
    ) {
        Box(
            Modifier
                .width(170.dp)
                .aspectRatio(2f / 3)
                .boxShadow(CardShape, palette.shadowRaised)
                .clip(CardShape)
                .background(palette.surfaceRaised),
        ) {
            TitlePoster(page.bigPoster ?: found.poster, 170.dp)
        }
        Column(verticalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.weight(1f)) {
            Text(
                found.title,
                fontSize = 30.sp,
                fontWeight = FontWeight.Bold,
                letterSpacing = (-0.02).em,
                color = palette.text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            page.tagline?.let {
                Text(it, fontSize = 16.sp, fontStyle = FontStyle.Italic, color = palette.textSoft, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Facts(page)
            Actions(page, memory, first, onAsk)
            found.overview?.let { Text(it, fontSize = 15.sp, lineHeight = 23.sp, color = palette.textSoft) }
        }
    }
}

/** Its rating, when it came out, how long it is, for whom, and its genres. */
@Composable
private fun Facts(page: TitlePage) {
    val palette = LocalPalette.current
    val facts = listOfNotNull(
        page.releaseDate?.let { readableDay(it, Locale.getDefault()) } ?: page.found.year?.toString(),
        page.seasons.size.takeIf { it > 0 }?.let { pluralStringResource(R.plurals.tv_season_count, it, it) },
        page.runtimeMinutes?.takeIf { it > 0 }?.let { howLong(it) },
    )
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        page.rating?.takeIf { it > 0 }?.let {
            RatingMark("TMDB", it, Brush.horizontalGradient(listOf(palette.tmdbFrom, palette.tmdbTo)), palette.tmdbInk)
        }
        if (facts.isNotEmpty()) {
            Text(facts.joinToString(" · "), fontSize = 15.sp, color = palette.textSoft)
        }
        page.ageRating?.let { Badge(it) }
        if (page.genres.isNotEmpty()) {
            Text(page.genres.joinToString(", "), fontSize = 15.sp, color = palette.textSoft)
        }
    }
}

/** What is offered beside the title, as the web's: opening it once it is
 *  here, this account's request while it waits, or asking for it. */
@Composable
private fun Actions(page: TitlePage, memory: FocusMemory, first: FocusRequester, onAsk: () -> Unit) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val requests = LocalRequests.current
    val standing = standingOf(page.found, requests.mine)
    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(top = 6.dp).focusRestorer(first),
    ) {
        when (standing) {
            is Standing.Here -> AccentButton(
                stringResource(R.string.tv_requests_open),
                onClick = { chrome.go(Place.Opened(standing.workId)) },
                modifier = Modifier.focusRequester(first),
                icon = null,
            )
            is Standing.Mine -> {
                PlainButton(
                    stringResource(R.string.tv_requests_asked_by_you),
                    onClick = { chrome.go(Place.TitleRequests) },
                    modifier = Modifier.focusRequester(first),
                    icon = Icons.Tick,
                )
                if (standing.request.state == "pending") {
                    PlainButton(
                        stringResource(R.string.tv_requests_cancel),
                        onClick = { requests.withdraw(standing.request.id) },
                        modifier = Modifier.remembered(memory, "withdraw"),
                    )
                }
            }
            is Standing.Free -> {
                AccentButton(
                    stringResource(if (standing.others > 0) R.string.tv_requests_ask_too else R.string.tv_requests_ask),
                    onClick = onAsk,
                    modifier = Modifier.focusRequester(first),
                    icon = Icons.Request,
                )
                if (standing.heldSeasons.isNotEmpty()) {
                    StatePill(stringResource(R.string.tv_requests_here_in_part, seasonsNamed(standing.heldSeasons)), palette.ok)
                }
            }
        }
        val others = when (standing) {
            is Standing.Here -> 0
            is Standing.Mine -> standing.others
            is Standing.Free -> standing.others
        }
        if (others > 0) {
            Text(pluralStringResource(R.plurals.tv_requests_others, others, others), fontSize = 14.sp, color = palette.textFaint)
        }
    }
}

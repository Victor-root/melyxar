package app.melyxar.tv.inside

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Card
import app.melyxar.server.Person
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.Synopsis
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.shadowRaised
import app.melyxar.tv.readableDay
import app.melyxar.tv.todayOf
import app.melyxar.tv.yearsBetween
import java.util.Calendar

/**
 * Somebody of a cast, as the web's page of a person: their photo, when and
 * where they were born, their biography, and what of theirs the library
 * holds, films apart from series.
 */
@Composable
fun PersonScreen(id: String, comebacks: Int) {
    var tries by remember { mutableIntStateOf(0) }
    val loaded = rememberLoaded("person:$id", comebacks, tries) { person(id) }.value
    val person = loaded.ready
    val memory = rememberFocusMemory()
    val biography = remember { FocusRequester() }
    val firstWork = remember { FocusRequester() }
    Box(Modifier.fillMaxSize()) {
        when {
            person != null -> Page(person, memory, biography, firstWork)
            loaded is Loaded.Failed -> Trouble(onRetry = { tries += 1 })
            else -> TopBar()
        }
    }
    // A biography short enough to be read whole has nothing to open: the
    // remote starts on their first work instead.
    RestoreFocus(memory, biography, ready = person != null, orElse = firstWork)
}

@Composable
private fun Page(person: Person, memory: FocusMemory, biography: FocusRequester, firstWork: FocusRequester) {
    val chrome = LocalChrome.current
    val list = rememberLazyListState()
    val films = person.works.filter { it.kind == "movie" }
    val series = person.works.filter { it.kind != "movie" }
    val opening = (films.firstOrNull() ?: series.firstOrNull())?.id
    // The first work the remote lands on as the page opens leaves the page at
    // its top, the person shown whole; it scrolls once the remote moves on.
    var landing by remember { mutableStateOf(true) }
    var landed by remember { mutableStateOf(false) }

    @Composable
    fun Work(card: Card, under: String, head: PageHead) = WorkCard(
        card,
        onClick = { chrome.go(Place.Opened(card.id)) },
        modifier = Modifier
            .remembered(memory, "$under/${card.id}")
            .then(
                if (card.id == opening) {
                    Modifier.focusRequester(firstWork).onFocusChanged {
                        if (it.hasFocus && landing) {
                            landed = true
                            head.hold("landing", true)
                        } else if (!it.hasFocus && landed) {
                            landing = false
                            landed = false
                            head.hold("landing", false)
                        }
                    }
                } else {
                    Modifier
                },
            ),
    )

    ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
        LazyColumn(state = list, modifier = Modifier.fillMaxSize()) {
            item("top") {
                Column(Modifier.head(head, "top")) {
                    TopBar()
                    AboutThem(person, memory, biography)
                }
            }
            section("films", films) {
                CardRow(stringResource(R.string.tv_person_films), Icons.Film, films, key = { it.id }) { Work(it, "film", head) }
            }
            section("series", series) {
                CardRow(stringResource(R.string.tv_person_series), Icons.Series, series, key = { it.id }) { Work(it, "series", head) }
            }
        }
    }
}

/** Their photo, their name, their days and their biography, which the
 *  remote starts on when it is long enough to be opened whole. */
@Composable
private fun AboutThem(person: Person, memory: FocusMemory, biography: FocusRequester) {
    val palette = LocalPalette.current
    val locale = LocalConfiguration.current.locales[0]
    val until = person.diedOn ?: todayOf(Calendar.getInstance())
    val age = person.bornOn?.let { yearsBetween(it, until) }
    val born = person.bornOn?.let { readableDay(it, locale) }
    val died = person.diedOn?.let { readableDay(it, locale) }
    Row(
        modifier = Modifier.padding(start = Gutter, end = Gutter, top = 30.dp, bottom = 34.dp),
        horizontalArrangement = Arrangement.spacedBy(36.dp),
    ) {
        Box(
            Modifier
                .width(190.dp)
                .boxShadow(CardShape, palette.shadowRaised)
                .clip(CardShape)
                .background(palette.surfaceRaised),
            contentAlignment = Alignment.Center,
        ) {
            Box(Modifier.fillMaxWidth().aspectRatio(2f / 3), contentAlignment = Alignment.Center) {
                Text(person.name.take(1), fontFamily = MelyxarDisplay, fontSize = 64.sp, color = palette.text.copy(alpha = 0.3f))
                ServerPicture(person.photo, 190.dp, Modifier.fillMaxSize())
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.weight(1f)) {
            Text(
                person.name,
                fontSize = 40.sp,
                lineHeight = 42.sp,
                fontWeight = FontWeight.Bold,
                letterSpacing = (-0.02).em,
                color = palette.text,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            born?.let {
                Fact(
                    stringResource(R.string.tv_person_born),
                    if (died != null || age == null) it else stringResource(R.string.tv_person_born_age, it, age),
                )
            }
            died?.let {
                Fact(stringResource(R.string.tv_person_died), if (age == null) it else stringResource(R.string.tv_person_died_age, it, age))
            }
            person.birthplace?.let { Fact(stringResource(R.string.tv_person_birthplace), it) }
            val biographyText = person.biography
            if (biographyText.isNullOrBlank()) {
                Text(stringResource(R.string.tv_person_no_biography), fontSize = 15.5.sp, color = palette.textFaint)
            } else {
                Synopsis(
                    biographyText,
                    title = person.name,
                    // As much as the web's page shows: the first screen filled,
                    // the rest a press away.
                    maxLines = 12,
                    modifier = Modifier.remembered(memory, "about").focusRequester(biography),
                )
            }
        }
    }
}

/** One line of what is known of them, as the web's list of credits. */
@Composable
private fun Fact(name: String, value: String) {
    val palette = LocalPalette.current
    Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        Text(name, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, color = palette.textFaint, modifier = Modifier.width(150.dp))
        Text(value, fontSize = 15.sp, color = palette.text)
    }
}

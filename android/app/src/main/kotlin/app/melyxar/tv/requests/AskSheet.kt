package app.melyxar.tv.requests

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import app.melyxar.R
import app.melyxar.server.Asking
import app.melyxar.server.Found
import app.melyxar.server.seasonsToAsk
import app.melyxar.tv.inside.Loaded
import app.melyxar.tv.inside.ready
import app.melyxar.tv.inside.rememberLoaded
import app.melyxar.tv.look.AccentButton
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.TvField
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.listLine
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.shadowRaised
import java.io.IOException
import kotlinx.coroutines.launch

/** The longest note the server keeps. */
private const val LONGEST_NOTE = 500

/**
 * Asking for one title, as the web's window: what it is, for a series the
 * whole of it or the seasons wanted (those already here cannot be chosen),
 * and a line to say more, such as which cut or which language. Closed once
 * the server has the request, or by the back key.
 */
@Composable
fun AskSheet(found: Found, onClose: () -> Unit) {
    val palette = LocalPalette.current
    val requests = LocalRequests.current
    val scope = rememberCoroutineScope()
    val language = requestLanguage()
    val series = found.catalogue == "series"
    val heldAny = found.held?.seasons.orEmpty().isNotEmpty()
    // A series held in part is asked for by its seasons: the whole of it is
    // here already.
    var whole by remember { mutableStateOf(!heldAny) }
    val chosen = remember { mutableStateListOf<Int>() }
    val note = rememberTextFieldState()
    var busy by remember { mutableStateOf(false) }
    var refused by remember { mutableStateOf(false) }
    val send = remember { FocusRequester() }
    val ready = !busy && (!series || whole || chosen.isNotEmpty())
    val shape = RoundedCornerShape(18.dp)
    Dialog(onDismissRequest = onClose) {
        Column(
            modifier = Modifier
                .width(640.dp)
                .boxShadow(shape, palette.shadowRaised)
                .clip(shape)
                .background(palette.surfaceRaised)
                .border(1.dp, palette.accent.copy(alpha = 0.55f), shape)
                .padding(28.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text(stringResource(R.string.tv_requests), fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = palette.text)
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Box(Modifier.size(64.dp, 96.dp).clip(RoundedCornerShape(8.dp))) { TitlePoster(found.poster, 64.dp) }
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(
                        listOfNotNull(found.title, found.year?.toString()).joinToString(" · "),
                        fontSize = 17.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = palette.text,
                    )
                    Text(catalogueName(found.catalogue), fontSize = 13.sp, color = palette.textFaint)
                    found.overview?.let { Text(it, fontSize = 14.sp, color = palette.textSoft, maxLines = 3, overflow = TextOverflow.Ellipsis) }
                }
            }
            if (refused) {
                Text(stringResource(R.string.tv_requests_not_sent), fontSize = 14.sp, color = palette.trouble)
            }
            if (series) {
                Text(stringResource(R.string.tv_requests_seasons), fontSize = 14.sp, fontWeight = FontWeight.SemiBold, color = palette.textSoft)
                TickLine(stringResource(R.string.tv_requests_whole_series), null, ticked = whole, enabled = !heldAny, onClick = { whole = !whole })
                if (!whole) {
                    Seasons(found.tmdbId, language, chosen)
                }
            }
            Text(stringResource(R.string.tv_requests_note), fontSize = 14.sp, fontWeight = FontWeight.SemiBold, color = palette.textSoft)
            TvField(state = note, placeholder = stringResource(R.string.tv_requests_note_example), onDone = {})
            AccentButton(
                stringResource(R.string.tv_requests_send),
                onClick = {
                    if (!ready) return@AccentButton
                    busy = true
                    refused = false
                    scope.launch {
                        try {
                            requests.ask(
                                Asking(
                                    catalogue = found.catalogue,
                                    tmdbId = found.tmdbId,
                                    seasons = if (series && !whole) chosen.sorted() else emptyList(),
                                    note = note.text.toString().trim().take(LONGEST_NOTE),
                                    language = language,
                                ),
                            )
                            onClose()
                        } catch (_: IOException) {
                            refused = true
                            busy = false
                        }
                    }
                },
                modifier = Modifier.focusRequester(send),
                icon = Icons.Request,
            )
        }
        LaunchedEffect(Unit) { send.requestFocus() }
    }
}

/** The seasons of a series, those already here ticked and not to be
 *  chosen, the others ticked as they are chosen. */
@Composable
private fun Seasons(tmdbId: String, language: String, chosen: MutableList<Int>) {
    val palette = LocalPalette.current
    val loaded = rememberLoaded("request-seasons:$language:$tmdbId") { seasonsToAsk(tmdbId, language) }.value
    if (loaded is Loaded.Failed) {
        Text(stringResource(R.string.tv_requests_seasons_unread), fontSize = 14.sp, color = palette.textSoft)
        return
    }
    LazyColumn(Modifier.heightIn(max = 240.dp)) {
        items(loaded.ready.orEmpty(), key = { it.number }) { season ->
            TickLine(
                stringResource(R.string.tv_season, season.number),
                if (season.held) stringResource(R.string.tv_requests_season_here) else pluralStringResource(R.plurals.tv_requests_episodes, season.episodes, season.episodes),
                ticked = season.held || season.number in chosen,
                enabled = !season.held,
                onClick = { if (season.number in chosen) chosen.remove(season.number) else chosen.add(season.number) },
            )
        }
    }
}

/** A line ticked or not, passed over by the remote when it cannot change. */
@Composable
private fun TickLine(name: String, aside: String?, ticked: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = Modifier
            .padding(top = 4.dp)
            .fillMaxWidth()
            .listLine { lit }
            .focusProperties { canFocus = enabled }
            .clickable(interaction, indication = null, enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(28.dp)) {
            if (ticked) Glyph(Icons.Tick, palette.accentBright, 18.dp)
        }
        Text(name, fontSize = 15.sp, color = if (enabled) palette.text else palette.textFaint, modifier = Modifier.weight(1f))
        aside?.let { Text(it, fontSize = 13.sp, color = palette.textFaint) }
    }
}

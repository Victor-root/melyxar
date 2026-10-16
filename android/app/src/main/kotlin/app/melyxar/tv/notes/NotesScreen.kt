package app.melyxar.tv.notes

import android.text.format.DateUtils
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Note
import app.melyxar.theme.Palette
import app.melyxar.tv.inside.Chrome
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.Place
import app.melyxar.tv.inside.ReadingColumn
import app.melyxar.tv.inside.RestoreFocus
import app.melyxar.tv.inside.ScrollingPage
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.head
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.remembered
import app.melyxar.tv.instantOf
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PlainButton
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.listLine
import java.text.DateFormat
import java.util.Date

/** The words notifications are said in on this device. */
@Composable
fun rememberWording(): Wording {
    val resources = LocalResources.current
    return remember(resources) {
        val format = DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT)
        ResourceWording(resources) { written -> instantOf(written)?.let { format.format(Date(it)) } }
    }
}

/** The colour of what a notification says: ok, attention, trouble or news. */
fun Palette.ofLevel(level: String): Color = when (level) {
    "ok" -> ok
    "attention" -> attention
    "trouble" -> trouble
    else -> news
}

/** Goes where pressing a notification leads, if anywhere. */
fun Chrome.follow(leads: Leads?) {
    when (leads) {
        is Leads.Work -> go(Place.Opened(leads.id))
        is Leads.Library -> libraries.firstOrNull { it.id == leads.id }?.let { go(Place.Shelf(it)) }
        null -> Unit
    }
}

/**
 * The account's notifications, newest first, as the web's page lists them:
 * pressing one reads it and opens what it is about; the remote moves down
 * them, and older ones are read as asked for.
 */
@Composable
fun NotesScreen() {
    val chrome = LocalChrome.current
    val notes = LocalNotes.current
    val palette = LocalPalette.current
    val wording = rememberWording()
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val list = rememberLazyListState()
    val held = notes.held
    // What the corner said is here now.
    LaunchedEffect(Unit) { notes.dismissAll() }
    ScrollingPage(toTop = { list.animateScrollToItem(0) }) { head ->
        LazyColumn(state = list, contentPadding = PaddingValues(bottom = 40.dp), modifier = Modifier.fillMaxSize()) {
            item("top") {
                Box(Modifier.head(head, "top")) { TopBar(first = first.takeIf { held.notes.isEmpty() }) }
            }
            item("title") {
                ReadingColumn {
                    Row(
                        modifier = Modifier.padding(top = 26.dp, bottom = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Glyph(Icons.Bell, palette.accentBright, 26.dp)
                        Text(
                            stringResource(R.string.tv_notes),
                            fontSize = 28.sp,
                            fontWeight = FontWeight.Bold,
                            letterSpacing = (-0.02).em,
                            color = palette.text,
                        )
                        if (held.unread > 0) {
                            Text("${held.unread}", fontSize = 16.sp, color = palette.textFaint, modifier = Modifier.padding(start = 4.dp))
                            Spacer(Modifier.weight(1f))
                            PlainButton(
                                stringResource(R.string.tv_notes_mark_all_read),
                                onClick = { notes.markRead() },
                                modifier = Modifier.remembered(memory, "all-read"),
                                icon = Icons.Tick,
                            )
                        }
                    }
                    if (notes.loaded && held.notes.isEmpty()) {
                        Text(stringResource(R.string.tv_notes_none), fontSize = 16.sp, color = palette.textSoft)
                    }
                }
            }
            items(held.notes, key = { it.id }) { note ->
                ReadingColumn {
                    NoteLine(
                        note,
                        wording,
                        onClick = {
                            notes.markRead(listOf(note.id))
                            chrome.follow(wording.say(note).leads)
                        },
                        // The first stands under the head, which stays shown
                        // while the remote is on it.
                        modifier = Modifier
                            .remembered(memory, note.id)
                            .then(if (note.id == held.notes.first().id) Modifier.focusRequester(first).head(head, "first") else Modifier),
                    )
                }
            }
            if (held.more) {
                item("older") {
                    ReadingColumn {
                        PlainButton(
                            stringResource(R.string.tv_notes_older),
                            onClick = notes::loadOlder,
                            modifier = Modifier.padding(top = 14.dp).remembered(memory, "older"),
                        )
                    }
                }
            }
        }
    }
    RestoreFocus(memory, first, ready = notes.loaded)
}

/** A notification in the list: its poster or its colour, what it says, how
 *  long ago, and whether it was read. */
@Composable
private fun NoteLine(note: Note, wording: Wording, onClick: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    val said = wording.say(note)
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = modifier
            .padding(top = 8.dp)
            .fillMaxWidth()
            .listLine { lit }
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        NoteMark(note, said)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                said.title,
                fontSize = 16.sp,
                fontWeight = if (note.read) FontWeight.Medium else FontWeight.SemiBold,
                color = if (lit > 0.5f) palette.accentBright else palette.text,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            said.detail?.let { Text(it, fontSize = 14.sp, color = palette.textSoft, maxLines = 2, overflow = TextOverflow.Ellipsis) }
            instantOf(note.createdAt)?.let { at ->
                Text(
                    DateUtils.getRelativeTimeSpanString(at, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString(),
                    fontSize = 12.sp,
                    color = palette.textFaint,
                )
            }
        }
        if (!note.read) {
            Box(Modifier.size(10.dp).clip(CircleShape).background(palette.accentBright))
        }
    }
}

/** What a notification wears: its poster, or a bell in its colour. */
@Composable
fun NoteMark(note: Note, said: NoteSaid) {
    val palette = LocalPalette.current
    if (said.poster.isNotEmpty()) {
        ServerPicture(said.poster, 48.dp, Modifier.size(48.dp, 72.dp).clip(RoundedCornerShape(6.dp)))
    } else {
        val colour = palette.ofLevel(note.level)
        Box(
            Modifier.size(48.dp).clip(CircleShape).background(colour.copy(alpha = 0.18f)),
            contentAlignment = Alignment.Center,
        ) {
            Glyph(Icons.Bell, colour, 24.dp)
        }
    }
}

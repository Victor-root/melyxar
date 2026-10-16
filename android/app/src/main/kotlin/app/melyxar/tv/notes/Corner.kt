package app.melyxar.tv.notes

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInHorizontally
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.server.Note
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.glassPiece
import app.melyxar.tv.look.rim
import kotlinx.coroutines.delay

/** How long a notification stays in the corner, by what it says. */
private fun staysForMs(level: String): Long = when (level) {
    "ok" -> 6_000
    "attention" -> 9_000
    "trouble" -> 10_000
    else -> 8_000
}

/**
 * The notifications said on the screen, in its top corner under the bar,
 * as the web's: each for a while, a sticky one until the page of
 * notifications is opened. Never in the way of the remote, which reads them
 * on that page.
 */
@Composable
fun NotesCorner(modifier: Modifier = Modifier) {
    val notes = LocalNotes.current
    val wording = rememberWording()
    Column(modifier.padding(top = 84.dp, end = 20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        notes.shown.forEach { note ->
            key(note.id) {
                val arriving = remember { MutableTransitionState(false).apply { targetState = true } }
                AnimatedVisibility(arriving, enter = fadeIn() + slideInHorizontally { it / 2 }) {
                    CornerNote(note, wording)
                }
                if (!note.sticky) {
                    LaunchedEffect(note.id) {
                        delay(note.shownForMs ?: staysForMs(note.level))
                        notes.dismiss(note)
                    }
                }
            }
        }
    }
}

@Composable
private fun CornerNote(note: Note, wording: Wording) {
    val palette = LocalPalette.current
    val said = wording.say(note)
    val shape = RoundedCornerShape(14.dp)
    Row(
        modifier = Modifier
            .width(380.dp)
            .glassPiece(shape)
            .rim(shape, palette.ofLevel(note.level), 1.5.dp)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        NoteMark(note, said)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(said.title, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = palette.text, maxLines = 2, overflow = TextOverflow.Ellipsis)
            said.detail?.let { Text(it, fontSize = 13.sp, color = palette.textSoft, maxLines = 2, overflow = TextOverflow.Ellipsis) }
        }
    }
}

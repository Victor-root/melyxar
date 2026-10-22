package app.melyxar.tv.inside

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Card
import app.melyxar.tv.lengthOfAPlay
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.outOfTen

/**
 * A work as a line of a list, as the web draws it: its picture, small, and
 * beside it its name and what is known of it. The picture takes the remote,
 * and the name is lit with it.
 */
@Composable
fun WorkLine(card: Card, lying: Boolean, frame: DpSize, onClick: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    var lit by remember { mutableStateOf(false) }
    val resume = LocalMarks.current.whereabouts(card).resume
    val facts = listOfNotNull(
        card.year?.toString(),
        card.rating?.let(::outOfTen),
        if (card.episodes > 0) {
            pluralStringResource(R.plurals.tv_episode_count, card.episodes, card.episodes)
        } else {
            card.runtimeMinutes?.let { howLong(it) }
        },
        resume?.let { whatIsLeft(it.toDouble(), lengthOfAPlay(card)) },
    )
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(20.dp)) {
        WorkCard(
            card = card,
            onClick = onClick,
            modifier = modifier.onFocusChanged { lit = it.isFocused },
            lying = lying,
            frame = frame,
            named = false,
        )
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                card.title,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                color = if (lit) palette.accentBright else palette.text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                facts.joinToString("  ·  "),
                fontSize = 14.sp,
                color = palette.textSoft,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

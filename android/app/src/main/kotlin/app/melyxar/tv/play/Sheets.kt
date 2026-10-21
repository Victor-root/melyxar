package app.melyxar.tv.play

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Work
import app.melyxar.tv.inside.CastMember
import app.melyxar.tv.inside.Facts
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.ServerPicture

/** How far down the film the sheets begin to shade it, and how much of
 *  their top is that shade alone, which the words may stand on. */
private val SHADED = 120.dp
private val SHADE = Brush.verticalGradient(0f to Color.Transparent, 0.35f to Color.Black.copy(alpha = 0.7f), 1f to Color.Black.copy(alpha = 0.9f))

/**
 * What the film is, over its foot as the web player's sheet of information
 * and as Emby lays it: the poster, the title, its facts and the whole of its
 * overview. The arrow down or the back key closes it.
 */
@Composable
fun InfoSheet(work: Work, onClose: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val held = remember { FocusRequester() }
    BackHandler(onBack = onClose)
    Row(
        modifier = modifier
            .fillMaxWidth()
            .standsAtTheFoot(SHADED)
            .background(SHADE)
            .padding(start = Gutter, end = Gutter, top = SHADED, bottom = 36.dp)
            .closesOnTheWayDown(onClose)
            .focusRequester(held)
            .focusable(),
        horizontalArrangement = Arrangement.spacedBy(28.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        ServerPicture(
            work.poster,
            150.dp,
            Modifier.width(150.dp).aspectRatio(2f / 3).clip(CardShape).background(palette.surfaceRaised),
        )
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(work.title, fontSize = 28.sp, fontWeight = FontWeight.Bold, color = palette.text, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Facts(work)
            work.overview?.let { Text(it, fontSize = 15.sp, lineHeight = 23.sp, color = palette.textSoft) }
        }
    }
    LaunchedEffect(Unit) { held.requestFocus() }
}

/**
 * Who is in the film, faces in a row the remote runs along, as the web
 * player's sheet of the cast. The arrow down or the back key closes it.
 */
@Composable
fun CastSheet(work: Work, onClose: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val first = remember { FocusRequester() }
    BackHandler(onBack = onClose)
    Column(
        modifier = modifier
            .fillMaxWidth()
            .standsAtTheFoot(SHADED)
            .background(SHADE)
            .padding(top = SHADED, bottom = 30.dp)
            .closesOnTheWayDown(onClose)
            .focusProperties { onExit = { cancelFocusChange() } }
            .focusGroup(),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Text(
            stringResource(R.string.tv_cast),
            fontSize = 18.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            modifier = Modifier.padding(start = Gutter),
        )
        LazyRow(contentPadding = PaddingValues(horizontal = Gutter), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            items(work.cast, key = { it.personId + it.role }) { credit ->
                val isFirst = credit == work.cast.first()
                Box(if (isFirst) Modifier.focusRequester(first) else Modifier) {
                    CastMember(credit, Modifier, onOpen = {})
                }
            }
        }
    }
    LaunchedEffect(Unit) { first.requestFocus() }
}

/** The arrow down, pressed on a sheet, closes it. */
private fun Modifier.closesOnTheWayDown(onClose: () -> Unit) = onPreviewKeyEvent { event ->
    if (event.type == KeyEventType.KeyDown && event.key == Key.DirectionDown) {
        onClose()
        true
    } else {
        false
    }
}

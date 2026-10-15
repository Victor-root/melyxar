package app.melyxar.tv.inside

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.server.Picture
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.tv.door.Face
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.MelyxarMark
import app.melyxar.tv.look.PillShape
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.glassPiece
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim

/**
 * The bar at the top of every screen, as the web's less its tabs, which the
 * band of the home screen stands for on a television: the way back and the
 * server's logo and name, which lead home, and whose profile this is. It
 * scrolls away with the screen, and the remote finds it again going up.
 * [first] is where the remote starts on a screen with nothing else to start
 * on; [edges] is how far it stands from the sides of what holds it, less on
 * a page that already keeps that margin.
 */
@Composable
fun TopBar(modifier: Modifier = Modifier, first: FocusRequester? = null, edges: Dp = Gutter) {
    val chrome = LocalChrome.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(start = edges, end = edges, top = 22.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(
            modifier = Modifier.height(46.dp).glassPiece().padding(horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            // Off the home screen only: there is nowhere to come back from
            // there, and the brand beside it already leads home.
            chrome.back?.let { back -> WayBack(back) }
            Brand(onClick = { chrome.go(Place.Home) })
        }
        Account(first?.let { Modifier.focusRequester(it) } ?: Modifier)
    }
}

/** The way back one screen, as the web's arrow at the start of the bar. */
@Composable
private fun WayBack(onClick: () -> Unit) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Box(
        modifier = Modifier
            .size(38.dp)
            .clip(CircleShape)
            .background(palette.accent.copy(alpha = 0.22f * lit))
            .rim(CircleShape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Glyph(Icons.Back, if (lit > 0.5f) palette.accentBright else palette.text, 22.dp)
    }
}

/** The server's logo and name, which lead home. */
@Composable
private fun Brand(onClick: () -> Unit) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = Modifier
            .height(38.dp)
            .clip(PillShape)
            .background(palette.accent.copy(alpha = 0.22f * lit))
            .rim(PillShape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(start = 5.dp, end = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val logo = chrome.branding.logo
        if (logo != null) {
            ServerPicture(
                listOf(Picture(logo)),
                28.dp,
                Modifier.height(28.dp).widthIn(max = 120.dp),
                contentScale = ContentScale.Fit,
            )
        } else {
            MelyxarMark(28.dp)
        }
        Text(
            chrome.branding.serverName,
            fontFamily = MelyxarDisplay,
            fontSize = 19.sp,
            letterSpacing = 0.08.em,
            color = if (lit > 0.5f) palette.accentBright else palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 220.dp),
        )
    }
}

/** Whose profile this is; choosing it goes back to "Who is watching?". */
@Composable
private fun Account(modifier: Modifier) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = modifier
            .height(46.dp)
            .glassPiece()
            .rim(PillShape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = chrome.changeProfile)
            .padding(start = 18.dp, end = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(
            chrome.account.name,
            fontSize = 14.sp,
            fontWeight = FontWeight.SemiBold,
            color = if (lit > 0.5f) palette.accentBright else palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.widthIn(max = 150.dp),
        )
        Box(Modifier.clip(RoundedCornerShape(7.dp))) {
            Face(chrome.account.name, chrome.account.avatar, 34.dp)
        }
    }
}

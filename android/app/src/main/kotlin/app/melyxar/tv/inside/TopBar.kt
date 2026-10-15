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
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Picture
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.tv.door.Face
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.MelyxarMark
import app.melyxar.tv.look.PillShape
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.glassPiece
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim
import androidx.compose.ui.layout.ContentScale

/**
 * The bar at the top of every screen, as the web's: the server's logo and
 * name, the libraries one tab each, and whose profile this is. It scrolls
 * away with the screen, and the remote finds it again going up.
 */
@Composable
fun TopBar(modifier: Modifier = Modifier, tabs: FocusRequester? = null) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(start = Gutter, end = Gutter, top = 22.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(
            modifier = Modifier.height(46.dp).glassPiece().padding(start = 9.dp, end = 18.dp),
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
                color = palette.text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.widthIn(max = 220.dp),
            )
        }
        Row(
            modifier = Modifier.glassPiece().padding(4.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Tab(
                stringResource(R.string.tv_home),
                active = chrome.here == Place.Home,
                onClick = { chrome.go(Place.Home) },
                modifier = tabs?.let { Modifier.focusRequester(it) } ?: Modifier,
            )
            chrome.libraries.forEach { library ->
                Tab(
                    library.name,
                    active = (chrome.here as? Place.Shelf)?.library?.id == library.id,
                    onClick = { chrome.go(Place.Shelf(library)) },
                )
            }
        }
        Account()
    }
}

@Composable
private fun Tab(text: String, active: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val focused = lit > 0.5f
    Text(
        text = text,
        fontSize = 15.sp,
        fontWeight = FontWeight.SemiBold,
        maxLines = 1,
        color = when {
            active -> palette.accentContrast
            focused -> palette.accentBright
            else -> palette.textSoft
        },
        modifier = modifier
            .clip(PillShape)
            .background(
                when {
                    active -> palette.accent
                    focused -> palette.accent.copy(alpha = 0.22f)
                    else -> palette.accent.copy(alpha = 0f)
                },
            )
            .rim(PillShape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/** Whose profile this is; choosing it goes back to "Who is watching?". */
@Composable
private fun Account() {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = Modifier
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

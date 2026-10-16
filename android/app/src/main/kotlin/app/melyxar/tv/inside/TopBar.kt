package app.melyxar.tv.inside

import android.text.format.DateFormat
import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import app.melyxar.R
import app.melyxar.server.Picture
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.tv.door.Face
import app.melyxar.tv.look.ChoiceList
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.MelyxarMark
import app.melyxar.tv.look.PillShape
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.glassPiece
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim
import app.melyxar.tv.music.LocalListening
import java.util.Date
import kotlinx.coroutines.delay

/** How far the bar stands from the sides and the top of the screen. */
private val BarEdge = 20.dp

/**
 * The bar at the top of every screen, as the web's less its tabs, which the
 * band of the home screen stands for on a television: on one piece of glass
 * the server's logo and name, which lead home, behind an arrow saying the
 * back key leads somewhere; on the other the buttons the account put on its
 * bar and whose profile this is, its menu holding the rest; and the time.
 * It scrolls away with the screen, and the remote finds it again going up.
 * [first] is where the remote starts on a screen with nothing else to start
 * on; [inset] is how far what holds it already stands from the sides of the
 * screen, the bar standing at the same place on every screen whatever it is.
 */
@Composable
fun TopBar(modifier: Modifier = Modifier, first: FocusRequester? = null, inset: Dp = 0.dp) {
    val chrome = LocalChrome.current
    Row(
        modifier = modifier
            .layout { measurable, constraints ->
                val out = inset.roundToPx()
                val across = constraints.maxWidth + 2 * out
                val bar = measurable.measure(constraints.copy(minWidth = across, maxWidth = across))
                layout(constraints.maxWidth, bar.height) { bar.place(-out, 0) }
            }
            .padding(start = BarEdge, end = BarEdge, top = BarEdge),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Brand()
        Spacer(Modifier.weight(1f))
        // Everything at this end on one piece of glass, as on the web: this
        // end is what is pressed, the other is where one is.
        Row(
            modifier = Modifier.height(46.dp).glassPiece().padding(horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            // The song playing, while one is.
            if (LocalListening.current.queue.current != null) {
                BarButton(Icons.Music) { chrome.go(Place.NowPlaying) }
            }
            chrome.buttons.bar.forEach { button ->
                barPlace(button)?.let { BarButton(it.icon) { it.press(chrome) } }
            }
            // A rule between who is here and everything that is not.
            Box(Modifier.padding(horizontal = 6.dp).width(1.dp).height(28.dp).background(LocalPalette.current.border))
            Account(first?.let { Modifier.focusRequester(it) } ?: Modifier)
        }
        Clock(Modifier.padding(start = 10.dp))
    }
}

/** What a button of the bar shows, its name in the account's menu, and
 *  what pressing it does. */
private class BarPlace(val icon: ImageVector, @param:StringRes val name: Int, val press: (Chrome) -> Unit)

private fun barPlace(button: String): BarPlace? = when (button) {
    "search" -> BarPlace(Icons.Search, R.string.tv_search) { it.search() }
    "favourites" -> BarPlace(Icons.Heart, R.string.tv_favourites) { it.go(Place.Listed(Shortlist.Favourites)) }
    "watch_later" -> BarPlace(Icons.Clock, R.string.tv_watch_later) { it.go(Place.Listed(Shortlist.WatchLater)) }
    "settings" -> BarPlace(Icons.Gear, R.string.tv_settings) { it.go(Place.Settings) }
    else -> null
}

/** The time, on its own piece of glass at the end of the bar, turning with
 *  the minute. */
@Composable
private fun Clock(modifier: Modifier) {
    val context = LocalContext.current
    val format = remember(context) { DateFormat.getTimeFormat(context) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(untilTheNextMinute(now))
            now = System.currentTimeMillis()
        }
    }
    Box(modifier.height(46.dp).glassPiece().padding(horizontal = 16.dp), contentAlignment = Alignment.Center) {
        Text(format.format(Date(now)), fontSize = 17.sp, fontWeight = FontWeight.SemiBold, color = LocalPalette.current.text)
    }
}

/**
 * The server's logo and name, which lead home, behind the arrow saying the
 * back key leads somewhere: the whole piece of glass lit as one by the
 * remote. On the home screen there is nowhere to go back to nor to lead to,
 * so it is passed over.
 */
@Composable
private fun Brand() {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Row(
        modifier = Modifier
            .height(46.dp)
            .glassPiece()
            .background(palette.accent.copy(alpha = 0.22f * lit))
            .rim(PillShape, palette.accentBright, 2.dp) { lit }
            .focusProperties { canFocus = chrome.canGoBack }
            .clickable(interaction, indication = null, enabled = chrome.canGoBack) { chrome.go(Place.Home) }
            .padding(start = if (chrome.canGoBack) 12.dp else 9.dp, end = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (chrome.canGoBack) {
            Glyph(Icons.Back, if (lit > 0.5f) palette.accentBright else palette.textSoft, 22.dp)
        }
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

/** A way somewhere from the bar, as the web's: the search, the favourites,
 *  the list to watch later; an icon on the bar's glass, lit as the logo is
 *  when the remote comes to it. */
@Composable
private fun BarButton(icon: ImageVector, onClick: () -> Unit) {
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
        Glyph(icon, if (lit > 0.5f) palette.accentBright else palette.text, 22.dp)
    }
}

/** Whose profile this is; choosing it opens the account's menu: the
 *  buttons it kept off the bar, then the way back to "Who is watching?". */
@Composable
private fun Account(modifier: Modifier) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            modifier = modifier
                .height(38.dp)
                .clip(PillShape)
                .background(palette.accent.copy(alpha = 0.22f * lit))
                .rim(PillShape, palette.accentBright, 2.dp) { lit }
                .clickable(interaction, indication = null) { open = true }
                .padding(start = 12.dp, end = 3.dp),
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
                Face(chrome.account.name, chrome.account.avatar, 32.dp)
            }
        }
        if (open) {
            val places = chrome.buttons.menu.mapNotNull(::barPlace)
            val close = { open = false }
            Popup(
                alignment = Alignment.TopEnd,
                offset = with(LocalDensity.current) { IntOffset(0, 46.dp.roundToPx()) },
                onDismissRequest = close,
                properties = PopupProperties(focusable = true),
            ) {
                ChoiceList(
                    title = chrome.account.name,
                    choices = places.map { stringResource(it.name) } + stringResource(R.string.tv_change_profile),
                    chosen = null,
                    onChoose = { at -> places.getOrNull(at)?.press?.invoke(chrome) ?: chrome.changeProfile() },
                    onClose = close,
                )
            }
        }
    }
}

package app.melyxar.tv.inside

import android.text.format.DateFormat
import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
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
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathMeasure
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
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
import app.melyxar.tv.look.GlassIcon
import app.melyxar.tv.look.GlassRound
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
import app.melyxar.tv.music.LocalListening
import app.melyxar.tv.notes.LocalNotes
import java.util.Date
import kotlinx.coroutines.delay

/** How far the bar stands from the top of the screen. */
private val BarTop = 20.dp

/**
 * The bar at the top of every screen, as the web's less its tabs, which the
 * band of the home screen stands for on a television: on one piece of glass
 * the server's logo and name, which lead home, behind an arrow saying the
 * back key leads somewhere; on the other the buttons the account put on its
 * bar and whose profile this is, with the time, its menu holding the rest.
 * It scrolls away with the screen, and the remote finds it again going up.
 * [first] is where the remote starts on a screen with nothing else to start
 * on; [inset] is how far what holds it already stands from the sides of the
 * screen, the bar standing at the page's margin on every screen whatever it
 * is. A page that is a library says so in the middle of the bar, [title].
 */
@Composable
fun TopBar(modifier: Modifier = Modifier, first: FocusRequester? = null, inset: Dp = 0.dp, title: (@Composable () -> Unit)? = null) {
    val chrome = LocalChrome.current
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(start = Gutter - inset, end = Gutter - inset, top = BarTop),
        contentAlignment = Alignment.Center,
    ) {
        title?.invoke()
        Bar(chrome, first)
    }
}

/** The bar's two ends. */
@Composable
private fun Bar(chrome: Chrome, first: FocusRequester?) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Brand()
        Spacer(Modifier.weight(1f))
        // Everything at this end on one piece of glass, as on the web: this
        // end is what is pressed, the other is where one is.
        Row(
            modifier = Modifier
                .height(46.dp)
                .glassPiece()
                .then(if (LocalListening.current.queue.current != null) Modifier.songPlayed() else Modifier)
                .padding(horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            // The song playing, while one is.
            if (LocalListening.current.queue.current != null) {
                PlayingNow { chrome.go(Place.NowPlaying) }
            }
            val unread = LocalNotes.current.held.unread
            chrome.buttons.bar.forEach { button ->
                barPlace(button)?.let { BarButton(it.icon, stringResource(it.name), if (button == "notifications") unread else 0) { it.press(chrome) } }
            }
            // A rule between who is here and everything that is not.
            Box(Modifier.padding(horizontal = 6.dp).width(1.dp).height(28.dp).background(LocalPalette.current.border))
            Account(first?.let { Modifier.focusRequester(it) } ?: Modifier)
        }
    }
}

/** The name of the library a page is, with its icon and, when it says, how
 *  many works it holds, for the middle of the bar. */
@Composable
fun BarTitle(icon: ImageVector, name: String, count: Int? = null) {
    val palette = LocalPalette.current
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Glyph(icon, palette.accentBright, 24.dp)
        Text(name, fontSize = 24.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.02).em, color = palette.text, maxLines = 1)
        count?.let { Text("$it", fontSize = 15.sp, color = palette.textFaint) }
    }
}

/** What a button of the bar shows, its name in the account's menu, and
 *  what pressing it does. */
private class BarPlace(val icon: ImageVector, @param:StringRes val name: Int, val press: (Chrome) -> Unit)

private fun barPlace(button: String): BarPlace? = when (button) {
    "search" -> BarPlace(Icons.Search, R.string.tv_search) { it.search() }
    "favourites" -> BarPlace(Icons.Heart, R.string.tv_favourites) { it.go(Place.Listed(Shortlist.Favourites)) }
    "watch_later" -> BarPlace(Icons.Clock, R.string.tv_watch_later) { it.go(Place.Listed(Shortlist.WatchLater)) }
    "notifications" -> BarPlace(Icons.Bell, R.string.tv_notes) { it.go(Place.Notifications) }
    "settings" -> BarPlace(Icons.Gear, R.string.tv_settings) { it.go(Place.Settings) }
    else -> null
}

/** The time, under the account's name, turning with the minute. */
@Composable
private fun Clock(colour: Color) {
    val context = LocalContext.current
    val format = remember(context) { DateFormat.getTimeFormat(context) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(untilTheNextMinute(now))
            now = System.currentTimeMillis()
        }
    }
    Text(format.format(Date(now)), fontSize = 11.sp, lineHeight = 13.sp, fontWeight = FontWeight.Medium, color = colour)
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
 *  when the remote comes to it, with how many are waiting when [count]
 *  says any are. */
@Composable
private fun BarButton(icon: ImageVector, label: String, count: Int = 0, onClick: () -> Unit) {
    val palette = LocalPalette.current
    Box {
        GlassIcon(icon, label, onClick)
        if (count > 0) {
            Text(
                if (count > MOST_COUNTED) "$MOST_COUNTED+" else "$count",
                fontSize = 10.sp,
                fontWeight = FontWeight.Bold,
                color = palette.accentContrast,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .offset(x = 4.dp, y = (-2).dp)
                    .clip(PillShape)
                    .background(palette.accent)
                    .padding(horizontal = 5.dp, vertical = 1.dp),
            )
        }
    }
}

/** The song playing: its cover in a round, or the note of music for one
 *  without. */
@Composable
private fun PlayingNow(onClick: () -> Unit) {
    val palette = LocalPalette.current
    val listening = LocalListening.current
    val cover = listening.queue.current?.cover.orEmpty()
    GlassRound(
        stringResource(R.string.tv_music_now_playing),
        onClick,
) { lit ->
        if (cover.isEmpty()) {
            Glyph(Icons.Music, if (lit() > 0.5f) palette.accentBright else palette.text, 20.dp)
        } else {
            ServerPicture(cover, 30.dp, Modifier.size(30.dp).clip(CircleShape))
        }
    }
}

/**
 * How far the song playing has played, drawn round the whole of the piece
 * of glass it is on, from the middle of its top and clockwise: the track
 * faint, what has played in the accent with its glow. Laid out once for the
 * piece; only drawn again as the song plays.
 */
@Composable
private fun Modifier.songPlayed(): Modifier {
    val palette = LocalPalette.current
    val listening = LocalListening.current
    return drawWithCache {
        val line = 2.5.dp.toPx()
        val inset = line / 2
        val round = size.height / 2 - inset
        val (left, right, top, bottom) = listOf(inset, size.width - inset, inset, size.height - inset)
        val outline = Path().apply {
            moveTo(size.width / 2, top)
            lineTo(right - round, top)
            arcTo(Rect(right - 2 * round, top, right, bottom), -90f, 180f, false)
            lineTo(left + round, bottom)
            arcTo(Rect(left, top, left + 2 * round, bottom), 90f, 180f, false)
            close()
        }
        val measure = PathMeasure().apply { setPath(outline, false) }
        val played = Path()
        onDrawWithContent {
            drawContent()
            drawPath(outline, palette.text.copy(alpha = 0.16f), style = Stroke(line))
            val done = listening.length?.takeIf { it > 0 }?.let { (listening.position / it).toFloat().coerceIn(0f, 1f) } ?: 0f
            played.reset()
            measure.getSegment(0f, measure.length * done, played, true)
            drawPath(played, palette.accentGlow, style = Stroke(line * 3, cap = StrokeCap.Round))
            drawPath(played, palette.accentBright, style = Stroke(line, cap = StrokeCap.Round))
        }
    }
}

/** The most waiting the bell counts before it says "more". */
private const val MOST_COUNTED = 99

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
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    chrome.account.name,
                    fontSize = 14.sp,
                    lineHeight = 16.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = if (lit > 0.5f) palette.accentBright else palette.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.widthIn(max = 150.dp),
                )
                Clock(palette.textSoft)
            }
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

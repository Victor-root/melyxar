package app.melyxar.tv.inside

import android.text.format.DateFormat
import androidx.annotation.StringRes
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.MarqueeSpacing
import androidx.compose.foundation.background
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import app.melyxar.R
import app.melyxar.server.Picture
import app.melyxar.server.Song
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
import app.melyxar.tv.music.NowDisc
import app.melyxar.tv.music.artistsOf
import app.melyxar.tv.music.rememberNow
import app.melyxar.tv.notes.LocalNotes
import java.util.Date
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

/*
 * What the remote is on in the bar is ringed in the colour of the text,
 * white on the dark theme, rather than the accent the bar's glass is
 * already tinted with.
 */

/** How far the bar stands from the top of the screen, and how tall it is. */
private val BarTop = 20.dp
private val BarHeight = 46.dp

/**
 * The bar at the top of every screen, as the web's less its tabs, which the
 * band of the home screen stands for on a television: on one piece of glass
 * the server's logo and name, which lead home, behind an arrow saying the
 * back key leads somewhere; on the other the buttons the account put on its
 * bar and whose profile this is, with the time, its menu holding the rest.
 * It scrolls away with the screen, and the remote finds it again going up;
 * while a song plays, it is pinned over the screen instead (PinnedTopBar).
 * [first] is where the remote starts on a screen with nothing else to start
 * on; [inset] is how far what holds it already stands from the sides of the
 * screen, the bar standing at the page's margin on every screen whatever it
 * is. A page that is a library says so in the middle of the bar, [title].
 */
@Composable
fun TopBar(modifier: Modifier = Modifier, first: FocusRequester? = null, inset: Dp = 0.dp, title: (@Composable () -> Unit)? = null) {
    // What this screen puts in the bar, kept for the bar pinned over it
    // while a song plays, whether or not this piece of the page is drawn.
    val pinned = LocalPinnedBar.current
    val here = LocalChrome.current.here
    SideEffect {
        val mine = pinned.shown[here] ?: BarShown().also { pinned.shown[here] = it }
        mine.first = first
        mine.title = title
    }
    if (LocalListening.current.queue.current != null) {
        // The bar is pinned over the screen instead; its room here is kept.
        Spacer(modifier.fillMaxWidth().padding(top = BarTop).height(BarHeight))
        return
    }
    BarLine(modifier.padding(start = Gutter - inset, end = Gutter - inset), first, title)
}

/** What a screen puts in the bar pinned over it. */
@Stable
class BarShown {
    var first by mutableStateOf<FocusRequester?>(null)
    var title by mutableStateOf<(@Composable () -> Unit)?>(null)
}

/** What each screen of the trail puts in the bar pinned over the screens
 *  while a song plays, and the last song announced in it. No bar is pinned
 *  over a film, nor over the screens that say they have none, a photo's. */
@Stable
class PinnedBar {
    val shown = mutableStateMapOf<Place, BarShown>()
    val barless = mutableStateListOf<Place>()
    var announced by mutableStateOf<String?>(null)

    /** Forgets the screens no longer in the trail. */
    fun keepOnly(trail: List<Place>) {
        shown.keys.retainAll(trail.toSet())
        barless.retainAll(trail.toSet())
    }
}

/** Says the screen shown has no bar, none to be pinned over it. */
@Composable
fun NoBarPinned() {
    val pinned = LocalPinnedBar.current
    val here = LocalChrome.current.here
    DisposableEffect(pinned, here) {
        pinned.barless += here
        onDispose { pinned.barless -= here }
    }
}

val LocalPinnedBar = staticCompositionLocalOf { PinnedBar() }

/**
 * The bar over every screen that has one while a song plays: the song is
 * played from it, so it stays where it is however far the screen scrolls,
 * its glass filling with the accent as the song plays. As each song starts,
 * its piece of glass gives way for a moment to the song's name beside its
 * cover, so the eye finds what began; the remote coming to it ends that.
 * Down from it, the remote goes back into [screen] where it left it: the
 * screen stands under the bar as much as below it, so nothing there is
 * found below the bar by looking.
 */
@Composable
fun PinnedTopBar(screen: FocusRequester, modifier: Modifier = Modifier) {
    val pinned = LocalPinnedBar.current
    val here = LocalChrome.current.here
    val song = LocalListening.current.queue.current ?: return
    if (here is Place.Playing || here in pinned.barless) {
        return
    }
    // Kept from one screen to the next, the remote with it: what a screen
    // puts in it comes a moment after the screen does.
    val shown = pinned.shown[here]
    var announcing by remember { mutableStateOf(false) }
    // How long the name is said, once the glass has measured it.
    var said by remember(song.id) { mutableStateOf<Long?>(null) }
    LaunchedEffect(song.id) {
        if (pinned.announced != song.id) {
            pinned.announced = song.id
            announcing = true
            delay(withTimeoutOrNull(MEASURED_WITHIN_MS) { snapshotFlow { said }.filterNotNull().first() } ?: SAID_WHOLE_MS)
            announcing = false
        }
    }
    val focus = LocalFocusManager.current
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(start = Gutter, end = Gutter, top = BarTop)
            .focusTraced("pinned bar")
            .onPreviewKeyEvent { event ->
                if (event.key != Key.DirectionDown || event.type != KeyEventType.KeyDown) {
                    return@onPreviewKeyEvent false
                }
                if (!focus.moveFocus(FocusDirection.Down)) {
                    screen.requestFocus()
                }
                true
            },
        contentAlignment = Alignment.Center,
    ) {
        shown?.title?.invoke()
        Bar(
            LocalChrome.current,
            shown?.first,
            announced = song.takeIf { announcing },
            onReached = { announcing = false },
            onMeasured = { said = it },
        )
    }
}

/** The bar's line: what the screen names in its middle, and its two ends. */
@Composable
private fun BarLine(modifier: Modifier, first: FocusRequester?, title: (@Composable () -> Unit)?) {
    val chrome = LocalChrome.current
    Box(
        modifier = modifier.fillMaxWidth().padding(top = BarTop),
        contentAlignment = Alignment.Center,
    ) {
        title?.invoke()
        Bar(chrome, first)
    }
}

/** The song that has just started, in place of what its piece of glass
 *  holds: its cover, and its name and who sings it running along beside it
 *  when they are longer than the glass. */
@Composable
private fun Announced(song: Song, onMeasured: (Long) -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    var room by remember { mutableIntStateOf(0) }
    var wide by remember { mutableIntStateOf(0) }
    val speed = with(LocalDensity.current) { NAME_RUNS.toPx() }
    LaunchedEffect(room, wide) {
        if (room > 0 && wide > 0) onMeasured(saidForMs(wide, room, speed))
    }
    Row(modifier.padding(start = 5.dp, end = 16.dp), verticalAlignment = Alignment.CenterVertically) {
        Cover(song)
        // Once round, and gone when it is: the glass it runs in, then the
        // name as long as it is.
        Row(
            Modifier
                .padding(start = 12.dp)
                .onSizeChanged { room = it.width }
                .basicMarquee(
                    iterations = 1,
                    initialDelayMillis = RUN_WAITS_MS.toInt(),
                    spacing = MarqueeSpacing.fractionOfContainer(RUN_GAP),
                    velocity = NAME_RUNS,
                )
                .onSizeChanged { wide = it.width },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(song.title, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = palette.text, maxLines = 1)
            artistsOf(song.artists).takeIf { it.isNotEmpty() }?.let {
                Text("  \u00B7  $it", fontSize = 14.sp, color = palette.textSoft, maxLines = 1)
            }
        }
    }
}

/** How long the glass is given to measure the name of a song that has just
 *  started, before it is said for the usual time. */
private const val MEASURED_WITHIN_MS = 1_000L

/** How fast a name that does not fit runs along the glass. */
private val NAME_RUNS = 40.dp

/** The bar's two ends; [announced], the song that has just started, takes
 *  the place of what the piece of glass of the song holds while it is said,
 *  that kept under it so the remote finds it where it was. */
@Composable
private fun Bar(
    chrome: Chrome,
    first: FocusRequester?,
    announced: Song? = null,
    onReached: () -> Unit = {},
    onMeasured: (Long) -> Unit = {},
) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Brand()
        Spacer(Modifier.weight(1f))
        // Everything at this end on one piece of glass, as on the web: this
        // end is what is pressed, the other is where one is.
        Box(
            modifier = Modifier
                .height(BarHeight)
                .glassPiece()
                .then(if (LocalListening.current.queue.current != null) Modifier.songFilled() else Modifier)
                .onFocusChanged { if (it.hasFocus) onReached() },
        ) {
            val hidden by animateFloatAsState(if (announced != null) 0f else 1f, label = "announced")
            Row(
                modifier = Modifier.fillMaxHeight().padding(horizontal = 4.dp).graphicsLayer { alpha = hidden },
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
            if (announced != null) {
                Announced(announced, onMeasured, Modifier.matchParentSize().graphicsLayer { alpha = 1f - hidden })
            }
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

/** The time, turning with the minute. */
@Composable
fun Clock(colour: Color, fontSize: TextUnit = 11.sp, weight: FontWeight = FontWeight.Medium) {
    val context = LocalContext.current
    val format = remember(context) { DateFormat.getTimeFormat(context) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(untilTheNextMinute(now))
            now = System.currentTimeMillis()
        }
    }
    Text(format.format(Date(now)), fontSize = fontSize, lineHeight = fontSize * 1.2f, fontWeight = weight, color = colour)
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
            .rim(PillShape, palette.text, 2.dp) { lit }
            .focusProperties { canFocus = chrome.canGoBack }
            .clickable(interaction, indication = null, enabled = chrome.canGoBack) { chrome.go(Place.Home) }
            .padding(start = if (chrome.canGoBack) 12.dp else 9.dp, end = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (chrome.canGoBack) {
            Glyph(Icons.Back, if (lit > 0.5f) palette.text else palette.textSoft, 22.dp)
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
            color = palette.text,
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
        GlassIcon(icon, label, onClick, ring = palette.text)
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

/** The song playing: its cover in a round, which leads to its page; under
 *  the remote, the mark of the song playing over it, which a ring alone on
 *  a cover does not say plainly enough. */
@Composable
private fun PlayingNow(onClick: () -> Unit) {
    val song = LocalListening.current.queue.current ?: return
    val now = rememberNow { true }
    GlassRound(stringResource(R.string.tv_music_now_playing), onClick, ring = LocalPalette.current.text) { lit ->
        Cover(song)
        // Made only under the remote: its bars keep the song's levels
        // running while they are shown.
        if (lit() > 0f) {
            NowDisc(now, Modifier.graphicsLayer { alpha = lit() })
        }
    }
}

/** A song's cover in a round, or the note of music for one without. */
@Composable
private fun Cover(song: Song) {
    if (song.cover.isEmpty()) {
        Glyph(Icons.Music, LocalPalette.current.text, 20.dp)
    } else {
        ServerPicture(song.cover, 36.dp, Modifier.size(36.dp).clip(CircleShape))
    }
}

/** The glass filled with the accent from its left end as far as the song
 *  playing has played, whole once it ends. Only drawn again as it plays. */
@Composable
private fun Modifier.songFilled(): Modifier {
    val palette = LocalPalette.current
    val listening = LocalListening.current
    return drawBehind {
        val done = listening.length?.takeIf { it > 0 }?.let { (listening.position / it).toFloat().coerceIn(0f, 1f) } ?: 0f
        drawRect(palette.accent.copy(alpha = 0.45f), size = size.copy(width = size.width * done))
    }
}

/** The room round the account's menu its shadow spreads into. */
private val MENU_SHADOW = 40.dp

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
                .rim(PillShape, palette.text, 2.dp) { lit }
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
                    color = palette.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.widthIn(max = 150.dp),
                )
                Clock(palette.text, 14.sp, FontWeight.SemiBold)
            }
            Box(Modifier.clip(RoundedCornerShape(7.dp))) {
                Face(chrome.account.name, chrome.account.avatar, 32.dp)
            }
        }
        if (open) {
            val places = chrome.buttons.menu.mapNotNull(::barPlace)
            val close = { open = false }
            // The window of the menu holds its shadow too, or the shadow
            // stops square at the window's edge.
            Popup(
                alignment = Alignment.TopEnd,
                offset = with(LocalDensity.current) { IntOffset(MENU_SHADOW.roundToPx(), (46.dp - MENU_SHADOW).roundToPx()) },
                onDismissRequest = close,
                properties = PopupProperties(focusable = true),
            ) {
                ChoiceList(
                    title = chrome.account.name,
                    choices = places.map { stringResource(it.name) } + stringResource(R.string.tv_change_profile),
                    chosen = null,
                    onChoose = { at -> places.getOrNull(at)?.press?.invoke(chrome) ?: chrome.changeProfile() },
                    onClose = close,
                    modifier = Modifier.padding(start = MENU_SHADOW, end = MENU_SHADOW, top = MENU_SHADOW, bottom = MENU_SHADOW + 16.dp),
                    icons = places.map { it.icon } + Icons.Leave,
                )
            }
        }
    }
}

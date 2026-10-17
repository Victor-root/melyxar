package app.melyxar.tv.requests

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Found
import app.melyxar.server.Picture
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.PictureCard
import app.melyxar.tv.inside.Place
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PosterWidth
import app.melyxar.tv.look.ServerPicture

/** What a catalogue is called: a film, or a series. */
@Composable
fun catalogueName(catalogue: String): String =
    stringResource(if (catalogue == "series") R.string.tv_requests_series else R.string.tv_requests_film)

/** A word on where something stands, in the colour of how it stands. */
@Composable
fun StatePill(text: String, colour: Color, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(percent = 50)
    Text(
        text,
        fontSize = 12.sp,
        fontWeight = FontWeight.SemiBold,
        color = colour,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = modifier
            .clip(shape)
            .background(palette.ground.copy(alpha = 0.82f))
            .border(1.dp, colour.copy(alpha = 0.6f), shape)
            .padding(horizontal = 9.dp, vertical = 3.dp),
    )
}

/** The poster of a title of the provider, or the mark of a title without
 *  one, never a broken picture. */
@Composable
fun TitlePoster(poster: String?, across: Dp) {
    val palette = LocalPalette.current
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Glyph(Icons.Film, palette.textFaint, across / 4)
        poster?.let { ServerPicture(listOf(Picture(it)), across, Modifier.fillMaxSize()) }
    }
}

/**
 * A title found or suggested, as the web's card of one: its poster, its
 * name, its year and whether it is a film or a series, and on the poster
 * whether it is here already or asked for by this account. Pressed, its
 * page, where it is asked for.
 */
@Composable
fun TitleTile(found: Found, modifier: Modifier = Modifier) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val standing = standingOf(found, LocalRequests.current.mine)
    PictureCard(
        across = PosterWidth,
        down = PosterWidth * 3 / 2,
        title = found.title,
        note = listOfNotNull(found.year?.toString(), catalogueName(found.catalogue)).joinToString(" · "),
        trailing = null,
        ground = palette.surfaceRaised,
        onClick = { chrome.go(Place.RequestedTitle(found.catalogue, found.tmdbId)) },
        modifier = modifier,
    ) {
        TitlePoster(found.poster, PosterWidth)
        val pill = when (standing) {
            is Standing.Here -> stringResource(R.string.tv_requests_here) to palette.ok
            is Standing.Mine -> stringResource(R.string.tv_requests_asked_by_you) to palette.news
            is Standing.Free -> null
        }
        pill?.let { (text, colour) -> StatePill(text, colour, Modifier.align(Alignment.BottomStart).padding(6.dp)) }
    }
}

/** The card at the end of a row of suggestions, which leads to many more. */
@Composable
fun MoreTile(onClick: () -> Unit, modifier: Modifier = Modifier) {
    val palette = LocalPalette.current
    PictureCard(
        across = PosterWidth,
        down = PosterWidth * 3 / 2,
        title = stringResource(R.string.tv_requests_more),
        note = null,
        trailing = null,
        ground = palette.surfaceRaised,
        onClick = onClick,
        modifier = modifier,
    ) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Glyph(Icons.ChevronRight, palette.accentBright, 40.dp)
        }
    }
}

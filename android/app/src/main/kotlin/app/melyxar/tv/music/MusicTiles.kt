package app.melyxar.tv.music

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Album
import app.melyxar.server.Artist
import app.melyxar.server.Credited
import app.melyxar.server.Genre
import app.melyxar.server.Picture
import app.melyxar.server.PlaylistSummary
import app.melyxar.server.Song
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.theme.parseHex
import app.melyxar.tv.clock
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.halo
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim
import app.melyxar.tv.look.shadowTile

/** How wide a tile of music is: an album, an artist, a playlist, a genre. */
val TileWidth = 176.dp

/** The artists of something, as a line: "Various artists" for a compilation. */
@Composable
fun artistsOf(artists: List<Credited>, compilation: Boolean = false): String =
    if (compilation) stringResource(R.string.tv_music_various_artists) else artists.joinToString(", ") { it.name }

/**
 * A square or round picture of music lit as the remote comes to it, as the
 * web's tiles: its own colour and its letter while there is no picture.
 */
@Composable
private fun Tile(
    pictures: List<Picture>,
    color: String?,
    letter: String,
    shape: Shape,
    title: String,
    under: String?,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Column(modifier.width(TileWidth).clickable(interaction, indication = null, onClick = onClick)) {
        Box(
            // As wide as what holds it, which a grid stretches to fill its
            // lines as the web's does.
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .grows({ lit })
                .boxShadow(shape, *palette.shadowTile) { 1f - lit }
                .halo(shape, palette) { lit }
                .clip(shape)
                .background(color?.let(::parseHex)?.let(::Color) ?: palette.surfaceRaised)
                .rim(shape, palette.accentBright, 1.5.dp) { lit },
            contentAlignment = Alignment.Center,
        ) {
            if (pictures.isEmpty()) {
                Text(letter.uppercase(), fontFamily = MelyxarDisplay, fontSize = 52.sp, color = palette.text.copy(alpha = 0.3f))
            } else {
                ServerPicture(pictures, TileWidth, Modifier.fillMaxSize())
            }
        }
        Text(
            title,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            color = if (lit > 0.5f) palette.text else palette.text.copy(alpha = 0.92f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 10.dp),
        )
        under?.let { Text(it, fontSize = 12.sp, color = palette.textFaint, maxLines = 1, overflow = TextOverflow.Ellipsis) }
    }
}

@Composable
fun AlbumTile(album: Album, onClick: () -> Unit, modifier: Modifier = Modifier) = Tile(
    pictures = album.cover,
    color = album.color,
    letter = album.initial.ifEmpty { album.title.take(1) },
    shape = CardShape,
    title = album.title,
    under = artistsOf(album.artists, album.compilation).ifEmpty { album.year?.toString() },
    onClick = onClick,
    modifier = modifier,
)

@Composable
fun ArtistTile(artist: Artist, onClick: () -> Unit, modifier: Modifier = Modifier) = Tile(
    pictures = artist.picture,
    color = artist.color,
    letter = artist.initial.ifEmpty { artist.name.take(1) },
    shape = CircleShape,
    title = artist.name,
    under = pluralStringResource(R.plurals.tv_music_album_count, artist.albums, artist.albums),
    onClick = onClick,
    modifier = modifier,
)

@Composable
fun PlaylistTile(playlist: PlaylistSummary, onClick: () -> Unit, modifier: Modifier = Modifier) = Tile(
    pictures = playlist.cover,
    color = null,
    letter = playlist.name.take(1),
    shape = CardShape,
    title = playlist.name,
    under = pluralStringResource(R.plurals.tv_music_song_count, playlist.songs, playlist.songs),
    onClick = onClick,
    modifier = modifier,
)

@Composable
fun GenreTile(genre: Genre, onClick: () -> Unit, modifier: Modifier = Modifier) = Tile(
    // The first of the albums it is shown by stands for it.
    pictures = genre.shown.firstOrNull()?.cover.orEmpty(),
    color = genre.shown.firstOrNull()?.color,
    letter = genre.name.take(1),
    shape = CardShape,
    title = genre.name,
    under = pluralStringResource(R.plurals.tv_music_album_count, genre.albums, genre.albums),
    onClick = onClick,
    modifier = modifier,
)

/** A song played on its own, as the rows of what was listened to show it:
 *  its cover, its name and its artists. */
@Composable
fun SongTile(song: Song, onClick: () -> Unit, modifier: Modifier = Modifier) = Tile(
    pictures = song.cover,
    color = null,
    letter = song.title.take(1),
    shape = CardShape,
    title = song.title,
    under = artistsOf(song.artists),
    onClick = onClick,
    modifier = modifier,
)

/** How wide the cover beside a song in a list is. */
private val LINE_COVER: Dp = 44.dp

/**
 * A song in a list, as the web's: its number or its cover, its name and its
 * artists, its album when the list is not one album's, and how long it
 * lasts; the one playing marked in the accent. Without [onClick], only
 * shown, the remote passing it by.
 */
@Composable
fun SongLine(
    song: Song,
    number: String?,
    playingNow: Boolean,
    onClick: (() -> Unit)?,
    modifier: Modifier = Modifier,
    withAlbum: Boolean = true,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val shape = RoundedCornerShape(10.dp)
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(palette.accent.copy(alpha = 0.16f * lit))
            .rim(shape, palette.accentBright, 1.5.dp) { lit }
            .then(if (onClick != null) Modifier.clickable(interaction, indication = null, onClick = onClick) else Modifier)
            .padding(horizontal = 14.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Box(Modifier.width(LINE_COVER), contentAlignment = Alignment.Center) {
            when {
                playingNow -> Glyph(Icons.Music, palette.accentBright, 20.dp)
                number != null -> Text(number, fontSize = 14.sp, color = palette.textFaint)
                else -> Box(Modifier.size(LINE_COVER).clip(RoundedCornerShape(6.dp)).background(palette.surfaceRaised)) {
                    ServerPicture(song.cover, LINE_COVER, Modifier.fillMaxSize())
                }
            }
        }
        Column(Modifier.weight(1f)) {
            Text(
                song.title,
                fontSize = 15.sp,
                fontWeight = FontWeight.Medium,
                color = if (playingNow) palette.accentBright else palette.text,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(artistsOf(song.artists), fontSize = 13.sp, color = palette.textFaint, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (withAlbum) {
            Text(
                song.album?.name.orEmpty(),
                fontSize = 13.sp,
                color = palette.textFaint,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(0.6f),
            )
        }
        Text(song.seconds?.let { clock(it.toDouble()) }.orEmpty(), fontSize = 13.sp, color = palette.textFaint)
    }
}

package app.melyxar.server

import kotlinx.serialization.Serializable

/*
 * What the server says of music, field for field as it writes it: albums,
 * artists, songs, playlists, and what an account has done with them.
 */

/** An artist or an album named on something else, with where it leads. */
@Serializable
data class Credited(val id: String, val name: String)

@Serializable
data class Album(
    val id: String,
    val title: String,
    val artists: List<Credited> = emptyList(),
    /** Songs of many artists, said "Various artists". */
    val compilation: Boolean = false,
    val year: Int? = null,
    /** How many songs it holds. */
    val songs: Int = 0,
    val color: String? = null,
    val initial: String = "",
    val cover: List<Picture> = emptyList(),
)

@Serializable
data class Artist(
    val id: String,
    val name: String,
    val initial: String = "",
    val albums: Int = 0,
    val songs: Int = 0,
    val color: String? = null,
    val picture: List<Picture> = emptyList(),
)

@Serializable
data class Song(
    val id: String,
    val title: String,
    val artists: List<Credited> = emptyList(),
    val album: Credited? = null,
    val track: Int? = null,
    val disc: Int? = null,
    val year: Int? = null,
    /** How long it lasts, to the nearest second. */
    val seconds: Long? = null,
    val cover: List<Picture> = emptyList(),
    /** How loud it was measured to be, for every song to sound as loud. */
    val lufs: Double? = null,
    val peakDbfs: Double? = null,
    val albumLufs: Double? = null,
)

/** A stretch of a long list, and how long the whole list is. */
@Serializable
data class MusicPage<T>(val items: List<T> = emptyList(), val total: Int = 0)

/** An album and its songs, in the order of its discs. */
@Serializable
data class AlbumPage(
    val id: String,
    val title: String,
    val artists: List<Credited> = emptyList(),
    val compilation: Boolean = false,
    val year: Int? = null,
    val color: String? = null,
    val cover: List<Picture> = emptyList(),
    val tracks: List<Song> = emptyList(),
)

/** An artist, their own albums and those they appear on. */
@Serializable
data class ArtistPage(
    val id: String,
    val name: String,
    val albums: Int = 0,
    val songs: Int = 0,
    val color: String? = null,
    val picture: List<Picture> = emptyList(),
    val theirAlbums: List<Album> = emptyList(),
    val appearsOn: List<Album> = emptyList(),
)

/** A playlist of the account, as a list of them shows it. */
@Serializable
data class PlaylistSummary(
    val id: String,
    val name: String,
    val songs: Int = 0,
    val seconds: Long = 0,
    val cover: List<Picture> = emptyList(),
)

@Serializable
data class Playlist(val id: String, val name: String, val tracks: List<Song> = emptyList())

/** What a search or the account's favourites found, by kind. */
@Serializable
data class FoundMusic(
    val albums: List<Album> = emptyList(),
    val artists: List<Artist> = emptyList(),
    val songs: List<Song> = emptyList(),
)

/** A genre, how many albums it holds, and the albums it is shown by. */
@Serializable
data class Genre(val name: String, val albums: Int = 0, val shown: List<GenreAlbum> = emptyList())

@Serializable
data class GenreAlbum(val id: String, val color: String? = null, val cover: List<Picture> = emptyList())

/** How the account listens: what its player does between and within songs,
 *  and the tabs of a library it hides. */
@Serializable
data class MusicPreferences(
    /** off, track or album. */
    val volumeMode: String = "track",
    /** Nought for songs that follow one another without a gap. */
    val crossfadeSeconds: Int = 0,
    val skipBackSeconds: Int = 10,
    val skipOnSeconds: Int = 10,
    val hiddenTabs: List<String> = emptyList(),
    /** Whether the wave of the song is drawn behind the player, and how
     *  high it reaches, in hundredths of the player's height. */
    val spectrum: Boolean = true,
    val spectrumAmplitude: Int = 75,
)

/** One line of a song's words, sung at a known moment. */
@Serializable
data class LyricLine(val atMs: Long, val text: String)

/** The words of a song: as they are written, and in lines sung at known
 *  moments when they carry them, the moments already moved by hand. */
@Serializable
data class SongLyrics(
    val plain: String = "",
    /** Empty when the words carry no moments. */
    val lines: List<LyricLine> = emptyList(),
    val instrumental: Boolean = false,
)

/** How the sound of a song is spread: [bands] levels for each of the
 *  [framesASecond] readings of every second, from nought for nothing to 255
 *  for as loud as the song gets. */
class SongSpectrum(val bands: Int, val framesASecond: Double, val levels: ByteArray)

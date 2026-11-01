package app.melyxar.car

import android.content.ContentResolver
import android.content.Context
import android.net.Uri
import android.os.Bundle
import androidx.annotation.DrawableRes
import androidx.annotation.OptIn
import androidx.core.net.toUri
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaConstants
import androidx.media3.session.MediaLibraryService.LibraryParams
import app.melyxar.R
import app.melyxar.music.Listening
import app.melyxar.music.coverAddress
import app.melyxar.server.Album
import app.melyxar.server.Api
import app.melyxar.server.Artist
import app.melyxar.server.FoundMusic
import app.melyxar.server.Picture
import app.melyxar.server.PlaylistSummary
import app.melyxar.server.Slice
import app.melyxar.server.Song
import app.melyxar.server.album
import app.melyxar.server.albums
import app.melyxar.server.artist
import app.melyxar.server.artistSongs
import app.melyxar.server.artists
import app.melyxar.server.fitting
import app.melyxar.server.listened
import app.melyxar.server.musicFavourites
import app.melyxar.server.playlist
import app.melyxar.server.playlists
import app.melyxar.server.searchMusic

/** How wide a cover is fetched for the car's screen, in pixels. */
private const val COVER_ACROSS = 300

/*
 * Every place of the catalogue has a name the car hands back as it is: what
 * it is, then what it names. A list of songs is named by its kind and its
 * own name; a song by the list it was chosen in and its own, so it plays with
 * that list around it.
 */
private const val ROOT = "root"
private const val HOME = "home"
private const val ALBUMS = "albums"
private const val ARTISTS = "artists"
private const val PLAYLISTS = "playlists"
private const val ALBUM = "album:"
private const val ARTIST = "artist:"
private const val PLAYLIST = "playlist:"
private const val FAVOURITES = "favourites:"
private const val RECENT = "recent:"
private const val MOST = "most:"
private const val SEARCH = "search:"
private const val SHUFFLE_LIBRARY = "shuffle:"
private const val ALL = "all|"
private const val MIX = "mix|"
private const val SONG = "song|"

/**
 * The music as a car shows it, asked of the server the phone's page is
 * signed in on: four tabs, Home (each music library shuffled, its
 * favourites, what was listened to lately and most), Albums, Artists and
 * Playlists, with a folder more for each library when there are several;
 * and what a press on a song, a search or a voice plays. Everything played
 * goes to the phone's player, as if pressed in the page.
 *
 * A server that does not answer, or refuses, throws its IOException;
 * nobody signed in throws [NobodySignedIn].
 */
@OptIn(UnstableApi::class)
class CarCatalogue(private val context: Context, private val listening: Listening, private val asking: () -> Api?) {
    /** The way in. */
    fun root(): MediaItem = folder(ROOT, context.getString(R.string.app_name))

    /** How the car draws the whole catalogue, said with the way in: what
     *  opens drawn as a grid of covers, what plays as lines. */
    fun drawn(): LibraryParams = LibraryParams.Builder()
        .setExtras(
            Bundle().apply {
                putInt(MediaConstants.EXTRAS_KEY_CONTENT_STYLE_BROWSABLE, MediaConstants.EXTRAS_VALUE_CONTENT_STYLE_GRID_ITEM)
                putInt(MediaConstants.EXTRAS_KEY_CONTENT_STYLE_PLAYABLE, MediaConstants.EXTRAS_VALUE_CONTENT_STYLE_LIST_ITEM)
            },
        )
        .build()

    /** What [parent] holds, read from the server, a page of [pageSize] at a
     *  time; nothing for a name the catalogue never gave. */
    suspend fun children(parent: String, page: Int, pageSize: Int): List<MediaItem> {
        val api = signedIn()
        val stretch = Slice(page * pageSize, pageSize)
        val all = when {
            parent == ROOT -> listOf(
                folder(HOME, context.getString(R.string.car_home)),
                folder(ALBUMS, context.getString(R.string.tv_music_albums)),
                folder(ARTISTS, context.getString(R.string.tv_music_artists)),
                folder(PLAYLISTS, context.getString(R.string.tv_music_playlists)),
            )
            parent == HOME -> home(api)
            parent == ALBUMS || parent == ARTISTS -> {
                val libraries = musicLibraries(api)
                val alone = libraries.singleOrNull()?.first
                    ?: return libraries.map { (id, name) -> folder("$parent:$id", name, icon = R.drawable.car_music) }.drop(stretch.offset).take(pageSize)
                return if (parent == ALBUMS) albumsOf(api, alone, stretch) else artistsOf(api, alone, stretch)
            }
            parent == PLAYLISTS -> api.playlists().map(::playlistItem)
            parent.startsWith("$ALBUMS:") -> return albumsOf(api, parent.substringAfter(':'), stretch)
            parent.startsWith("$ARTISTS:") -> return artistsOf(api, parent.substringAfter(':'), stretch)
            parent.startsWith(ARTIST) -> artistPage(api, parent.removePrefix(ARTIST))
            else -> songList(api, parent, songsOf(api, parent))
        }
        return all.drop(stretch.offset).take(pageSize)
    }

    /** One place of the catalogue by its name, for a car coming back to it. */
    suspend fun item(id: String): MediaItem? {
        val api = signedIn()
        return when {
            id == ROOT -> root()
            id.startsWith(ALBUM) -> api.album(id.removePrefix(ALBUM)).let {
                folder(id, it.title, it.artists.joinToString(", ") { artist -> artist.name }, it.cover, MediaMetadata.MEDIA_TYPE_ALBUM)
            }
            id.startsWith(ARTIST) -> api.artist(id.removePrefix(ARTIST)).let {
                folder(id, it.name, cover = it.picture, type = MediaMetadata.MEDIA_TYPE_ARTIST, icon = R.drawable.car_artist)
            }
            id.startsWith(PLAYLIST) -> folder(id, api.playlist(id.removePrefix(PLAYLIST)).name, type = MediaMetadata.MEDIA_TYPE_PLAYLIST)
            else -> null
        }
    }

    /** What [words] find in all the music: songs first, each playing with
     *  the others found from there, then albums and artists. */
    suspend fun search(words: String): List<MediaItem> {
        val api = signedIn()
        val found = api.searchMusic(words)
        return found.songs.map { songItem(api, "$SEARCH$words", it) } + found.albums.map(::albumItem) + found.artists.map(::artistItem)
    }

    /**
     * Plays what a car or a voice asked for: a song with the list around it,
     * a list whole or shuffled, a library shuffled; or, said aloud, whatever
     * answers [spoken] best, and with nothing said, the queue kept or else
     * the first library shuffled. Answers the song that plays first, as the
     * system shows it, or nothing when nothing could be played.
     */
    suspend fun play(id: String, spoken: String?): MediaItem? {
        val api = signedIn()
        when {
            id.startsWith(SONG) -> {
                val container = id.removePrefix(SONG).substringBeforeLast('|')
                val chosen = id.substringAfterLast('|')
                val songs = songsOf(api, container)
                listening.play(songs, songs.indexOfFirst { it.id == chosen }.coerceAtLeast(0))
            }
            id.startsWith(ALL) -> listening.play(songsOf(api, id.removePrefix(ALL)))
            id.startsWith(MIX) -> {
                val songs = songsOf(api, id.removePrefix(MIX))
                listening.play(songs, songs.indices.randomOrNull() ?: 0, shuffle = true)
            }
            id.startsWith(SHUFFLE_LIBRARY) -> listening.playLibrary(id.removePrefix(SHUFFLE_LIBRARY), shuffle = true)
            id.startsWith(ALBUM) || id.startsWith(PLAYLIST) || id.startsWith(ARTIST) -> listening.play(songsOf(api, id))
            !spoken.isNullOrBlank() -> playFound(api, api.searchMusic(spoken))
            listening.queue.current != null -> listening.resume()
            else -> musicLibraries(api).firstOrNull()?.let { (library, _) -> listening.playLibrary(library, shuffle = true) }
        }
        return nowPlaying()
    }

    /** The song playing, as the system shows it, if any. */
    fun nowPlaying(): MediaItem? {
        val song = listening.queue.current ?: return null
        val metadata = MediaMetadata.Builder()
            .setTitle(song.title)
            .setArtist(song.artists.joinToString(", ") { it.name })
            .setAlbumTitle(song.album?.name)
            .setIsBrowsable(false)
            .setIsPlayable(true)
            .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
            .build()
        return MediaItem.Builder().setMediaId(song.id).setMediaMetadata(metadata).build()
    }

    private fun signedIn(): Api = asking() ?: throw NobodySignedIn()

    private suspend fun playFound(api: Api, found: FoundMusic) {
        when {
            found.songs.isNotEmpty() -> listening.play(found.songs)
            found.albums.isNotEmpty() -> listening.play(api.album(found.albums.first().id).tracks)
            found.artists.isNotEmpty() -> listening.play(api.artistSongs(found.artists.first().id))
        }
    }

    private suspend fun home(api: Api): List<MediaItem> {
        val libraries = musicLibraries(api)
        val alone = libraries.size == 1
        return libraries.flatMap { (id, name) ->
            fun named(what: Int) = if (alone) context.getString(what) else "${context.getString(what)} · $name"
            listOf(
                playable(
                    "$SHUFFLE_LIBRARY$id",
                    if (alone) context.getString(R.string.car_shuffle_everything) else context.getString(R.string.car_shuffle_library, name),
                    R.drawable.car_shuffle,
                ),
                folder("$FAVOURITES$id", named(R.string.tv_music_favourites), icon = R.drawable.car_favourites),
                folder("$RECENT$id", named(R.string.tv_music_listened_lately), icon = R.drawable.car_recent),
                folder("$MOST$id", named(R.string.tv_music_listened_most), icon = R.drawable.car_most),
            )
        }
    }

    private suspend fun albumsOf(api: Api, library: String, stretch: Slice): List<MediaItem> =
        api.albums(library, "title", false, stretch).items.map(::albumItem)

    private suspend fun artistsOf(api: Api, library: String, stretch: Slice): List<MediaItem> =
        api.artists(library, albumArtists = true, slice = stretch).items.map(::artistItem)

    private suspend fun artistPage(api: Api, id: String): List<MediaItem> {
        val page = api.artist(id)
        return listOf(
            playable("$ALL$ARTIST$id", context.getString(R.string.tv_music_play_all), R.drawable.car_play),
            playable("$MIX$ARTIST$id", context.getString(R.string.tv_music_shuffle), R.drawable.car_shuffle),
        ) + (page.theirAlbums + page.appearsOn).distinctBy { it.id }.map(::albumItem)
    }

    /** A list of songs, played whole or shuffled from its head. */
    private fun songList(api: Api, container: String, songs: List<Song>): List<MediaItem> = listOf(
        playable("$ALL$container", context.getString(R.string.tv_music_play_all), R.drawable.car_play),
        playable("$MIX$container", context.getString(R.string.tv_music_shuffle), R.drawable.car_shuffle),
    ) + songs.map { songItem(api, container, it) }

    /** The songs a list of the catalogue holds, in its order; none for a
     *  name it never gave. */
    private suspend fun songsOf(api: Api, container: String): List<Song> {
        val named = container.substringAfter(':')
        return when ("${container.substringBefore(':')}:") {
            ALBUM -> api.album(named).tracks
            PLAYLIST -> api.playlist(named).tracks
            ARTIST -> api.artistSongs(named)
            FAVOURITES -> api.musicFavourites(named).songs
            RECENT -> api.listened(named, "lately")
            MOST -> api.listened(named, "most")
            SEARCH -> api.searchMusic(named).songs
            else -> emptyList()
        }
    }

    private suspend fun musicLibraries(api: Api): List<Pair<String, String>> =
        api.libraries().filter { it.kind == "music" }.map { it.id to it.name }

    private fun albumItem(album: Album): MediaItem =
        folder("$ALBUM${album.id}", album.title, album.artists.joinToString(", ") { it.name }, album.cover, MediaMetadata.MEDIA_TYPE_ALBUM)

    private fun artistItem(artist: Artist): MediaItem =
        folder("$ARTIST${artist.id}", artist.name, cover = artist.picture, type = MediaMetadata.MEDIA_TYPE_ARTIST, icon = R.drawable.car_artist)

    private fun playlistItem(playlist: PlaylistSummary): MediaItem =
        folder("$PLAYLIST${playlist.id}", playlist.name, cover = playlist.cover, type = MediaMetadata.MEDIA_TYPE_PLAYLIST)

    private fun songItem(api: Api, container: String, song: Song): MediaItem {
        val metadata = MediaMetadata.Builder()
            .setTitle(song.title)
            .setArtist(song.artists.joinToString(", ") { it.name })
            .setAlbumTitle(song.album?.name)
            .setArtworkUri(coverOf(api, song.cover)?.toUri() ?: iconOf(R.drawable.car_music))
            .setIsBrowsable(false)
            .setIsPlayable(true)
            .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
            .build()
        return MediaItem.Builder().setMediaId("$SONG$container|${song.id}").setMediaMetadata(metadata).build()
    }

    private fun folder(
        id: String,
        title: String,
        subtitle: String? = null,
        cover: List<Picture> = emptyList(),
        type: Int = MediaMetadata.MEDIA_TYPE_FOLDER_MIXED,
        /** Shown when it has no cover: a car draws a warning sign over an
         *  empty tile. */
        @DrawableRes icon: Int = R.drawable.car_music,
    ): MediaItem {
        val metadata = MediaMetadata.Builder()
            .setTitle(title)
            .setSubtitle(subtitle)
            .setArtworkUri(asking()?.let { coverOf(it, cover) }?.toUri() ?: iconOf(icon))
            .setIsBrowsable(true)
            .setIsPlayable(false)
            .setMediaType(type)
            .build()
        return MediaItem.Builder().setMediaId(id).setMediaMetadata(metadata).build()
    }

    private fun playable(id: String, title: String, @DrawableRes icon: Int): MediaItem {
        val metadata = MediaMetadata.Builder()
            .setTitle(title)
            .setArtworkUri(iconOf(icon))
            .setIsBrowsable(false)
            .setIsPlayable(true)
            .setMediaType(MediaMetadata.MEDIA_TYPE_PLAYLIST)
            .build()
        return MediaItem.Builder().setMediaId(id).setMediaMetadata(metadata).build()
    }

    /** One of the application's own drawings, as a car reads it. */
    private fun iconOf(@DrawableRes icon: Int): Uri = Uri.Builder()
        .scheme(ContentResolver.SCHEME_ANDROID_RESOURCE)
        .authority(context.resources.getResourcePackageName(icon))
        .appendPath(context.resources.getResourceTypeName(icon))
        .appendPath(context.resources.getResourceEntryName(icon))
        .build()

    private fun coverOf(api: Api, pictures: List<Picture>): String? =
        pictures.fitting(COVER_ACROSS)?.url?.let { coverAddress(context, api.address(it)) }
}

/** No server opened in the application, or nobody signed in on it. */
class NobodySignedIn : Exception("nobody signed in")

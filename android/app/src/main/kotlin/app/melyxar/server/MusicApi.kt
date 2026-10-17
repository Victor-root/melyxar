package app.melyxar.server

import java.net.HttpURLConnection
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import okhttp3.HttpUrl
import okhttp3.Request

/*
 * The server's music, asked as the web's player asks it.
 */

/** How a long list of music is read, a stretch at a time. */
data class Slice(val offset: Int, val limit: Int)

private fun HttpUrl.Builder.slice(slice: Slice) {
    addQueryParameter("offset", slice.offset.toString())
    addQueryParameter("limit", slice.limit.toString())
}

/** The albums of a library in [order]: title, artist, year or added. */
suspend fun Api.albums(library: String, order: String, descending: Boolean, slice: Slice, genre: String? = null): MusicPage<Album> =
    get(
        asking("/api/v1/music/$library/albums") {
            addQueryParameter("order", order)
            if (descending) addQueryParameter("descending", "true")
            genre?.let { addQueryParameter("genre", it) }
            slice(slice)
        },
        MusicPage.serializer(Album.serializer()),
    )

/** The artists of a library, only those with an album of their own when
 *  [albumArtists] says so. */
suspend fun Api.artists(library: String, albumArtists: Boolean, slice: Slice): MusicPage<Artist> =
    get(
        asking("/api/v1/music/$library/artists") {
            if (albumArtists) addQueryParameter("album_artists", "true")
            slice(slice)
        },
        MusicPage.serializer(Artist.serializer()),
    )

/** The songs of a library in [order]: title, album or added. */
suspend fun Api.songs(library: String, order: String, descending: Boolean, slice: Slice): MusicPage<Song> =
    get(
        asking("/api/v1/music/$library/songs") {
            addQueryParameter("order", order)
            if (descending) addQueryParameter("descending", "true")
            slice(slice)
        },
        MusicPage.serializer(Song.serializer()),
    )

/** The songs a whole library is played or shuffled from, in album order. */
suspend fun Api.libraryQueue(library: String, offset: Int): MusicPage<Song> =
    get(asking("/api/v1/music/$library/queue") { addQueryParameter("offset", offset.toString()) }, MusicPage.serializer(Song.serializer()))

suspend fun Api.genres(library: String): List<Genre> = get("/api/v1/music/$library/genres", ListSerializer(Genre.serializer()))

/** What the account listened to in a library: lately or most. */
suspend fun Api.listened(library: String, order: String): List<Song> =
    get(asking("/api/v1/music/$library/listened") { addQueryParameter("order", order) }, ListSerializer(Song.serializer()))

suspend fun Api.musicFavourites(library: String): FoundMusic = get("/api/v1/music/$library/favourites", FoundMusic.serializer())

/** Every album, artist and song the account has hearted. */
suspend fun Api.musicHearts(): List<String> = get("/api/v1/music/favourites", ListSerializer(String.serializer()))

suspend fun Api.album(id: String): AlbumPage = get("/api/v1/music/albums/$id", AlbumPage.serializer())

suspend fun Api.artist(id: String): ArtistPage = get("/api/v1/music/artists/$id", ArtistPage.serializer())

suspend fun Api.artistSongs(id: String): List<Song> = get("/api/v1/music/artists/$id/songs", ListSerializer(Song.serializer()))

suspend fun Api.playlists(): List<PlaylistSummary> = get("/api/v1/music/playlists", ListSerializer(PlaylistSummary.serializer()))

/** Puts [songs] at the end of the playlist [id]. */
suspend fun Api.addToPlaylist(id: String, songs: List<String>) {
    send("POST", "/api/v1/music/playlists/$id/songs", buildJsonObject { put("songs", JsonArray(songs.map(::JsonPrimitive))) })
}

suspend fun Api.playlist(id: String): Playlist = get("/api/v1/music/playlists/$id", Playlist.serializer())

/** What [words] name in the music, in one [library] or in every one. */
suspend fun Api.searchMusic(words: String, library: String? = null): FoundMusic =
    get(
        asking("/api/v1/music/search") {
            addQueryParameter("words", words)
            library?.let { addQueryParameter("library", it) }
        },
        FoundMusic.serializer(),
    )

suspend fun Api.musicPreferences(): MusicPreferences = get("/api/v1/music/preferences", MusicPreferences.serializer())

/** Changes some of how the account listens, named as the server names them:
 *  the server takes them all at once, so what is not changed is sent back as
 *  it was read. Gives back all of them as it kept them. */
suspend fun Api.changeMusicPreferences(changes: JsonObject): MusicPreferences {
    val now = get("/api/v1/music/preferences", JsonObject.serializer())
    val kept = send("PUT", "/api/v1/music/preferences", JsonObject(now + changes)) ?: now
    return serverJson.decodeFromJsonElement(MusicPreferences.serializer(), kept)
}

/** The words of a song, nothing when none were found. */
suspend fun Api.lyrics(song: String): SongLyrics? = get("/api/v1/music/songs/$song/lyrics", SongLyrics.serializer().nullable)

/** How the sound of a song is spread, nothing for a song not read for it
 *  yet or an answer that does not hold together. */
suspend fun Api.spectrum(song: String): SongSpectrum? = withContext(Dispatchers.IO) {
    val asked = Request.Builder().url(address("/api/v1/music/songs/$song/spectrum")).header("Accept", "application/octet-stream").build()
    http.newCall(asked).execute().use { answer ->
        if (answer.code == HttpURLConnection.HTTP_NO_CONTENT) {
            return@use null
        }
        if (!answer.isSuccessful) {
            throw Refusal(answer.code, null)
        }
        val bands = answer.header("x-spectrum-bands")?.toIntOrNull() ?: 0
        val framesASecond = answer.header("x-spectrum-frames-a-second")?.toDoubleOrNull() ?: 0.0
        val levels = answer.body.bytes()
        SongSpectrum(bands, framesASecond, levels).takeIf {
            bands >= 2 && framesASecond > 0 && levels.size >= bands && levels.size % bands == 0
        }
    }
}

/** Says a song was listened to, once per time it is. */
suspend fun Api.listenedTo(song: String) {
    send("POST", "/api/v1/music/songs/$song/listened", JsonObject(emptyMap()))
}

/**
 * Where a song is heard: the file itself when the device plays its form,
 * otherwise made into one it does, from [start] seconds.
 */
fun Api.soundOf(song: String, plays: String, start: Double? = null): String =
    asking("/api/v1/music/songs/$song/sound") {
        addQueryParameter("plays", plays)
        start?.takeIf { it > 0 }?.let { addQueryParameter("start", "%.3f".format(java.util.Locale.ROOT, it)) }
    }

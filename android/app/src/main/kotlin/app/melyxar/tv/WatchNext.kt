package app.melyxar.tv

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.os.Build
import androidx.core.net.toUri
import androidx.tvprovider.media.tv.PreviewChannelHelper
import androidx.tvprovider.media.tv.TvContractCompat
import androidx.tvprovider.media.tv.WatchNextProgram
import app.melyxar.connect.ConnectActivity
import app.melyxar.server.Api
import app.melyxar.server.Card
import app.melyxar.server.EpisodeMore
import app.melyxar.server.WithCard
import app.melyxar.server.fitting
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException

/** How wide the pictures handed to the system are, in pixels: a card of its
 *  row, with room to spare. */
private const val PICTURE_ACROSS = 640

/**
 * The works the account has started, in the system's own Continue watching
 * row as in Melyxar's, each opening its page. Its pictures are kept here and
 * handed over by [WatchNextPictures], the system having no session to read
 * the server's.
 */
// The program's getters and setters are public, inherited from base classes
// the library keeps to itself, which lint takes for the library's own use.
@SuppressLint("RestrictedApi")
class WatchNext(private val context: Context) {
    private val pictures = File(context.filesDir, PICTURES)

    /** Whether a picture could not be fetched, the row then placed again
     *  the next time even when nothing else changed. */
    private var missed = false

    /** Puts [rows] in the system's row, in their order, and takes out what
     *  is no longer among them. What the viewer took out of the row is left
     *  out. Nothing on a television without the row, and nothing again for
     *  the rows it was last given whole: placing them reads and writes the
     *  system's store of programs, in another process, for every row. */
    suspend fun show(api: Api, rows: List<WithCard<EpisodeMore>>) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return
        }
        withContext(Dispatchers.IO) {
            if (rows == lastPlaced) {
                return@withContext
            }
            try {
                place(api, rows)
                lastPlaced = if (missed) null else rows
            } catch (_: IllegalArgumentException) {
                // A television whose system keeps no such row, Fire TV's.
            } catch (_: SecurityException) {
                // Nor one that lets applications write in it.
            }
        }
    }

    private suspend fun place(api: Api, rows: List<WithCard<EpisodeMore>>) {
        val helper = PreviewChannelHelper(context)
        val placed = ours().associateBy { it.internalProviderId }
        val now = System.currentTimeMillis()
        val kept = HashSet<String>()
        rows.forEachIndexed { at, row ->
            val card = row.card
            val before = placed[card.id]
            if (before != null && !before.isBrowsable) {
                return@forEachIndexed
            }
            val picture = pictureOf(api, card) ?: return@forEachIndexed
            kept += picture.name
            val position = (card.resumeFromSeconds ?: 0) * 1000
            // Moved up the row only when watched further, so opening
            // Melyxar does not put all of it before what was watched since.
            val engaged = before?.takeIf { it.lastPlaybackPositionMillis.toLong() == position }?.lastEngagementTimeUtcMillis
                ?: (now - at)
            val program = programOf(row, picture, position, engaged)
            if (before == null) helper.publishWatchNextProgram(program) else helper.updateWatchNextProgram(program, before.id)
        }
        val shown = rows.map { it.card.id }.toSet()
        placed.values.filter { it.internalProviderId !in shown }.forEach {
            context.contentResolver.delete(TvContractCompat.buildWatchNextProgramUri(it.id), null, null)
        }
        pictures.listFiles()?.filter { it.name !in kept }?.forEach(File::delete)
    }

    /** What this application has in the row now. */
    private fun ours(): List<WatchNextProgram> =
        context.contentResolver.query(TvContractCompat.WatchNextPrograms.CONTENT_URI, WatchNextProgram.PROJECTION, null, null, null)
            ?.use { it.programs() }
            .orEmpty()

    private fun Cursor.programs(): List<WatchNextProgram> = buildList {
        while (moveToNext()) {
            add(WatchNextProgram.fromCursor(this@programs))
        }
    }

    /** The card's picture, kept under a name that changes with it, fetched
     *  only when it is not kept already. */
    private suspend fun pictureOf(api: Api, card: Card): File? {
        val chosen = card.wide.ifEmpty { card.poster }.fitting(PICTURE_ACROSS) ?: return null
        val file = File(pictures, "${card.id}-${Integer.toHexString(chosen.url.hashCode())}.jpg")
        if (!file.exists()) {
            try {
                pictures.mkdirs()
                file.writeBytes(api.file(chosen.url))
            } catch (_: IOException) {
                file.delete()
                missed = true
                return null
            }
        }
        return file
    }

    private fun programOf(row: WithCard<EpisodeMore>, picture: File, position: Long, engaged: Long): WatchNextProgram {
        val card = row.card
        val lying = card.wide.isNotEmpty()
        val opening = Intent(context, ConnectActivity::class.java).putExtra(TvActivity.OPENING, card.id)
        return WatchNextProgram.Builder()
            .setInternalProviderId(card.id)
            .setWatchNextType(TvContractCompat.WatchNextPrograms.WATCH_NEXT_TYPE_CONTINUE)
            .setLastEngagementTimeUtcMillis(engaged)
            .setLastPlaybackPositionMillis(position.toInt())
            .apply {
                lengthOfAPlay(card)?.let { setDurationMillis((it * 1000).toInt()) }
                val series = row.more.seriesTitle
                if (card.kind == "episode" && series != null) {
                    setType(TvContractCompat.PreviewPrograms.TYPE_TV_EPISODE)
                    setTitle(series)
                    setEpisodeTitle(card.title)
                    row.more.seasonNumber?.let { setSeasonNumber(it) }
                    row.more.episodeNumber?.let { setEpisodeNumber(it) }
                } else {
                    setType(TvContractCompat.PreviewPrograms.TYPE_MOVIE)
                    setTitle(card.title)
                }
            }
            .setPosterArtUri("content://${context.packageName}$AUTHORITY_SUFFIX/${picture.name}".toUri())
            .setPosterArtAspectRatio(
                if (lying) TvContractCompat.PreviewPrograms.ASPECT_RATIO_16_9 else TvContractCompat.PreviewPrograms.ASPECT_RATIO_MOVIE_POSTER,
            )
            .setIntent(opening)
            .build()
    }

    companion object {
        /** The rows last placed whole while the application runs. */
        @Volatile
        private var lastPlaced: List<WithCard<EpisodeMore>>? = null

        /** Where the pictures handed to the system are kept. */
        const val PICTURES = "watch-next"

        /** What follows the application's name in the address of those
         *  pictures, as the manifest declares it. */
        const val AUTHORITY_SUFFIX = ".watchnext"
    }
}

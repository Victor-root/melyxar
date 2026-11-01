package app.melyxar.music

import android.content.Context
import androidx.core.content.edit
import app.melyxar.server.serverJson
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerializationException
import java.io.File
import java.io.IOException

/**
 * The phone's music kept from one run of the application to the next: the
 * queue's songs in a file of their own, there being up to two thousand of
 * them, and where it had got to, the loudness and the account it is
 * listened to by among the application's small settings.
 */
class KeptOnThePhone(context: Context) : KeptMusic {
    private val small = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE)
    private val songs = File(context.filesDir, QUEUE_FILE)

    override suspend fun queue(): KeptQueue? = withContext(Dispatchers.IO) {
        val written = try {
            songs.takeIf(File::exists)?.readText()
        } catch (_: IOException) {
            null
        } ?: return@withContext null
        val kept = try {
            serverJson.decodeFromString(KeptQueue.serializer(), written)
        } catch (_: SerializationException) {
            return@withContext null
        }
        kept.copy(at = small.getInt(AT, 0), position = small.getFloat(POSITION, 0f).toDouble())
    }

    override suspend fun keepQueue(queue: Queue?) {
        withContext(Dispatchers.IO) { write(queue) }
    }

    private fun write(queue: Queue?) {
        if (queue == null) {
            songs.delete()
            small.edit { remove(AT).remove(POSITION) }
            return
        }
        val written = serverJson.encodeToString(KeptQueue.serializer(), KeptQueue(queue.songs, queue.order, queue.shuffle, queue.repeat))
        try {
            // Written whole beside it, then put in its place: a run stopped
            // halfway never leaves half a queue.
            val beside = File(songs.parentFile, "$QUEUE_FILE.new")
            beside.writeText(written)
            beside.renameTo(songs)
        } catch (_: IOException) {
            // Not kept this time: the next change of the queue tries again.
        }
    }

    override fun keepPlace(at: Int, position: Double) =
        small.edit { putInt(AT, at).putFloat(POSITION, position.toFloat()) }

    override fun loudness(): Loudness = Loudness(small.getFloat(VOLUME, 1f), small.getBoolean(MUTED, false))

    override fun keepLoudness(loudness: Loudness) =
        small.edit { putFloat(VOLUME, loudness.volume).putBoolean(MUTED, loudness.muted) }

    /** The account the queue kept is listened to by, once a page said so. */
    fun heldBy(): String? = small.getString(HELD_BY, null)

    fun keepHeldBy(account: String) = small.edit { putString(HELD_BY, account) }

    private companion object {
        const val KEPT = "music"
        const val QUEUE_FILE = "music-queue.json"
        const val AT = "at"
        const val POSITION = "position"
        const val VOLUME = "volume"
        const val MUTED = "muted"
        const val HELD_BY = "held_by"
    }
}

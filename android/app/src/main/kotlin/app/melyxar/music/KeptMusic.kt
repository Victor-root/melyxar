package app.melyxar.music

import app.melyxar.server.Song
import kotlinx.serialization.Serializable

/** A queue as it is kept for the next time: its songs, its order, where in
 *  it the song playing was and how far in, and the ways it played. */
@Serializable
data class KeptQueue(
    val songs: List<Song>,
    val order: List<Int>,
    val shuffle: Boolean,
    val repeat: Repeat,
    val at: Int = 0,
    val position: Double = 0.0,
)

/** How loud the player plays, under the device's own volume. */
@Serializable
data class Loudness(val volume: Float = 1f, val muted: Boolean = false)

/**
 * Where a player keeps its queue and its loudness from one run of the
 * application to the next, as a browser keeps the page's. The queue's songs
 * are written only when they change; where it has got to, often.
 */
interface KeptMusic {
    /** Read away from the thread that draws, the songs being many. */
    suspend fun queue(): KeptQueue?

    /** Written away from that thread too; nothing forgets it. */
    suspend fun keepQueue(queue: Queue?)

    fun keepPlace(at: Int, position: Double)

    fun loudness(): Loudness

    fun keepLoudness(loudness: Loudness)
}

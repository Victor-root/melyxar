package app.melyxar.tv.music

import app.melyxar.server.Song
import kotlin.random.Random

/*
 * The queue of songs: what is playing, what comes after it, and what shuffle
 * and repeat make of that, by the web player's own rules.
 *
 * Pure, so every rule is tested without a sound being played. The songs are
 * kept in the order they were queued; shuffling draws another order over
 * them, and turning shuffle off goes back to the first one from the song that
 * is playing.
 */

enum class Repeat { Off, All, One }

data class Queue(
    /** In the order they were queued. */
    val songs: List<Song> = emptyList(),
    /** The order they are played in: places in [songs]. */
    val order: List<Int> = emptyList(),
    /** Where in [order] the song playing is. */
    val at: Int = 0,
    val shuffle: Boolean = false,
    val repeat: Repeat = Repeat.Off,
) {
    /** The song playing, if any. */
    val current: Song? get() = order.getOrNull(at)?.let(songs::getOrNull)

    /** The songs still to come, in the order they will play. */
    val upNext: List<Song> get() = order.drop(at + 1).map(songs::get)

    /** Whether there is a song to ask for on: one to come, or one to come
     *  back to under repeat. A lone song has none, repeat or not. */
    val canStepOn: Boolean get() = at + 1 < order.size || (repeat != Repeat.Off && order.size > 1)
}

/** How far into a song "previous" still means "this one from the start". */
const val BACK_TO_THE_START_SECONDS = 3.0

/** A new queue playing [songs] from the one at [first]. Shuffled, that one
 *  plays first and the rest follow in a drawn order. */
fun playing(songs: List<Song>, first: Int, shuffle: Boolean, repeat: Repeat, draw: Random = Random): Queue {
    val places = songs.indices.toList()
    val start = first.coerceIn(0, (songs.size - 1).coerceAtLeast(0))
    if (!shuffle) {
        return Queue(songs, places, start, shuffle, repeat)
    }
    return Queue(songs, listOf(start) + places.filter { it != start }.shuffled(draw), 0, shuffle, repeat)
}

/** Where the queue goes when a song ends on its own: the same one again
 *  under repeat one, otherwise as on a press of next. */
fun Queue.afterTheEnd(): Queue? = if (repeat == Repeat.One) this else forward()

/** Where the queue goes when somebody asks for the next song: always on,
 *  even under repeat one, which is a way to stop repeating that song. */
fun Queue.forward(): Queue? = when {
    at + 1 < order.size -> copy(at = at + 1)
    repeat != Repeat.Off && order.isNotEmpty() -> copy(at = 0)
    else -> null
}

/** Where the queue goes when somebody asks for the previous song: a few
 *  seconds in, the same song from its start; before that, the one before.
 *  [restart] says the song playing starts again. */
fun Queue.backward(secondsIn: Double): Backward = when {
    secondsIn > BACK_TO_THE_START_SECONDS -> Backward(this, restart = true)
    at > 0 -> Backward(copy(at = at - 1), restart = false)
    repeat == Repeat.All && order.size > 1 -> Backward(copy(at = order.size - 1), restart = false)
    else -> Backward(this, restart = true)
}

data class Backward(val queue: Queue, val restart: Boolean)

/** Shuffle turned on or off, the song playing staying where it is. */
fun Queue.withShuffle(on: Boolean, draw: Random = Random): Queue {
    val playingNow = order.getOrNull(at)
    if (playingNow == null || on == shuffle) {
        return copy(shuffle = on)
    }
    if (on) {
        return copy(order = listOf(playingNow) + songs.indices.filter { it != playingNow }.shuffled(draw), at = 0, shuffle = true)
    }
    return copy(order = songs.indices.toList(), at = playingNow, shuffle = false)
}

/** The next way round of repeat: off, the whole queue, the one song. */
fun Repeat.next(): Repeat = when (this) {
    Repeat.Off -> Repeat.All
    Repeat.All -> Repeat.One
    Repeat.One -> Repeat.Off
}

/** How many songs a queue holds at most, the same as the server's. */
private const val LONGEST_QUEUE = 2000

/** Where a shuffled queue starts in a library of [total] songs, given a
 *  number from nought up to but not including one: in a larger library
 *  than a queue holds, a stretch starting anywhere, so a shuffle does not
 *  draw from the same first songs every time. */
fun shuffleOffset(total: Int, random: Double): Int {
    val spare = total - LONGEST_QUEUE
    return if (spare > 0) (random * (spare + 1)).toInt() else 0
}

/** A jump to one of the songs of the queue, by its place in the order. */
fun Queue.jumpTo(place: Int): Queue = if (place in order.indices) copy(at = place) else this

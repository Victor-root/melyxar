package app.melyxar.tv.music

import app.melyxar.server.Song
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import kotlin.random.Random

class QueueTest {
    private val songs = listOf("Dawn", "Harbour", "Lanterns", "Tide").map { Song(id = it.lowercase(), title = it) }

    @Test
    fun playsFromTheSongChosen() {
        val queue = playing(songs, 2, shuffle = false, repeat = Repeat.Off)
        assertEquals("Lanterns", queue.current?.title)
        assertEquals(listOf("Tide"), queue.upNext.map { it.title })
    }

    @Test
    fun shuffledThePickedSongStillPlaysFirst() {
        val queue = playing(songs, 1, shuffle = true, repeat = Repeat.Off, draw = Random(7))
        assertEquals("Harbour", queue.current?.title)
        assertEquals(songs.size, queue.order.toSet().size)
    }

    @Test
    fun theEndStopsUnlessRepeated() {
        val last = playing(songs, 3, shuffle = false, repeat = Repeat.Off)
        assertNull(last.afterTheEnd())
        assertEquals("Dawn", last.copy(repeat = Repeat.All).afterTheEnd()?.current?.title)
        assertEquals("Tide", last.copy(repeat = Repeat.One).afterTheEnd()?.current?.title)
        assertEquals("Dawn", last.copy(repeat = Repeat.One).forward()?.current?.title)
    }

    @Test
    fun previousRestartsASongWellUnderWay() {
        val queue = playing(songs, 2, shuffle = false, repeat = Repeat.Off)
        assertTrue(queue.backward(10.0).restart)
        assertEquals("Harbour", queue.backward(1.0).queue.current?.title)
        val first = playing(songs, 0, shuffle = false, repeat = Repeat.All)
        assertEquals("Tide", first.backward(1.0).queue.current?.title)
    }

    @Test
    fun shuffleOffComesBackToTheQueuedOrder() {
        val shuffled = playing(songs, 0, shuffle = false, repeat = Repeat.Off).withShuffle(true, Random(3)).jumpTo(2)
        val back = shuffled.withShuffle(false)
        assertEquals(shuffled.current, back.current)
        assertEquals(songs.indices.toList(), back.order)
    }

    @Test
    fun aLoneSongHasNothingToStepOnTo() {
        assertFalse(playing(songs.take(1), 0, shuffle = false, repeat = Repeat.All).canStepOn)
        assertTrue(playing(songs, 3, shuffle = false, repeat = Repeat.All).canStepOn)
    }

    @Test
    fun aLargeLibraryIsShuffledFromAnywhere() {
        assertEquals(0, shuffleOffset(1500, 0.9))
        assertEquals(500, shuffleOffset(2500, 0.999))
        assertEquals(0, shuffleOffset(2500, 0.0))
    }

    @Test
    fun repeatGoesRound() {
        assertEquals(Repeat.All, Repeat.Off.next())
        assertEquals(Repeat.One, Repeat.All.next())
        assertEquals(Repeat.Off, Repeat.One.next())
    }

    private val more = listOf("Echo", "Fjord").map { Song(id = it.lowercase(), title = it) }

    @Test
    fun songsPlayedNextComeRightAfterTheOnePlaying() {
        val queue = playing(songs, 1, shuffle = false, repeat = Repeat.Off).withNext(more)
        assertEquals("Harbour", queue.current?.title)
        assertEquals(listOf("Echo", "Fjord", "Lanterns", "Tide"), queue.upNext.map { it.title })
    }

    @Test
    fun songsAddedToTheQueueComeAtItsEnd() {
        val queue = playing(songs, 1, shuffle = false, repeat = Repeat.Off).withLast(more)
        assertEquals(listOf("Lanterns", "Tide", "Echo", "Fjord"), queue.upNext.map { it.title })
    }

    @Test
    fun songsAddedToNothingPlayFromTheFirst() {
        assertEquals("Echo", Queue().withNext(more).current?.title)
        assertEquals(listOf("Fjord"), Queue().withLast(more).upNext.map { it.title })
    }
}

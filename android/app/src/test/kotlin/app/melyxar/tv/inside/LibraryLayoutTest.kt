package app.melyxar.tv.inside

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class LibraryLayoutTest {
    @Test
    fun aLayoutIsReadFromTheWordTheServerKeeps() {
        LibraryLayout.entries.forEach { assertEquals(it, LibraryLayout.of(it.word)) }
        assertEquals(LibraryLayout.Posters, LibraryLayout.of("mosaic"))
        assertEquals(LibraryLayout.Posters, LibraryLayout.of(null))
    }

    @Test
    fun theShapeOfALibraryHoldsWhereTheLayoutSetsNone() {
        assertTrue(LibraryLayout.Posters.lies(lying = true))
        assertTrue(LibraryLayout.Lines.lies(lying = true))
        assertFalse(LibraryLayout.Lines.lies(lying = false))
        assertTrue(LibraryLayout.Thumbnails.lies(lying = false))
        assertFalse(LibraryLayout.SmallPosters.lies(lying = true))
    }

    @Test
    fun smallerPostersAndTheLinesOfAListAreSmallerThanACard() {
        val card = frameOf(lies = false)
        assertTrue(LibraryLayout.SmallPosters.frame(lying = false).width < card.width)
        assertTrue(LibraryLayout.Lines.frame(lying = false).width < card.width)
        assertEquals(card, LibraryLayout.Posters.frame(lying = false))
    }
}

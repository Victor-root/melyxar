package app.melyxar.tv.inside

import app.melyxar.server.Album
import app.melyxar.server.Picture
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class BandTest {
    private fun album(title: String, covered: Boolean) =
        Album(id = title, title = title, cover = if (covered) listOf(Picture("/$title.jpg")) else emptyList())

    @Test
    fun theCoversComeFirstAndFillAFan() {
        val albums = listOf(album("Dawn", false), album("Tide", true), album("Harbour", true)) +
            (1..6).map { album("Lantern $it", true) }
        val fan = coversOf(albums)
        assertEquals(5, fan.size)
        assertEquals(listOf("Tide", "Harbour", "Lantern 1"), fan.take(3).map { it.title })
    }
}

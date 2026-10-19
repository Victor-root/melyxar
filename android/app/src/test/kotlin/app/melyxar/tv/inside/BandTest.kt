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

    @Test
    fun theFloorFadesUpToTheFeetAndStopsThereInTheLinesOfTheWater() {
        assertEquals(0f, waterAt(36f, 100f), "nothing above where it fades out")
        assertEquals(0f, waterAt(69f, 100f), "cut dead at the line the fan stands on")
        assertEquals(0.74f * 2 / 32, waterAt(39f, 100f), 1e-6f)
        assertEquals(0.74f * 23 / 32, waterAt(60f, 100f), 1e-6f)
        assertEquals(0.74f * 4 / 32 * 0.62f * 0.74f, waterAt(41f, 100f), 1e-6f, "a row on two of the lines")
    }
}

package app.melyxar.server

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class ViewsTest {
    private val poster = listOf(Picture("/p-800", 800), Picture("/p-200", 200), Picture("/p-400", 400))

    @Test
    fun takesTheSmallestSizeThatIsLargeEnough() {
        assertEquals("/p-400", poster.fitting(264)?.url)
        assertEquals("/p-200", poster.fitting(200)?.url)
        assertEquals("/p-800", poster.fitting(1200)?.url)
        assertNull(emptyList<Picture>().fitting(100))
    }

    @Test
    fun readsACardAndWhatItsRowAddsFromTheSameObject() {
        val home = serverJson.decodeFromString(
            Home.serializer(),
            """{"hero":[{"id":"w1","title":"Les Marées de Verre","kind":"movie","seen":"in_progress",
                "resume_from_seconds":2892,"backdrop":[{"url":"/b","width":1920}],"hdr":"dolby_vision",
                "because":"started","something_new":true}],
              "carry_on":[{"id":"e4","title":"Nuit Blanche","kind":"episode","position_seconds":10.5,
                "series_title":"Brigade du Givre","season_number":1,"episode_number":7}],
              "sections":["band","carry_on"]}""",
        )
        assertEquals("Les Marées de Verre", home.hero.single().card.title)
        assertEquals(2892L, home.hero.single().card.resumeFromSeconds)
        assertEquals("dolby_vision", home.hero.single().more.hdr)
        assertEquals("Brigade du Givre", home.carryOn.single().more.seriesTitle)
        assertEquals(listOf("band", "carry_on"), home.sections)
    }
}

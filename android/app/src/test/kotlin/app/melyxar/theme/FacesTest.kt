package app.melyxar.theme

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class FacesTest {
    @Test
    fun picksThePictureTheWebPicks() {
        // The names and answers of web/src/faces.test.ts.
        assertEquals(
            listOf(
                FacePicture.Comet,
                FacePicture.Eclipse,
                FacePicture.Orbits,
                FacePicture.Crystal,
                FacePicture.Comet,
                FacePicture.Rings,
                FacePicture.Comet,
            ),
            listOf("Victor", "Élodie", "Zoë", "Marie-Hélène", "Bob", "", "🎬 Cinéma").map(::facePictureOf),
        )
    }
}

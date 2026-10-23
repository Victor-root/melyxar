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
                FacePicture.Orbits,
                FacePicture.Comet,
                FacePicture.Comet,
                FacePicture.Rings,
                FacePicture.Orbits,
                FacePicture.Crescent,
            ),
            listOf("Victor", "Élodie", "Zoë", "Marie-Hélène", "Bob", "", "🎬 Cinéma").map(::facePictureOf),
        )
    }
}

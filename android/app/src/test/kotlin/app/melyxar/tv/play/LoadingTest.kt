package app.melyxar.tv.play

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class LoadingTest {
    @Test
    fun theNumberFollowsTheStageAndWhatIsWritten() {
        assertEquals(3f, percentOf(LoadingStage.SessionOpened, 0f))
        assertEquals(8f + 77f * 0.5f, percentOf(LoadingStage.Producing, 0.5f), 0.001f)
        assertEquals(85f, percentOf(LoadingStage.Produced, 1f))
        assertEquals(100f, percentOf(LoadingStage.Done, 0.2f))
    }

    @Test
    fun theNumberShownGlidesAndNeverGoesBack() {
        assertEquals(30f, glideToward(0f, 100f), 0.001f)
        assertEquals(50f, glideToward(50f, 20f))
        assertEquals(10f, glideToward(9.9f, 10f))
    }
}

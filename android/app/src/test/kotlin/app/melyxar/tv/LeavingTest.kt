package app.melyxar.tv

import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class LeavingTest {
    @Test
    fun onlyASecondPressSoonAfterLeaves() {
        assertFalse(leavesNow(null, 10_000))
        assertTrue(leavesNow(10_000, 11_500))
        assertFalse(leavesNow(10_000, 12_500))
    }
}

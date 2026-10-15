package app.melyxar.server

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class AddressTest {
    private fun usable(typed: String) = (readAddress(typed) as Reading.Usable).origin.value

    @Test
    fun `a bare address of the home network is taken as http`() {
        assertEquals("http://192.168.1.20:2100", usable("192.168.1.20:2100"))
        assertEquals("http://nas:2100", usable(" nas:2100/ "))
        assertEquals("http://melyxar.local", usable("Melyxar.local"))
    }

    @Test
    fun `https is usable anywhere and keeps its port`() {
        assertEquals("https://films.example.org", usable("https://films.example.org/"))
        assertEquals("https://films.example.org:8443", usable("HTTPS://Films.Example.org:8443"))
    }

    @Test
    fun `plain http outside the home is refused`() {
        assertEquals(Reading.OpenOverInternet, readAddress("http://films.example.org"))
        assertEquals(Reading.OpenOverInternet, readAddress("films.example.org:2100"))
        assertEquals(Reading.OpenOverInternet, readAddress("http://8.8.8.8:2100"))
    }

    @Test
    fun `what is not an address is said so`() {
        for (typed in listOf("", "   ", "ftp://nas", "http://", "nas 2100", "https://films.example.org/melyxar")) {
            assertEquals(Reading.NotAnAddress, readAddress(typed), typed)
        }
    }

    @Test
    fun `the home network is told apart from the internet`() {
        for (host in listOf(
            "localhost", "nas", "box.lan", "server.home.arpa", "10.0.0.4", "127.0.0.1",
            "172.16.0.1", "172.31.255.255", "192.168.0.10", "169.254.3.3", "100.64.1.1",
            "[::1]", "[fd12:3456::1]", "[fe80::1]",
        )) {
            assertTrue(isAtHome(host), host)
        }
        for (host in listOf(
            "example.org", "8.8.8.8", "172.32.0.1", "192.169.0.1", "100.128.0.1",
            "[2001:db8::1]", "300.1.1.1",
        )) {
            assertFalse(isAtHome(host), host)
        }
    }
}

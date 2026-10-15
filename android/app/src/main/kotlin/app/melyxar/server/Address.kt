package app.melyxar.server

import java.net.URI

/**
 * Where a server answers, as typed by someone and brought to one form: a
 * scheme, a host and a port when there is one, nothing after them.
 */
@JvmInline
value class ServerOrigin(val value: String) {
    override fun toString() = value
}

/** What an address typed for a server is worth. */
sealed interface Reading {
    /** Usable as it is. */
    data class Usable(val origin: ServerOrigin) : Reading

    /** Plain http to a machine outside the home: a password would cross the
     *  internet readable by anyone on the way. */
    data object OpenOverInternet : Reading

    /** Not an address at all. */
    data object NotAnAddress : Reading
}

/**
 * Reads an address typed for a server.
 *
 * A bare host is taken as http, which is what a server on the home network
 * answers by default. Plain http is accepted only to a machine of the home
 * network; anywhere else https is asked for.
 */
fun readAddress(typed: String): Reading {
    val trimmed = typed.trim()
    if (trimmed.isEmpty() || trimmed.any { it.isWhitespace() }) {
        return Reading.NotAnAddress
    }
    val written = if ("://" in trimmed) trimmed else "http://$trimmed"
    val uri = try {
        URI(written.trimEnd('/'))
    } catch (_: java.net.URISyntaxException) {
        return Reading.NotAnAddress
    }
    val scheme = uri.scheme?.lowercase()
    val host = uri.host?.lowercase()
    if (scheme !in setOf("http", "https") || host.isNullOrEmpty()) {
        return Reading.NotAnAddress
    }
    if (!uri.rawPath.isNullOrEmpty()) {
        return Reading.NotAnAddress
    }
    if (scheme == "http" && !isAtHome(host)) {
        return Reading.OpenOverInternet
    }
    val port = if (uri.port == -1) "" else ":${uri.port}"
    return Reading.Usable(ServerOrigin("$scheme://$host$port"))
}

/**
 * Whether a host is a machine of the home network: a name without a dot, a
 * name the home network gives, or an address no router sends to the
 * internet.
 */
fun isAtHome(host: String): Boolean {
    val name = host.removePrefix("[").removeSuffix("]").lowercase()
    if (name == "localhost" || '.' !in name && ':' !in name) {
        return true
    }
    if (HOME_SUFFIXES.any { name.endsWith(it) }) {
        return true
    }
    ipv4(name)?.let { parts ->
        val (a, b) = parts
        return a == 10 ||
            a == 127 ||
            (a == 172 && b in 16..31) ||
            (a == 192 && b == 168) ||
            (a == 169 && b == 254) ||
            (a == 100 && b in 64..127)
    }
    if (':' in name) {
        val first = name.substringBefore(':').ifEmpty { "0" }.toIntOrNull(16) ?: return false
        return name == "::1" ||
            first and 0xfe00 == 0xfc00 ||
            first and 0xffc0 == 0xfe80
    }
    return false
}

/** The names a home network gives its machines. */
private val HOME_SUFFIXES = listOf(".local", ".lan", ".home.arpa", ".internal")

/** The four numbers of an IPv4 address, or nothing when it is not one. */
private fun ipv4(name: String): List<Int>? {
    val parts = name.split('.')
    if (parts.size != 4) {
        return null
    }
    val numbers = parts.map { part -> part.toIntOrNull()?.takeIf { it in 0..255 } ?: return null }
    return numbers
}

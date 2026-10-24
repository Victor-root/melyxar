package app.melyxar.tv.play

import java.util.Locale

/**
 * One rung of what the picture can be held to, as the web's ladder: a height
 * and a rate together, the film as it is first. The choice travels to the
 * server inside the profile; a film already below it is handed over as it is.
 */
data class Quality(
    /** Its name in storage, never on screen. */
    val key: String,
    /** Tallest picture accepted; nothing for the film as it is. */
    val height: Int?,
    /** Rate accepted, in bits per second; nothing for the film as it is. */
    val bitrate: Long?,
)

/** The rungs, best first, the same as the web's. */
val QUALITIES = listOf(
    Quality("auto", null, null),
    // Not a rung either: the rung chosen for the connection, and changed
    // with it while the film plays (see Adaptive.kt).
    Quality("adaptive", null, null),
    Quality("1080p-20", 1080, 20_000_000),
    Quality("1080p-10", 1080, 10_000_000),
    Quality("1080p-6", 1080, 6_000_000),
    Quality("720p-4", 720, 4_000_000),
    Quality("720p-2", 720, 2_000_000),
    Quality("480p-1.5", 480, 1_500_000),
    Quality("360p-0.7", 360, 700_000),
)

/** The film as it was written, which a player starts on. */
val AS_IT_IS = QUALITIES.first()

/** The rung chosen for the connection rather than by the viewer. */
val ADAPTIVE = QUALITIES[1]

/** The rung by that name, or the film as it is when the name means nothing. */
fun qualityCalled(key: String?): Quality = QUALITIES.firstOrNull { it.key == key } ?: AS_IT_IS

/** A rung as it reads on screen, built from its own numbers as the web's:
 *  "1080p" and a rate are the same words in every language. */
fun Quality.name(asItIs: String): String {
    val rate = (bitrate ?: return asItIs) / 1_000_000.0
    val written = (if (rate % 1.0 == 0.0) "${rate.toLong()}" else String.format(Locale.ROOT, "%.1f", rate)) + " Mb/s"
    // A rate with no size is the film at its own size, held to that rate.
    return height?.let { "${it}p · $written" } ?: written
}

/** A rung in a word or two, for the automatic quality to say where it
 *  stands in the width of its row. */
fun Quality.shortName(original: String): String = when {
    bitrate == null -> original
    height == null -> name(original)
    else -> "${height}p"
}

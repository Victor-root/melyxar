package app.melyxar.tv.inside

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.interaction.FocusInteraction
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.center
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.server.Album
import app.melyxar.server.Library
import app.melyxar.server.Picture
import app.melyxar.server.Shelf
import app.melyxar.theme.mix
import app.melyxar.theme.parseHex
import app.melyxar.tv.look.BoxShadow
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.SectionGap
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.accentAir
import app.melyxar.tv.look.accentBloom
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.keptReady
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.litPast
import app.melyxar.tv.look.rememberRowScrolling
import app.melyxar.tv.look.rim
import app.melyxar.tv.look.shadowRaised
import app.melyxar.tv.probe.Part
import app.melyxar.tv.probe.Probe
import kotlinx.coroutines.flow.first

/** As many covers as the other tiles fan out posters. */
private const val IN_A_FAN = 5

/** What the tile of music fans out: the newest albums with a cover first,
 *  as the other tiles put their posters ahead of the holes. */
fun coversOf(albums: List<Album>): List<Album> = albums.sortedBy { it.cover.isEmpty() }.take(IN_A_FAN)

/** How many covers, from the middle out, have a record behind them. */
private const val WITH_A_RECORD = 3

/** How large a cover is drawn next to the height a poster would have, the
 *  outer ones stopping just inside the tile's sides. */
private const val COVER_SIZE = 0.75f

/** A tile of the band. */
private val TileWidth = 208.dp
private val TileHeight = TileWidth * 9 / 16

/** A picture of a fan: a poster standing, or a square cover of music. */
private data class Leafed(val pictures: List<Picture>, val color: String?)

/** Each poster of a fan, from the front one out: its height in the tile, how
 *  far it stands from the middle in its own widths, how far it leans, and how
 *  much more of each it takes when the fan opens. */
private class Leaf(val height: Float, val shift: Float, val opensBy: Float, val tilt: Float, val tiltsBy: Float)

private val FAN = listOf(
    Leaf(0.65f, 0f, 0f, 0f, 0f),
    Leaf(0.56f, -0.84f, -0.10f, -7f, -2f),
    Leaf(0.56f, 0.84f, 0.10f, 7f, 2f),
    Leaf(0.47f, -1.68f, -0.16f, -14f, -3f),
    Leaf(0.47f, 1.68f, 0.16f, 14f, 3f),
)

/**
 * The band under the banner, as the web's: a tile for each library, in the
 * account's [order], a fan of its newest posters standing in the accent's
 * light, opening when the remote comes to it. A library of music, which the
 * server's shelves know nothing of, fans out the covers of its newest
 * [albums], square as a cover is.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Band(
    shelves: List<Shelf>,
    albums: Map<String, List<Album>>,
    libraries: List<Library>,
    order: List<String>,
    onOpen: (Library) -> Unit,
    memory: FocusMemory,
    /** Given to the first tile, for a home that starts the remote there. */
    first: FocusRequester,
) {
    val shown = order.mapNotNull { id -> libraries.firstOrNull { it.id == id } }
    val nearest = rememberRowScrolling()
    val row = remember { RowMemory() }
    // Coming into the band lands on the tile it left, or on the first.
    CompositionLocalProvider(LocalBringIntoViewSpec provides nearest) {
        LazyRow(
            contentPadding = PaddingValues(start = Gutter, end = Gutter, top = 20.dp, bottom = SectionGap),
            horizontalArrangement = Arrangement.spacedBy(18.dp),
            modifier = Modifier.landsAsLeft(row, shown.firstOrNull()?.id),
        ) {
            itemsIndexed(shown, key = { _, library -> library.id }) { at, library ->
                val music = library.kind == "music"
                BandTile(
                    kind = library.kind,
                    name = library.name,
                    fan = if (music) {
                        coversOf(albums[library.id].orEmpty()).map { Leafed(it.cover, it.color) }
                    } else {
                        shelves.firstOrNull { it.library == library.id }?.fan.orEmpty().map { Leafed(it.poster, it.color) }
                    },
                    square = music,
                    onClick = { onOpen(library) },
                    modifier = Modifier
                        .remembered(memory, "band/${library.id}")
                        .inRow(row, library.id)
                        .then(if (at == 0) Modifier.focusRequester(first) else Modifier),
                )
            }
        }
    }
}

@Composable
private fun BandTile(kind: String, name: String, fan: List<Leafed>, square: Boolean, onClick: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    // The records are drawn the first time the remote comes to the tile,
    // since nothing of them is seen before.
    var woken by remember { mutableStateOf(false) }
    if (square) {
        LaunchedEffect(interaction) {
            interaction.interactions.first { it is FocusInteraction.Focus }
            woken = true
        }
    }
    val front = fan.firstOrNull()?.color?.let(::parseHex) ?: palette.accent.toArgb()
    val force = if (palette.light) 0.34f else 1f
    Box(
        modifier = modifier
            .size(TileWidth, TileHeight)
            .grows({ lit }, 1.035f)
            // At rest `0 0 22px -8px` of the accent at 45%, giving way to the
            // halo of a card as the tile lights.
            .then(
                if (Probe.shows(Part.BandGlow)) {
                    Modifier
                        .boxShadow(CardShape, BoxShadow(palette.accent.copy(alpha = 0.45f), blur = 22.dp, spread = (-8).dp)) { 1f - lit }
                        .boxShadow(CardShape, accentAir(palette.accent), accentBloom(palette.accent)) { lit }
                } else {
                    Modifier
                },
            )
            .clip(CardShape)
            .keptReady()
            // Laid out once for the tile; only the halo behind the fan is
            // drawn again as it lights.
            .drawWithCache {
                val ground = Brush.verticalGradient(
                    listOf(Color(mix(front, palette.surfaceRaised.toArgb(), 0.24f)), palette.ground),
                )
                val pools = listOf(
                    glowAt(Offset(size.width * 0.17f, size.height * 0.09f), size.width * 0.84f, palette.accent.copy(alpha = 0.30f * force)),
                    glowAt(Offset(size.width * 0.89f, size.height * 0.13f), size.width * 0.72f, Color(front).copy(alpha = 0.52f * force)),
                    glowAt(Offset(size.width * 0.5f, size.height * 0.97f), size.width * 0.78f, palette.accentDim.copy(alpha = 0.24f * force)),
                )
                val halo = glowAt(Offset(size.width * 0.5f, size.height * 0.5f), size.width * 0.5f, palette.accent.copy(alpha = 0.30f))
                onDrawBehind {
                    drawRect(ground)
                    pools.forEach { drawRect(it) }
                    if (lit > 0f) {
                        drawRect(halo, alpha = lit)
                    }
                }
            }
            .clickable(interaction, indication = null, onClick = onClick),
    ) {
        Glyph(
            Icons.ofKind(kind),
            palette.text.copy(alpha = 0.04f),
            TileHeight * 0.9f,
            Modifier.align(Alignment.TopEnd).padding(top = 4.dp, end = 4.dp),
        )
        Fan(fan, square, woken, { lit })
        // The foot of the tile sinks into the ground, under the name.
        Box(
            Modifier.fillMaxSize().drawWithCache {
                val foot = Brush.verticalGradient(
                    0.70f to Color.Transparent,
                    0.80f to palette.ground.copy(alpha = 0.55f),
                    0.87f to palette.ground,
                )
                onDrawBehind { drawRect(foot) }
            },
        )
        Row(
            modifier = Modifier.align(Alignment.BottomStart).padding(start = 12.dp, end = 10.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Glyph(Icons.ofKind(kind), palette.accentBright, 17.dp)
            Text(name, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = palette.text, maxLines = 1, modifier = Modifier.weight(1f))
            Arrow({ lit })
        }
        Box(Modifier.fillMaxSize().rim(CardShape, palette.accent.copy(alpha = 0.62f), 1.dp).rim(CardShape, palette.accentBright, 1.5.dp) { lit })
    }
}

/**
 * The way into the library, as the web's `.band-arrow`: a round of the
 * accent at 20% ringed by it at 45% and lit round, filled with the accent
 * as the tile lights. Its sizes are the web's measured against a tile of a
 * large screen, 340 points across, scaled to this one: the web's own 30
 * points on a tile this small stood over the posters.
 */
@Composable
private fun Arrow(lit: () -> Float) {
    val palette = LocalPalette.current
    val shown = litPast(lit)
    Box(
        Modifier
            .size(30.dp * ARROW_SCALE)
            .boxShadow(
                CircleShape,
                BoxShadow(palette.accent.copy(alpha = 0.45f), blur = 0.dp, spread = 1.dp),
                BoxShadow(palette.accentGlow, blur = 14.dp * ARROW_SCALE, spread = 2.dp * ARROW_SCALE),
            ) { 1f - lit() }
            .boxShadow(
                CircleShape,
                BoxShadow(palette.accent, blur = 0.dp, spread = 1.dp),
                BoxShadow(palette.accentGlow, blur = 22.dp * ARROW_SCALE, spread = 4.dp * ARROW_SCALE),
                shown = lit,
            )
            .clip(CircleShape)
            .drawBehind { drawRect(palette.accent.copy(alpha = 0.2f + 0.8f * lit())) },
        contentAlignment = Alignment.Center,
    ) {
        Glyph(Icons.ChevronRight, if (shown) palette.accentContrast else palette.accentBright, 18.dp * ARROW_SCALE)
    }
}

/** The tile of a large screen the web's arrow is drawn for, against this one. */
private val ARROW_SCALE = TileWidth / 340.dp

/** The fan of posters, or of square covers, standing on a line at 69% of
 *  the tile. Covers stand where the posters would, two thirds of their
 *  height across apart, and the [records] slide out from behind the middle
 *  ones as the fan opens. */
@Composable
private fun Fan(fan: List<Leafed>, square: Boolean, records: Boolean, open: () -> Float) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(8.dp)
    val scale = if (square) COVER_SIZE else 1f
    // A shift counted in the widths of a poster, whatever is drawn.
    val reach = if (square) 2f / 3f / COVER_SIZE else 1f
    Box(Modifier.fillMaxSize()) {
        // The outer leaves first, so the front one is drawn over them.
        FAN.withIndex().reversed().forEach { (at, leaf) ->
            val shown = fan.getOrNull(at) ?: return@forEach
            val down: Dp = TileHeight * leaf.height * scale
            val across = if (square) down else down * 2 / 3
            Box(
                Modifier
                    .align(Alignment.TopCenter)
                    .padding(top = TileHeight * 0.69f - down)
                    .size(across, down)
                    .graphicsLayer {
                        val opened = open()
                        transformOrigin = TransformOrigin(0.5f, 1f)
                        translationX = (leaf.shift + leaf.opensBy * opened) * reach * size.width
                        translationY = if (at == 0) -0.04f * size.height * opened else 0f
                        rotationZ = leaf.tilt + leaf.tiltsBy * opened
                    },
            ) {
                if (records && at < WITH_A_RECORD) {
                    Record(open)
                }
                Box(
                    Modifier
                        .fillMaxSize()
                        // The front poster stands raised, with a brighter edge
                        // and the air of the accent just off it; the others
                        // lie on the tile with a fainter edge and no shadow of
                        // their own. They lean as the fan opens, and the
                        // graphics card works the blur of a leaning box out
                        // afresh at every frame of it, which a television
                        // cannot keep up with.
                        .then(
                            if (!Probe.shows(Part.FanShadows)) {
                                Modifier
                            } else if (at == 0) {
                                Modifier.boxShadow(
                                    shape,
                                    palette.shadowRaised,
                                    BoxShadow(palette.text.copy(alpha = 0.20f), blur = 0.dp, spread = 1.dp),
                                    BoxShadow(palette.accentGlow, blur = 38.dp, spread = (-6).dp),
                                )
                            } else {
                                Modifier.boxShadow(shape, BoxShadow(palette.text.copy(alpha = 0.14f), blur = 0.dp, spread = 1.dp))
                            },
                        )
                        .clip(shape),
                ) {
                    ServerPicture(shown.pictures, across, Modifier.fillMaxSize())
                    Box(
                        Modifier.fillMaxSize().drawWithCache {
                            // The sheen across each poster.
                            val sheen = Brush.linearGradient(
                                0f to palette.text.copy(alpha = 0.11f),
                                0.34f to Color.Transparent,
                                start = Offset(0f, 0f),
                                end = Offset(size.width * 0.4f, size.height),
                            )
                            onDrawBehind { drawRect(sheen) }
                        },
                    )
                }
            }
        }
    }
}

/** The record behind a cover, as the web's `.band-record`: the size of the
 *  cover, risen by a third of it as the fan opens. Painted once; only its
 *  layer moves. */
@Composable
private fun Record(open: () -> Float) {
    val palette = LocalPalette.current
    Box(
        Modifier
            .fillMaxSize()
            .graphicsLayer { translationY = -0.32f * size.height * open() }
            .drawWithCache {
                val radius = size.minDimension / 2
                val shine = palette.text
                // The two glints a record catches from a light above it, the
                // brighter right of the middle, the fainter left of it.
                val glints = Brush.sweepGradient(
                    // Turned a quarter back from the web's, which starts at
                    // the top where this one starts on the right.
                    210f / 360f to Color.Transparent,
                    232f / 360f to shine.copy(alpha = 0.32f),
                    252f / 360f to Color.Transparent,
                    278f / 360f to Color.Transparent,
                    294f / 360f to shine.copy(alpha = 0.50f),
                    314f / 360f to Color.Transparent,
                    center = size.center,
                )
                val grooves = (1..((radius - radius * 0.18f) / 3.dp.toPx()).toInt()).map { radius * 0.18f + it * 3.dp.toPx() }
                val groove = Stroke(1.dp.toPx())
                onDrawBehind {
                    drawCircle(palette.ground, radius)
                    grooves.forEach { drawCircle(shine.copy(alpha = 0.07f), it, style = groove) }
                    drawCircle(glints, radius)
                    drawCircle(shine.copy(alpha = 0.16f), radius, style = Stroke(1.dp.toPx()))
                    drawCircle(palette.accent, radius * 0.17f)
                    drawCircle(palette.ground, radius * 0.03f)
                }
            },
    )
}

private fun glowAt(center: Offset, radius: Float, colour: Color): Brush =
    Brush.radialGradient(listOf(colour, Color.Transparent), center = center, radius = radius)

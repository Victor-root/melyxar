package app.melyxar.tv.inside

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
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
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.server.Card
import app.melyxar.server.Library
import app.melyxar.server.Shelf
import app.melyxar.theme.mix
import app.melyxar.theme.parseHex
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.Pivot
import app.melyxar.tv.look.SectionGap
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.halo
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim

/** A tile of the band. */
private val TileWidth = 208.dp
private val TileHeight = TileWidth * 9 / 16

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
 * The band under the banner, as the web's: a tile for each kind of library,
 * a fan of its newest posters standing in the accent's light, opening when
 * the remote comes to it.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun Band(shelves: List<Shelf>, libraries: List<Library>, order: List<String>, onOpen: (Library) -> Unit, memory: FocusMemory) {
    val kinds = order.filter { kind -> libraries.any { it.kind == kind } }
    val lead = with(LocalDensity.current) { Gutter.toPx() }
    val pivot = remember(lead) { Pivot(lead) }
    // Coming into the band lands on the tile it left, or on the first.
    val first = remember { FocusRequester() }
    CompositionLocalProvider(LocalBringIntoViewSpec provides pivot) {
        LazyRow(
            contentPadding = PaddingValues(start = Gutter, end = Gutter, top = 20.dp, bottom = SectionGap),
            horizontalArrangement = Arrangement.spacedBy(18.dp),
            modifier = Modifier.focusRestorer(first),
        ) {
            itemsIndexed(kinds, key = { _, kind -> kind }) { at, kind ->
                val library = libraries.first { it.kind == kind }
                BandTile(
                    kind = kind,
                    fan = shelves.firstOrNull { it.kind == kind }?.fan.orEmpty(),
                    onClick = { onOpen(library) },
                    modifier = Modifier
                        .remembered(memory, "band/$kind")
                        .then(if (at == 0) Modifier.focusRequester(first) else Modifier),
                )
            }
        }
    }
}

@Composable
private fun BandTile(kind: String, fan: List<Card>, onClick: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val glowing by remember { derivedStateOf { lit > 0.001f } }
    val front = fan.firstOrNull()?.color?.let(::parseHex) ?: palette.accent.toArgb()
    val force = if (palette.light) 0.34f else 1f
    Box(
        modifier = modifier
            .size(TileWidth, TileHeight)
            .grows({ lit }, 1.035f)
            .dropShadow(CardShape) {
                radius = 22.dp.toPx()
                spread = (-8).dp.toPx()
                color = palette.accent
                alpha = 0.45f * (1 - lit)
            }
            .then(if (glowing) Modifier.halo(CardShape, palette.accent) { lit } else Modifier)
            .clip(CardShape)
            .drawBehind {
                drawRect(
                    Brush.verticalGradient(
                        listOf(Color(mix(front, palette.surfaceRaised.toArgb(), 0.24f)), palette.ground),
                    ),
                )
                glowAt(Offset(size.width * 0.17f, size.height * 0.09f), size.width * 0.84f, palette.accent.copy(alpha = 0.30f * force))
                glowAt(Offset(size.width * 0.89f, size.height * 0.13f), size.width * 0.72f, Color(front).copy(alpha = 0.52f * force))
                glowAt(Offset(size.width * 0.5f, size.height * 0.97f), size.width * 0.78f, palette.accentDim.copy(alpha = 0.24f * force))
                // The halo behind the fan.
                glowAt(Offset(size.width * 0.5f, size.height * 0.5f), size.width * 0.5f, palette.accent.copy(alpha = 0.30f * lit))
            }
            .clickable(interaction, indication = null, onClick = onClick),
    ) {
        Glyph(
            Icons.ofKind(kind),
            palette.text.copy(alpha = 0.04f),
            TileHeight * 0.9f,
            Modifier.align(Alignment.TopEnd).padding(top = 4.dp, end = 4.dp),
        )
        Fan(fan, { lit })
        // The foot of the tile sinks into the ground, under the name.
        Box(
            Modifier.fillMaxSize().drawBehind {
                drawRect(
                    Brush.verticalGradient(
                        0.70f to Color.Transparent,
                        0.80f to palette.ground.copy(alpha = 0.55f),
                        0.87f to palette.ground,
                    ),
                )
            },
        )
        Row(
            modifier = Modifier.align(Alignment.BottomStart).padding(start = 12.dp, end = 10.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Glyph(Icons.ofKind(kind), palette.accentBright, 17.dp)
            Text(kindName(kind), fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = palette.text, maxLines = 1, modifier = Modifier.weight(1f))
            Box(
                Modifier
                    .size(26.dp)
                    .dropShadow(CircleShape) {
                        radius = 14.dp.toPx()
                        spread = 2.dp.toPx()
                        color = palette.accentGlow
                    }
                    .clip(CircleShape)
                    .drawBehind { drawRect(if (lit > 0.5f) palette.accent else palette.accent.copy(alpha = 0.2f)) },
                contentAlignment = Alignment.Center,
            ) {
                Glyph(Icons.ChevronRight, if (lit > 0.5f) palette.accentContrast else palette.accentBright, 16.dp)
            }
        }
        Box(Modifier.fillMaxSize().rim(CardShape, palette.accent.copy(alpha = 0.62f), 1.dp).rim(CardShape, palette.accentBright, 1.5.dp) { lit })
    }
}

/** The fan of posters, standing on a line at 69% of the tile. */
@Composable
private fun Fan(fan: List<Card>, open: () -> Float) {
    val palette = LocalPalette.current
    val shape = RoundedCornerShape(8.dp)
    Box(Modifier.fillMaxSize()) {
        // The outer leaves first, so the front one is drawn over them.
        FAN.withIndex().reversed().forEach { (at, leaf) ->
            val card = fan.getOrNull(at) ?: return@forEach
            val down: Dp = TileHeight * leaf.height
            val across = down * 2 / 3
            Box(
                Modifier
                    .align(Alignment.TopCenter)
                    .padding(top = TileHeight * 0.69f - down)
                    .size(across, down)
                    .graphicsLayer {
                        val opened = open()
                        transformOrigin = TransformOrigin(0.5f, 1f)
                        translationX = (leaf.shift + leaf.opensBy * opened) * size.width
                        translationY = if (at == 0) -0.04f * size.height * opened else 0f
                        rotationZ = leaf.tilt + leaf.tiltsBy * opened
                    }
                    .then(
                        if (at == 0) {
                            Modifier.dropShadow(shape) {
                                radius = 38.dp.toPx()
                                spread = (-6).dp.toPx()
                                color = palette.accentGlow
                            }
                        } else {
                            Modifier
                        },
                    )
                    .clip(shape),
            ) {
                ServerPicture(card.poster, across, Modifier.fillMaxSize())
                Box(
                    Modifier.fillMaxSize().drawBehind {
                        // The sheen across each poster.
                        drawRect(
                            Brush.linearGradient(
                                0f to palette.text.copy(alpha = 0.11f),
                                0.34f to Color.Transparent,
                                start = Offset(0f, 0f),
                                end = Offset(size.width * 0.4f, size.height),
                            ),
                        )
                    },
                )
            }
        }
    }
}

private fun androidx.compose.ui.graphics.drawscope.DrawScope.glowAt(center: Offset, radius: Float, colour: Color) {
    drawRect(Brush.radialGradient(listOf(colour, Color.Transparent), center = center, radius = radius))
}

package app.melyxar.tv.inside

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.GenericShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.server.Card
import app.melyxar.theme.MelyxarDisplay
import app.melyxar.theme.mix
import app.melyxar.theme.parseHex
import app.melyxar.tv.howFarIn
import app.melyxar.tv.lengthOfAPlay
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.PosterWidth
import app.melyxar.tv.look.ServerPicture
import app.melyxar.tv.look.WideWidth
import app.melyxar.tv.look.boxShadow
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.halo
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim
import app.melyxar.tv.look.shadowTile
import app.melyxar.tv.probe.cardLayer

/** The corner a watched card folds over, as a triangle. */
private val Corner = GenericShape { size, _ ->
    moveTo(0f, 0f)
    lineTo(size.width, 0f)
    lineTo(size.width, size.height)
    close()
}

/**
 * A work in a row or a grid, as the web draws it: its poster standing, or
 * its still lying down for what is told apart by the still, with how far in
 * somebody is and whether it was watched.
 */
@Composable
fun WorkCard(
    card: Card,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    lying: Boolean = false,
    title: String = card.title,
    note: String? = card.year?.toString(),
    trailing: String? = null,
) {
    val palette = LocalPalette.current
    val whereabouts = LocalMarks.current.whereabouts(card)
    val progress = whereabouts.resume?.let { howFarIn(it.toDouble(), lengthOfAPlay(card)) }
    val across = if (lying) WideWidth else PosterWidth
    PictureCard(
        across = across,
        down = if (lying) across * 9 / 16 else across * 3 / 2,
        title = title,
        note = note,
        trailing = trailing,
        ground = card.color?.let(::parseHex)?.let(::Color) ?: palette.surfaceRaised,
        onClick = onClick,
        modifier = modifier.offersMenuOf(Offered.Work(card)),
    ) { glowing ->
        val pictures = if (lying && card.wide.isNotEmpty()) card.wide else card.poster
        if (pictures.isEmpty()) {
            NoPicture(card, across)
        } else {
            ServerPicture(pictures, across, Modifier.matchParentSize())
        }
        progress?.takeIf { it > 0f }?.let { done ->
            Box(
                Modifier
                    .align(Alignment.BottomStart)
                    .fillMaxWidth()
                    .height(4.dp)
                    .background(if (glowing) Color.Transparent else Color.Black.copy(alpha = 0.55f)),
            ) {
                Box(
                    Modifier
                        .fillMaxHeight()
                        .fillMaxWidth(done)
                        .background(if (glowing) palette.accentBright else palette.accent),
                )
            }
        }
        if (card.watchedMarks) {
            SeenCorner(whereabouts, card.episodes, Modifier.align(Alignment.TopEnd))
        }
    }
}

/**
 * A card of a row or a grid: its [picture] in a frame [across] by [down] on
 * [ground], then its title and what is said under it. The remote coming to
 * it lights it as the pointer does on the web; [picture] is told whether it
 * is lit.
 */
@Composable
fun PictureCard(
    across: Dp,
    down: Dp,
    title: String,
    note: String?,
    trailing: String?,
    ground: Color,
    onClick: () -> Unit,
    modifier: Modifier,
    picture: @Composable BoxScope.(glowing: Boolean) -> Unit,
) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    val glowing by remember { derivedStateOf { lit > 0.001f } }
    Column(
        modifier = modifier
            .width(across)
            .clickable(interaction, indication = null, onClick = onClick),
    ) {
        Box(
            // As wide as what holds it, which a grid stretches to fill its
            // lines as the web's does.
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(across / down)
                .grows({ lit })
                // What a card rests on, giving way to its halo as it lights.
                .boxShadow(CardShape, *palette.shadowTile) { 1f - lit }
                .halo(CardShape, palette) { lit }
                .clip(CardShape)
                .cardLayer()
                .background(ground)
                .rim(CardShape, palette.accentBright, 1.5.dp) { lit },
        ) {
            picture(glowing)
        }
        Text(
            title,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            lineHeight = 18.sp,
            color = if (glowing) palette.text else palette.text.copy(alpha = 0.92f),
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        Row {
            note?.let {
                Text(
                    it,
                    fontSize = 12.sp,
                    color = palette.textFaint,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
            }
            trailing?.let {
                Text(
                    it,
                    fontSize = 12.sp,
                    color = palette.textFaint,
                    maxLines = 1,
                    modifier = Modifier.padding(start = if (note != null) 8.dp else 0.dp),
                )
            }
        }
    }
}

/** How many episodes are left, or a tick once a work is watched. */
@Composable
private fun SeenCorner(whereabouts: Whereabouts, episodes: Int, modifier: Modifier) {
    val palette = LocalPalette.current
    val counting = episodes > 0 && whereabouts.unwatched > 0
    if (!counting && whereabouts.seen != "watched") {
        return
    }
    val written = if (counting) whereabouts.unwatched.toString() else ""
    val side = when {
        written.length > 3 -> 56.dp
        written.length > 2 -> 48.dp
        else -> 40.dp
    }
    Box(modifier.size(side).clip(Corner).background(palette.accent)) {
        if (counting) {
            Text(
                written,
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold,
                color = palette.accentContrast,
                modifier = Modifier.align(Alignment.TopEnd).padding(top = 4.dp, end = 6.dp),
            )
        } else {
            Glyph(
                Icons.Tick,
                palette.accentContrast,
                13.dp,
                Modifier.align(Alignment.TopEnd).padding(top = 5.dp, end = 5.dp),
            )
        }
    }
}

/** A work with no picture: its letter, large, over its own colour. */
@Composable
private fun NoPicture(card: Card, across: Dp) {
    val palette = LocalPalette.current
    val own = card.color?.let(::parseHex) ?: palette.surfaceRaised.toArgb()
    val ground = palette.ground.toArgb()
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .fillMaxHeight()
            .drawWithCache {
                val fade = Brush.verticalGradient(listOf(Color(mix(own, ground, 0.6f)), Color(mix(own, ground, 0.25f))))
                val light = Brush.radialGradient(
                    listOf(palette.accent.copy(alpha = 0.16f), Color.Transparent),
                    center = Offset(size.width / 2, 0f),
                    radius = size.width * 1.2f,
                )
                onDrawBehind {
                    drawRect(fade)
                    drawRect(light)
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            card.initial.ifEmpty { card.title.take(1) }.uppercase(),
            fontFamily = MelyxarDisplay,
            fontSize = (across.value * 0.31f).sp,
            color = palette.text.copy(alpha = 0.3f),
        )
    }
}

package app.melyxar.tv.look

import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer

/** Kept by the graphics card as a ready picture, drawn again only when
 *  something on it changes: redrawing every card on every frame cost the
 *  box most of its time. */
fun Modifier.keptReady(): Modifier = graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }

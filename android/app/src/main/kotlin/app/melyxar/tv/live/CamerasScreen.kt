package app.melyxar.tv.live

import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.melyxar.R
import app.melyxar.server.Camera
import app.melyxar.server.Library
import app.melyxar.tv.inside.BarTitle
import app.melyxar.tv.inside.LocalChrome
import app.melyxar.tv.inside.Place
import app.melyxar.tv.inside.RestoreFocus
import app.melyxar.tv.inside.TopBar
import app.melyxar.tv.inside.ready
import app.melyxar.tv.inside.rememberFocusMemory
import app.melyxar.tv.inside.rememberLoaded
import app.melyxar.tv.inside.remembered
import app.melyxar.tv.look.CardShape
import app.melyxar.tv.look.Glyph
import app.melyxar.tv.look.Gutter
import app.melyxar.tv.look.Icons
import app.melyxar.tv.look.LocalPalette
import app.melyxar.tv.look.grows
import app.melyxar.tv.look.litBy
import app.melyxar.tv.look.rim

/** How many cameras stand side by side. */
private const val ACROSS = 4

/**
 * A library of cameras: a tile for each, the camera's mark and its name,
 * with nothing taken from its picture. Choosing one watches it live.
 */
@Composable
fun CamerasScreen(library: Library, comebacks: Int) {
    val chrome = LocalChrome.current
    val palette = LocalPalette.current
    val memory = rememberFocusMemory()
    val first = remember { FocusRequester() }
    val cameras = rememberLoaded("cameras:${library.id}", comebacks) { cameras(library.id) }.value.ready
    LazyVerticalGrid(
        columns = GridCells.Fixed(ACROSS),
        contentPadding = PaddingValues(start = Gutter, end = Gutter, bottom = Gutter),
        horizontalArrangement = Arrangement.spacedBy(20.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            TopBar { BarTitle(Icons.SecurityCamera, library.name, cameras?.size) }
        }
        if (cameras?.isEmpty() == true) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Text(stringResource(R.string.tv_cameras_none), fontSize = 17.sp, color = palette.textSoft)
            }
        }
        itemsIndexed(cameras.orEmpty(), key = { _, camera -> camera.id }) { at, camera ->
            CameraTile(
                camera,
                onClick = { chrome.go(Place.Live(camera)) },
                modifier = Modifier
                    .remembered(memory, "camera/${camera.id}")
                    .then(if (at == 0) Modifier.focusRequester(first) else Modifier),
            )
        }
    }
    RestoreFocus(memory, first, ready = !cameras.isNullOrEmpty())
}

@Composable
private fun CameraTile(camera: Camera, onClick: () -> Unit, modifier: Modifier) {
    val palette = LocalPalette.current
    val interaction = remember { MutableInteractionSource() }
    val lit by litBy(interaction)
    Box(
        modifier
            .fillMaxWidth()
            .aspectRatio(16f / 9f)
            .grows({ lit })
            .clip(CardShape)
            .drawBehind { drawRect(palette.surfaceRaised) }
            .rim(CardShape, palette.accent.copy(alpha = 0.4f), 1.dp)
            .rim(CardShape, palette.accentBright, 2.dp) { lit }
            .clickable(interaction, indication = null, onClick = onClick),
    ) {
        Glyph(Icons.SecurityCamera, palette.accentBright, 44.dp, Modifier.align(Alignment.Center))
        Text(
            camera.name,
            fontSize = 16.sp,
            fontWeight = FontWeight.SemiBold,
            color = palette.text,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.align(Alignment.BottomStart).padding(start = 14.dp, end = 14.dp, bottom = 10.dp),
        )
    }
}

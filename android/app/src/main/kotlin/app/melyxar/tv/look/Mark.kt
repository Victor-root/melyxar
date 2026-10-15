package app.melyxar.tv.look

import android.content.res.Resources
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.unit.Dp
import app.melyxar.R
import app.melyxar.theme.markPixel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** The logo in a colour, made once per colour for as long as the
 *  application runs. */
private val made = HashMap<Int, ImageBitmap>()

private fun markIn(resources: Resources, colour: Int): ImageBitmap = synchronized(made) {
    made.getOrPut(colour) {
        val relief = BitmapFactory.decodeResource(
            resources,
            R.drawable.melyxar_relief,
            BitmapFactory.Options().apply { inPremultiplied = false },
        )
        val pixels = IntArray(relief.width * relief.height)
        relief.getPixels(pixels, 0, relief.width, 0, 0, relief.width, relief.height)
        for (at in pixels.indices) {
            pixels[at] = markPixel(pixels[at], colour)
        }
        Bitmap.createBitmap(pixels, relief.width, relief.height, Bitmap.Config.ARGB_8888).asImageBitmap()
    }
}

/** Melyxar's logo in the account's colour, as the web draws it. */
@Composable
fun MelyxarMark(size: Dp, modifier: Modifier = Modifier) {
    val colour = LocalPalette.current.mark.toArgb()
    val resources = LocalResources.current
    val mark by produceState<ImageBitmap?>(null, colour) {
        value = withContext(Dispatchers.Default) { markIn(resources, colour) }
    }
    Box(modifier.size(size)) {
        mark?.let { Image(it, contentDescription = null, modifier = Modifier.fillMaxSize()) }
    }
}

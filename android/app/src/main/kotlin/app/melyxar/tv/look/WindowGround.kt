package app.melyxar.tv.look

import android.graphics.drawable.ColorDrawable
import androidx.activity.compose.LocalActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb

/** The window's own ground, drawn under every frame anyway, painted in
 *  [ground]: the screens then need no fill of the whole screen over it. */
@Composable
fun WindowGround(ground: Color) {
    val window = LocalActivity.current?.window ?: return
    LaunchedEffect(window, ground) { window.setBackgroundDrawable(ColorDrawable(ground.toArgb())) }
}

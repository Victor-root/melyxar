package app.melyxar.theme

import android.graphics.Color.TRANSPARENT
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

/** The colours of the phone's native screens, which are always dark, in the
 *  usual accent. */
val Melyxar = paletteOf()

@Composable
fun MelyxarTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Melyxar.accent,
            onPrimary = Melyxar.accentContrast,
            background = Melyxar.ground,
            onBackground = Melyxar.text,
            surface = Melyxar.surface,
            onSurface = Melyxar.text,
            surfaceVariant = Melyxar.surfaceRaised,
            onSurfaceVariant = Melyxar.textSoft,
            outline = Melyxar.border,
            error = Melyxar.trouble,
        ),
        content = content,
    )
}

/**
 * The screen drawn under the phone's own bars, see-through, with their icons
 * light: every screen here is dark whatever the phone is set to, and left to
 * follow the phone, a phone in its light theme drew dark icons on a dark
 * screen and a white strip under the buttons.
 */
fun ComponentActivity.drawUnderTheBars() = enableEdgeToEdge(
    statusBarStyle = SystemBarStyle.dark(TRANSPARENT),
    navigationBarStyle = SystemBarStyle.dark(TRANSPARENT),
)

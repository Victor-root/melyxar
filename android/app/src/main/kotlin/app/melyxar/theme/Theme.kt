package app.melyxar.theme

import android.graphics.Color.TRANSPARENT
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/**
 * The colours of the native screens: the same tokens as the web interface's
 * dark theme (`web/src/theme.css`), so a screen drawn here and one drawn by
 * the page beside it are one interface. Every colour of a native screen comes
 * from here, the accent included.
 */
object Melyxar {
    val accent = Color(0xFFC81E1E)
    val accentBright = Color(0xFFE63535)
    val accentDim = Color(0xFF9B1616)
    val accentContrast = Color(0xFFFFFFFF)
    val ground = Color(0xFF0C0D10)
    val surface = Color(0xFF14161B)
    val surfaceRaised = Color(0xFF1B1E25)
    val border = Color(0xFF2A2F39)
    val text = Color(0xFFF2F4F8)
    val textSoft = Color(0xFFAAB2C0)
    val textFaint = Color(0xFF6F7888)
    val trouble = Color(0xFFF0555A)
}

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

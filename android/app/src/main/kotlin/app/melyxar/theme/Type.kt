package app.melyxar.theme

import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import app.melyxar.R

/** The web interface's letters (`web/src/fonts`): Inter for everything. */
val Inter = FontFamily(
    Font(R.font.inter_regular, FontWeight.Normal),
    Font(R.font.inter_medium, FontWeight.Medium),
    Font(R.font.inter_semibold, FontWeight.SemiBold),
    Font(R.font.inter_bold, FontWeight.Bold),
)

/** Space Grotesk, which the web calls Melyxar Display: the server's name, the
 *  greeting, and the letter of a work without a picture. */
val MelyxarDisplay = FontFamily(Font(R.font.space_grotesk_bold, FontWeight.Bold))

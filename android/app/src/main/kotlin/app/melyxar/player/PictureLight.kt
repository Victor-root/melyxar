package app.melyxar.player

import androidx.annotation.OptIn
import androidx.media3.common.Effect
import androidx.media3.common.util.UnstableApi
import androidx.media3.effect.Contrast
import androidx.media3.effect.RgbAdjustment

/*
 * How bright and how contrasted a film's picture is shown on this device, as
 * its viewer asked: a film converted to standard range can come out darker
 * than the screen wants. Drawn by Media3 as the page's player draws it with a
 * filter (`web/src/player/picture-light.ts`): every colour scaled, then the
 * contrast stretched round the middle grey.
 */

/** The steps offered, as a share of the picture as it comes. */
val LIGHTS = listOf(0.8f, 0.9f, 1f, 1.1f, 1.2f, 1.3f, 1.4f, 1.5f)

/** Media3's contrast for a picture's contrast multiplied by [factor]: it
 *  stretches by (1 + c) / (1 - c), so c is (f - 1) / (f + 1). */
fun contrastOf(factor: Float): Float = (factor - 1f) / (factor + 1f)

/** The effects lighting a picture so, none when it is left as it comes. */
@OptIn(UnstableApi::class)
fun lightEffects(brightness: Float, contrast: Float): List<Effect> = buildList {
    if (brightness != 1f) {
        add(RgbAdjustment.Builder().setRedScale(brightness).setGreenScale(brightness).setBlueScale(brightness).build())
    }
    if (contrast != 1f) {
        add(Contrast(contrastOf(contrast)))
    }
}

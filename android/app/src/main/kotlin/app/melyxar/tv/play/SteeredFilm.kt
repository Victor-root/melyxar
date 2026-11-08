package app.melyxar.tv.play

import app.melyxar.tv.inside.Order
import kotlin.math.roundToInt

/**
 * The film on the television, steered from another device of the account
 * (see the decision in docs/architecture/README.md): every order about what
 * is on the screen is done by the film, the music being left alone while
 * one is.
 */

/** Does what the order asks of the film. [next] is absent when there is no
 *  episode after it. */
fun obeyFilm(order: Order, playback: Playback, leave: () -> Unit, next: (() -> Unit)?) {
    when (order.act) {
        "toggle" -> playback.toggle()
        "pause" -> playback.pause()
        "resume" -> playback.play()
        "stop" -> leave()
        "seek" -> order.seconds?.let(playback::goTo)
        "step" -> order.seconds?.let { playback.withStep(it.roundToInt()) }
        "next" -> next?.invoke()
        "audio" -> playback.plan?.audio?.firstOrNull { it.id == order.track }?.let(playback::chooseAudio)
        "subtitle" -> playback.chooseSubtitle(playback.plan?.subtitles?.firstOrNull { it.id == order.track })
    }
}

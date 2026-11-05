package app.melyxar.tv.play

import androidx.media3.exoplayer.ExoPlayer
import app.melyxar.tv.inside.Order
import app.melyxar.tv.inside.Steered

/**
 * The film on the television, steered from another device of the account
 * (see the decision in docs/architecture/README.md): every order about what
 * is on the screen is done by the film, the music being left alone while
 * one is.
 */

/** Does what the order asks of the film. [next] is absent when there is no
 *  episode after it. */
fun obeyFilm(order: Order, playback: Playback, steered: Steered, leave: () -> Unit, next: (() -> Unit)?) {
    when (order.act) {
        "toggle" -> playback.toggle()
        "pause" -> playback.pause()
        "resume" -> playback.play()
        "stop" -> leave()
        "seek" -> order.seconds?.let(playback::goTo)
        "step" -> order.seconds?.let { playback.goTo(playback.seconds + it) }
        "next" -> next?.invoke()
        "volume" -> order.level?.let {
            steered.filmLoudness = it.toFloat()
            soundTo(playback.player, steered)
        }
        "muted" -> order.muted?.let {
            steered.filmMuted = it
            soundTo(playback.player, steered)
        }
        "audio" -> playback.plan?.audio?.firstOrNull { it.id == order.track }?.let(playback::chooseAudio)
        "subtitle" -> playback.chooseSubtitle(playback.plan?.subtitles?.firstOrNull { it.id == order.track })
    }
}

/** Plays the film as loud as the remote last asked, a player made anew
 *  included. */
fun soundTo(player: ExoPlayer, steered: Steered) {
    player.volume = if (steered.filmMuted) 0f else steered.filmLoudness ?: 1f
}

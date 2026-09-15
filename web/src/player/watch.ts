/*
 * Watching a film play, and telling the journal when it does not.
 *
 * The server can say what it produced and when it handed it over. It cannot
 * say whether anything was ever shown: the film stopping in the middle of
 * itself, the picture standing still while the sound runs on, the moment a
 * viewer jumped and what the browser was holding when they did. All of that
 * happens here and nowhere else, and a fault nobody can see from either side
 * alone is a fault nobody fixes.
 *
 * Nothing here chooses its own words. Every line is one of the facts the
 * server names, sent as numbers, and the server writes the sentence.
 */

import { api } from "../api";

/** How still the clock has to be before the film counts as stopped. */
const STOPPED_AFTER_MS = 1_000;

/** How often the film is looked at. */
const LOOK_EVERY_MS = 250;

/**
 * How long a standing still picture goes unreported.
 *
 * Reported when it ends, so that the length is the real one. A picture that
 * never starts again would then never be reported at all, so one line goes out
 * while it is still going, once.
 */
const SAY_SO_ANYWAY_AFTER_MS = 5_000;

/**
 * How still the film has to be before a drag counts as over.
 *
 * Long enough to bridge the gap between two twitches of a hand on the bar,
 * short enough that the line is written down while somebody is still looking
 * at what it describes.
 */
const A_GESTURE_ENDS_AFTER_MS = 400;

/** The stretch the browser holds around one moment, and how many it holds. */
function whatIsHeldAround(element: HTMLVideoElement, moment: number) {
  const held = element.buffered;
  for (let index = 0; index < held.length; index += 1) {
    if (held.start(index) <= moment && moment <= held.end(index)) {
      return {
        held_from_second: held.start(index),
        held_to_second: held.end(index),
        stretches: held.length,
      };
    }
  }
  return { held_from_second: null, held_to_second: null, stretches: held.length };
}

/** Pictures the browser says it has shown, when it counts them. */
function picturesShown(element: HTMLVideoElement): number {
  const quality = element.getVideoPlaybackQuality?.();
  return quality ? quality.totalVideoFrames : 0;
}

/**
 * Pictures the browser decoded and threw away rather than showing.
 *
 * It separates the two ways of showing nothing: climbing, the browser is
 * producing pictures and refusing them, which is one fault; standing still, it
 * is producing none at all, which is another.
 */
function picturesDropped(element: HTMLVideoElement): number {
  const quality = element.getVideoPlaybackQuality?.();
  return quality ? quality.droppedVideoFrames : 0;
}

/**
 * Follows one film and reports what the server cannot see.
 *
 * Answers the way to stop following it, which the player calls when the
 * element goes.
 */
export function watchTheReading(element: HTMLVideoElement, session: string): () => void {
  const tell = (said: Parameters<typeof api.tellTheJournal>[0]) => {
    // Nothing waits on a line in a journal, and a film that plays matters
    // more than knowing how it played.
    api.tellTheJournal(said).catch(() => {});
  };

  /* Where the film was before it moved. Read from the clock as it runs rather
     than from the jump itself, because by the time a jump is announced the
     clock is already at the other end of it. */
  let wasAt = element.currentTime;
  const followTheClock = () => {
    if (!element.seeking) {
      wasAt = element.currentTime;
    }
  };

  /* One line per gesture, not per move. A finger dragged along the bar moves
     the film at every twitch, which is dozens of jumps a second: written down
     one by one they would bury the one thing worth reading, which is where the
     hand let go. So the beginning of the first is kept and the end of the last
     is waited for. */
  let jumpedFrom: number | null = null;
  let settling = 0;
  const jumpStarted = () => {
    if (jumpedFrom === null) {
      jumpedFrom = wasAt;
    }
    window.clearTimeout(settling);
  };
  const jumpFinished = () => {
    window.clearTimeout(settling);
    settling = window.setTimeout(() => {
      const from = jumpedFrom;
      jumpedFrom = null;
      if (from === null || element.seeking) {
        return;
      }
      tell({
        session,
        saw: "viewer_jumped",
        from_second: from,
        to_second: element.currentTime,
        was_playing: !element.paused,
      });
    }, A_GESTURE_ENDS_AFTER_MS);
  };
  element.addEventListener("timeupdate", followTheClock);
  element.addEventListener("seeking", jumpStarted);
  element.addEventListener("seeked", jumpFinished);

  /* The clock standing still, and the picture standing still, which are two
     different faults wearing the same complaint. */
  let stoppedSince: number | null = null;
  let stoppedSaidSoAlready = false;
  let stillAt = element.currentTime;
  let frozenSince: number | null = null;
  let frozenSaidSoAlready = false;
  let frozenPictures = picturesShown(element);
  let frozenClock = element.currentTime;

  const look = () => {
    const now = performance.now();
    const at = element.currentTime;
    const shown = picturesShown(element);
    /* A move that never finishes counts as playing: it is the one shape of a
       stopped film that this used to be blind to. The clock, the picture and
       the sound all stop at once, and read as a film nobody was playing it
       wrote nothing down at all. */
    const running = !element.paused && !element.ended;

    if (!running) {
      stoppedSince = null;
      stoppedSaidSoAlready = false;
      frozenSince = null;
      frozenSaidSoAlready = false;
      stillAt = at;
      frozenClock = at;
      frozenPictures = shown;
      return;
    }

    // The clock itself has stopped: the film is waiting for something.
    if (at === stillAt) {
      if (stoppedSince === null) {
        stoppedSince = now;
      } else if (!stoppedSaidSoAlready && now - stoppedSince >= STOPPED_AFTER_MS) {
        // Said once for this stop, and its end said once when it comes.
        stoppedSaidSoAlready = true;
        tell({
          session,
          saw: "playback_stalled",
          at_second: at,
          was_seeking: element.seeking,
          ready_state: element.readyState,
          pictures_shown: shown,
          ...whatIsHeldAround(element, at),
        });
      }
    } else {
      if (stoppedSaidSoAlready && stoppedSince !== null) {
        tell({
          session,
          saw: "playback_picked_up_again",
          at_second: at,
          waited_ms: Math.round(now - stoppedSince),
        });
      }
      stoppedSince = null;
      stoppedSaidSoAlready = false;
      stillAt = at;
    }

    // The clock running on with nothing new on the screen. Counted rather than
    // watched: from the outside this and the stop above are one complaint.
    if (at !== frozenClock && shown === frozenPictures) {
      if (frozenSince === null) {
        frozenSince = now;
      } else if (!frozenSaidSoAlready && now - frozenSince >= SAY_SO_ANYWAY_AFTER_MS) {
        frozenSaidSoAlready = true;
        tell({
          session,
          saw: "the_picture_stood_still",
          at_second: at,
          for_ms: Math.round(now - frozenSince),
          pictures_shown: shown,
          pictures_dropped: picturesDropped(element),
          ready_state: element.readyState,
          ...whatIsHeldAround(element, at),
        });
      }
    } else if (shown !== frozenPictures) {
      if (frozenSince !== null && !frozenSaidSoAlready && now - frozenSince >= STOPPED_AFTER_MS) {
        tell({
          session,
          saw: "the_picture_stood_still",
          at_second: frozenClock,
          for_ms: Math.round(now - frozenSince),
          pictures_shown: shown,
          pictures_dropped: picturesDropped(element),
          ready_state: element.readyState,
          ...whatIsHeldAround(element, frozenClock),
        });
      }
      frozenSince = null;
      frozenSaidSoAlready = false;
    }
    frozenClock = at;
    frozenPictures = shown;
  };

  const ticking = window.setInterval(look, LOOK_EVERY_MS);

  return () => {
    window.clearInterval(ticking);
    window.clearTimeout(settling);
    element.removeEventListener("timeupdate", followTheClock);
    element.removeEventListener("seeking", jumpStarted);
    element.removeEventListener("seeked", jumpFinished);
  };
}

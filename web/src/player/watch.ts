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

/**
 * How far before the landing a picture still counts as the landing.
 *
 * A browser puts up the picture nearest the moment asked for, which sits on
 * the film's own grid and so falls a fraction of a second short: measured at
 * thirty four thousandths on a film of twenty four pictures a second. A
 * quarter of a second covers every rate a film is made at and stays far
 * shorter than landing in the wrong place, which is counted in seconds and is
 * the thing this must not quietly swallow.
 */
const CLOSE_ENOUGH_TO_THE_LANDING_MS = 250;

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

  /* The shape of the picture, said once when it is known and again whenever it
     changes. A film shown stretched is the server and the browser disagreeing
     about that shape, and neither of them could see the other's answer. Said
     from here rather than from the library, because it is the element that
     holds both the picture's shape and the box it is drawn in.

     Both halves have to be followed, and for a while only the picture was: the
     box was read once, at the instant the shape became known, which is before
     the page has finished laying itself out. What that wrote down was a box
     nobody ever saw. */
  let saidTheShape = "";
  const sayTheShape = () => {
    if (!element.videoWidth || !element.videoHeight) {
      return;
    }
    const shape = [
      element.videoWidth,
      element.videoHeight,
      element.clientWidth,
      element.clientHeight,
    ].join("x");
    if (shape === saidTheShape) {
      return;
    }
    saidTheShape = shape;
    tell({
      session,
      saw: "the_picture_arrived",
      across: element.videoWidth,
      down: element.videoHeight,
      drawn_across: element.clientWidth,
      drawn_down: element.clientHeight,
    });
  };
  element.addEventListener("loadedmetadata", sayTheShape);
  element.addEventListener("resize", sayTheShape);
  const boxChanged = new ResizeObserver(() => sayTheShape());
  boxChanged.observe(element);
  sayTheShape();

  /* Where the film was before it moved. Read from the clock as it runs rather
     than from the jump itself, because by the time a jump is announced the
     clock is already at the other end of it. */
  let wasAt = element.currentTime;
  const followTheClock = () => {
    if (!element.seeking) {
      wasAt = element.currentTime;
    }
  };

  /* How long a jump takes to put the film back on the screen, which is the
     only part of a jump a viewer actually counts and the one number neither
     side could produce alone. Timed from the last move of the gesture, and
     read from a picture the browser says it has put up rather than one it has
     decoded: decoding ahead of a screen with nothing on it is exactly the
     fault worth catching. */
  let waitingForThePicture: {
    askedAt: number;
    askedFor: number;
    wasHeldAlready: boolean;
    stretches: number;
    pending: number | null;
  } | null = null;

  const watchForThePicture = () => {
    if (!element.requestVideoFrameCallback) {
      return;
    }
    const waiting = waitingForThePicture;
    if (waiting === null) {
      return;
    }
    waiting.pending = element.requestVideoFrameCallback((now, picture) => {
      if (waitingForThePicture !== waiting) {
        return;
      }
      // A picture from before the landing is the film still where it was, not
      // the film back where the viewer asked for it.
      if (picture.mediaTime * 1000 < waiting.askedFor * 1000 - CLOSE_ENOUGH_TO_THE_LANDING_MS) {
        watchForThePicture();
        return;
      }
      waitingForThePicture = null;
      tell({
        session,
        saw: "the_picture_came_back",
        asked_for_second: waiting.askedFor,
        showed_second: picture.mediaTime,
        after_ms: Math.round(now - waiting.askedAt),
        was_held_already: waiting.wasHeldAlready,
        stretches: waiting.stretches,
      });
    });
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
    /* One line per gesture here too: every twitch of a drag replaces the
       previous one, so what is timed is the move the hand finished on. */
    if (waitingForThePicture?.pending != null) {
      element.cancelVideoFrameCallback?.(waitingForThePicture.pending);
    }
    const askedFor = element.currentTime;
    const held = whatIsHeldAround(element, askedFor);
    waitingForThePicture = {
      askedAt: performance.now(),
      askedFor,
      wasHeldAlready: held.held_from_second !== null,
      stretches: held.stretches,
      pending: null,
    };
    watchForThePicture();
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
    if (waitingForThePicture?.pending != null) {
      element.cancelVideoFrameCallback?.(waitingForThePicture.pending);
    }
    waitingForThePicture = null;
    boxChanged.disconnect();
    element.removeEventListener("loadedmetadata", sayTheShape);
    element.removeEventListener("resize", sayTheShape);
    element.removeEventListener("timeupdate", followTheClock);
    element.removeEventListener("seeking", jumpStarted);
    element.removeEventListener("seeked", jumpFinished);
  };
}

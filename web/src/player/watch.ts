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

import { api, type HowItMoved, type Reading } from "../api";

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
 * How long a stretch of dropped pictures covers before it is said.
 *
 * Short enough to place the trouble in time against what the server was
 * doing at the same moment, long enough that a single incidental drop, which
 * happens even on a healthy film, does not write a line for nothing. A
 * stretch that dropped nothing says nothing at all.
 */
const DROPPED_PICTURES_OVER_MS = 2_000;

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

/**
 * How long after being handed the film the browser is asked where it stands.
 *
 * Long enough for any film that starts at all to have started, short enough
 * that the line is there while somebody is still looking at a film that has
 * not.
 */
const SAY_HOW_IT_STARTED_AFTER_MS = 5_000;

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

/** What following one film hands back to whoever set it going. */
export interface Watching {
  /** Stops following it, which the player calls when the element goes. */
  stop: () => void;
  /**
   * What moved the film, said by the controls that moved it.
   *
   * Only the controls know a click from a drag: by the time the element says
   * it is seeking, the two look exactly alike. Said before the line goes out,
   * because every gesture reaches here before the browser has finished moving
   * the film. What nothing says stays what nobody on the page asked for.
   */
  movedBy: (how: HowItMoved) => void;
}

/**
 * Follows one film and reports what the server cannot see.
 */
export function watchTheReading(element: HTMLVideoElement, reading: Reading): Watching {
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
      ...reading,
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
        ...reading,
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
  /* What moved the film, until something on the page says otherwise. Reset
     once the line is out, so that the next jump answers for itself: a film
     moved by the library right after a viewer moved it would otherwise be
     written down as the viewer's doing, which is the one reading that would
     send somebody looking in the wrong place. */
  let movedBy: HowItMoved = "not_the_page";
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
        ...reading,
        saw: "viewer_jumped",
        from_second: from,
        to_second: element.currentTime,
        was_playing: !element.paused,
        moved_by: movedBy,
      });
      movedBy = "not_the_page";
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
  /* A browser draws nothing on a page nobody is looking at, so a film playing
     behind another window counts no new pictures while its clock runs on:
     the very shape of a standing still picture, and none of its substance.
     Carried with the fault rather than used to swallow it, because a line
     that quietly disappears under a rule is a line nobody can check. */
  let hiddenWhileFrozen = false;

  /* Pictures dropped without ever costing the clock a whole second, which is
     what the two checks above are blind to: a decoder shedding pictures to
     stay caught up looks, from the clock's side, exactly like a film playing
     perfectly well. Windowed rather than read at a single instant, so what is
     said is a rate rather than a count that only ever grows. */
  let droppedWindowSince = performance.now();
  let droppedAtWindowStart = picturesDropped(element);
  let shownAtWindowStart = picturesShown(element);

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
      hiddenWhileFrozen = false;
      stillAt = at;
      frozenClock = at;
      frozenPictures = shown;
      droppedWindowSince = now;
      droppedAtWindowStart = picturesDropped(element);
      shownAtWindowStart = shown;
      return;
    }

    if (now - droppedWindowSince >= DROPPED_PICTURES_OVER_MS) {
      const droppedNow = picturesDropped(element);
      const dropped = droppedNow - droppedAtWindowStart;
      if (dropped > 0) {
        tell({
          ...reading,
          saw: "pictures_were_dropped",
          at_second: at,
          over_ms: Math.round(now - droppedWindowSince),
          pictures_shown: shown - shownAtWindowStart,
          pictures_dropped: dropped,
        });
      }
      droppedWindowSince = now;
      droppedAtWindowStart = droppedNow;
      shownAtWindowStart = shown;
    }

    // The clock itself has stopped: the film is waiting for something.
    if (at === stillAt) {
      if (stoppedSince === null) {
        stoppedSince = now;
      } else if (!stoppedSaidSoAlready && now - stoppedSince >= STOPPED_AFTER_MS) {
        // Said once for this stop, and its end said once when it comes.
        stoppedSaidSoAlready = true;
        tell({
          ...reading,
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
          ...reading,
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
      hiddenWhileFrozen = hiddenWhileFrozen || document.hidden;
      if (frozenSince === null) {
        frozenSince = now;
      } else if (!frozenSaidSoAlready && now - frozenSince >= SAY_SO_ANYWAY_AFTER_MS) {
        frozenSaidSoAlready = true;
        tell({
          ...reading,
          saw: "the_picture_stood_still",
          at_second: at,
          for_ms: Math.round(now - frozenSince),
          pictures_shown: shown,
          pictures_dropped: picturesDropped(element),
          ready_state: element.readyState,
          page_was_hidden: hiddenWhileFrozen,
          ...whatIsHeldAround(element, at),
        });
      }
    } else if (shown !== frozenPictures) {
      if (frozenSince !== null && !frozenSaidSoAlready && now - frozenSince >= STOPPED_AFTER_MS) {
        tell({
          ...reading,
          saw: "the_picture_stood_still",
          at_second: frozenClock,
          for_ms: Math.round(now - frozenSince),
          pictures_shown: shown,
          pictures_dropped: picturesDropped(element),
          ready_state: element.readyState,
          page_was_hidden: hiddenWhileFrozen,
          ...whatIsHeldAround(element, frozenClock),
        });
      }
      frozenSince = null;
      frozenSaidSoAlready = false;
      hiddenWhileFrozen = false;
    }
    frozenClock = at;
    frozenPictures = shown;
  };

  const ticking = window.setInterval(look, LOOK_EVERY_MS);

  /* Where the film stands a little after the browser was handed it, said once
     whatever the answer. Everything above follows a film that is playing: a
     film the browser left paused, or set going without ever showing a
     picture, went by without a line. */
  const handedAt = performance.now();
  const startedOrNot = window.setTimeout(() => {
    const at = element.currentTime;
    tell({
      ...reading,
      saw: "how_it_started",
      after_ms: Math.round(performance.now() - handedAt),
      at_second: at,
      paused: element.paused,
      ready_state: element.readyState,
      network_state: element.networkState,
      pictures_shown: picturesShown(element),
      pictures_dropped: picturesDropped(element),
      error_code: element.error?.code ?? null,
      ...whatIsHeldAround(element, at),
    });
  }, SAY_HOW_IT_STARTED_AFTER_MS);

  return {
    movedBy: (how) => {
      movedBy = how;
    },
    stop: () => {
      window.clearInterval(ticking);
      window.clearTimeout(startedOrNot);
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
    },
  };
}

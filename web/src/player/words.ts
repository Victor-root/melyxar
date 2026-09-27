/*
 * The words of a film: whether they are on their way, which are on screen at
 * this instant, and how far a viewer has shifted them against the picture.
 *
 * Drawn by this page rather than by the browser, because where they stand is
 * not the browser's to decide: the strip of controls stands over the same
 * part of the picture, and only this page knows when that is true.
 */

import { useCallback, useRef, useState } from "react";
import type { RefObject } from "react";

/**
 * The lines to draw for the cues on screen, one entry per line.
 *
 * Stripped of the handful of tags a subtitle file can carry, such as an
 * italic aside: drawn by hand rather than by the browser, nothing here knows
 * what to do with a tag, and a viewer seeing one written out in full would
 * think the file was broken rather than this page.
 */
export function linesOf(cues: string[]): string[] {
  return cues.flatMap((text) => text.replace(/<\/?[^>]*>/g, "").split("\n"));
}

/** The words of the film playing in `video`, and what a viewer does to them. */
export function useWords(video: RefObject<HTMLVideoElement | null>) {
  /* Whether the words are still on their way, and whether they never came.
     Pulling a subtitle out of a film means reading the whole file through,
     because the words are interleaved with the picture from end to end:
     measured at fifteen to twenty seconds on a 4K film, and seven of those on
     one film. Said nowhere, that wait is a subtitle that does not work. */
  const [words, setWords] = useState<"coming" | "refused" | null>(null);
  /* The element carrying the words, so the moment they finish being read can
     be waited for. */
  const subtitleTrack = useRef<HTMLTrackElement | null>(null);
  /* The words on screen at this instant, one entry per cue active at once,
     which is almost always none or one. Read from the track rather than drawn
     by the browser, because where they are drawn is not the browser's to
     decide here: the strip of controls stands over the same part of the
     picture, and only this page knows when that is true. */
  const [shownWords, setShownWords] = useState<string[]>([]);
  /* How far the words are shifted against the picture, in seconds, and how far
     they are shifted at this moment.
     The two are not the same number for as long as it takes a cue to be told,
     and the difference is what is applied: cues carry their own times and
     there is nowhere to keep the originals, so each change moves them by the
     step rather than setting them to a total. */
  const [wordsOffset, setWordsOffsetState] = useState(0);
  const wordsShiftedBy = useRef(0);
  /* The same number as the state above, read from a hand that does not
     change: closing over the state itself would remake every callback that
     reads it each time a viewer moved it, and one of those is the ref that
     hangs the words on the picture, which a viewer moving them at all was
     then pulling off and hanging again for no reason a viewer could see. */
  const wordsOffsetNow = useRef(wordsOffset);

  /* Moves every word that is on the element by this many seconds.
     On the cues themselves rather than on the element: the words are a track
     of their own with its own clock, and a film whose subtitles run early is a
     film whose picture is right. */
  const shiftTheWords = useCallback((by: number) => {
    const tracks = video.current?.textTracks;
    if (!tracks || by === 0) {
      return;
    }
    for (const track of Array.from(tracks)) {
      for (const cue of Array.from(track.cues ?? [])) {
        cue.startTime = Math.max(0, cue.startTime + by);
        cue.endTime = Math.max(0, cue.endTime + by);
      }
    }
  }, []);

  const setWordsOffset = useCallback(
    (seconds: number) => {
      shiftTheWords(seconds - wordsShiftedBy.current);
      wordsShiftedBy.current = seconds;
      wordsOffsetNow.current = seconds;
      setWordsOffsetState(seconds);
    },
    [shiftTheWords],
  );

  /* The words are in: shifted to wherever this viewer had already put them,
     put where they want them on the picture, and the notice saying they were
     on their way comes down. In that order, because the other way round shows
     one frame of words in the wrong place or at the wrong moment.

     A fresh set of cues arrives unshifted however far the last set was moved,
     so the shift is applied from nothing rather than carried over.

     Read off the hand above rather than closed over the state itself, so
     that moving the offset never has to remake this. */
  const wereRead = useCallback(() => {
    const offset = wordsOffsetNow.current;
    wordsShiftedBy.current = 0;
    shiftTheWords(offset);
    wordsShiftedBy.current = offset;
    setWords(null);
  }, [shiftTheWords]);
  const neverCame = useCallback(() => setWords("refused"), []);

  /* Fired by the track itself the instant the words on screen change, which is
     the one moment this can be read: nothing elsewhere is told when a cue
     starts or ends. */
  const cueChanged = useCallback(() => {
    const cues = subtitleTrack.current?.track.activeCues;
    setShownWords(linesOf(cues ? Array.from(cues).map((cue) => (cue as VTTCue).text) : []));
  }, []);

  /* Waited for on the element carrying the words, and attached again whenever
     that is a different one: a film being rebuilt gets a fresh picture
     whenever the soundtrack changes, and the words come with it. Hidden
     rather than shown, because where they are drawn is decided here, not by
     the browser: the browser only knows the picture, not the strip of
     controls standing over the bottom of it. */
  const holdTheWords = useCallback(
    (element: HTMLTrackElement | null) => {
      const held = subtitleTrack.current;
      held?.removeEventListener("load", wereRead);
      held?.removeEventListener("error", neverCame);
      held?.track.removeEventListener("cuechange", cueChanged);
      subtitleTrack.current = element;
      if (!element) {
        setWords(null);
        setShownWords([]);
        return;
      }
      setWords("coming");
      element.addEventListener("load", wereRead);
      element.addEventListener("error", neverCame);
      element.track.addEventListener("cuechange", cueChanged);
      // Said outright rather than left to the default mark: that mark is read
      // when the picture itself is first read, and words added to a picture
      // already playing would simply stay switched off.
      element.track.mode = "hidden";
    },
    [wereRead, neverCame, cueChanged],
  );

  return { words, shownWords, holdTheWords, wordsOffset, setWordsOffset };
}

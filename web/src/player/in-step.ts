/*
 * Where the sound and the words of one file stand against its picture, for a
 * file whose own sound or words run early or late: shown the moment a viewer
 * moves them, and kept on the server for this account and this file once
 * they stop.
 *
 * The words are moved here, on the cues the player holds. The sound can only
 * be moved by the server, which rebuilds it: a browser can hold a sound back
 * but never bring it forward. Moving it is a new answer and a new session, so
 * it is asked for once the hand has stopped rather than at every step.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { PlaybackPlan } from "../api";

/** How long after the last step the sound is asked for and the place kept. */
const SETTLES_MS = 1000;

/** A number of seconds said for one file. */
interface ForAFile {
  source: string;
  seconds: number;
}

/** Where one file's sound and words are kept, in seconds. */
interface Kept {
  source: string;
  sound: number;
  words: number;
}

function keep({ source, sound, words }: Kept) {
  api
    .keepInStep({ source_id: source, sound_offset_seconds: sound, words_offset_seconds: words })
    .catch(() => {
      // Still moved for this sitting, which is the part being watched.
    });
}

export function useInStep(
  sourceId: string,
  plan: PlaybackPlan | null,
  wordsOffset: number,
  moveTheWords: (seconds: number) => void,
) {
  /* What the hand has moved the sound to, shown at once, and what the server
     is asked for once it has stopped. Each held with the file it was said
     for: the next episode starts where its own file was left, not where the
     last one was put. */
  const [shownSound, setShownSound] = useState<ForAFile | null>(null);
  const [askedSound, setAskedSound] = useState<ForAFile | null>(null);
  const forHere = (said: ForAFile | null) => (said?.source === sourceId ? said.seconds : null);

  /* What is to be kept on the server, read when the timer fires. */
  const kept = useRef<Kept>({ source: sourceId, sound: 0, words: 0 });
  const toKeep = useRef(0);
  const keepNow = useCallback(() => {
    if (toKeep.current) {
      window.clearTimeout(toKeep.current);
      toKeep.current = 0;
      keep(kept.current);
    }
  }, []);
  const keepSoon = useCallback(
    (sound: number, words: number) => {
      kept.current = { ...kept.current, sound, words };
      window.clearTimeout(toKeep.current);
      toKeep.current = window.setTimeout(keepNow, SETTLES_MS);
    },
    [keepNow],
  );

  /* The first answer for a file says where this account left its words, and
     they are put there before the first of them is shown. What was still to
     be kept for the file before goes to the server first. */
  const placed = useRef<string | null>(null);
  useEffect(() => {
    if (!plan || placed.current === sourceId) {
      return;
    }
    placed.current = sourceId;
    keepNow();
    kept.current = { source: sourceId, sound: plan.sound_offset_seconds, words: plan.words_offset_seconds };
    moveTheWords(plan.words_offset_seconds);
  }, [plan, sourceId, moveTheWords, keepNow]);

  const toAsk = useRef(0);
  const setSoundOffset = useCallback(
    (seconds: number) => {
      setShownSound({ source: sourceId, seconds });
      keepSoon(seconds, kept.current.words);
      window.clearTimeout(toAsk.current);
      toAsk.current = window.setTimeout(() => setAskedSound({ source: sourceId, seconds }), SETTLES_MS);
    },
    [sourceId, keepSoon],
  );

  const setWordsOffset = useCallback(
    (seconds: number) => {
      moveTheWords(seconds);
      keepSoon(kept.current.sound, seconds);
    },
    [moveTheWords, keepSoon],
  );

  /* A player closed before the hand had stopped still keeps where it stopped. */
  useEffect(
    () => () => {
      window.clearTimeout(toAsk.current);
      keepNow();
    },
    [keepNow],
  );

  return {
    soundOffset: forHere(shownSound) ?? plan?.sound_offset_seconds ?? 0,
    /** What the server is asked to move the sound by, or null to leave it
     *  where this account last put it for this file. */
    askedSoundOffset: forHere(askedSound),
    setSoundOffset,
    wordsOffset,
    setWordsOffset,
  };
}

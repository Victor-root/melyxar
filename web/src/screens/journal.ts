/*
 * What the journal screen is driven by, and nothing about how it looks.
 *
 * Which tags are ticked and which word narrows the list are the question put
 * to the server, so they live here with it. So does the beat: this screen is
 * open while a test is being run, and a journal that only fills when somebody
 * reloads is a journal nobody reads during the test it was opened for.
 *
 * Copying is here too, because it is not a button doing something visual: the
 * server renders the text so that what is pasted is what the command line
 * would print, and the browser may refuse the clipboard outright, in which
 * case the text has to be handed over some other way.
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { Journal } from "../api";
import { useAsked } from "../asking";
import { handOver } from "../copying";

/** How often the screen looks again while it is open. */
const EVERY_MS = 2_000;

/** Everything the journal screen is handed to draw itself and be driven by. */
export interface JournalScreen {
  journal: Journal;
  /** Whether the server could not be asked. */
  failed: boolean;
  /** The tags ticked, and ticking one. */
  ticked: string[];
  toggle: (tag: string) => void;
  everyTag: () => void;
  /** The word the list is narrowed by. */
  holding: string;
  setHolding: (word: string) => void;
  /** Copying what is on screen, as the server renders it. */
  copy: () => Promise<void>;
  /** Whether the clipboard took it, said for a moment on the button itself. */
  copied: boolean;
  /** The text to be selected by hand, when the browser refused the clipboard.
      The last resort and never the ordinary answer: being handed a box to
      select is not what anybody means by copy. */
  shown: string;
  /** Throwing away what the server has said so far, so that what is copied
      afterwards is about one test alone. */
  forget: () => void;
  /** Throwing away the converted subtitles, for trying the slow path again. */
  forgetConvertedSubtitles: () => Promise<number | null>;
}

export function useJournalScreen(): JournalScreen {
  const [ticked, setTicked] = useState<string[]>([]);
  const [holding, setHolding] = useState("");
  const [copied, setCopied] = useState(false);
  const [shown, setShown] = useState("");
  const [couldNotCopy, setCouldNotCopy] = useState(false);

  const asked = useAsked(
    (signal) => api.journal({ tags: ticked, holding, most: 500 }, signal),
    [ticked, holding],
  );

  /* On its own beat, so a test running right now fills the screen as it goes
     rather than after a reload. Looking rather than asking again: what is on
     screen must not blink twice a second. */
  const { look } = asked;
  useEffect(() => {
    const beat = window.setInterval(look, EVERY_MS);
    return () => window.clearInterval(beat);
  }, [look]);

  /* A copy that failed is said until the server answers something, exactly as
     a journal that could not be read is: one line saying the server is not
     answering, whichever question went unanswered. */
  const { answer } = asked;
  useEffect(() => {
    if (answer) {
      setCouldNotCopy(false);
    }
  }, [answer]);

  const toggle = useCallback(
    (tag: string) =>
      setTicked((was) => (was.includes(tag) ? was.filter((one) => one !== tag) : [...was, tag])),
    [],
  );
  const everyTag = useCallback(() => setTicked([]), []);

  const copy = useCallback(async () => {
    setShown("");
    const handed = await handOver(() => api.journalText({ tags: ticked, holding }));
    if (handed.how === "not at all") {
      setCouldNotCopy(true);
      return;
    }
    if (handed.how === "clipboard") {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2_000);
      return;
    }
    setShown(handed.text);
  }, [ticked, holding]);

  const { again } = asked;
  const forget = useCallback(() => {
    void api.forgetJournal().then(again);
  }, [again]);

  const forgetConvertedSubtitles = useCallback(async () => {
    try {
      const { forgotten } = await api.forgetConvertedSubtitles();
      return forgotten;
    } catch {
      setCouldNotCopy(true);
      return null;
    }
  }, []);

  return {
    journal: asked.answer ?? { tags: [], lines: [] },
    failed: asked.failure !== null || couldNotCopy,
    ticked,
    toggle,
    everyTag,
    holding,
    setHolding,
    copy,
    copied,
    shown,
    forget,
    forgetConvertedSubtitles,
  };
}

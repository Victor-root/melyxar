/*
 * One way to ask the server, for every screen that asks.
 *
 * Every screen needs the same four things, and every screen was writing them
 * out again: a controller so that leaving the page stops the question, a way
 * of telling a question somebody walked away from apart from one the server
 * failed to answer, somewhere to put the answer, and a reading of the refusal
 * when there is one. Counted before this existed: the controller and its
 * cancellation thirteen times, the line that ignores an abandoned question ten
 * times, and the reading of a refusal eight times in three shapes that did not
 * agree with each other.
 *
 * That is why this is not a tidying. Three readings of the same refusal are
 * three answers to "what does the server say when it says no", and a screen
 * written next week gets whichever one was copied. Here there is one.
 *
 * Nothing here draws anything. It is meant to survive an interface written
 * again from nothing, which is the whole point of it being next door rather
 * than inside a page.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { ApiError } from "./api";
import { refusalKey } from "./i18n";
import { keep, recall } from "./kept";

/**
 * Whether this failure is the interface having walked away from its own
 * question.
 *
 * A screen that is left, a search that is typed over, a film that is closed:
 * the question is cancelled and the failure that comes back is not about the
 * server at all. Shown to a viewer it reads as a fault, on a screen they have
 * already left.
 */
export function wasAbandoned(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}

/** The failure as the server described it, or nothing when it was abandoned. */
function whatWentWrong(error: unknown): ApiError | null {
  if (wasAbandoned(error)) {
    return null;
  }
  return error instanceof ApiError ? error : new ApiError("generic", 0);
}

/**
 * Why the server refused, as the code it sent.
 *
 * Anything that is not a refusal from the server is the general one: a fault
 * in this interface is not a sentence a viewer can act on.
 */
export function refusalOf(error: unknown): string {
  return error instanceof ApiError ? error.code : "generic";
}

/**
 * The same, worded, and naming the field when the server named one.
 *
 * A refusal met while filling a form in has to say which box is wrong.
 * `invalid_input` on its own says nothing anybody can act on, so the server
 * sends the field beside it and this is where the two are put back together.
 */
export function refusalAbout(error: unknown, subject: string): string {
  if (error instanceof ApiError && error.reason) {
    return `refused.${subject}.${error.reason}`;
  }
  return refusalKey(refusalOf(error));
}

/** A question put to the server, and what has come of it so far. */
export interface Asked<T> {
  /** What came back, or nothing until it has. The last good answer is kept
      when a later one fails: emptying a screen over one failed question says
      the collection is gone, which is worse than saying nothing. */
  answer: T | null;
  /** Why it could not be answered, or nothing. Never set for a question this
      interface abandoned itself. */
  failure: ApiError | null;
  /** Whether a question is in flight and nothing new has come back yet. True
      again each time somebody asks afresh, so a screen that says it is looking
      says it every time it is sent looking. */
  waiting: boolean;
  /** Whether the answer is the server's own, given since this screen asked,
      rather than the copy kept from an earlier visit that stands in for it
      meanwhile. What is drawn can go by the copy; what is acted on at once,
      like which episode to start, waits for this. */
  current: boolean;
  /** Somebody asked for it again: the last refusal is put aside and the screen
      says it is looking, because a button that changes nothing when it is
      pressed is a button that looks broken. */
  again: () => void;
  /** The screen looking again of its own accord, on a beat nobody asked for.
      Nothing on screen changes until something new comes back: a screen that
      empties its own refusal twice a second flickers, and a viewer reading a
      failure watches it blink at them. */
  look: () => void;
}

/**
 * Asks the server something, and asks again whenever `watching` changes.
 *
 * The question is handed a signal and is expected to pass it on: leaving the
 * screen cancels it. `watching` is the list of things that make the answer
 * stale, and it must be the same length on every render, exactly like the list
 * React itself takes.
 *
 * The question is read at the moment it is asked rather than watched, which is
 * the whole difference between this and writing it out by hand. A question
 * rebuilt on every render is a question asked on every render: the screen
 * answers, the answer is written down, the screen draws again, and it asks
 * once more for ever. Held in a box that is filled beside the render, the
 * question can be written plainly at the call and still be asked once.
 *
 * `keptAs` names what is asked, for a screen that is walked back to: the
 * answer is kept under that name, and a screen asking under a name already
 * answered is handed that answer at once, from its very first drawing, while
 * the question goes to the server all the same and brings it up to date.
 */
export function useAsked<T>(
  ask: (signal: AbortSignal) => Promise<T>,
  watching: unknown[] = [],
  keptAs?: string,
): Asked<T> {
  /* The answer, with the name it was asked under, so an answer to another
     name is never handed out as this one's. */
  const [answer, setAnswer] = useState<{ value: T; name: string | undefined } | null>(null);
  const [failure, setFailure] = useState<ApiError | null>(null);
  const [waiting, setWaiting] = useState(true);
  const [asked, setAsked] = useState(0);
  /* Whether this round was asked for by somebody. A question the screen put
     on its own beat leaves the screen exactly as it is until an answer or a
     refusal really arrives. */
  const wanted = useRef(true);

  const question = useRef(ask);
  useEffect(() => {
    question.current = ask;
  });

  useEffect(() => {
    const controller = new AbortController();
    if (wanted.current) {
      setFailure(null);
      setWaiting(true);
    }
    wanted.current = true;
    question
      .current(controller.signal)
      .then((came) => {
        if (keptAs !== undefined) {
          keep(keptAs, came);
        }
        setAnswer({ value: came, name: keptAs });
        // An answer is the end of whatever was wrong before it, which is what
        // a screen looking again on its own beat is waiting to be told.
        setFailure(null);
        setWaiting(false);
      })
      .catch((error) => {
        const wrong = whatWentWrong(error);
        if (wrong) {
          setFailure(wrong);
          setWaiting(false);
        }
      });
    return () => controller.abort();
    // The question itself is deliberately not watched: see above.
  }, [asked, keptAs, ...watching]);

  const again = useCallback(() => {
    wanted.current = true;
    setAsked((count) => count + 1);
  }, []);

  const look = useCallback(() => {
    wanted.current = false;
    setAsked((count) => count + 1);
  }, []);

  /* What was kept under this name stands in until the server has answered
     it afresh, and nothing is being waited for in the meantime: the screen
     has something true to draw. */
  const known = keptAs !== undefined ? recall<T>(keptAs) : undefined;
  const fresh = answer !== null && answer.name === keptAs ? answer : undefined;
  return {
    answer: fresh ? fresh.value : known ? known.value : null,
    failure,
    waiting: known !== undefined && fresh === undefined ? false : waiting,
    current: fresh !== undefined,
    again,
    look,
  };
}

/** Something the server was told to do, and what became of it. */
export interface Told<A extends unknown[]> {
  /** Tell it. Never throws: what went wrong is in `failure`. */
  tell: (...args: A) => Promise<void>;
  /** Whether the server is being told right now, for a button to go quiet. */
  busy: boolean;
  /** Why it refused, or nothing. */
  failure: ApiError | null;
  /** Put the refusal aside, for a screen that offers another go. */
  forget: () => void;
}

/**
 * Tells the server to do something, and holds what became of it.
 *
 * A button that fails in silence is the same thing as a button that does
 * nothing, and it sends somebody to a terminal. So the refusal is kept beside
 * the button that caused it rather than left in a log.
 */
export function useTold<A extends unknown[]>(act: (...args: A) => Promise<unknown>): Told<A> {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<ApiError | null>(null);

  const doing = useRef(act);
  useEffect(() => {
    doing.current = act;
  });

  const tell = useCallback(async (...args: A) => {
    setBusy(true);
    setFailure(null);
    try {
      await doing.current(...args);
    } catch (error) {
      setFailure(whatWentWrong(error));
    } finally {
      setBusy(false);
    }
  }, []);

  const forget = useCallback(() => setFailure(null), []);

  return { tell, busy, failure, forget };
}

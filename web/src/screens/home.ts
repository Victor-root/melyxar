/*
 * What the home screen is driven by, and nothing about how it looks.
 *
 * Asking the server what arrived last, asking again when work ends because a
 * scan is what makes a library grow, offering the two kinds of work this
 * screen can start, and saying how far into a film somebody got. None of that
 * is a question about where a button sits, and a home screen drawn another way
 * tomorrow needs every bit of it unchanged.
 */

import { api } from "../api";
import type { Home, Job, Library } from "../api";
import { useAsked } from "../asking";
import { useRunning, useStartIdentification, useStartScan } from "../running";
import type { Starter } from "../running";

/**
 * How far into a film somebody is, between nothing and one.
 *
 * Absent when the film has no length recorded, since a fraction of an unknown
 * is not a fraction. Capped, because a position past the end is a report that
 * arrived oddly and not a film watched twice over.
 */
export function howFarIn(seconds: number, runtimeMinutes: number | null): number | undefined {
  if (!runtimeMinutes || runtimeMinutes <= 0) {
    return undefined;
  }
  return Math.min(1, seconds / (runtimeMinutes * 60));
}

/** Everything the home screen is handed to draw itself and to be driven by. */
export interface HomeScreen {
  /** What arrived last, or nothing until the server has said. */
  home: Home | null;
  /** Whether the server could not be asked at all. */
  failed: boolean;
  /** Ask it again, for whoever offers another go. */
  again: () => void;
  /** What the server is doing right now. */
  jobs: Job[];
  /** Starting a scan of every library. */
  scan: Starter;
  /** Looking up again what a previous run could not name: a provider that was
      down, a title nobody recognised, a key added since. */
  lookUp: Starter;
  /** Why the last thing started was refused, as a code to be worded. The look
      up comes first: it is the one somebody pressed on purpose. */
  refused: string | null;
}

export function useHomeScreen(libraries: Library[]): HomeScreen {
  /* Work takes minutes on a real library, so this reads again what it
     produced when it ends. Without that, pressing a button looks exactly like
     pressing a button that does nothing. */
  const { jobs, finished } = useRunning();
  const asked = useAsked((signal) => api.home(undefined, signal), [finished]);

  const scan = useStartScan(libraries);
  const lookUp = useStartIdentification(libraries);

  return {
    home: asked.answer,
    failed: asked.failure !== null,
    again: asked.again,
    jobs,
    scan,
    lookUp,
    refused: lookUp.refused ?? scan.refused,
  };
}

/*
 * What the home screen is driven by, and nothing about how it looks.
 *
 * Asking the server what arrived last, asking again when work ends because a
 * scan is what makes a library grow, offering the two kinds of work this
 * screen can start, and saying how far into a film somebody got. None of that
 * is a question about where a button sits, and a home screen drawn another way
 * tomorrow needs every bit of it unchanged.
 */

import { useEffect, useMemo } from "react";
import { useLocation } from "react-router-dom";
import { api } from "../api";
import type { Card, Home, HomeSection, Job, Library, LibraryKind } from "../api";
import { useAsked } from "../asking";
import { keep, recall } from "../kept";
import { useMarks } from "../marks";
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

/** The kind of library a section shows the newest of, when it is one of
 *  those. */
export function kindOfSection(section: HomeSection): LibraryKind | undefined {
  return section.startsWith("newest:") ? (section.slice("newest:".length) as LibraryKind) : undefined;
}

/** The sections a settings screen offers: every one, except the rows of
 *  kinds of library this account does not hold, which would lead nowhere.
 *  They keep their place all the same, for the day such a library comes. */
export function sectionsOnOffer(sections: readonly HomeSection[], held: readonly LibraryKind[]): HomeSection[] {
  return sections.filter((section) => {
    const kind = kindOfSection(section);
    return kind === undefined || held.includes(kind);
  });
}

/** One section of the home page as it is laid out, or the two rows of what
 *  is under way when they follow each other: those two share a line while
 *  both are short enough for one. */
export type Laid = HomeSection | readonly [HomeSection, HomeSection];

/** The sections in the order the viewer chose, the two rows of what is
 *  under way paired when nothing stands between them. */
export function laidOut(sections: readonly HomeSection[]): Laid[] {
  const underWay = (section: HomeSection | undefined) =>
    section === "carry_on" || section === "up_next";
  const laid: Laid[] = [];
  for (let place = 0; place < sections.length; place += 1) {
    const section = sections[place];
    const next = sections[place + 1];
    if (underWay(section) && next !== undefined && underWay(next)) {
      laid.push([section, next]);
      place += 1;
    } else {
      laid.push(section);
    }
  }
  return laid;
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
  /* And the same whenever something pressed on a card changes which works a
     row holds: a film put on the front page, an episode ticked off. What
     takes its place is the server's answer, so it is asked again. */
  const marks = useMarks();
  const asked = useAsked(
    (signal) => api.home(undefined, signal),
    [finished, marks.rowsMoved],
    "home",
  );

  /* The rows of unfinished work say what is unfinished now, not what was when
     the page was read. A tick pressed on one of their cards has to empty its
     place at once: the answer coming back from the server says the same
     thing, but it says it a round trip later, and a card that lingers for
     that long is a card somebody presses again.
     Only taken away, never added: which episode a series waits on once this
     one is watched is a question only the server can answer, and it is on its
     way. */
  /* The banner drawn at random is drawn once per visit of the page: walked
     back to from a work opened from it, it is the same banner, not a new
     draw brought by the answer read again on the way back. A new visit of
     the page draws again. Only a banner drawn at random is held: any other
     is the server's answer as it stands, and a banner just switched away
     from random has to leave the old draw behind at once. */
  const visit = useLocation().key;
  const lineup = `hero:${visit}`;
  const answer = asked.answer;
  useEffect(() => {
    if (answer?.hero_at_random && !recall(lineup)) {
      keep(lineup, answer.hero);
    }
  }, [answer, lineup]);

  const home = useMemo(() => {
    if (!answer) {
      return null;
    }
    const unfinished = (card: Card) => marks.resumeOf(card) !== null;
    const drawn = answer.hero_at_random
      ? (recall<Home["hero"]>(lineup)?.value ?? answer.hero)
      : answer.hero;
    return {
      ...answer,
      hero: drawn.filter((entry) => entry.because !== "started" || unfinished(entry)),
      carry_on: answer.carry_on.filter(unfinished),
      up_next: answer.up_next.filter((card) => marks.seenOf(card) !== "watched"),
    };
  }, [answer, lineup, marks]);

  const scan = useStartScan(libraries);
  const lookUp = useStartIdentification(libraries);

  return {
    home,
    failed: asked.failure !== null,
    again: asked.again,
    jobs,
    scan,
    lookUp,
    refused: lookUp.refused ?? scan.refused,
  };
}

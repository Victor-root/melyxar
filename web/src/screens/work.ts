/*
 * What the screen of one film is driven by, and nothing about how it looks.
 *
 * Asking the server about the film, asking again when a match chosen by hand
 * has changed what it is, which copy of it the viewer is looking at, where
 * they stopped watching it, and what is being played right now. None of that
 * is a question about where a button sits.
 *
 * Where the viewer stopped is asked for as soon as the screen opens rather
 * than when play is pressed, because the button has to say what it will do
 * before it is pressed.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { api } from "../api";
import type { Version, Work } from "../api";
import { useAsked } from "../asking";

/**
 * What the address says when a page is meant to start playing by itself.
 *
 * This is how one press of "carry on" on a series reaches the episode's own
 * page and starts it: the episode is the thing being watched, so it is its
 * page that plays it and its own progress that is written down. Playing it
 * from the series page would record a series as watched and show its title
 * over somebody else's episode.
 */
const START_AT_ONCE = "play";

/**
 * Where a work lives at the site that named it.
 *
 * The kind decides the road: one of these catalogues keeps films and series
 * apart, and a series asked for down the films road is a page that does not
 * exist. The other names everything the same way and does not care.
 *
 * Nothing at all for a season or an episode. They carry an identifier of
 * their own there too, but the page that shows one is reached through its
 * series and cannot be built from that identifier alone. A link that leads
 * nowhere is worse than no link.
 */
export function elsewhere(provider: string, id: string, kind: string): string {
  if (kind !== "movie" && kind !== "series") {
    return "";
  }
  switch (provider) {
    case "tmdb":
      return `https://www.themoviedb.org/${kind === "movie" ? "movie" : "tv"}/${id}`;
    case "imdb":
      return `https://www.imdb.com/title/${id}/`;
    default:
      return "";
  }
}

/**
 * The order the parts a film was made by are looked for in: whoever made the
 * film first, and the people a page mentions out of completeness last.
 */
const CREW_ORDER = ["director", "writer", "composer", "producer"];

/**
 * Groups the crew so one person credited three times is read once per part,
 * and puts the parts in that order.
 */
export function groupCrew(crew: { name: string; role: string }[]): [string, string[]][] {
  const grouped = new Map<string, string[]>();
  for (const credit of crew) {
    const names = grouped.get(credit.role) ?? [];
    if (!names.includes(credit.name)) {
      names.push(credit.name);
    }
    grouped.set(credit.role, names);
  }

  return Array.from(grouped.entries()).sort(([left], [right]) => {
    const leftRank = CREW_ORDER.indexOf(left);
    const rightRank = CREW_ORDER.indexOf(right);
    return (
      (leftRank < 0 ? CREW_ORDER.length : leftRank) - (rightRank < 0 ? CREW_ORDER.length : rightRank)
    );
  });
}

/**
 * Which trailer this film can be offered by, if any.
 *
 * One sitting next to the film on the disk plays from here and is preferred:
 * it needs nobody else's server and nothing loads from outside. One hosted
 * elsewhere is watched where it lives instead.
 */
export interface TrailerOnOffer {
  here: string | null;
  away: string | null;
}

function trailerToOffer(work: Work | null): TrailerOnOffer {
  const here = work?.trailers.find((one) => one.url)?.url ?? null;
  return {
    here,
    away: here ? null : (work?.trailers.find((one) => one.remote_url)?.remote_url ?? null),
  };
}

/** What is being watched right now, when something is. */
export interface Watching {
  source: string;
  fromTheStart: boolean;
}

/** Everything the screen of one film is handed to draw itself and be driven by. */
export interface WorkScreen {
  /** The film, or nothing until the server has said. */
  work: Work | null;
  /** Why it could not be shown: this film is not there, or the server is not
      answering. Two different things to say and two different things to do. */
  failed: "not_found" | "unreachable" | null;
  /** Which copy of the film the viewer is looking at, out of the ones held. */
  chosen: number;
  choose: (which: number) => void;
  version: Version | undefined;
  /** Where this viewer stopped, when they did. */
  resumeFrom: number | null;
  /** Read the film again, which is what a match chosen by hand asks for. */
  readAgain: () => void;
  /** What is being watched, and how to start and stop it. */
  playing: Watching | null;
  play: (source: string, fromTheStart: boolean) => void;
  stopPlaying: () => void;
  /** Which trailer the film can be offered by. */
  onOffer: TrailerOnOffer;
  /** The trailer being watched here, when one is. */
  trailer: string | null;
  watchTrailer: (url: string) => void;
  stopTrailer: () => void;
  /** Where to go when this one ends, so an episode is followed by the next.
      Absent for anything with nothing after it. */
  andThen: string | null;
  /** Where to go to step back into the episode before this one. Absent for
      anything that is not an episode, and for the first one of a series. */
  goBack: string | null;
}

export function useWorkScreen(id: string | undefined): WorkScreen {
  const [address, setAddress] = useSearchParams();
  const [chosen, setChosen] = useState(0);
  const [playing, setPlaying] = useState<Watching | null>(null);
  const [trailer, setTrailer] = useState<string | null>(null);
  const [again, setAgain] = useState(0);

  const asked = useAsked(
    (signal) => (id ? api.work(id, signal) : Promise.resolve(null)),
    [id, again],
  );

  /* A different film is a different copy list, so the copy being looked at
     goes back to the first one. */
  useEffect(() => {
    setChosen(0);
  }, [id, again]);

  /* Nothing while a different film is being asked about, rather than the one
     before it: a screen that keeps the last film on it for half a second is a
     screen that shows the wrong title, the wrong poster and the wrong copies
     to whoever clicked. */
  const work = asked.waiting ? null : asked.answer;
  const version = work?.versions[chosen];

  const resume = useAsked(
    (signal) =>
      version && !version.missing
        ? api.plan(version.id, {}, signal).then((plan) => plan.resume_from_seconds)
        : Promise.resolve(null),
    [work, chosen],
  );

  /* Started once and only once. The address is cleared as soon as it is
     acted on, so coming back to this page later does not start the film
     again, and the hand is what stops a screen drawn twice from starting
     twice before the address has been cleared. */
  const started = useRef(false);
  useEffect(() => {
    if (!address.has(START_AT_ONCE) || started.current || !version || version.missing) {
      return;
    }
    started.current = true;
    setPlaying({ source: version.id, fromTheStart: false });
    const rest = new URLSearchParams(address);
    rest.delete(START_AT_ONCE);
    setAddress(rest, { replace: true });
  }, [address, setAddress, version]);

  /* A different work is a different film to start, so the one press the
     address carried is spent and a new one may be honoured. */
  useEffect(() => {
    started.current = false;
  }, [id]);

  const readAgain = useCallback(() => setAgain((count) => count + 1), []);
  const play = useCallback(
    (source: string, fromTheStart: boolean) => setPlaying({ source, fromTheStart }),
    [],
  );
  const stopPlaying = useCallback(() => setPlaying(null), []);
  const watchTrailer = useCallback((url: string) => setTrailer(url), []);
  const stopTrailer = useCallback(() => setTrailer(null), []);

  return {
    work,
    failed: asked.failure && (asked.failure.code === "not_found" ? "not_found" : "unreachable"),
    chosen,
    choose: setChosen,
    version,
    /* A copy that is not on the disk cannot be resumed, and a reading that
       failed says nothing about where anybody stopped. */
    resumeFrom: resume.failure ? null : resume.answer,
    readAgain,
    playing,
    play,
    stopPlaying,
    onOffer: trailerToOffer(work),
    trailer,
    watchTrailer,
    stopTrailer,
    /* Only after an episode: a film that ends is a film that ended, and a
       season has nothing playing to follow. */
    andThen:
      work?.kind === "episode" && work.carry_on_with
        ? `/work/${work.carry_on_with.id}?${START_AT_ONCE}=1`
        : null,
    goBack:
      work?.kind === "episode" && work.previous_episode
        ? `/work/${work.previous_episode.id}?${START_AT_ONCE}=1`
        : null,
  };
}

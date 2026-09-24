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
import { useNavigate, useSearchParams } from "react-router-dom";
import { api } from "../api";
import type { PlaybackPlan, Version, Work } from "../api";
import { useAsked } from "../asking";
import type { Trailer } from "../components/trailer";
import { embedOf } from "../trailers";

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
 * One sitting next to the film on the disk is preferred: it needs nobody
 * else's server. Failing that, one on a site that offers its player to other
 * pages plays here too, in that player. Any other link is watched where it
 * lives.
 */
export interface TrailerOnOffer {
  here: Trailer | null;
  away: string | null;
}

function trailerToOffer(work: Work | null): TrailerOnOffer {
  const file = work?.trailers.find((one) => one.url)?.url ?? null;
  if (file) {
    return { here: { file }, away: null };
  }
  const links = (work?.trailers ?? []).flatMap((one) => (one.remote_url ? [one.remote_url] : []));
  const embed = links.map(embedOf).find((address) => address !== null) ?? null;
  return embed ? { here: { embed }, away: null } : { here: null, away: links[0] ?? null };
}

/** What is being watched right now, when something is. */
export interface Watching {
  source: string;
  fromTheStart: boolean;
  /** Where to start, in seconds, when a moment was chosen by hand: a
      chapter. */
  at?: number;
}

/** The soundtrack and the subtitle a film will start with. */
export interface Tracks {
  audio: string | null;
  subtitle: string | null;
}

/** Enough of an episode to step straight to it: a `NextEpisode` and a `Child`
 *  both carry this much, and stepping to one asks for nothing more. */
export interface Playable {
  id: string;
  source_id: string | null;
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
  /** What playing the chosen copy would be, asked before anything plays: its
      tracks, the ones it would start with, its chapters and the little
      pictures of them. Nothing until it has answered, and for a copy that is
      not on the disk. */
  plan: PlaybackPlan | null;
  /** The tracks the film will start with, the viewer's own choice winning
      the moment it is made. */
  tracks: Tracks;
  /** Chooses them, and remembers the choice the way the player does, so the
      player opens with it. */
  chooseTracks: (tracks: Tracks) => void;
  /** Read the film again, which is what a match chosen by hand asks for. */
  readAgain: () => void;
  /** What is being watched, and how to start and stop it. */
  playing: Watching | null;
  /** True while this page was opened only in order to play, and the film has
      not started yet. The page under it is not drawn at all during that
      moment: it is a page nobody asked for, and it showed for long enough to
      be seen every time a film was started from a card. */
  openingToPlay: boolean;
  play: (source: string, fromTheStart: boolean, at?: number) => void;
  stopPlaying: () => void;
  /** Which trailer the film can be offered by. */
  onOffer: TrailerOnOffer;
  /** The trailer being watched here, when one is. */
  trailer: Trailer | null;
  watchTrailer: (trailer: Trailer) => void;
  stopTrailer: () => void;
  /** Steps straight to the episode after this one, playing it at once: what
      an episode ending on its own asks for, and what the button beside play
      asks for by hand. Absent for anything that is not an episode, and for
      the last one of a series. */
  nextEpisode: (() => void) | null;
  /** The same, a step back into the episode before this one. Absent for the
      first one of a series. */
  previousEpisode: (() => void) | null;
  /** Steps straight to any other episode, the way `nextEpisode` and
      `previousEpisode` do: what the "up next" row asks for when a hand
      chooses one out of order rather than the one right after or before. */
  playEpisode: (episode: Playable) => void;
}

export function useWorkScreen(id: string | undefined): WorkScreen {
  const navigate = useNavigate();
  const [address, setAddress] = useSearchParams();
  const [chosen, setChosen] = useState(0);
  const [playing, setPlaying] = useState<Watching | null>(null);
  const [trailer, setTrailer] = useState<Trailer | null>(null);
  const [again, setAgain] = useState(0);

  const asked = useAsked(
    (signal) => (id ? api.work(id, signal) : Promise.resolve(null)),
    [id, again],
    id ? `work:${id}` : undefined,
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

  const planned = useAsked(
    (signal) =>
      version && !version.missing ? api.plan(version.id, {}, signal) : Promise.resolve(null),
    [work, chosen],
    version ? `plan:${version.id}` : undefined,
  );
  /* A copy that is not on the disk has nothing to play, and a reading that
     failed says nothing about where anybody stopped or what they chose. */
  const plan = planned.failure ? null : planned.answer;
  const { look: planAgain } = planned;

  /* The choice just made, shown at once and until the plan asked again says
     the same: a list that snaps back to the old track for the length of a
     round trip is a list somebody picks from twice. */
  const [picked, setPicked] = useState<Tracks | null>(null);
  useEffect(() => {
    setPicked(null);
  }, [plan]);
  const tracks: Tracks = picked ?? {
    audio: plan?.chosen_audio_id ?? null,
    subtitle: plan?.chosen_subtitle_id ?? null,
  };
  const chooseTracks = useCallback(
    (wanted: Tracks) => {
      if (!work || !version) {
        return;
      }
      setPicked(wanted);
      api
        .rememberTracks({
          work_id: work.id,
          source_id: version.id,
          audio_track_id: wanted.audio,
          subtitle_track_id: wanted.subtitle,
        })
        .then(planAgain)
        .catch(() => setPicked(null));
    },
    [work, version, planAgain],
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

  /* Set the moment a step to another episode asks for it, and read back only
     once, right below: it says the id is about to change on purpose, with
     the file to play already chosen, rather than by a back button or a link
     that knows nothing about what was playing here. */
  const stepping = useRef(false);

  /* A different work is a different film to start, so the one press the
     address carried is spent and a new one may be honoured.

     What is playing is cleared with it, unless a step to this id already
     said what to play: without that exception, the file chosen a moment ago
     by `stepTo` would be wiped out the instant this runs, since both answer
     to the same change of id. Left to a fetch instead, a step from one
     episode to the next would render this screen once with the new work's
     title and the old work's file still playing, the new work having
     arrived before the address was ever acted on. */
  useEffect(() => {
    started.current = false;
    if (stepping.current) {
      stepping.current = false;
    } else {
      setPlaying(null);
    }
  }, [id]);

  /* Between the address saying "play" and the film being on the screen there
     is a moment where everything needed to start it is still being fetched.
     Nothing of the page is drawn in that moment. It ends the instant the
     film starts, and also the instant it turns out there is nothing to play:
     a blank screen for ever would be worse than a page that flashed. */
  const openingToPlay =
    address.has(START_AT_ONCE) &&
    playing === null &&
    !asked.failure &&
    (work === null || (version !== undefined && !version.missing));

  const readAgain = useCallback(() => setAgain((count) => count + 1), []);
  const play = useCallback(
    (source: string, fromTheStart: boolean, at?: number) =>
      setPlaying({ source, fromTheStart, at }),
    [],
  );
  /* The page under the player was left as it was when the film started, and
     what was watched changed it: where to carry on from, how far the bar
     goes, whether it now counts as seen. Read again without emptying the
     screen, which is still the same film. */
  const { look } = asked;
  const stopPlaying = useCallback(() => {
    setPlaying(null);
    look();
  }, [look]);
  const watchTrailer = useCallback((wanted: Trailer) => setTrailer(wanted), []);
  const stopTrailer = useCallback(() => setTrailer(null), []);

  /* Goes straight to another episode's own page, playing the file already
     named for it rather than waiting for that page to answer for itself:
     what it would answer is the very thing already in hand, and waiting for
     it back is what let the file just left keep playing under the title of
     the one just reached. */
  const stepTo = useCallback(
    (next: Playable) => {
      stepping.current = true;
      navigate(`/work/${next.id}`);
      if (next.source_id) {
        setPlaying({ source: next.source_id, fromTheStart: false });
      }
    },
    [navigate],
  );

  /* Named once so the closures below narrow to `NextEpisode` rather than to
     `NextEpisode | null`: an episode is the only kind either one is ever set
     for, a season and a series have nothing to step to or back from. */
  const nextUp = work?.kind === "episode" ? work.carry_on_with : null;
  const previousUp = work?.kind === "episode" ? work.previous_episode : null;

  return {
    work,
    failed: asked.failure && (asked.failure.code === "not_found" ? "not_found" : "unreachable"),
    chosen,
    choose: setChosen,
    version,
    resumeFrom: plan?.resume_from_seconds ?? null,
    plan,
    tracks,
    chooseTracks,
    readAgain,
    playing,
    openingToPlay,
    play,
    stopPlaying,
    onOffer: trailerToOffer(work),
    trailer,
    watchTrailer,
    stopTrailer,
    /* Only after an episode: a film that ends is a film that ended, and a
       season has nothing playing to follow. */
    nextEpisode: nextUp ? () => stepTo(nextUp) : null,
    previousEpisode: previousUp ? () => stepTo(previousUp) : null,
    playEpisode: stepTo,
  };
}

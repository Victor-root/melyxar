/*
 * Following the subtitles while a film plays, and telling the journal what
 * the browser holds and shows.
 *
 * What a viewer sees is two things that can disagree: the cues the browser
 * read out of the file, and the ones it decides are on the screen at this
 * moment. A subtitle that does not follow where the film is is one of them
 * being wrong, and only this side can tell which, by setting what is shown
 * beside what the times of the file give for the same instant.
 *
 * Nothing here chooses its own words: the facts are the ones the server names.
 */

import type { PageFact, SubtitleCue } from "../api";

type Fact = Extract<PageFact, { saw: "subtitles_read" | "subtitles_on_screen" }>;

/** How often a track that has not been read yet is looked at. */
const LOOK_EVERY_MS = 500;

/** How long a track is given to be read before it is said to be empty. */
const GIVE_UP_AFTER_MS = 10_000;

/** How many cues are told one by one. */
const MOST_CUES_TOLD = 6;

/** The cues of a track, as plain numbers and words. */
export function cuesOf(track: TextTrack): SubtitleCue[] {
  return Array.from(track.cues ?? []).map((cue) => ({
    start_second: cue.startTime,
    end_second: cue.endTime,
    text: (cue as VTTCue).text ?? "",
  }));
}

/** What a set of cues holds, in the terms of the fact the server names. */
export function whatTheyHold(cues: SubtitleCue[], hidden: boolean): Extract<Fact, { saw: "subtitles_read" }> {
  let outOfOrder = 0;
  let emptyOrBackwards = 0;
  cues.forEach((cue, position) => {
    if (position > 0 && cue.start_second < cues[position - 1].start_second) {
      outOfOrder += 1;
    }
    if (cue.end_second <= cue.start_second) {
      emptyOrBackwards += 1;
    }
  });
  return {
    saw: "subtitles_read",
    cues: cues.length,
    first_start_second: cues.length > 0 ? Math.min(...cues.map((cue) => cue.start_second)) : null,
    last_end_second: cues.length > 0 ? Math.max(...cues.map((cue) => cue.end_second)) : null,
    out_of_order: outOfOrder,
    empty_or_backwards: emptyOrBackwards,
    hidden,
  };
}

/** The cues the times of the file put on the screen at this moment. */
export function expectedAt(cues: SubtitleCue[], second: number): SubtitleCue[] {
  return cues.filter((cue) => cue.start_second <= second && second < cue.end_second);
}

/**
 * Follows every track of the film, now and as they are added, and returns what
 * stops it.
 */
export function followTheSubtitles(
  element: HTMLVideoElement,
  say: (fact: Fact) => void,
): () => void {
  const followed = new Map<TextTrack, () => void>();

  const sayWhatIsShown = (track: TextTrack, why: "changed" | "jumped") => {
    const cues = cuesOf(track);
    const shown = Array.from(track.activeCues ?? []).map((cue) => ({
      start_second: cue.startTime,
      end_second: cue.endTime,
      text: (cue as VTTCue).text ?? "",
    }));
    say({
      saw: "subtitles_on_screen",
      why,
      at_second: element.currentTime,
      cues: cues.length,
      shown: shown.slice(0, MOST_CUES_TOLD),
      expected: expectedAt(cues, element.currentTime).slice(0, MOST_CUES_TOLD),
    });
  };

  const follow = (track: TextTrack) => {
    if (followed.has(track) || (track.kind !== "subtitles" && track.kind !== "captions")) {
      return;
    }
    const changed = () => sayWhatIsShown(track, "changed");
    track.addEventListener("cuechange", changed);

    let waited = 0;
    const looking = window.setInterval(() => {
      waited += LOOK_EVERY_MS;
      if ((track.cues?.length ?? 0) > 0 || waited >= GIVE_UP_AFTER_MS) {
        window.clearInterval(looking);
        say(whatTheyHold(cuesOf(track), track.mode !== "showing"));
      }
    }, LOOK_EVERY_MS);

    followed.set(track, () => {
      window.clearInterval(looking);
      track.removeEventListener("cuechange", changed);
    });
  };

  const added = (event: TrackEvent) => {
    if (event.track instanceof TextTrack) {
      follow(event.track);
    }
  };
  const jumped = () => followed.forEach((_stop, track) => sayWhatIsShown(track, "jumped"));

  Array.from(element.textTracks).forEach(follow);
  element.textTracks.addEventListener("addtrack", added);
  element.addEventListener("seeked", jumped);

  return () => {
    element.textTracks.removeEventListener("addtrack", added);
    element.removeEventListener("seeked", jumped);
    followed.forEach((stop) => stop());
    followed.clear();
  };
}

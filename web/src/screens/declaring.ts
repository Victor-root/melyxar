/*
 * What declaring a library, and putting one right, is driven by.
 *
 * The screen this belongs to replaces opening the configuration file on the
 * server. Its whole job is to gather four things: a name, what kind of thing
 * the library holds, the language its films are described in, and the folders
 * it looks in. None of that is drawing, and a form drawn another way tomorrow
 * needs every rule here unchanged.
 *
 * Everything the server refuses comes back as a word rather than a sentence,
 * so it can be shown where it was typed: a form that says "the server would
 * not" sends somebody back to try the same thing again.
 */

import { useCallback, useState } from "react";
import { api } from "../api";
import type { Library, LibraryChoices, LibraryKind, WouldGo } from "../api";
import { refusalAbout, useAsked, useTold } from "../asking";
import { NO_MUSIC_OPTIONS, music } from "../music/api";
import type { MusicLibraryOptions } from "../music/api";

/** Turns whatever the server refused about a library into a sentence's key. */
export function refusal(error: unknown): string {
  return refusalAbout(error, "library");
}

/**
 * What the last change came to, when it came to something worth saying.
 *
 * Kept as what happened rather than as a sentence: a language changed sets a
 * run going on every film of the library, and a library taken away takes a
 * count of films and files with it. Wording either is the screen's business.
 */
export type Outcome =
  | { kind: "asked_about_again"; count: number }
  | { kind: "removal_done"; works: number; files: number }
  | null;

/** Putting an existing library right, one piece at a time. */
export interface LibraryEditing {
  /** Why the server refused the last change, as a sentence's key. */
  refused: string | null;
  /** What the last change came to. Somebody who just changed the language of
      a library is owed its size. */
  outcome: Outcome;
  /** Said by a part of the form that finished something on its own. */
  report: (outcome: Outcome) => void;
  /** The three settings of a library travel together, because they are one
      answer to one question: sending half would leave the other half to be
      guessed at, and the guess would be wrong every other time. */
  settle: (library: Library, changes: Partial<Library>) => void;
  addFolderTo: (library: string, path: string) => Promise<void>;
  rename: (library: Library, name: string) => Promise<void>;
  renameFolder: (library: Library, root: string, label: string) => Promise<void>;
  /** Said by a part of the form that refuses something on its own. */
  refuse: (key: string) => void;
}

export function useLibraryEditing(onChanged: () => void): LibraryEditing {
  const [refused, setRefused] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<Outcome>(null);

  const settle = (library: Library, changes: Partial<Library>) => {
    const wanted = { ...library, ...changes };
    setRefused(null);
    api
      .setLibraryOptions(library.id, {
        extract_subtitles: wanted.extract_subtitles,
        make_thumbnails: wanted.make_thumbnails,
        detect_openings: wanted.detect_openings,
        process_on_arrival: wanted.process_on_arrival,
        watch_in_real_time: wanted.watch_in_real_time,
        keeps_resume_points: wanted.keeps_resume_points,
        keeps_watched_marks: wanted.keeps_watched_marks,
        metadata_language: wanted.metadata_language,
      })
      .then((kept) => {
        setOutcome(
          kept.asked_about_again === null
            ? null
            : { kind: "asked_about_again", count: kept.asked_about_again },
        );
        onChanged();
      })
      .catch((error) => {
        setRefused(refusal(error));
        onChanged();
      });
  };

  const addFolderTo = async (library: string, path: string) => {
    setRefused(null);
    try {
      await api.addRoot(library, path);
      onChanged();
    } catch (error) {
      setRefused(refusal(error));
    }
  };

  const rename = async (library: Library, name: string) => {
    if (name.trim() === library.name) {
      return;
    }
    setRefused(null);
    try {
      await api.renameLibrary(library.id, name);
      onChanged();
    } catch (error) {
      setRefused(refusal(error));
      onChanged();
    }
  };

  const renameFolder = async (library: Library, root: string, label: string) => {
    setRefused(null);
    try {
      await api.renameRoot(library.id, root, label);
      onChanged();
    } catch (error) {
      setRefused(refusal(error));
      onChanged();
    }
  };

  return {
    refused,
    outcome,
    report: setOutcome,
    settle,
    addFolderTo,
    rename,
    renameFolder,
    refuse: setRefused,
  };
}

/** What taking something away would cost, and the taking away itself. */
export interface Removing {
  /** What would go with it, or nothing until the server has counted. */
  going: WouldGo | null;
  /** Why the count could not be had, shown where it was asked for rather than
      at the top of the screen: nobody has asked for anything yet, so there is
      nothing to put right anywhere else. */
  counting: string | null;
  /** Whether it is being taken away right now. */
  busy: boolean;
  goAhead: () => Promise<void>;
}

export function useRemoving(
  library: Library,
  root: string | undefined,
  onDone: (went: WouldGo) => void,
  onRefused: (key: string) => void,
): Removing {
  const asked = useAsked(
    (signal) =>
      root
        ? api.whatRemovingAFolderTakes(library.id, root, signal)
        : api.whatRemovingTakes(library.id, signal),
    [library.id, root],
  );

  const told = useTold(async () => {
    try {
      onDone(root ? await api.removeRoot(library.id, root) : await api.removeLibrary(library.id));
    } catch (error) {
      onRefused(refusal(error));
    }
  });

  return {
    going: asked.answer,
    counting: asked.failure && refusal(asked.failure),
    busy: told.busy,
    goAhead: () => told.tell(),
  };
}

/** What a library does until somebody chooses otherwise, the same as the
 *  server's: every heavy reading wanted and left to the night, the folders
 *  left unwatched, and every play remembered. */
const USUAL_CHOICES: LibraryChoices = {
  extract_subtitles: true,
  make_thumbnails: true,
  detect_openings: true,
  process_on_arrival: false,
  watch_in_real_time: false,
  keeps_resume_points: true,
  keeps_watched_marks: true,
};

/** A library being declared, and what it is made of so far. */
export interface Declaring {
  name: string;
  setName: (name: string) => void;
  kind: LibraryKind;
  setKind: (kind: LibraryKind) => void;
  /** The language the films are described in. The interface language to begin
      with: somebody who reads this in French is the likeliest to want their
      films described in it. */
  metadata: string;
  setMetadata: (language: string) => void;
  /** What it will do with its files and with each play, the usual choices
      to begin with. */
  choices: LibraryChoices;
  choose: (changes: Partial<LibraryChoices>) => void;
  /** What a library of music does beyond that, chosen from the start. */
  musicOptions: MusicLibraryOptions;
  setMusicOptions: (options: MusicLibraryOptions) => void;
  roots: string[];
  addRoot: (path: string) => void;
  dropRoot: (path: string) => void;
  busy: boolean;
  create: () => Promise<void>;
}

export function useDeclaring(
  language: string,
  onDone: () => void,
  onRefused: (key: string) => void,
  /** What a kind is called, which a library left unnamed takes for a name. */
  nameOf: (kind: LibraryKind) => string,
): Declaring {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<LibraryKind>("movies");
  const [metadata, setMetadata] = useState<string>(language);
  const [choices, setChoices] = useState<LibraryChoices>(USUAL_CHOICES);
  const choose = useCallback(
    (changes: Partial<LibraryChoices>) => setChoices((was) => ({ ...was, ...changes })),
    [],
  );
  const [musicOptions, setMusicOptions] = useState<MusicLibraryOptions>(NO_MUSIC_OPTIONS);
  const [roots, setRoots] = useState<string[]>([]);

  const told = useTold(async () => {
    try {
      const made = await api.createLibrary({
        name: name.trim() || nameOf(kind),
        kind,
        metadata_language: metadata,
        roots,
        options: choices,
      });
      if (kind === "music") {
        await music.setLibraryOptions(made.id, musicOptions);
      }
      onDone();
    } catch (error) {
      onRefused(refusal(error));
    }
  });

  const addRoot = useCallback(
    (path: string) => setRoots((was) => (was.includes(path) ? was : [...was, path])),
    [],
  );
  const dropRoot = useCallback(
    (path: string) => setRoots((was) => was.filter((one) => one !== path)),
    [],
  );

  return {
    name,
    setName,
    kind,
    setKind,
    metadata,
    setMetadata,
    choices,
    choose,
    musicOptions,
    setMusicOptions,
    roots,
    addRoot,
    dropRoot,
    busy: told.busy,
    create: () => told.tell(),
  };
}

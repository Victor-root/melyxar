/*
 * What an account may do and see, kept consistent as it is changed on screen.
 *
 * The server decides, and settles whatever it is sent the same way: this is
 * only so the page shows at once what the server will answer, rather than a
 * switch that jumps back a moment later.
 */

import type { Library, Rights } from "../../api";
import type { Wording } from "../../readable";

/** The most simultaneous streams the page offers. The server accepts up to
 *  twenty; nobody in a household means more than this. */
export const MOST_STREAMS_OFFERED = 10;

/** What a new account starts with: every library, nothing else. */
export const AN_ORDINARY_ACCOUNT: Rights = {
  is_administrator: false,
  sees_every_library: true,
  libraries: [],
  may_delete: false,
  may_delete_from_disk: false,
  most_streams: null,
};

/**
 * These rights as the server keeps them.
 *
 * An administrator holds every right and sees every library. Erasing from the
 * disk goes with the right to delete. An account that sees every library
 * holds no list, and a library granted twice is granted once.
 */
export function settled(rights: Rights): Rights {
  if (rights.is_administrator) {
    return {
      is_administrator: true,
      sees_every_library: true,
      libraries: [],
      may_delete: true,
      may_delete_from_disk: true,
      most_streams: null,
    };
  }
  return {
    ...rights,
    libraries: rights.sees_every_library ? [] : [...new Set(rights.libraries)],
    may_delete_from_disk: rights.may_delete && rights.may_delete_from_disk,
  };
}

/** The same rights with one library granted or taken away. */
export function granting(rights: Rights, library: string, granted: boolean): Rights {
  const others = rights.libraries.filter((one) => one !== library);
  return settled({ ...rights, libraries: granted ? [...others, library] : others });
}

/** What an account reaches, in a few words: every library, the ones it was
 *  granted by name, or none at all. */
export function reachOf(rights: Rights, libraries: Library[], t: Wording): string {
  if (rights.sees_every_library) {
    return t("users.every_library");
  }
  const names = libraries
    .filter((library) => rights.libraries.includes(library.id))
    .map((library) => library.name);
  return names.length > 0 ? names.join(", ") : t("users.no_library_short");
}

/*
 * The shortcuts a long press on the Android application's icon offers, said
 * by the page to the application: the page knows the account's language,
 * whether it is an administrator's and which libraries it reaches, and the
 * application none of it. Absent in a browser, and in an application older
 * than this page.
 */

import type { Library } from "./api";
import type { Wording } from "./readable";

/** One shortcut: which, what it says, and the page it opens. */
export interface Shortcut {
  id: "music" | "dashboard" | "settings" | "link";
  label: string;
  path: string;
}

interface AppShortcuts {
  /** The shortcuts, as JSON, in the order they are offered. */
  offer(shortcuts: string): void;
}

declare global {
  interface Window {
    MelyxarShortcuts?: AppShortcuts;
  }
}

/** The shortcuts of this account: its music when it reaches some, the
 *  administration's summary for an administrator, its settings, and letting
 *  a television in by its QR code. */
export function shortcutsFor(libraries: Library[], administrator: boolean, t: Wording): Shortcut[] {
  const music = libraries.find((library) => library.kind === "music");
  const all: (Shortcut | null)[] = [
    music ? { id: "music", label: t("shortcut.music"), path: `/library/${music.id}` } : null,
    administrator ? { id: "dashboard", label: t("shortcut.dashboard"), path: "/admin" } : null,
    { id: "settings", label: t("shortcut.settings"), path: "/settings" },
    { id: "link", label: t("shortcut.link"), path: "/link?scan" },
  ];
  return all.filter((shortcut): shortcut is Shortcut => shortcut !== null);
}

/** Hands the shortcuts to the application, when there is one. */
export function offerShortcuts(shortcuts: Shortcut[]): void {
  window.MelyxarShortcuts?.offer(JSON.stringify(shortcuts));
}

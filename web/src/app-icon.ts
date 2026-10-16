/*
 * The icon Melyxar's Android application shows in the phone's launcher, when
 * this page runs inside it. The application carries a few and the page
 * chooses among them; the choice belongs to the phone, not to the account.
 *
 * Absent in a browser, where nothing here is used.
 */

/** The icons the application carries. */
export type LauncherIcon = "light" | "dark" | "clear";

export interface AppIcon {
  /** The icon shown now. */
  current(): LauncherIcon;
  /** Shows this icon in the launcher in place of the current one. */
  choose(icon: LauncherIcon): void;
}

declare global {
  interface Window {
    MelyxarIcon?: AppIcon;
  }
}

/** The application's icon, or nothing in a browser. */
export const appIcon: AppIcon | null =
  typeof window !== "undefined" ? (window.MelyxarIcon ?? null) : null;

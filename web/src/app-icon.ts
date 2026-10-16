/*
 * The icon Melyxar's Android application shows in the phone's launcher, when
 * this page runs inside it. The application carries the logo on a few grounds,
 * each in every accent offered, and the page chooses the ground and whether
 * the logo follows the account's accent, which it hands on as it changes. The
 * choice belongs to the phone, not to the account.
 *
 * Absent in a browser, where nothing here is used.
 */

/** The grounds the logo can sit on. */
export type LauncherGround = "light" | "dark" | "clear";

export interface AppIcon {
  /** The ground shown now. */
  ground(): LauncherGround;
  /** Whether the logo wears the account's accent rather than the usual red. */
  follows(): boolean;
  /** Shows the logo on this ground, in the account's accent or not. */
  choose(ground: LauncherGround, follows: boolean): void;
  /** The account's accent, given whenever it is known or changes. */
  wear(accent: string): void;
}

declare global {
  interface Window {
    MelyxarIcon?: AppIcon;
  }
}

/** The application's icon, or nothing in a browser. */
export const appIcon: AppIcon | null =
  typeof window !== "undefined" ? (window.MelyxarIcon ?? null) : null;

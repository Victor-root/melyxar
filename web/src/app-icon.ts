/*
 * The icon Melyxar's Android application shows in the phone's launcher, when
 * this page runs inside it. The application carries the logo on a few grounds,
 * each in every accent offered, and the page chooses the ground and whether
 * the logo wears the account's accent. Changing it closes the application on
 * many phones, so it is applied only when asked, after a warning. The choice
 * belongs to the phone, not to the account.
 *
 * Absent in a browser, where nothing here is used.
 */

/** The grounds the logo can sit on. */
export type LauncherGround = "light" | "dark" | "clear";

export interface AppIcon {
  /** The ground shown now. */
  ground(): LauncherGround;
  /** Whether the logo shown wears the account's accent rather than the
   *  usual red. */
  follows(): boolean;
  /** Whether applying these would show another icon than the one shown. */
  differs(ground: LauncherGround, follows: boolean, accent: string): boolean;
  /** Keeps whether the logo follows the account, and shows the logo on
   *  this ground, in this accent or in the usual red, which closes the
   *  application on many phones when it is another icon. */
  apply(ground: LauncherGround, follows: boolean, accent: string): void;
}

declare global {
  interface Window {
    MelyxarIcon?: AppIcon;
  }
}

/** The application's icon, or nothing in a browser. */
export const appIcon: AppIcon | null =
  typeof window !== "undefined" ? (window.MelyxarIcon ?? null) : null;

/*
 * The theme, turned with one press: light to dark and back. Offered wherever
 * somebody is before the settings are in reach: the door and the first steps.
 *
 * Every press changes what is on the screen. It used to go round automatic,
 * light and dark, and automatic always looks like one of the other two, so one
 * press in three did nothing anybody could see and the button looked broken.
 * Going back to automatic is in the settings. On a phone with a theme of its
 * own, it is that one that turns: turning the larger screens' changed nothing
 * there either.
 */

import { themeSeen, useSettings } from "../settings";
import type { ThemeChoice } from "../settings";

export function ThemeToggle() {
  const { t, shownTheme, setShownTheme } = useSettings();
  return (
    <button
      type="button"
      className="theme-toggle"
      onClick={() => setShownTheme(themeSeen(shownTheme) === "light" ? "dark" : "light")}
    >
      <ThemeIcon theme={shownTheme} />
      {t("door.theme_toggle")}
    </button>
  );
}

/** The theme in force, drawn rather than named: a sun, a moon, or the two
 *  halved for whichever one automatic currently means. */
function ThemeIcon({ theme }: { theme: ThemeChoice }) {
  if (theme === "light") {
    return (
      <svg
        className="theme-toggle-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.8}
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="4.3" />
        <path d="M12 2.6v2.7M12 18.7v2.7M4.1 12H1.4M22.6 12h-2.7M5.4 5.4l1.9 1.9M16.7 16.7l1.9 1.9M18.6 5.4l-1.9 1.9M7.3 16.7l-1.9 1.9" />
      </svg>
    );
  }
  if (theme === "dark") {
    return (
      <svg
        className="theme-toggle-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.8}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M20.2 13.4A8.4 8.4 0 1 1 10.6 3.8a6.7 6.7 0 0 0 9.6 9.6Z" />
      </svg>
    );
  }
  return (
    <svg
      className="theme-toggle-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="8.4" />
      <path d="M12 3.6a8.4 8.4 0 0 1 0 16.8Z" fill="currentColor" stroke="none" />
    </svg>
  );
}

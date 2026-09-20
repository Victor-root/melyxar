/*
 * What the viewer chose: the language, the theme, and the colour of what is
 * active.
 *
 * Two copies of it on purpose, and the account is the one that counts. The
 * browser keeps what was last chosen so that the door is drawn in the right
 * language before anybody is known, and so that a server that is slow to
 * answer does not show the wrong theme for a moment. The account carries it
 * across machines, which is the whole reason it is not simply kept here.
 *
 * Saving is done without waiting and without complaining: what has already
 * happened on the screen is the answer, and a preference that did not reach
 * the server is a preference this browser still holds and sends again the
 * next time one is changed.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { api } from "./api";
import type { ViewerPreferences } from "./api";
import { initialLanguage, rememberLanguage, safeRead, safeWrite, translate } from "./i18n";
import type { Language } from "./i18n";

export type ThemeChoice = "dark" | "light" | "system";

const STORED_THEME = "melyxar.theme";
const STORED_ACCENT = "melyxar.accent";

/** The red of the Melyxar theme, which needs none of the work below. */
const THE_USUAL_ACCENT = "#c81e1e";

interface Settings {
  language: Language;
  setLanguage: (language: Language) => void;
  theme: ThemeChoice;
  setTheme: (theme: ThemeChoice) => void;
  /** What the account chose, once the server has said. */
  adopt: (chosen: ViewerPreferences) => void;
  /** The wording of one key, in the language in force. */
  t: (key: string, values?: Record<string, string | number>) => string;
}

const SettingsContext = createContext<Settings | null>(null);

function initialTheme(): ThemeChoice {
  const stored = safeRead(STORED_THEME);
  return stored === "dark" || stored === "light" || stored === "system" ? stored : "system";
}

function initialAccent(): string {
  return safeRead(STORED_ACCENT) ?? THE_USUAL_ACCENT;
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [language, setLanguageState] = useState<Language>(initialLanguage);
  const [theme, setThemeState] = useState<ThemeChoice>(initialTheme);
  const [accent, setAccentState] = useState<string>(initialAccent);

  // The theme is put on the document rather than passed down, so a stylesheet
  // can answer it without a single component knowing a colour.
  useEffect(() => {
    const root = document.documentElement;
    const apply = () => {
      const system = window.matchMedia("(prefers-color-scheme: light)").matches
        ? "light"
        : "dark";
      root.dataset.theme = theme === "system" ? system : theme;
    };
    apply();

    if (theme !== "system") {
      return;
    }
    const watcher = window.matchMedia("(prefers-color-scheme: light)");
    watcher.addEventListener("change", apply);
    return () => watcher.removeEventListener("change", apply);
  }, [theme]);

  // The same for the accent, and only when it is not the one the theme was
  // drawn around: the four shades of that one were measured, and working them
  // out again would move them for no reason.
  useEffect(() => {
    const root = document.documentElement;
    if (accent === THE_USUAL_ACCENT) {
      delete root.dataset.accent;
      root.style.removeProperty("--accent");
      root.style.removeProperty("--accent-contrast");
      return;
    }
    root.dataset.accent = "chosen";
    root.style.setProperty("--accent", accent);
    root.style.setProperty("--accent-contrast", readableOn(accent));
  }, [accent]);

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const setLanguage = useCallback((next: Language) => {
    rememberLanguage(next);
    setLanguageState(next);
    tellTheServer({ interface_language: next });
  }, []);

  const setTheme = useCallback((next: ThemeChoice) => {
    safeWrite(STORED_THEME, next);
    setThemeState(next);
    tellTheServer({ theme_mode: next });
  }, []);

  const adopt = useCallback((chosen: ViewerPreferences) => {
    const language = chosen.interface_language === "fr" ? "fr" : "en";
    rememberLanguage(language);
    setLanguageState(language);

    const mode = chosen.theme_mode;
    const theme: ThemeChoice =
      mode === "dark" || mode === "light" || mode === "system" ? mode : "system";
    safeWrite(STORED_THEME, theme);
    setThemeState(theme);

    safeWrite(STORED_ACCENT, chosen.accent_color);
    setAccentState(chosen.accent_color);
  }, []);

  const value = useMemo<Settings>(
    () => ({
      language,
      setLanguage,
      theme,
      setTheme,
      adopt,
      t: (key, values) => translate(language, key, values),
    }),
    [language, setLanguage, theme, setTheme, adopt],
  );

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

export function useSettings(): Settings {
  const settings = useContext(SettingsContext);
  if (!settings) {
    throw new Error("the settings were asked for outside their provider");
  }
  return settings;
}

/**
 * Tells the server what was chosen, without waiting and without complaining.
 *
 * Refused for anybody who is not signed in, which is the ordinary answer on
 * the door and not a fault: the browser has already kept it, and it will be
 * sent again the next time one is changed.
 */
function tellTheServer(changed: Partial<ViewerPreferences>) {
  void api.savePreferences(changed).catch(() => {});
}

/**
 * Black or white on this colour, whichever can be read on it.
 *
 * White on the red of the theme was measured at five and a bit to one, which
 * is comfortable. White on a pale accent somebody picks for themselves is not
 * readable at all, and a button nobody can read is worse than a button of the
 * wrong colour. This is the plain relative brightness the contrast rules are
 * built on, which is enough to choose between two.
 */
function readableOn(colour: string): string {
  const channels = [1, 3, 5].map((at) => parseInt(colour.slice(at, at + 2), 16) / 255);
  const [red, green, blue] = channels.map((one) =>
    one <= 0.03928 ? one / 12.92 : ((one + 0.055) / 1.055) ** 2.4,
  );
  const brightness = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
  return brightness > 0.36 ? "#14161b" : "#ffffff";
}

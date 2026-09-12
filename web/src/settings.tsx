/*
 * What the viewer chose: the language and the theme.
 *
 * Both are remembered, and both have a sensible answer when nothing was ever
 * chosen: the language of the browser, and the theme of the system.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { initialLanguage, rememberLanguage, safeRead, safeWrite, translate } from "./i18n";
import type { Language } from "./i18n";

export type ThemeChoice = "dark" | "light" | "system";

const STORED_THEME = "melyxar.theme";

interface Settings {
  language: Language;
  setLanguage: (language: Language) => void;
  theme: ThemeChoice;
  setTheme: (theme: ThemeChoice) => void;
  /** The wording of one key, in the language in force. */
  t: (key: string, values?: Record<string, string | number>) => string;
}

const SettingsContext = createContext<Settings | null>(null);

function initialTheme(): ThemeChoice {
  const stored = safeRead(STORED_THEME);
  return stored === "dark" || stored === "light" || stored === "system" ? stored : "system";
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [language, setLanguageState] = useState<Language>(initialLanguage);
  const [theme, setThemeState] = useState<ThemeChoice>(initialTheme);

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

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const setLanguage = useCallback((next: Language) => {
    rememberLanguage(next);
    setLanguageState(next);
  }, []);

  const setTheme = useCallback((next: ThemeChoice) => {
    safeWrite(STORED_THEME, next);
    setThemeState(next);
  }, []);

  const value = useMemo<Settings>(
    () => ({
      language,
      setLanguage,
      theme,
      setTheme,
      t: (key, values) => translate(language, key, values),
    }),
    [language, setLanguage, theme, setTheme],
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

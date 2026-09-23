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
const STORED_BANNER_HEIGHT = "melyxar.banner.height";
const STORED_BANNER_CUT = "melyxar.banner.cut";
const STORED_BANNER_WHOLE = "melyxar.banner.whole";
const STORED_HEADER_HIDES = "melyxar.header.hides";

/** The red of the Melyxar theme, which needs none of the work below. */
const THE_USUAL_ACCENT = "#c81e1e";

/** What the banner measures when nobody has moved it, matching the
 *  stylesheet: a little under a third of the screen's width, cut an eighth of
 *  the way down. Held here as well so the banner is the right size on the
 *  very first frame, before the server has said anything. */
const THE_USUAL_BANNER = { height: 0.31, cut: 0.13 };

interface Settings {
  language: Language;
  setLanguage: (language: Language) => void;
  theme: ThemeChoice;
  setTheme: (theme: ThemeChoice) => void;
  /** How tall the banner is, as a share of the screen's width. */
  bannerHeight: number;
  setBannerHeight: (share: number) => void;
  /** Where a band is cut out of a picture, nought at its top, one at its
      foot. */
  bannerCut: number;
  setBannerCut: (share: number) => void;
  /** Whether the banner takes the whole window, the height above then having
      nothing left to decide. */
  bannerFillsTheScreen: boolean;
  setBannerFillsTheScreen: (whole: boolean) => void;
  /** Whether the bar at the top slides away while a page is read down, and
      comes back at the first move up. */
  headerHides: boolean;
  setHeaderHides: (hides: boolean) => void;
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

/** A number this browser kept, or the one the stylesheet already carries. */
function initialNumber(key: string, usual: number): number {
  const stored = Number(safeRead(key));
  return Number.isFinite(stored) && stored > 0 ? stored : usual;
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [language, setLanguageState] = useState<Language>(initialLanguage);
  const [theme, setThemeState] = useState<ThemeChoice>(initialTheme);
  const [accent, setAccentState] = useState<string>(initialAccent);
  const [bannerHeight, setBannerHeightState] = useState(() =>
    initialNumber(STORED_BANNER_HEIGHT, THE_USUAL_BANNER.height),
  );
  const [bannerCut, setBannerCutState] = useState(() =>
    initialNumber(STORED_BANNER_CUT, THE_USUAL_BANNER.cut),
  );
  const [bannerFillsTheScreen, setBannerFillsState] = useState(
    () => safeRead(STORED_BANNER_WHOLE) === "yes",
  );
  const [headerHides, setHeaderHidesState] = useState(
    () => safeRead(STORED_HEADER_HIDES) !== "no",
  );

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

  // The banner writes two numbers onto the document, the way the accent
  // writes a colour: the stylesheet carries the same two, so nothing has to
  // be removed when they are back where they started.
  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--banner-share", (bannerHeight * 100).toFixed(2));
    root.style.setProperty("--where-a-band-is-cut", `${(bannerCut * 100).toFixed(1)}%`);
    // Taking the whole window is a height of its own rather than a share of
    // the width, so it is written over what the stylesheet works out, and
    // taken back off when it is turned off. The whole window really is the
    // whole of it: the banner runs up behind the bar, which floats on it.
    // The dynamic unit rather than the plain one: on a telephone the plain
    // one counts the bars of the browser as screen, and the banner ends up
    // taller than what can be seen.
    if (bannerFillsTheScreen) {
      root.style.setProperty("--hero-height", "100dvh");
    } else {
      root.style.removeProperty("--hero-height");
    }
  }, [bannerHeight, bannerCut, bannerFillsTheScreen]);

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

  const setBannerHeight = useCallback((share: number) => {
    safeWrite(STORED_BANNER_HEIGHT, String(share));
    setBannerHeightState(share);
    tellTheServerOnceTheHandStops({ banner_height: share });
  }, []);

  const setBannerCut = useCallback((share: number) => {
    safeWrite(STORED_BANNER_CUT, String(share));
    setBannerCutState(share);
    tellTheServerOnceTheHandStops({ banner_cut: share });
  }, []);

  const setBannerFillsTheScreen = useCallback((whole: boolean) => {
    safeWrite(STORED_BANNER_WHOLE, whole ? "yes" : "no");
    setBannerFillsState(whole);
    tellTheServer({ banner_fills_the_screen: whole });
  }, []);

  const setHeaderHides = useCallback((hides: boolean) => {
    safeWrite(STORED_HEADER_HIDES, hides ? "yes" : "no");
    setHeaderHidesState(hides);
    tellTheServer({ header_hides_on_scroll: hides });
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

    safeWrite(STORED_BANNER_HEIGHT, String(chosen.banner_height));
    setBannerHeightState(chosen.banner_height);
    safeWrite(STORED_BANNER_CUT, String(chosen.banner_cut));
    setBannerCutState(chosen.banner_cut);
    safeWrite(STORED_BANNER_WHOLE, chosen.banner_fills_the_screen ? "yes" : "no");
    setBannerFillsState(chosen.banner_fills_the_screen);
    safeWrite(STORED_HEADER_HIDES, chosen.header_hides_on_scroll ? "yes" : "no");
    setHeaderHidesState(chosen.header_hides_on_scroll);
  }, []);

  const value = useMemo<Settings>(
    () => ({
      language,
      setLanguage,
      theme,
      setTheme,
      bannerHeight,
      setBannerHeight,
      bannerCut,
      setBannerCut,
      bannerFillsTheScreen,
      setBannerFillsTheScreen,
      headerHides,
      setHeaderHides,
      adopt,
      t: (key, values) => translate(language, key, values),
    }),
    [
      language,
      setLanguage,
      theme,
      setTheme,
      bannerHeight,
      setBannerHeight,
      bannerCut,
      setBannerCut,
      bannerFillsTheScreen,
      setBannerFillsTheScreen,
      headerHides,
      setHeaderHides,
      adopt,
    ],
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
 * The same, once whoever is dragging has let go.
 *
 * A slider answers on every step it travels, so saying it each time would be
 * one request and one row written per pixel crossed. The screen is already
 * right either way: it is drawn from what this browser holds, and only the
 * saving waits. What accumulates in the meantime goes in one request, so the
 * two numbers moved one after the other still travel together.
 */
const STILL_MOVING = 400;
let waiting: Partial<ViewerPreferences> = {};
let soon: ReturnType<typeof setTimeout> | undefined;

function tellTheServerOnceTheHandStops(changed: Partial<ViewerPreferences>) {
  waiting = { ...waiting, ...changed };
  clearTimeout(soon);
  soon = setTimeout(() => {
    const said = waiting;
    waiting = {};
    tellTheServer(said);
  }, STILL_MOVING);
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

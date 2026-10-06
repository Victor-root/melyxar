/*
 * The bar at the top: what you can look for, and who you are.
 *
 * Not one bar but a handful of pieces of glass floating at the top of the
 * screen, with the page passing between them. The bar itself draws nothing at
 * all and takes no press; each piece carries the wash, the blur and the
 * hairline the whole width used to carry. What that buys is a top that reads
 * as a few small things over a picture rather than as a lid closed on it.
 *
 * It holds no list of places. The libraries are reached from the band under
 * the banner, which is wide enough to show what each one is rather than only
 * name it, and the way back to the front page is the name of the server
 * itself, which is where everybody presses anyway. What is left is the
 * search, the tools, what the server is doing, and who is here, and each of
 * those is a piece.
 *
 * The categories the search can be narrowed to are read from the libraries
 * this server really holds rather than written down here. A server with no
 * anime offers no anime, a collection of films spread over four disks is one
 * category rather than four, and nobody has to remember to add one the day a
 * kind of library is invented.
 *
 * What the engine cannot do yet is shown all the same, greyed and saying so.
 * A function that is simply absent is a function nobody knows is coming; one
 * that is there and says when tells the truth about where this is going.
 */

import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  Link,
  NavLink,
  useLocation,
  useNavigate,
  useSearchParams,
} from "react-router-dom";
import type { Card, HeaderButton, Library, LibraryKind } from "../api";
import { api, pictureSet } from "../api";
import { wasAbandoned } from "../asking";
import { useRunning, useStartScan } from "../running";
import { refusalKey } from "../i18n";
import { useAccount } from "../account";
import { Bell, BellLine } from "../notifications/bell";
import { useRequests } from "../requests/store";
import { Dropdown } from "./dropdown";
import { KINDS, nameOfKind } from "../libraries";
import { useBranding } from "../player/logo";
import { offeredButtons } from "../buttons";
import { addressOf, find } from "../findable";
import type { Area, Found } from "../findable";
import { useSettings } from "../settings";
import { isSectioned } from "./sectioned";
import { Face } from "./face";
import { headroomAt } from "../headroom";
import { PHONE, useMediaQuery } from "../media-query";
import type { Headroom } from "../headroom";
import {
  BackIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ClockIcon,
  CollectionIcon,
  PlaylistIcon,
  GearIcon,
  HeartIcon,
  KindIcon,
  LeaveIcon,
  RefreshIcon,
  RequestIcon,
  ScreenCastIcon,
  SearchIcon,
  DashboardIcon,
} from "../icons";
import { ServerMark } from "./server-mark";
import { music } from "../music/api";
import { musicScopeOf, quickLinesOf } from "../music/search";
import type { QuickLine } from "../music/search";

/** How long the buttons of the bar stay unfolded once nobody has touched it. */
const FOLDS_AFTER_MS = 3000;

/**
 * The two keys that reach the search field, written the way this machine
 * writes them.
 *
 * A badge saying Ctrl on a Mac is a badge saying the wrong thing, and the
 * badge is there precisely to be believed.
 */
function theShortcut(): string {
  const apple = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  return apple ? "⌘K" : "Ctrl K";
}

/** One category: a kind of library, and the libraries of that kind. */
interface Category {
  kind: LibraryKind;
  libraries: Library[];
}

/** How many results the field offers on its own, before somebody presses
 *  enter for the rest of them. */
const QUICK_RESULTS = 6;

/** How long a pause in typing has to last before it is read as a question. A
 *  word typed at speed is one question, not five. */
const QUICK_DEBOUNCE_MS = 200;

/** The heads of a page that rise into the band of the bar and keep clear of
 *  its two pieces: the only things that read where those end. */
const HEADS_CLEAR_OF_THE_BAR = ".page-head, .browse-head";

/** The widths at which something rises into the band beside the bar, and
 *  so reads where its pieces end. Narrower, nothing does, and writing it
 *  only had the whole page styled again at the end of every movement of
 *  the bar. */
const HEADS_RISE_INTO_THE_BAND = "(min-width: 1280px)";

/** How long the bar must stand still before the page as a whole is told where
 *  its pieces end. */
const SETTLES_AFTER_MS = 150;

/** The room the folded piece keeps before its arrow, as its own edge does. */
const FOLDED_AIR = 5;

/** Where the full grid of results for these words lives, which is also what
 *  the enter key sends somebody to. */
export function searchAddress(words: string, scope: string): string {
  const asked = new URLSearchParams({ search: words });
  if (scope) {
    asked.set("in", scope);
  }
  return `/search?${asked.toString()}`;
}

/** The scope as the works endpoint reads it, split back out of the one value
 *  the address holds it as. */
export function scopeToBrowse(scope: string): { kind?: LibraryKind; library?: string } {
  if (scope.startsWith("kind:")) {
    return { kind: scope.slice("kind:".length) as LibraryKind };
  }
  if (scope.startsWith("library:")) {
    return { library: scope.slice("library:".length) };
  }
  return {};
}

/** The settings a scope searches instead of the library, if it is one of
 *  them. */
function areaOf(scope: string): Area | null {
  return scope === "area:settings" ? "settings" : scope === "area:admin" ? "admin" : null;
}

/** The scope a page opens the search on: its own settings on the pages of
 *  settings and of the administration, the whole library anywhere else. */
function scopeOfPage(pathname: string): string {
  if (/^\/admin(\/|$)/.test(pathname)) {
    return "area:admin";
  }
  if (/^\/settings(\/|$)/.test(pathname)) {
    return "area:settings";
  }
  return "";
}

/** The categories this server really has, in the order they are offered. */
function categoriesOf(libraries: Library[]): Category[] {
  return KINDS.map((kind) => ({
    kind,
    libraries: libraries.filter((library) => library.kind === kind),
  })).filter((category) => category.libraries.length > 0);
}

export function Header({
  libraries,
  scrolling,
}: {
  libraries: Library[];
  /** The box the page scrolls in, which is what the bar steps aside for. */
  scrolling: React.RefObject<HTMLDivElement | null>;
}) {
  const { t, headerHides, headerHidesOnPhone, headerButtons, buttonsInTheBar } = useSettings();
  const onAPhone = useMediaQuery(PHONE);
  const navigate = useNavigate();
  const location = useLocation();
  const [parameters] = useSearchParams();
  const [query, setQuery] = useState(parameters.get("search") ?? "");
  const words = query.trim();
  const [scope, setScope] = useState(
    () => parameters.get("in") ?? scopeOfPage(location.pathname),
  );
  const area = areaOf(scope);
  /* Stepping into the settings or the administration searches them, and
     stepping out of them the library again: a search on a page of settings
     that answers with films is answering a question nobody asked there. A
     scope chosen by hand elsewhere stays as it was. */
  const pageScope = scopeOfPage(location.pathname);
  useEffect(() => {
    setScope((before) => (pageScope || areaOf(before) ? pageScope : before));
  }, [pageScope]);
  /* Whether the field is out of its magnifier. Open already when the address
     carries a search: landing on a page of results with the words hidden
     inside an icon is a page answering a question nobody can see. */
  const [looking, setLooking] = useState(() => (parameters.get("search") ?? "") !== "");
  /* Held in place on the pages laid out beside a list of sections: the list
     is pinned under the bar, and a bar that slid away would leave a hole of
     its own height over it. */
  const sectioned = isSectioned(location.pathname);
  const out = useHeadroom(scrolling, (onAPhone ? headerHidesOnPhone : headerHides) && !sectioned);
  const field = useRef<HTMLInputElement>(null);
  /* Set by whatever opens the field, so the cursor goes into it once it is
     there: moved into the menu, the field is not drawn at all until then. */
  const reaching = useRef(false);
  const searchForm = useRef<HTMLFormElement>(null);
  const { jobs } = useRunning();
  const { account, leave } = useAccount();
  const administrator = account?.is_administrator === true;
  const { access } = useRequests();
  const branding = useBranding();
  const start = useRef<HTMLDivElement>(null);
  const side = useRef<HTMLDivElement>(null);
  /* Whether the buttons of the bar are put away behind the arrow beside the
     account, which a phone offers and nothing wider draws. */
  const [folded, setFolded] = useState(true);

  /* Unfolded, the buttons put themselves away again after a few seconds of
     nobody touching the bar, unless one of its lists or its field is open. */
  useEffect(() => {
    const piece = side.current;
    if (folded || !piece) {
      return;
    }
    let timer = 0;
    const wait = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        const open = piece.querySelector('[aria-expanded="true"]:not(.header-fold), input:focus');
        if (open) {
          wait();
        } else {
          setFolded(true);
        }
      }, FOLDS_AFTER_MS);
    };
    wait();
    const events = ["pointerdown", "keydown", "focusin", "scroll"] as const;
    for (const type of events) {
      piece.addEventListener(type, wait, { capture: true, passive: true });
    }
    return () => {
      window.clearTimeout(timer);
      for (const type of events) {
        piece.removeEventListener(type, wait, { capture: true });
      }
    };
  }, [folded]);
  /* How much of the right end of the band the piece shows folded: from just
     before the arrow to the end. On a phone the piece is the whole band and
     its glass slides out from there, so nothing is laid out or drawn again
     at each step of the movement. Written on the piece, read by its glass. */
  const fold = useRef<HTMLButtonElement>(null);
  useLayoutEffect(() => {
    const piece = side.current;
    const arrow = fold.current;
    if (!piece || !arrow) {
      return;
    }
    const measure = () => {
      const shown = piece.getBoundingClientRect().right - arrow.getBoundingClientRect().left;
      piece.style.setProperty("--header-folded", `${Math.ceil(shown) + FOLDED_AIR}px`);
    };
    measure();
    const watching = new ResizeObserver(measure);
    watching.observe(piece);
    watching.observe(arrow);
    return () => watching.disconnect();
  }, []);

  /* Where the piece at the left end stops and how wide the one at the right
     end is, written on the page: the first as wide as the server's name and
     logo, the second as the account's name and the search field opened in
     it. What rises into the band between them keeps clear of both rather
     than sliding under either. */
  useLayoutEffect(() => {
    const left = start.current;
    const right = side.current;
    if (!left || !right) {
      return;
    }
    const root = document.documentElement;
    const wide = window.matchMedia(HEADS_RISE_INTO_THE_BAND);
    const write = (target: HTMLElement, startEnd: string, sideWidth: string) => {
      target.style.setProperty("--header-start-end", startEnd);
      target.style.setProperty("--header-side-width", sideWidth);
    };
    /* Said to the heads that read it as the bar moves, and to the page as a
       whole only once it has stopped. Written on the page at every step of the
       search opening, it had every element of the screen styled again on
       every frame: four tenths of a second of a second's animation, measured,
       on a screen where nothing read it. */
    let settle = 0;
    const forget = () => {
      window.clearTimeout(settle);
      for (const target of [root, ...document.querySelectorAll<HTMLElement>(HEADS_CLEAR_OF_THE_BAR)]) {
        target.style.removeProperty("--header-start-end");
        target.style.removeProperty("--header-side-width");
      }
    };
    const measure = () => {
      if (!wide.matches) {
        return;
      }
      const startEnd = `${left.getBoundingClientRect().right}px`;
      const sideWidth = `${right.getBoundingClientRect().width}px`;
      document.querySelectorAll<HTMLElement>(HEADS_CLEAR_OF_THE_BAR).forEach((head) => write(head, startEnd, sideWidth));
      window.clearTimeout(settle);
      settle = window.setTimeout(() => write(root, startEnd, sideWidth), SETTLES_AFTER_MS);
    };
    const reread = () => {
      if (wide.matches) {
        write(root, `${left.getBoundingClientRect().right}px`, `${right.getBoundingClientRect().width}px`);
        measure();
      } else {
        forget();
      }
    };
    reread();
    const watching = new ResizeObserver(measure);
    watching.observe(left);
    watching.observe(right);
    window.addEventListener("resize", measure);
    wide.addEventListener("change", reread);
    return () => {
      watching.disconnect();
      window.removeEventListener("resize", measure);
      wide.removeEventListener("change", reread);
      forget();
    };
  }, []);
  /* How much of the top of the page the bar covers right now, written on the
     page as its width is: all of its band while it is out, none once it has
     stepped aside. What puts something at the top of the screen reads it, so
     that thing lands under the bar rather than behind it. */
  const shown = out || looking;
  useLayoutEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--header-room", shown ? "var(--header-height)" : "0px");
    return () => {
      root.style.removeProperty("--header-room");
    };
  }, [shown]);

  /* A scan is the one thing an administrator needs from wherever they happen
     to be: films were added, a name was corrected, a disk came back. */
  const scan = useStartScan(libraries);

  const categories = categoriesOf(libraries);

  // A slash puts the cursor in the search field, the way every list of things
  // has worked for thirty years, and so does the command key with a K, which
  // is how everything written in the last ten years does it. Either one opens
  // the field first if it is shut, since a cursor put into nothing is a key
  // that did nothing.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const typing =
        target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
      const held = event.metaKey || event.ctrlKey;
      if ((event.key === "/" && !typing) || (held && event.key.toLowerCase() === "k")) {
        event.preventDefault();
        openSearch();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function openSearch() {
    reaching.current = true;
    setLooking(true);
  }

  useLayoutEffect(() => {
    if (looking && reaching.current) {
      reaching.current = false;
      field.current?.focus();
    }
  }, [looking]);

  const ask = () => {
    if (!words) {
      navigate("/");
      return;
    }
    setQuickDismissed(true);
    // Settings have no page of results: the enter key goes to the first one.
    if (area) {
      const first = foundSettings[0];
      if (first) {
        chooseQuickResult();
        navigate(addressOf(first));
      }
      return;
    }
    navigate(searchAddress(words, scope));
  };

  const look = (event: React.FormEvent) => {
    event.preventDefault();
    ask();
  };

  /*
   * The magnifier does whatever there is left to do: open the field, send
   * what is in it, or fold it away again when it is empty.
   *
   * It refuses the focus a press would otherwise give it, which is what keeps
   * the cursor in the field: were the field to lose it first, the fold below
   * would fire and this would find a shut field and open it straight back.
   * Folding it away is therefore also what lets the cursor go, or the next
   * thing typed would go into a field nobody can see.
   */
  const magnifier = () => {
    if (!looking) {
      openSearch();
      return;
    }
    if (words) {
      ask();
      return;
    }
    field.current?.blur();
    setLooking(false);
  };

  /* Folded away again only when it is empty and nobody is in it any more. A
     field with words in it stays open wherever the next press lands, because
     those words are the question whatever is on the screen is answering. */
  const letGo = (event: React.FocusEvent<HTMLFormElement>) => {
    if (words) {
      return;
    }
    const next = event.relatedTarget as Node | null;
    if (next && event.currentTarget.contains(next)) {
      return;
    }
    setLooking(false);
  };

  /*
   * The few results the field offers on its own, right under the bar, while
   * the grid the enter key leads to waits behind a press.
   *
   * Fetched on a short pause rather than on every key: a word typed at speed
   * is one question, not five, and the FTS index this asks of is built for
   * exactly this, prefix and all.
   */
  const [quickResults, setQuickResults] = useState<QuickLine[] | null>(null);
  /* Put aside by a press outside, the enter key, or a result chosen: what it
     answers is done with, and typing again is what asks it back. */
  const [quickDismissed, setQuickDismissed] = useState(false);
  const quickPanel = useRef<HTMLDivElement>(null);
  const [quickUnder, setQuickUnder] = useState({ top: 0, left: 0 });

  useEffect(() => setQuickDismissed(false), [words, scope]);

  useEffect(() => {
    if (!looking || !words || area) {
      setQuickResults(null);
      return;
    }
    const controller = new AbortController();
    const inMusic = musicScopeOf(scope, libraries);
    const timer = window.setTimeout(() => {
      Promise.all([
        inMusic.only
          ? []
          : api
              .works({ search: words, limit: QUICK_RESULTS, ...scopeToBrowse(scope) }, controller.signal)
              .then((page) => page.cards.map(quickLineOf)),
        inMusic.looks
          ? music
              .search(words, inMusic.library, controller.signal)
              .then((found) => quickLinesOf(found, QUICK_RESULTS))
          : [],
      ])
        .then(([films, songs]) => setQuickResults([...films, ...songs]))
        .catch((error) => {
          if (!wasAbandoned(error)) {
            setQuickResults([]);
          }
        });
    }, QUICK_DEBOUNCE_MS);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [words, scope, looking, area, libraries]);

  /* The settings are found here and at once: a few hundred names in memory
     need no server and no pause. */
  const foundSettings: Found[] = area ? find(words, area, t, QUICK_RESULTS) : [];

  const quickOpen =
    looking && words !== "" && (area !== null || quickResults !== null) && !quickDismissed;

  /* Measured against the bar rather than against the field: the field grows
     while somebody watches, and a panel chasing that growth would jump under
     them. The bar it hangs from is fixed to the window, so a page scrolled
     underneath moves nothing here. */
  useLayoutEffect(() => {
    if (!quickOpen) {
      return;
    }
    const place = () => {
      const it = searchForm.current?.getBoundingClientRect();
      if (it) {
        setQuickUnder({ top: it.bottom + 8, left: it.left });
      }
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [quickOpen]);

  useEffect(() => {
    if (!quickOpen) {
      return;
    }
    const elsewhere = (event: MouseEvent) => {
      const on = event.target as Node;
      if (!searchForm.current?.contains(on) && !quickPanel.current?.contains(on)) {
        setQuickDismissed(true);
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setQuickDismissed(true);
      }
    };
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
    };
  }, [quickOpen]);

  /* A result chosen closes the field along with the panel: the question is
     answered, and there is nothing left for either to keep showing. Closing
     the field is enough on its own, since the panel only ever shows while it
     is open. */
  const chooseQuickResult = () => {
    setLooking(false);
    setQuery("");
  };

  const inTheMenu = (button: HeaderButton) => !buttonsInTheBar.includes(button);
  /* What only an administrator has any use for is nowhere for anybody else,
     and a scan nowhere on a server with nothing to scan. */
  const offered = offeredButtons(headerButtons, {
    administrator,
    mayRequest: access?.may_ask === true,
  }).filter((button) => button !== "scan" || libraries.length > 0);

  /* The press that starts a scan, on the bar or in the menu. Nothing to
     start while something is already running, and the bar is saying so
     meanwhile. */
  const scanEntry = (inMenu: boolean) => {
    if (jobs.length > 0) {
      return null;
    }
    if (scan.refused && inMenu) {
      return (
        <span key="scan" className="header-menu-line header-menu-refused" role="alert">
          {t(refusalKey(scan.refused))}
        </span>
      );
    }
    const said = scan.refused ? t(refusalKey(scan.refused)) : t("home.scan");
    return (
      <button
        key="scan"
        type="button"
        className={inMenu ? "header-menu-line" : "header-icon"}
        onClick={scan.start}
        disabled={scan.starting}
        title={inMenu ? undefined : said}
        aria-label={inMenu ? undefined : said}
      >
        <RefreshIcon size={inMenu ? 16 : 22} />
        {inMenu && said}
      </button>
    );
  };

  return (
    <header className={`header${shown ? "" : " header-away"}`}>
      <div className="header-inner">
        <div
          className={`header-piece header-start${location.pathname !== "/" ? " header-start-back" : ""}`}
          ref={start}
        >
          {/* Off the front page only: there is nowhere to come back from
              there, and the brand right next to it already leads home. */}
          {location.pathname !== "/" && (
            <button
              type="button"
              className="header-icon"
              onClick={() => navigate(-1)}
              title={t("nav.back")}
              aria-label={t("nav.back")}
            >
              <BackIcon size={22} />
            </button>
          )}
          <Link className="brand" to="/">
            <ServerMark branding={branding} size={28} logoClassName="brand-logo" />
            <span className="brand-name">{branding?.server_name}</span>
          </Link>
        </div>

        {/* Everything at this end on one piece of glass rather than three.
            Three of them read as three decisions about what goes with what,
            and there is only one: this end is what you press, the other is
            where you are. */}
        <div className="header-piece header-side" ref={side} data-folded={folded || undefined}>
          {/* The buttons of the bar in one box, which on a phone is pulled
              sideways when they are more than the width allows. Everywhere
              else it takes no room of its own. */}
          <span className="header-glass-halo" aria-hidden="true">
            <span className="glass-rim header-glass-halo-run" />
          </span>
          <span className="header-glass" aria-hidden="true">
            <span className="glass-slice header-glass-run" />
          </span>
          <span className="header-glass-cap" aria-hidden="true">
            <span className="glass-rim header-glass-end" />
          </span>
          <div className="header-buttons">
            <div className="header-buttons-track">
          {/* Everything anybody can press, one icon each, in the order this
              account put them; those it moved into its menu are drawn there
              instead. A menu of five entries opened by one press is five
              presses for every one of them, and the name it used to hang
              under said nothing anybody needed to read twice. */}
          {offered.map((button) => {
            if (button === "search") {
              /* Moved into the menu, the field still comes out here once
                 asked for, and goes back into the menu once let go. */
              return (
                (!inTheMenu(button) || looking) && (
                  <form
                    ref={searchForm}
                    key={button}
                    className={`search${looking ? " search-open" : ""}`}
                    role="search"
                    onSubmit={look}
                    onBlur={letGo}
                  >
                    {/* Where the field comes out of, and where it goes back into.
                        The two keys that reach it are said here rather than on a
                        badge inside it: there is no inside to put one in until it
                        is already open. */}
                    <button
                      type="button"
                      className="header-icon search-glass"
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={magnifier}
                      title={`${t("nav.search")} (${theShortcut()})`}
                      aria-label={t("nav.search")}
                      aria-expanded={looking}
                    >
                      <SearchIcon size={22} />
                    </button>
                    <input
                      ref={field}
                      type="search"
                      value={query}
                      onChange={(event) => setQuery(event.target.value)}
                      /* The escape key steps out of the field, and the fold above
                         decides from there: empty, it goes back into its magnifier;
                         with words still in it, it stays out with them. The browser
                         would otherwise empty a field of this kind on that key and
                         leave it standing open over nothing, which is neither of
                         those two answers. */
                      onKeyDown={(event) => {
                        if (event.key === "Escape") {
                          event.preventDefault();
                          event.currentTarget.blur();
                        }
                      }}
                      placeholder={t(
                        area === "admin"
                          ? "search.placeholder_admin"
                          : area === "settings"
                            ? "search.placeholder_settings"
                            : "search.placeholder",
                      )}
                      aria-label={t("nav.search")}
                      /* Out of the way of the tab key while it is folded away: a
                         field nobody can see is not a stop on the way round. */
                      tabIndex={looking ? undefined : -1}
                    />
                    {/* The scope, next to the words rather than on the page of
                        results: it narrows what is being asked, so it belongs where
                        the asking happens. */}
                    <Scope
                      categories={categories}
                      administrator={administrator}
                      scope={scope}
                      onChoose={(chosen) => {
                        setScope(chosen);
                        // Narrowed, the question is still to be asked: back to the words.
                        field.current?.focus();
                      }}
                      reachable={looking}
                    />
                  </form>
                )
              );
            }
            if (inTheMenu(button)) {
              return null;
            }
            if (button === "notifications") {
              return <Bell key={button} administrator={administrator} />;
            }
            if (button === "scan") {
              return scanEntry(false);
            }
            return <Place key={button} place={button} inTheMenu={false} />;
          })}
            </div>
          </div>

          {/* The few results the field offers on its own, drawn at the end of
              the page for the same reason the menus are: a panel frosting
              what is behind it becomes the backdrop of anything drawn inside
              the field, which is nothing at all. */}
          {quickOpen &&
            createPortal(
              <div
                className="quick-search"
                ref={quickPanel}
                style={{ top: quickUnder.top, left: quickUnder.left }}
              >
                {area ? (
                  foundSettings.length === 0 ? (
                    <p className="quick-search-empty">{t("search.no_setting")}</p>
                  ) : (
                    foundSettings.map((found) => (
                      <Link
                        key={`${found.path}:${found.key ?? ""}`}
                        className="quick-search-line"
                        to={addressOf(found)}
                        onClick={chooseQuickResult}
                      >
                        <span className="quick-search-title">{found.said}</span>
                        {found.key && <span className="quick-search-year">{found.section}</span>}
                      </Link>
                    ))
                  )
                ) : !quickResults ? null : quickResults.length === 0 ? (
                  <p className="quick-search-empty">{t("library.empty")}</p>
                ) : (
                  <>
                    {quickResults.map((line) => (
                      <Link
                        key={line.key}
                        className="quick-search-line"
                        to={line.to}
                        onClick={chooseQuickResult}
                      >
                        <QuickPicture line={line} />
                        <span className="quick-search-title">{line.title}</span>
                        {line.note !== null && (
                          <span className="quick-search-year">{line.note}</span>
                        )}
                      </Link>
                    ))}
                    <div className="quick-search-divider" aria-hidden="true" />
                    <Link
                      className="quick-search-all"
                      to={searchAddress(words, scope)}
                      onClick={() => setQuickDismissed(true)}
                    >
                      {t("home.see_all")}
                    </Link>
                  </>
                )}
              </div>,
              document.body,
            )}

          {/*
            * Who is signed in, and under their name whatever this account
            * chose to keep off the bar, then the way out.
            *
            * By default the bar keeps what is looked at from any screen: the
            * search, what was marked, what is waiting. The rest is opened
            * when it is wanted, which is what stops a bar of eight icons
            * reading as eight things somebody is expected to know.
            */}
          <button
            type="button"
            ref={fold}
            className="header-icon header-fold"
            aria-label={t(folded ? "nav.bar_show" : "nav.bar_hide")}
            aria-expanded={!folded}
            onClick={() => setFolded((was) => !was)}
          >
            {folded ? <ChevronLeftIcon size={20} /> : <ChevronRightIcon size={20} />}
          </button>

          <Dropdown
            className="header-account"
            reachable
            label={
              <>
                <Face
                  className="avatar"
                  name={account?.name ?? t("nav.account")}
                  avatar={account?.avatar ?? null}
                />
                <span className="header-who">{account?.name ?? t("nav.account")}</span>
              </>
            }
          >
            {/* What is not on the bar, in the same order as the bar, and
                leaving last whatever was chosen. */}
            {offered.filter(inTheMenu).map((button) =>
              button === "search" ? (
                <button key={button} type="button" className="header-menu-line" onClick={openSearch}>
                  <SearchIcon size={16} />
                  {t("nav.search")}
                </button>
              ) : button === "notifications" ? (
                <BellLine key={button} administrator={administrator} />
              ) : button === "scan" ? (
                scanEntry(true)
              ) : (
                <Place key={button} place={button} inTheMenu />
              ),
            )}

            <button type="button" className="header-menu-line" onClick={() => void leave()}>
              <LeaveIcon size={16} />
              {t("nav.sign_out")}
            </button>
          </Dropdown>
        </div>
      </div>
    </header>
  );
}

/**
 * What the search is narrowed to.
 *
 * A list of this interface's own making rather than the browser's: a system
 * menu is drawn by the machine, in its own colours and its own corners, and
 * sitting inside a rounded field it is the one thing on this bar that belongs
 * to something else.
 *
 * A kind is offered whenever the server holds one; a library by name only
 * where its kind holds more than one, since "Films" listed under "Films" says
 * the same thing twice and leaves whoever reads it working out which is
 * which.
 */
function Scope({
  categories,
  administrator,
  scope,
  onChoose,
  reachable,
}: {
  categories: Category[];
  /** Whether the administration is offered as well as the settings. */
  administrator: boolean;
  scope: string;
  onChoose: (scope: string) => void;
  /** Whether the field it belongs to is open. Folded away it is out of the
   *  way of the tab key, the same as the field itself. */
  reachable: boolean;
}) {
  const { t } = useSettings();

  const named = (value: string): string => {
    const kind = categories.find((category) => `kind:${category.kind}` === value);
    if (kind) {
      return nameOfKind(kind.kind, kind.libraries, t);
    }
    const library = categories
      .flatMap((category) => category.libraries)
      .find((entry) => `library:${entry.id}` === value);
    if (library) {
      return library.name;
    }
    const area = areaOf(value);
    return area ? t(area === "admin" ? "search.in_admin" : "search.in_settings") : t("search.everywhere");
  };

  return (
    <Dropdown className="search-scope" label={named(scope)} reachable={reachable}>
      <ScopeLine value="" scope={scope} onChoose={onChoose}>
        {t("search.everywhere")}
      </ScopeLine>
      {categories.map((category) => (
        <Fragment key={category.kind}>
          <ScopeLine value={`kind:${category.kind}`} scope={scope} onChoose={onChoose}>
            <KindIcon kind={category.kind} size={16} />
            {nameOfKind(category.kind, category.libraries, t)}
          </ScopeLine>
          {category.libraries.length > 1 &&
            category.libraries.map((library) => (
              <ScopeLine
                key={library.id}
                value={`library:${library.id}`}
                scope={scope}
                onChoose={onChoose}
                under
              >
                {library.name}
              </ScopeLine>
            ))}
        </Fragment>
      ))}
      <div className="header-menu-divider" aria-hidden="true" />
      <ScopeLine value="area:settings" scope={scope} onChoose={onChoose}>
        <GearIcon size={16} />
        {t("search.in_settings")}
      </ScopeLine>
      {administrator && (
        <ScopeLine value="area:admin" scope={scope} onChoose={onChoose}>
          <DashboardIcon size={16} />
          {t("search.in_admin")}
        </ScopeLine>
      )}
    </Dropdown>
  );
}

/**
 * One work's poster, at the size a line of the quick results is drawn at.
 *
 * The colour behind it and the initial in front of it are the same fallback
 * every other card in this interface shows: a poster that has not arrived is
 * not a hole, and one that never will is not a broken picture.
 */
/** A work of the catalogue as a line of the few results. */
function quickLineOf(card: Card): QuickLine {
  return {
    key: card.id,
    to: `/work/${card.id}`,
    title: card.title,
    note: card.year === null ? null : String(card.year),
    pictures: card.poster,
    color: card.color,
  };
}

function QuickPicture({ line }: { line: QuickLine }) {
  const picture = pictureSet(line.pictures);
  return (
    <span
      className={`quick-search-picture${line.shape ? ` quick-search-picture-${line.shape}` : ""}`}
      style={{ ["--card-color" as string]: line.color ?? "var(--surface-raised)" }}
    >
      {picture ? (
        <img src={picture.src} alt="" loading="lazy" decoding="async" draggable={false} />
      ) : (
        <span aria-hidden="true">{line.title.slice(0, 1)}</span>
      )}
    </span>
  );
}

function ScopeLine({
  value,
  scope,
  onChoose,
  under,
  children,
}: {
  value: string;
  scope: string;
  onChoose: (scope: string) => void;
  /** One of the libraries under a kind, stepped in so the two read as a list
   *  and a sub-list rather than as one flat run of names. */
  under?: boolean;
  children: React.ReactNode;
}) {
  const chosen = scope === value;
  return (
    <button
      type="button"
      className={`header-menu-line${under ? " header-menu-under" : ""}${
        chosen ? " header-menu-line-on" : ""
      }`}
      aria-current={chosen}
      onClick={() => onChoose(value)}
    >
      {children}
    </button>
  );
}

/** Where a button of the bar leads, and what it is called. */
const PLACES = {
  favourites: { to: "/favourites", name: "nav.favourites" },
  watch_later: { to: "/watch-later", name: "nav.watch_later" },
  collections: { to: "/collections", name: "nav.collections" },
  playlists: { to: "/playlists", name: "nav.playlists" },
  requests: { to: "/requests", name: "requests.title" },
  administration: { to: "/admin", name: "nav.administration" },
  settings: { to: "/settings", name: "nav.settings" },
  cast: { to: null, name: "nav.cast" },
} as const;

function PlaceIcon({ place, size }: { place: keyof typeof PLACES; size: number }) {
  switch (place) {
    case "favourites":
      return <HeartIcon size={size} filled={false} />;
    case "watch_later":
      return <ClockIcon size={size} />;
    case "collections":
      return <CollectionIcon size={size} />;
    case "playlists":
      return <PlaylistIcon size={size} />;
    case "requests":
      return <RequestIcon size={size} />;
    case "administration":
      return <DashboardIcon size={size} />;
    case "settings":
      return <GearIcon size={size} />;
    case "cast":
      return <ScreenCastIcon size={size} />;
  }
}

/**
 * A button of the bar that leads somewhere, as an icon of the bar or as a
 * line of the account's menu.
 *
 * Casting has no engine behind it yet: greyed, and when it is coming is what
 * the mouse is told rather than a second run of words.
 */
function Place({ place, inTheMenu }: { place: keyof typeof PLACES; inTheMenu: boolean }) {
  const { t } = useSettings();
  const { to, name } = PLACES[place];
  const said = t(name);
  const icon = <PlaceIcon place={place} size={inTheMenu ? 16 : 24} />;
  if (to === null) {
    return inTheMenu ? (
      <span className="header-menu-line header-menu-later" aria-disabled="true" title={t("nav.later")}>
        {icon}
        {said}
      </span>
    ) : (
      <button
        type="button"
        className="header-icon"
        disabled
        title={t("nav.later")}
        aria-label={`${said} (${t("nav.later")})`}
      >
        {icon}
      </button>
    );
  }
  return (
    <NavLink
      to={to}
      className={inTheMenu ? "header-menu-line" : "header-icon"}
      title={inTheMenu ? undefined : said}
      aria-label={inTheMenu ? undefined : said}
    >
      {icon}
      {inTheMenu && said}
    </NavLink>
  );
}

/**
 * The letters standing for a name, where a picture would go.
 *
 * Accounts carry no picture yet, and a blank circle says nothing. Two letters
 * for a name in two parts, one otherwise, which is what tells two accounts
 * apart at the size this is drawn.
 */

/**
 * Whether the bar is out, as the page is scrolled, for an account that asked
 * it to step aside; always, for one that did not.
 *
 * Read once a frame at most, whatever the wheel sends: the page can report
 * its place many times between two frames, and only the last one is drawn.
 */
function useHeadroom(
  scrolling: React.RefObject<HTMLDivElement | null>,
  stepsAside: boolean,
): boolean {
  const [out, setOut] = useState(true);

  useEffect(() => {
    const box = scrolling.current;
    if (!stepsAside || !box) {
      setOut(true);
      return;
    }
    // Where the top of the page ends: while the bar still stands over the
    // first of it, it stays.
    const top = parseFloat(getComputedStyle(box).getPropertyValue("--header-height")) || 0;
    let bar: Headroom = { shown: true, turnedAt: box.scrollTop };
    let asked = 0;
    const read = () => {
      asked = 0;
      bar = headroomAt(bar, box.scrollTop, top);
      setOut(bar.shown);
    };
    const soon = () => {
      if (!asked) {
        asked = requestAnimationFrame(read);
      }
    };
    box.addEventListener("scroll", soon, { passive: true });
    return () => {
      box.removeEventListener("scroll", soon);
      cancelAnimationFrame(asked);
    };
  }, [scrolling, stepsAside]);

  return out;
}

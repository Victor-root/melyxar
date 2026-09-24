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

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  Link,
  NavLink,
  useLocation,
  useNavigate,
  useSearchParams,
} from "react-router-dom";
import type { Card, Library, LibraryKind } from "../api";
import { api, pictureSet } from "../api";
import { wasAbandoned } from "../asking";
import { outOfAHundred } from "../readable";
import { useRunning, useStartScan } from "../running";
import { refusalKey } from "../i18n";
import { useAccount } from "../account";
import { useAttention } from "../attention";
import { sayPoint } from "../pages/admin/activity";
import { KINDS, nameOfKind } from "../libraries";
import { useBranding } from "../player/logo";
import { useSettings } from "../settings";
import { isSectioned } from "./sectioned";
import { Face } from "./face";
import { headroomAt } from "../headroom";
import type { Headroom } from "../headroom";
import {
  BackIcon,
  BellIcon,
  ChevronDownIcon,
  GearIcon,
  HeartIcon,
  KindIcon,
  LeaveIcon,
  MelyxarMark,
  RefreshIcon,
  ScreenCastIcon,
  TickIcon,
  SearchIcon,
  SlidersIcon,
} from "../icons";

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
  const { t, headerHides } = useSettings();
  const navigate = useNavigate();
  const location = useLocation();
  const [parameters] = useSearchParams();
  const [query, setQuery] = useState(parameters.get("search") ?? "");
  const words = query.trim();
  const [scope, setScope] = useState(parameters.get("in") ?? "");
  /* Whether the field is out of its magnifier. Open already when the address
     carries a search: landing on a page of results with the words hidden
     inside an icon is a page answering a question nobody can see. */
  const [looking, setLooking] = useState(() => (parameters.get("search") ?? "") !== "");
  /* Held in place on the pages laid out beside a list of sections: the list
     is pinned under the bar, and a bar that slid away would leave a hole of
     its own height over it. */
  const sectioned = isSectioned(location.pathname);
  const out = useHeadroom(scrolling, headerHides && !sectioned);
  const field = useRef<HTMLInputElement>(null);
  const searchForm = useRef<HTMLFormElement>(null);
  const { jobs } = useRunning();
  const { account, leave } = useAccount();
  const branding = useBranding();
  const start = useRef<HTMLDivElement>(null);

  /* Where the piece at the left end stops, written on the page: as wide as
     the server's name and logo, and what rises into the band beside it
     keeps clear of it rather than sliding under it. */
  useLayoutEffect(() => {
    const piece = start.current;
    if (!piece) {
      return;
    }
    const root = document.documentElement;
    const measure = () =>
      root.style.setProperty("--header-start-end", `${piece.getBoundingClientRect().right}px`);
    measure();
    const watching = new ResizeObserver(measure);
    watching.observe(piece);
    window.addEventListener("resize", measure);
    return () => {
      watching.disconnect();
      window.removeEventListener("resize", measure);
      root.style.removeProperty("--header-start-end");
    };
  }, []);
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
        setLooking(true);
        field.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const ask = () => {
    if (!words) {
      navigate("/");
      return;
    }
    setQuickDismissed(true);
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
      setLooking(true);
      field.current?.focus();
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
  const [quickResults, setQuickResults] = useState<Card[] | null>(null);
  /* Put aside by a press outside, the enter key, or a result chosen: what it
     answers is done with, and typing again is what asks it back. */
  const [quickDismissed, setQuickDismissed] = useState(false);
  const quickPanel = useRef<HTMLDivElement>(null);
  const [quickUnder, setQuickUnder] = useState({ top: 0, left: 0 });

  useEffect(() => setQuickDismissed(false), [words, scope]);

  useEffect(() => {
    if (!looking || !words) {
      setQuickResults(null);
      return;
    }
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      api
        .works({ search: words, limit: QUICK_RESULTS, ...scopeToBrowse(scope) }, controller.signal)
        .then((page) => setQuickResults(page.cards))
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
  }, [words, scope, looking]);

  const quickOpen = looking && words !== "" && quickResults !== null && !quickDismissed;

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

  const administrator = account?.is_administrator === true;

  return (
    <header className={`header${out || looking ? "" : " header-away"}`}>
      <div className="header-inner">
        <div className="header-piece header-start" ref={start}>
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
            {/* Decorative: the name is written right next to it, and an
                image announced twice over is exactly what a screen reader
                must not have to hear. The administrator's own logo keeps its
                own colours; the one Melyxar ships takes the accent. Nothing
                until the server has said which, so Melyxar's never flashes
                up in front of somebody else's. */}
            {branding &&
              (branding.logo ? (
                <img className="brand-logo" src={branding.logo} alt="" aria-hidden="true" />
              ) : (
                <MelyxarMark size={28} />
              ))}
            <span className="brand-name">{branding?.server_name}</span>
          </Link>
        </div>

        {/* Everything at this end on one piece of glass rather than three.
            Three of them read as three decisions about what goes with what,
            and there is only one: this end is what you press, the other is
            where you are. */}
        <div className="header-piece header-side">
          {/* What the server is doing, and only while it is doing it. It is
              the one thing here that is news rather than a way to somewhere:
              a scan that started ten minutes ago and is still going is worth
              a glance from any screen, and a glance is not something anybody
              should have to open a menu for. */}
          {administrator && jobs.length > 0 && (
            <Link className="header-busy" to="/admin/tasks">
              <span className="header-busy-mark" aria-hidden="true" />
              {t(`jobs.${jobs[0].kind}`)}
              {jobs[0].ratio !== null && ` ${outOfAHundred(jobs[0].ratio)} %`}
            </Link>
          )}

          {/* Everything anybody can press, one icon each. A menu of five
              entries opened by one press is five presses for every one of
              them, and the name it used to hang under said nothing anybody
              needed to read twice. */}
          <form
            ref={searchForm}
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
              placeholder={t("search.placeholder")}
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
              scope={scope}
              onChoose={setScope}
              reachable={looking}
            />
          </form>

          {/* The few results the field offers on its own, drawn at the end of
              the page for the same reason the menus are: a panel frosting
              what is behind it becomes the backdrop of anything drawn inside
              the field, which is nothing at all. */}
          {quickOpen &&
            quickResults &&
            createPortal(
              <div
                className="quick-search"
                ref={quickPanel}
                style={{ top: quickUnder.top, left: quickUnder.left }}
              >
                {quickResults.length === 0 ? (
                  <p className="quick-search-empty">{t("library.empty")}</p>
                ) : (
                  <>
                    {quickResults.map((card) => (
                      <Link
                        key={card.id}
                        className="quick-search-line"
                        to={`/work/${card.id}`}
                        onClick={chooseQuickResult}
                      >
                        <QuickPicture card={card} />
                        <span className="quick-search-title">{card.title}</span>
                        {card.year !== null && (
                          <span className="quick-search-year">{card.year}</span>
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

          <Bell administrator={administrator} />

          <NavLink
            to="/favourites"
            className="header-icon"
            title={t("nav.favourites")}
            aria-label={t("nav.favourites")}
          >
            <HeartIcon size={24} filled={false} />
          </NavLink>

          {/*
            * Who is signed in, and under their name everything that is
            * theirs to do rather than somewhere to go.
            *
            * The bar keeps what is looked at from any screen: the search,
            * what is waiting, what was marked. The rest is opened when it
            * is wanted, which is what stops a bar of eight icons reading as
            * eight things somebody is expected to know.
            *
            * What the server is doing is under here too, and so is the
            * press that starts it: it belongs to whoever runs the server,
            * and their name is the one place on the bar that is already
            * about them.
            */}
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
            {administrator && (
              <>
                {/* Nothing to start while something is already running, and
                    the bar is saying so meanwhile. */}
                {jobs.length === 0 &&
                  (scan.refused ? (
                    <span className="header-menu-line header-menu-refused" role="alert">
                      {t(refusalKey(scan.refused))}
                    </span>
                  ) : (
                    libraries.length > 0 && (
                      <button
                        type="button"
                        className="header-menu-line"
                        onClick={scan.start}
                        disabled={scan.starting}
                      >
                        <RefreshIcon size={16} />
                        {t("home.scan")}
                      </button>
                    )
                  ))}
                <NavLink to="/admin" className="header-menu-line">
                  <SlidersIcon size={16} />
                  {t("nav.administration")}
                </NavLink>
              </>
            )}

            {/* No engine behind this one yet either. Greyed, and when it is
                coming is what the mouse is told rather than a second run of
                words down the side of the list. */}
            <span
              className="header-menu-line header-menu-later"
              aria-disabled="true"
              title={t("nav.later")}
            >
              <ScreenCastIcon size={16} />
              {t("nav.cast")}
            </span>

            <NavLink to="/settings" className="header-menu-line">
              <GearIcon size={16} />
              {t("nav.settings")}
            </NavLink>

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
  scope,
  onChoose,
  reachable,
}: {
  categories: Category[];
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
    return library?.name ?? t("search.everywhere");
  };

  return (
    <Dropdown className="search-scope" label={named(scope)} reachable={reachable}>
      <ScopeLine value="" scope={scope} onChoose={onChoose}>
        {t("search.everywhere")}
      </ScopeLine>
      {categories.map((category) => (
        <div key={category.kind}>
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
        </div>
      ))}
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
function QuickPicture({ card }: { card: Card }) {
  const picture = pictureSet(card.poster);
  return (
    <span
      className="quick-search-picture"
      style={{ ["--card-color" as string]: card.color ?? "var(--surface-raised)" }}
    >
      {picture ? (
        <img src={picture.src} alt="" loading="lazy" decoding="async" draggable={false} />
      ) : (
        <span aria-hidden="true">{card.title.slice(0, 1)}</span>
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

/**
 * A word that opens a short list under it.
 *
 * Closes on a click anywhere else and on the escape key, which are the two
 * ways anybody ever tries to close one.
 *
 * The list is drawn at the end of the page rather than inside the bar, and
 * placed against the window by hand. That is not a preference: the pieces
 * of the bar are frosted glass, and an element that frosts what is behind
 * it becomes the backdrop of everything inside it. A list that frosts the
 * page from in there frosts the inside of its own piece, which is nothing
 * at all, and comes out as clear glass over whatever film is playing
 * underneath. Outside it, there is a page behind it to frost.
 */
function Dropdown({
  label,
  className,
  reachable,
  icon,
  listClassName,
  children,
}: {
  label: React.ReactNode;
  /** What this one is, for the few that are not a word in the bar. */
  className?: string;
  /** Whether what holds it is open. Folded away, the tab key passes it by
   *  and any list it had left hanging is shut. */
  reachable: boolean;
  /** Opened by an icon of the bar rather than a word: drawn as the other
   *  icons are, without the fold, and named for whoever cannot see it. */
  icon?: string;
  /** What the list is, for one that holds more than short lines. */
  listClassName?: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const holder = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  /** Where the list goes: under what opened it, and ending where it ends. */
  const [under, setUnder] = useState({ top: 0, right: 0 });

  /* Measured when it opens and again if the window changes shape. The bar it
     hangs from is fixed to the window, so a page scrolled underneath moves
     nothing here. */
  useLayoutEffect(() => {
    if (!open) {
      return;
    }
    const place = () => {
      const it = holder.current?.getBoundingClientRect();
      if (it) {
        setUnder({ top: it.bottom + 6, right: Math.max(window.innerWidth - it.right, 0) });
      }
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    /* The list is no longer inside what opened it, so a press in it is a
       press outside as far as the page is concerned: both are asked. */
    const elsewhere = (event: MouseEvent) => {
      const on = event.target as Node;
      if (!holder.current?.contains(on) && !list.current?.contains(on)) {
        setOpen(false);
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
    };
  }, [open]);

  /* Shut along with whatever folded it away, or it would be left hanging
     over a field that is no longer there. */
  useEffect(() => {
    if (!reachable) {
      setOpen(false);
    }
  }, [reachable]);

  return (
    <div className={`header-menu${className ? ` ${className}` : ""}`} ref={holder}>
      <button
        type="button"
        className={`${icon ? "header-icon" : "header-link"}${open ? " header-link-on" : ""}`}
        aria-expanded={open}
        aria-label={icon}
        title={icon}
        tabIndex={reachable ? undefined : -1}
        onClick={() => setOpen((was) => !was)}
      >
        {label}
        {!icon && <ChevronDownIcon size={15} />}
      </button>
      {open &&
        createPortal(
          <div
            className={`header-menu-list${listClassName ? ` ${listClassName}` : ""}`}
            ref={list}
            style={{ top: under.top, right: under.right }}
            onClick={() => setOpen(false)}
          >
            {children}
          </div>,
          document.body,
        )}
    </div>
  );
}

/**
 * What deserves a look, for an administrator: how many points, the worst of
 * them in its colour, and the list itself on a press.
 *
 * For anybody else there is nothing behind it yet, and it stays greyed and
 * saying so rather than left out: a function nobody can see is a function
 * nobody knows is coming.
 */
function Bell({ administrator }: { administrator: boolean }) {
  const { t, language } = useSettings();
  const { points, markSeen } = useAttention();

  if (!administrator) {
    return (
      <button
        type="button"
        className="header-icon"
        disabled
        title={t("nav.later")}
        aria-label={`${t("nav.notifications")} (${t("nav.later")})`}
      >
        <BellIcon size={24} />
      </button>
    );
  }

  const shown = points ?? [];
  const worst = shown.some((point) => point.state === "trouble") ? "trouble" : "attention";
  return (
    <Dropdown
      className="header-bell"
      icon={t("admin.watch")}
      listClassName="bell-list"
      reachable
      label={
        <>
          <BellIcon size={24} />
          {shown.length > 0 && (
            <span className={`bell-count bell-count-${worst}`}>{shown.length}</span>
          )}
        </>
      }
    >
      {/* Marking them seen sits with the title rather than under the last
          point, where a long list would push it out of reach. */}
      <span className="bell-head">
        <span className="bell-title">{t("admin.watch")}</span>
        {shown.some((point) => point.may_be_seen) && (
          <button type="button" className="bell-seen" onClick={() => void markSeen()}>
            <TickIcon size={14} />
            {t("attention.mark_seen")}
          </button>
        )}
      </span>
      {shown.length === 0 && <span className="bell-none">{t("attention.none")}</span>}
      {shown.map((point, index) => {
        const said = sayPoint(point, t, language);
        return (
          <Link key={index} to={said.to} className="header-menu-line bell-point">
            <span className={`state-dot state-${point.state}`} aria-hidden="true" />
            <span>{said.title}</span>
          </Link>
        );
      })}
    </Dropdown>
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

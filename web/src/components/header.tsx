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

import { useEffect, useRef, useState } from "react";
import {
  Link,
  NavLink,
  useLocation,
  useNavigate,
  useSearchParams,
} from "react-router-dom";
import type { Library, LibraryKind } from "../api";
import { outOfAHundred } from "../readable";
import { useRunning, useStartScan } from "../running";
import { refusalKey } from "../i18n";
import { useAccount } from "../account";
import { useSettings } from "../settings";
import {
  ActivityIcon,
  BackIcon,
  BellIcon,
  ChevronDownIcon,
  GearIcon,
  HeartIcon,
  JournalIcon,
  KindIcon,
  LeaveIcon,
  RefreshIcon,
  ScreenCastIcon,
  SearchIcon,
} from "../icons";

/** The order categories are offered in, which is the order they are read in. */
const KINDS: LibraryKind[] = ["movies", "series", "anime", "shows", "music"];

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

/** The categories this server really has, in the order they are offered. */
function categoriesOf(libraries: Library[]): Category[] {
  return KINDS.map((kind) => ({
    kind,
    libraries: libraries.filter((library) => library.kind === kind),
  })).filter((category) => category.libraries.length > 0);
}

export function Header({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const location = useLocation();
  const [parameters] = useSearchParams();
  const [query, setQuery] = useState(parameters.get("search") ?? "");
  const [scope, setScope] = useState(parameters.get("in") ?? "");
  /* Whether the field is out of its magnifier. Open already when the address
     carries a search: landing on a page of results with the words hidden
     inside an icon is a page answering a question nobody can see. */
  const [looking, setLooking] = useState(() => (parameters.get("search") ?? "") !== "");
  const field = useRef<HTMLInputElement>(null);
  const { jobs } = useRunning();
  const { account, leave } = useAccount();
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
    const words = query.trim();
    if (!words) {
      navigate("/");
      return;
    }
    const asked = new URLSearchParams({ search: words });
    if (scope) {
      asked.set("in", scope);
    }
    navigate(`/search?${asked.toString()}`);
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
    if (query.trim()) {
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
    if (query.trim()) {
      return;
    }
    const next = event.relatedTarget as Node | null;
    if (next && event.currentTarget.contains(next)) {
      return;
    }
    setLooking(false);
  };

  const administrator = account?.is_administrator === true;

  return (
    <header className="header">
      <div className="header-inner">
        <div className="header-piece header-start">
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
                must not have to hear. */}
            <img className="brand-mark" src="/melyxar-64.png" alt="" aria-hidden="true" />
            <span className="brand-name">{t("app.name")}</span>
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
            <Link className="header-busy" to="/activity">
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
              <SearchIcon size={20} />
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

          {/* No engine behind it yet. Shown greyed and saying when rather
              than left out: a function nobody can see is a function nobody
              knows is coming. */}
          <button
            type="button"
            className="header-icon"
            disabled
            title={t("nav.later")}
            aria-label={`${t("nav.notifications")} (${t("nav.later")})`}
          >
            <BellIcon size={22} />
          </button>

          <NavLink
            to="/favourites"
            className="header-icon"
            title={t("nav.favourites")}
            aria-label={t("nav.favourites")}
          >
            <HeartIcon size={22} filled={false} />
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
                <span className="avatar" aria-hidden="true">
                  {initialsOf(account?.name ?? t("nav.account"))}
                </span>
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
                <NavLink to="/activity" className="header-menu-line">
                  <ActivityIcon size={16} />
                  {t("nav.jobs")}
                </NavLink>
                <NavLink to="/journal" className="header-menu-line">
                  <JournalIcon size={16} />
                  {t("nav.journal")}
                </NavLink>
              </>
            )}

            {/* No engine behind this one yet either, and it says so here in
                words rather than in what the mouse is told: there is room
                for the sentence in a menu, and there was none on the bar. */}
            <span className="header-menu-line header-menu-later" aria-disabled="true">
              <ScreenCastIcon size={16} />
              {t("nav.cast")}
              <span className="header-menu-when">{t("nav.later")}</span>
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
      return t(`kind.${kind.kind}`);
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
            {t(`kind.${category.kind}`)}
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
 */
function Dropdown({
  label,
  className,
  reachable,
  children,
}: {
  label: React.ReactNode;
  /** What this one is, for the few that are not a word in the bar. */
  className?: string;
  /** Whether what holds it is open. Folded away, the tab key passes it by
   *  and any list it had left hanging is shut. */
  reachable: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const holder = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    const elsewhere = (event: MouseEvent) => {
      if (!holder.current?.contains(event.target as Node)) {
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
        className={`header-link${open ? " header-link-on" : ""}`}
        aria-expanded={open}
        tabIndex={reachable ? undefined : -1}
        onClick={() => setOpen((was) => !was)}
      >
        {label}
        <ChevronDownIcon size={15} />
      </button>
      {open && (
        <div className="header-menu-list" onClick={() => setOpen(false)}>
          {children}
        </div>
      )}
    </div>
  );
}

/**
 * The letters standing for a name, where a picture would go.
 *
 * Accounts carry no picture yet, and a blank circle says nothing. Two letters
 * for a name in two parts, one otherwise, which is what tells two accounts
 * apart at the size this is drawn.
 */
function initialsOf(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) {
    return "?";
  }
  const letters = parts.length > 1 ? `${parts[0][0]}${parts[1][0]}` : parts[0].slice(0, 1);
  return letters.toLocaleUpperCase();
}

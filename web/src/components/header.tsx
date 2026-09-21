/*
 * The bar at the top: where you are, what you can look for, and who you are.
 *
 * The categories are read from the libraries this server really holds rather
 * than written down here. A server with no anime has no category of anime, a
 * collection of films spread over four disks is one category rather than
 * four, and nobody has to remember to add a category the day a kind of
 * library is invented.
 *
 * What the engine cannot do yet is shown all the same, greyed and saying so.
 * A function that is simply absent is a function nobody knows is coming; one
 * that is there and says when tells the truth about where this is going.
 */

import { useEffect, useRef, useState } from "react";
import { Link, NavLink, useNavigate, useSearchParams } from "react-router-dom";
import type { Library, LibraryKind } from "../api";
import { outOfAHundred } from "../readable";
import { useRunning, useStartScan } from "../running";
import { refusalKey } from "../i18n";
import { useAccount } from "../account";
import { useSettings } from "../settings";
import {
  BellIcon,
  ChevronDownIcon,
  HeartIcon,
  HomeIcon,
  KindIcon,
  ScreenCastIcon,
  SearchIcon,
} from "../icons";

/** The order categories are offered in, which is the order they are read in. */
const KINDS: LibraryKind[] = ["movies", "series", "anime", "shows", "music"];

/** How many categories stand in the bar before the rest go behind one word. */
const IN_THE_BAR = 4;

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
  const [parameters] = useSearchParams();
  const [query, setQuery] = useState(parameters.get("search") ?? "");
  const [scope, setScope] = useState(parameters.get("in") ?? "");
  const field = useRef<HTMLInputElement>(null);
  const { jobs } = useRunning();
  const { account } = useAccount();
  /* A scan is the one thing an administrator needs from wherever they happen
     to be: films were added, a name was corrected, a disk came back. */
  const scan = useStartScan(libraries);

  const categories = categoriesOf(libraries);
  const inTheBar = categories.slice(0, IN_THE_BAR);
  const behindMore = categories.slice(IN_THE_BAR);

  // A slash puts the cursor in the search field, the way every list of things
  // has worked for thirty years, and so does the command key with a K, which
  // is how everything written in the last ten years does it.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const typing =
        target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
      const held = event.metaKey || event.ctrlKey;
      if ((event.key === "/" && !typing) || (held && event.key.toLowerCase() === "k")) {
        event.preventDefault();
        field.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const look = (event: React.FormEvent) => {
    event.preventDefault();
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

  return (
    <header className="header">
      <div className="header-inner">
        <Link className="brand" to="/">
          {/* Decorative: the name is written right next to it, and an image
              announced twice over is exactly what a screen reader must not
              have to hear. */}
          <img className="brand-mark" src="/melyxar-64.png" alt="" aria-hidden="true" />
          <span className="brand-name">{t("app.name")}</span>
        </Link>

        <nav className="header-nav" aria-label={t("nav.libraries")}>
          <NavLink to="/" end className="header-link">
            <HomeIcon size={17} />
            {t("nav.home")}
          </NavLink>

          {inTheBar.map((category) => (
            <CategoryLink key={category.kind} category={category} />
          ))}

          {behindMore.length > 0 && (
            <Dropdown label={t("nav.more")}>
              {behindMore.flatMap((category) =>
                category.libraries.map((library) => (
                  <NavLink
                    key={library.id}
                    to={`/library/${library.id}`}
                    className="header-menu-line"
                  >
                    {library.name}
                  </NavLink>
                )),
              )}
            </Dropdown>
          )}

          <NavLink to="/favourites" className="header-link">
            <HeartIcon size={17} filled={false} />
            {t("nav.favourites")}
          </NavLink>
        </nav>

        <form className="search" role="search" onSubmit={look}>
          <SearchIcon size={17} />
          <input
            ref={field}
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("search.placeholder")}
            aria-label={t("nav.search")}
          />
          {/* What opens it without the mouse, said rather than left to be
              discovered. Hidden from readers who are not looking at it: it is
              a picture of two keys, and the field already announces itself. */}
          <kbd className="search-key" aria-hidden="true">
            {theShortcut()}
          </kbd>
          {/* The scope, next to the words rather than on the page of results:
              it narrows what is being asked, so it belongs where the asking
              happens. */}
          <select
            className="search-scope"
            value={scope}
            onChange={(event) => setScope(event.target.value)}
            aria-label={t("search.scope")}
          >
            <option value="">{t("search.everywhere")}</option>
            {categories.map((category) => (
              <option key={category.kind} value={`kind:${category.kind}`}>
                {t(`kind.${category.kind}`)}
              </option>
            ))}
            {libraries.length > 1 &&
              libraries.map((library) => (
                <option key={library.id} value={`library:${library.id}`}>
                  {library.name}
                </option>
              ))}
          </select>
        </form>

        <div className="header-side">
          {/* Neither of these has an engine behind it yet. Shown greyed and
              saying when rather than left out: a function nobody can see is a
              function nobody knows is coming. */}
          <button
            type="button"
            className="header-icon"
            disabled
            title={t("nav.later")}
            aria-label={`${t("nav.cast")} (${t("nav.later")})`}
          >
            <ScreenCastIcon size={19} />
          </button>
          <button
            type="button"
            className="header-icon"
            disabled
            title={t("nav.later")}
            aria-label={`${t("nav.notifications")} (${t("nav.later")})`}
          >
            <BellIcon size={19} />
          </button>

          {/* What the server is doing, and how to set it going. Both belong to
              whoever runs the server, so neither is drawn for anybody else. */}
          {account?.is_administrator &&
            (jobs.length > 0 ? (
              <Link className="header-busy" to="/activity">
                <span className="header-busy-mark" aria-hidden="true" />
                {t(`jobs.${jobs[0].kind}`)}
                {jobs[0].ratio !== null && ` ${outOfAHundred(jobs[0].ratio)} %`}
              </Link>
            ) : scan.refused ? (
              <span className="header-refused" role="alert">
                {t(refusalKey(scan.refused))}
              </span>
            ) : (
              libraries.length > 0 && (
                <button
                  className="button button-small"
                  onClick={scan.start}
                  disabled={scan.starting}
                >
                  {t("home.scan")}
                </button>
              )
            ))}

          <AccountMenu />
        </div>
      </div>
    </header>
  );
}

/**
 * One category in the bar.
 *
 * A kind holding one library leads straight to it, since a menu of one entry
 * is a click that asks nothing. A kind holding several opens on them, because
 * "films" then means four folders on four disks and only their owner knows
 * which one they meant.
 */
function CategoryLink({ category }: { category: Category }) {
  const { t } = useSettings();
  const name = t(`kind.${category.kind}`);

  if (category.libraries.length === 1) {
    return (
      <NavLink to={`/library/${category.libraries[0].id}`} className="header-link">
        <KindIcon kind={category.kind} size={17} />
        {name}
      </NavLink>
    );
  }
  return (
    <Dropdown
      label={
        <>
          <KindIcon kind={category.kind} size={17} />
          {name}
        </>
      }
    >
      {category.libraries.map((library) => (
        <NavLink key={library.id} to={`/library/${library.id}`} className="header-menu-line">
          {library.name}
        </NavLink>
      ))}
    </Dropdown>
  );
}

/**
 * A word that opens a short list under it.
 *
 * Closes on a click anywhere else and on the escape key, which are the two
 * ways anybody ever tries to close one.
 */
function Dropdown({ label, children }: { label: React.ReactNode; children: React.ReactNode }) {
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

  return (
    <div className="header-menu" ref={holder}>
      <button
        type="button"
        className={`header-link${open ? " header-link-on" : ""}`}
        aria-expanded={open}
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
 * Who is here, and what belongs to them.
 *
 * The two screens that belong to whoever runs the server are drawn here and
 * only for them: they are the last of the administration still sitting in the
 * main interface, and it will have one of its own.
 */
function AccountMenu() {
  const { t } = useSettings();
  const { account, leave } = useAccount();
  const name = account?.name ?? t("nav.account");

  return (
    <Dropdown
      label={
        <>
          <span className="avatar" aria-hidden="true">
            {initialsOf(name)}
          </span>
          <span className="account-name">{name}</span>
        </>
      }
    >
      <NavLink to="/settings" className="header-menu-line">
        {t("nav.settings")}
      </NavLink>
      {account?.is_administrator && (
        <>
          <NavLink to="/activity" className="header-menu-line">
            {t("nav.jobs")}
          </NavLink>
          <NavLink to="/journal" className="header-menu-line">
            {t("nav.journal")}
          </NavLink>
        </>
      )}
      <button type="button" className="header-menu-line" onClick={() => void leave()}>
        {t("nav.sign_out")}
      </button>
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
function initialsOf(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) {
    return "?";
  }
  const letters = parts.length > 1 ? `${parts[0][0]}${parts[1][0]}` : parts[0].slice(0, 1);
  return letters.toLocaleUpperCase();
}

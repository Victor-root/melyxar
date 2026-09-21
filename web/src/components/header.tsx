/*
 * The bar at the top: what you can look for, and who you are.
 *
 * It holds no list of places. The libraries are reached from the band under
 * the banner, which is wide enough to show what each one is rather than only
 * name it, and the way back to the front page is the name of the server
 * itself, which is where everybody presses anyway. What is left is the
 * search, in the middle where it belongs, and the account at the end.
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
  KindIcon,
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
          <Scope categories={categories} scope={scope} onChoose={setScope} />
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
}: {
  categories: Category[];
  scope: string;
  onChoose: (scope: string) => void;
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
    <Dropdown className="search-scope" label={named(scope)}>
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
  children,
}: {
  label: React.ReactNode;
  /** What this one is, for the few that are not a word in the bar. */
  className?: string;
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

  return (
    <div className={`header-menu${className ? ` ${className}` : ""}`} ref={holder}>
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
      {/* Somebody's own marks, which left the bar when the bar stopped being
          a list of places: they are theirs, so they live under their name. */}
      <NavLink to="/favourites" className="header-menu-line">
        <HeartIcon size={16} filled={false} />
        {t("nav.favourites")}
      </NavLink>
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

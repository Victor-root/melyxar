/*
 * The bar at the top: where you are, what you can look for, and the two
 * choices that belong to the viewer rather than to the server.
 */

import { useEffect, useRef, useState } from "react";
import { Link, NavLink, useNavigate, useSearchParams } from "react-router-dom";
import type { Library } from "../api";
import { useRunning, useStartScan } from "../running";
import { useSettings } from "../settings";
import type { ThemeChoice } from "../settings";

export function Header({ libraries }: { libraries: Library[] }) {
  const { t, language, setLanguage, theme, setTheme } = useSettings();
  const navigate = useNavigate();
  const [parameters] = useSearchParams();
  const [query, setQuery] = useState(parameters.get("search") ?? "");
  const field = useRef<HTMLInputElement>(null);
  const { jobs } = useRunning();
  /* A scan is the one thing somebody needs from wherever they happen to be:
     films were added, a name was corrected, a disk came back. It lives here so
     that nobody has to find the page it belongs to. */
  const { start: startScan, starting } = useStartScan(libraries);

  // A slash puts the cursor in the search field, the way every list of things
  // has worked for thirty years.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const typing =
        target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
      if (event.key === "/" && !typing) {
        event.preventDefault();
        field.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <header className="header">
      <div className="header-inner">
        <Link className="brand" to="/">
          <span className="brand-mark" aria-hidden="true" />
          {t("app.name")}
        </Link>

        <nav className="header-nav" aria-label={t("nav.libraries")}>
          <NavLink to="/" end className="header-link">
            {t("nav.home")}
          </NavLink>
          {libraries.map((library) => (
            <NavLink key={library.id} to={`/library/${library.id}`} className="header-link">
              {library.name}
            </NavLink>
          ))}
          <NavLink to="/activity" className="header-link">
            {t("nav.jobs")}
          </NavLink>
          <NavLink to="/settings" className="header-link">
            {t("nav.settings")}
          </NavLink>
        </nav>

        <form
          className="search"
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            navigate(query.trim() ? `/search?search=${encodeURIComponent(query.trim())}` : "/");
          }}
        >
          <input
            ref={field}
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("search.placeholder")}
            aria-label={t("nav.search")}
          />
        </form>

        {/* While something runs, the button becomes what is running: the one
            place somebody looks for the work is the place that started it. */}
        {jobs.length > 0 ? (
          <Link className="header-busy" to="/activity">
            <span className="header-busy-mark" aria-hidden="true" />
            {t(`jobs.${jobs[0].kind}`)}
            {jobs[0].ratio !== null && ` ${Math.round(jobs[0].ratio * 100)} %`}
          </Link>
        ) : (
          libraries.length > 0 && (
            <button className="button button-small" onClick={startScan} disabled={starting}>
              {t("home.scan")}
            </button>
          )
        )}

        <div className="header-choices">
          <Choice
            label={t("nav.theme")}
            value={theme}
            onChange={(value) => setTheme(value as ThemeChoice)}
            options={[
              ["system", t("theme.system")],
              ["dark", t("theme.dark")],
              ["light", t("theme.light")],
            ]}
          />
          <Choice
            label={t("nav.language")}
            value={language}
            onChange={(value) => setLanguage(value === "fr" ? "fr" : "en")}
            options={[
              ["en", "English"],
              ["fr", "Français"],
            ]}
          />
        </div>
      </div>
    </header>
  );
}

function Choice({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: [string, string][];
}) {
  return (
    <label className="choice">
      <span className="choice-label">{label}</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        {options.map(([option, wording]) => (
          <option key={option} value={option}>
            {wording}
          </option>
        ))}
      </select>
    </label>
  );
}

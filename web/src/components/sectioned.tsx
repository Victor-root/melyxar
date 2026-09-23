/*
 * A page made of sections, with the list of them down the side.
 *
 * The administration and somebody's own settings are both built this way:
 * one place per subject rather than one long column, and the list always in
 * view so that nobody has to remember where a setting lives.
 *
 * On a narrow screen the list becomes a strip of the same entries across the
 * top, which scrolls sideways: a column of twelve down a phone would push the
 * page itself below the fold.
 */

import { createContext, useContext } from "react";
import type { ComponentType, ReactNode } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import type { IconProps } from "../icons";
import { useSettings } from "../settings";

export interface Section {
  /** Under the page's own address; empty for the first one. */
  path: string;
  icon: ComponentType<IconProps>;
  /** The key of its name. */
  label: string;
  /** Drawn, and not wired to anything yet. */
  soon?: boolean;
}

export interface SectionGroup {
  /** The key of the word above them, when they have one. */
  label?: string;
  sections: Section[];
}

interface Where {
  /** The key of the name of the whole place, for the head of every page. */
  place: string;
  section: Section;
}

const WhereContext = createContext<Where | null>(null);

/** The section a page is drawn in. */
export function useSection(): Where {
  const where = useContext(WhereContext);
  if (!where) {
    throw new Error("a page head has to be drawn inside a sectioned page");
  }
  return where;
}

export function Sectioned({
  base,
  place,
  groups,
  foot,
}: {
  /** The address every section hangs off. */
  base: string;
  place: string;
  groups: SectionGroup[];
  /** What is written at the foot of the list. */
  foot?: ReactNode;
}) {
  const { t } = useSettings();
  const location = useLocation();
  const every = groups.flatMap((group) => group.sections);
  const here =
    every.find(
      (section) => section.path !== "" && location.pathname.startsWith(`${base}/${section.path}`),
    ) ?? every[0];

  return (
    <div className="sectioned">
      {/* The library's own light, behind the tools as well: it is the same
          server, and the two should not be set in two different rooms. */}
      <div className="home-backdrop drift" aria-hidden="true" />

      <aside className="side">
        <nav className="side-list" aria-label={t(place)}>
          {groups.map((group, index) => (
            <div className="side-group" key={group.label ?? index}>
              {group.label && <span className="side-group-name">{t(group.label)}</span>}
              {group.sections.map((section) => {
                const SectionIcon = section.icon;
                return (
                  <NavLink
                    key={section.path}
                    to={section.path === "" ? base : `${base}/${section.path}`}
                    end={section.path === ""}
                    className="side-line"
                  >
                    <SectionIcon size={20} />
                    <span className="side-line-name">{t(section.label)}</span>
                    {section.soon && <span className="soon">{t("admin.soon")}</span>}
                  </NavLink>
                );
              })}
            </div>
          ))}
        </nav>
        {foot && <div className="side-foot">{foot}</div>}
      </aside>

      <main className="sectioned-page">
        <WhereContext.Provider value={{ place, section: here }}>
          <Outlet />
        </WhereContext.Provider>
      </main>
    </div>
  );
}

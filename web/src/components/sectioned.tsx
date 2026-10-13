/*
 * A page made of sections, with the list of them down the side.
 *
 * The administration and somebody's own settings are both built this way:
 * one place per subject rather than one long column, and the list always in
 * view so that nobody has to remember where a setting lives.
 *
 * On a narrow screen the list becomes a strip of the same entries across the
 * top, which scrolls sideways: a column of twelve down a phone would push the
 * page itself below the fold. On a phone it is a page of its own, as the
 * settings of a phone are: the bare address shows the list, a section opens
 * whole, and a link above its title leads back.
 */

import { createContext, useContext, useRef } from "react";
import type { ComponentType, ReactNode } from "react";
import { Link, Outlet, useLocation, useNavigate } from "react-router-dom";
import { ChevronLeftIcon, ChevronRightIcon } from "../icons";
import type { IconProps } from "../icons";
import { useMasonry } from "../masonry";
import { ON_A_PHONE, useMediaQuery } from "../media-query";
import { useFindOnArrival } from "./finding";
import { useSettings } from "../settings";
import { useTabPage } from "../tab-page";
import { PageBackdrop } from "./backdrop";

export interface Section {
  /** Under the page's own address. The bare address shows the first one,
   *  except on a phone, where it is the list. */
  path: string;
  icon: ComponentType<IconProps>;
  /** The key of its name. */
  label: string;
  /** Drawn, and not wired to anything yet. */
  soon?: boolean;
  /** Takes the whole width of the screen instead of a reading width, for a
   *  page of figures and curves rather than of settings. */
  spread?: boolean;
}

export interface SectionGroup {
  /** The key of the word above them, when they have one. */
  label?: string;
  sections: Section[];
}

const SectionContext = createContext<Section | null>(null);

/** The section a page is drawn in. */
export function useSection(): Section {
  const section = useContext(SectionContext);
  if (!section) {
    throw new Error("a page head has to be drawn inside a sectioned page");
  }
  return section;
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
  const page = useRef<HTMLElement>(null);
  useMasonry(page);
  useFindOnArrival(page);
  const onAPhone = useMediaQuery(ON_A_PHONE);
  const every = groups.flatMap((group) => group.sections);
  const found = every.find((section) => location.pathname.startsWith(`${base}/${section.path}`));
  const here = found ?? every[0];
  const theList = onAPhone && !found;
  useTabPage(theList ? t(place) : `${t(place)} · ${t(here.label)}`);

  return (
    <div className="sectioned">
      {/* The library's own light, behind the tools as well: it is the same
          server, and the two should not be set in two different rooms. */}
      <PageBackdrop />

      {!onAPhone && (
        <aside className="side">
          <nav className="side-list" aria-label={t(place)}>
            {groups.map((group, index) => (
              <div className="side-group" key={group.label ?? index}>
                {group.label && <span className="side-group-name">{t(group.label)}</span>}
                {group.sections.map((section) => {
                  const SectionIcon = section.icon;
                  const active = section === here;
                  return (
                    <Link
                      key={section.path}
                      to={`${base}/${section.path}`}
                      className={`side-line${active ? " active" : ""}`}
                      aria-current={active ? "page" : undefined}
                    >
                      <SectionIcon size={20} />
                      <span className="side-line-name">{t(section.label)}</span>
                      {/* The short word: the list is narrow, and the name beside it has to stay whole. */}
                      {section.soon && (
                        <span className="soon" title={t("admin.soon")}>
                          {t("admin.soon_short")}
                        </span>
                      )}
                    </Link>
                  );
                })}
              </div>
            ))}
          </nav>
          {foot && <div className="side-foot">{foot}</div>}
        </aside>
      )}

      <main className="sectioned-page" data-spread={here.spread ? "yes" : undefined} ref={page}>
        {theList ? (
          <SectionList base={base} place={place} groups={groups} foot={foot} />
        ) : (
          <SectionContext.Provider value={here}>
            {onAPhone && <BackToTheList base={base} place={place} />}
            <Outlet />
          </SectionContext.Provider>
        )}
      </main>
    </div>
  );
}

/** Said by the list to the section it opens, so the way back is a step back
 *  rather than the list a second time on top of the history. */
const FROM_THE_LIST = { fromTheList: true };

/** Above a section on a phone: back to the list it was opened from, or to
 *  the list in place of the section when it was opened from elsewhere. */
function BackToTheList({ base, place }: { base: string; place: string }) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const { state } = useLocation();
  const cameFromTheList = (state as typeof FROM_THE_LIST | null)?.fromTheList === true;
  return (
    <button
      type="button"
      className="sectioned-back"
      onClick={() => (cameFromTheList ? navigate(-1) : navigate(base, { replace: true }))}
    >
      <ChevronLeftIcon size={18} />
      {t(place)}
    </button>
  );
}

/** The sections as a phone lists its settings: a card per group, a line per
 *  section, each leading to it. */
function SectionList({
  base,
  place,
  groups,
  foot,
}: {
  base: string;
  place: string;
  groups: SectionGroup[];
  foot?: ReactNode;
}) {
  const { t } = useSettings();
  return (
    <div className="section-list">
      <h1 className="section-list-title">{t(place)}</h1>
      {groups.map((group, index) => (
        <nav
          className="section-list-group"
          key={group.label ?? index}
          aria-label={group.label ? t(group.label) : t(place)}
        >
          {group.label && <span className="section-list-group-name">{t(group.label)}</span>}
          <div className="section-list-card">
            {group.sections.map((section) => {
              const SectionIcon = section.icon;
              return (
                <Link
                  key={section.path}
                  to={`${base}/${section.path}`}
                  state={FROM_THE_LIST}
                  className="section-list-line"
                >
                  <span className="section-list-mark" aria-hidden="true">
                    <SectionIcon size={20} />
                  </span>
                  <span className="section-list-name">{t(section.label)}</span>
                  {section.soon && (
                    <span className="soon" title={t("admin.soon")}>
                      {t("admin.soon_short")}
                    </span>
                  )}
                  <ChevronRightIcon size={18} />
                </Link>
              );
            })}
          </div>
        </nav>
      ))}
      {foot && <div className="section-list-foot">{foot}</div>}
    </div>
  );
}

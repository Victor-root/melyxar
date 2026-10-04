/*
 * The requests, on a page of their own for an account that may make them:
 * looking a title up to ask for it, and the requests it made. Reached from
 * the account's menu, by a line that shows only while requests are on and
 * this account may ask.
 */

import { NavLink } from "react-router-dom";
import { PageBackdrop } from "../components/backdrop";
import { RequestIcon } from "../icons";
import { useSettings } from "../settings";
import { MyRequests } from "./mine";
import { AskSearch } from "./search";
import { useRequests } from "./store";

export function RequestsPage({ view }: { view: "ask" | "mine" }) {
  const { t } = useSettings();
  const { access } = useRequests();
  if (access === null) {
    return null;
  }
  if (!access.may_ask) {
    return (
      <main className="page">
        <p className="notice">{t("error.not_found")}</p>
      </main>
    );
  }
  return (
    <>
      <PageBackdrop />
      <main className="page requests-page">
        <div className="browse-head">
          <div className="section-head">
            <h1>{t("requests.title")}</h1>
          </div>
          <div className="browse-bar">
            <nav className="browse-piece music-tabs" aria-label={t("requests.title")}>
              {(
                [
                  ["/requests", "requests.tab_ask", true],
                  ["/requests/mine", "requests.tab_mine", false],
                ] as const
              ).map(([to, name, end]) => (
                <NavLink
                  key={to}
                  to={to}
                  end={end}
                  className={({ isActive }) => `music-tab${isActive ? " music-tab-on" : ""}`}
                >
                  {t(name)}
                </NavLink>
              ))}
            </nav>
          </div>
        </div>
        {view === "ask" ? <AskSearch /> : <MyRequests />}
      </main>
    </>
  );
}

/** The line of the account's menu that leads here, while this account may
 *  ask. */
export function RequestsLine() {
  const { t } = useSettings();
  const { access } = useRequests();
  if (!access?.may_ask) {
    return null;
  }
  return (
    <NavLink to="/requests" className="header-menu-line">
      <RequestIcon size={16} />
      {t("requests.title")}
    </NavLink>
  );
}

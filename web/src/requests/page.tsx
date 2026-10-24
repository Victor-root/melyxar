/*
 * The requests, on a page of their own for an account that may make them:
 * looking a title up to ask for it, and the requests it made. Reached from a
 * button of the bar at the top, which shows only while requests are on and
 * this account may ask.
 */

import { useMemo } from "react";
import { PageBackdrop } from "../components/backdrop";
import { TabsBar } from "../components/tabs-bar";
import { useGoTo } from "../navigating";
import { useSettings } from "../settings";
import { MyRequests } from "./mine";
import { MoreTitles } from "./more";
import { AskSearch } from "./search";
import { useRequests } from "./store";
import { useTabPage } from "../tab-page";

export function RequestsPage({ view }: { view: "ask" | "mine" | "more" }) {
  const { t } = useSettings();
  const { access } = useRequests();
  const goTo = useGoTo();
  /* The same list from one drawing to the next: the bar of tabs measures
     itself again whenever it is handed another. */
  const tabs = useMemo(
    () =>
      [
        ["ask", t("requests.tab_ask")],
        ["mine", t("requests.tab_mine")],
      ] as const,
    [t],
  );
  useTabPage(t("requests.title"));
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
          {/* The same tabs as a library of music's, in the band at the top
              on a phone. Asking for more of a kind is part of asking. */}
          <div className="browse-bar">
            <TabsBar
              tabs={tabs}
              open={view === "mine" ? "mine" : "ask"}
              onOpen={(tab) => goTo(tab === "mine" ? "/requests/mine" : "/requests")}
              label={t("requests.title")}
            />
          </div>
        </div>
        {view === "ask" && <AskSearch />}
        {view === "mine" && <MyRequests />}
        {view === "more" && <MoreTitles />}
      </main>
    </>
  );
}

/*
 * What is said and offered of a title beside it: here already, asked for by
 * this account, asked for by others, or free to ask for. The same on a card
 * of a search and on the page of the title.
 */

import { Link } from "react-router-dom";
import { seasonsNamed } from "../readable";
import { useSettings } from "../settings";
import type { Found } from "./api";
import { standingOf } from "./standing";
import { useRequests } from "./store";

export function StandingBar({ found, onAsk }: { found: Found; onAsk: () => void }) {
  const { t } = useSettings();
  const { mine } = useRequests();
  const standing = standingOf(found, mine);
  const others = standing.is === "here" ? 0 : standing.others;
  return (
    <div className="request-standing">
      {standing.is === "here" && (
        <>
          <span className="state-pill state-ok">{t("requests.here")}</span>
          {standing.workId && (
            <Link className="button button-accent" to={`/work/${standing.workId}`}>
              {t("requests.open")}
            </Link>
          )}
        </>
      )}
      {standing.is === "mine" && (
        <Link className="state-pill state-news" to="/requests/mine">
          {t("requests.asked_by_you")}
        </Link>
      )}
      {standing.is === "free" && (
        <>
          {standing.heldSeasons.length > 0 && (
            <span className="state-pill state-ok">
              {t("requests.here_in_part", { seasons: seasonsNamed(standing.heldSeasons, t) })}
            </span>
          )}
          <button type="button" className="button button-accent" onClick={onAsk}>
            {others > 0 ? t("requests.ask_too") : t("requests.ask")}
          </button>
        </>
      )}
      {others > 0 && (
        <span className="request-others">
          {others === 1 ? t("requests.others_one") : t("requests.others", { count: others })}
        </span>
      )}
    </div>
  );
}

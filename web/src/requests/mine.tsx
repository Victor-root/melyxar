/*
 * The requests this account made, newest first, each with what became of
 * it: waiting, accepted, refused with the administrator's word, or added.
 * One that waits can be cancelled, and one refused or added cleared from
 * the list; an accepted one stays until it arrives.
 */

import { Link } from "react-router-dom";
import { refusalAbout } from "../asking";
import { useToast } from "../notifications/toasts";
import { readableDate, seasonsNamed } from "../readable";
import { useSettings } from "../settings";
import type { RequestState, TitleRequest } from "./api";
import { TitleCard, titleAddress } from "./card";
import { useRequests } from "./store";

/** The colour each state wears. */
export const STATE_COLOUR: Record<RequestState, string> = {
  pending: "state-news",
  accepted: "state-ok",
  refused: "state-trouble",
  added: "state-ok",
};

function MyRequest({ request }: { request: TitleRequest }) {
  const { t, language } = useSettings();
  const toast = useToast();
  const { withdraw } = useRequests();
  const takeBack = () =>
    withdraw(request.id).catch((error: unknown) =>
      toast({ state: "trouble", title: t("requests.not_withdrawn"), detail: t(refusalAbout(error, "requests")) }),
    );
  return (
    <TitleCard
      catalogue={request.catalogue}
      title={request.title}
      year={request.year}
      poster={request.poster}
      to={titleAddress(request.catalogue, request.tmdb_id)}
    >
      <div className="request-standing">
        <span className={`state-pill ${STATE_COLOUR[request.state]}`}>{t(`requests.state.${request.state}`)}</span>
        <span className="request-when">{readableDate(request.created_at, language)}</span>
      </div>
      {request.seasons.length > 0 && <p className="request-line">{seasonsNamed(request.seasons, t)}</p>}
      {request.note && <p className="request-line request-quoted">{request.note}</p>}
      {request.state === "refused" && request.answer && (
        <p className="request-line">
          <span className="request-label">{t("requests.answer")}</span> {request.answer}
        </p>
      )}
      <div className="request-actions">
        {request.state === "added" && request.work_id && (
          <Link className="button button-accent" to={`/work/${request.work_id}`}>
            {t("requests.open")}
          </Link>
        )}
        {request.state === "pending" && (
          <button type="button" className="button" onClick={() => void takeBack()}>
            {t("requests.cancel")}
          </button>
        )}
        {(request.state === "refused" || request.state === "added") && (
          <button type="button" className="button" onClick={() => void takeBack()}>
            {t("requests.clear")}
          </button>
        )}
      </div>
    </TitleCard>
  );
}

export function MyRequests() {
  const { t } = useSettings();
  const { mine } = useRequests();
  if (mine === null) {
    return null;
  }
  if (mine.length === 0) {
    return (
      <p className="notice">
        {t("requests.none_yet")} <Link to="/requests">{t("requests.go_ask")}</Link>
      </p>
    );
  }
  return (
    <div className="request-grid">
      {mine.map((request) => (
        <MyRequest key={request.id} request={request} />
      ))}
    </div>
  );
}

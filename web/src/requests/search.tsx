/*
 * Looking a title up to ask for it: a name, an identifier or a link pasted
 * whole, answered by the provider, films and series together. Each answer
 * says whether it is here already, asked for already, or free to ask for.
 * The words stay in the address, so coming back finds the same answers.
 */

import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { refusalAbout, useAsked } from "../asking";
import { SearchIcon } from "../icons";
import { seasonsNamed } from "../readable";
import { useSettings } from "../settings";
import { requestsApi } from "./api";
import type { Found } from "./api";
import { AskDialog } from "./ask";
import { TitleCard } from "./card";
import { standingOf } from "./standing";
import { useRequests } from "./store";

/** How long typing rests before the provider is asked. */
const ASKED_AFTER_MS = 450;

function Answer({ found, onAsk }: { found: Found; onAsk: () => void }) {
  const { t } = useSettings();
  const { mine } = useRequests();
  const standing = standingOf(found, mine);
  const others = standing.is === "here" ? 0 : standing.others;
  return (
    <TitleCard
      catalogue={found.catalogue}
      title={found.title}
      year={found.year}
      poster={found.poster}
      overview={found.overview}
    >
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
    </TitleCard>
  );
}

export function AskSearch() {
  const { t, language } = useSettings();
  const [parameters, setParameters] = useSearchParams();
  const words = parameters.get("query") ?? "";
  const [typed, setTyped] = useState(words);
  const [asking, setAsking] = useState<Found | null>(null);

  useEffect(() => {
    const resting = window.setTimeout(() => {
      if (typed.trim() !== words) {
        setParameters(typed.trim() ? { query: typed.trim() } : {}, { replace: true });
      }
    }, ASKED_AFTER_MS);
    return () => window.clearTimeout(resting);
  }, [typed, words, setParameters]);

  const found = useAsked(
    (signal) => (words ? requestsApi.search(words, language, signal) : Promise.resolve([])),
    [words, language],
    `requests:${language}:${words}`,
  );

  return (
    <>
      <form
        className="request-search"
        role="search"
        onSubmit={(event) => {
          event.preventDefault();
          setParameters(typed.trim() ? { query: typed.trim() } : {}, { replace: true });
        }}
      >
        <SearchIcon size={26} />
        <input
          type="search"
          autoFocus
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
          placeholder={t("requests.search_placeholder")}
          aria-label={t("requests.search_placeholder")}
        />
      </form>
      <p className="request-hint">{t("requests.search_hint")}</p>
      {found.failure && <p className="notice">{t(refusalAbout(found.failure, "requests"))}</p>}
      {words && found.answer?.length === 0 && !found.waiting && (
        <p className="notice">{t("requests.nothing_found")}</p>
      )}
      <div className="request-grid">
        {(found.answer ?? []).map((one) => (
          <Answer key={`${one.catalogue}:${one.tmdb_id}`} found={one} onAsk={() => setAsking(one)} />
        ))}
      </div>
      {asking && <AskDialog found={asking} onClose={() => setAsking(null)} />}
    </>
  );
}

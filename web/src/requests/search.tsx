/*
 * Looking a title up to ask for it: a name, an identifier or a link pasted
 * whole, answered by the provider, films and series together. Each answer
 * says whether it is here already, asked for already, or free to ask for.
 * The words stay in the address, so coming back finds the same answers.
 * While nothing is typed, the page shows suggestions instead of staying empty.
 */

import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { refusalAbout, useAsked } from "../asking";
import { SearchIcon } from "../icons";
import { useSettings } from "../settings";
import { requestsApi } from "./api";
import type { Found } from "./api";
import { AskDialog } from "./ask";
import { Answer } from "./answer";
import { Suggestions } from "./suggestions";

/** How long typing rests before the provider is asked. */
const ASKED_AFTER_MS = 450;

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
        <SearchIcon size={20} />
        <input
          type="search"
          autoFocus
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
          placeholder={t("requests.search_placeholder")}
          aria-label={t("requests.search_placeholder")}
        />
      </form>
      {found.failure && <p className="notice">{t(refusalAbout(found.failure, "requests"))}</p>}
      {words && found.answer?.length === 0 && !found.waiting && (
        <p className="notice">{t("requests.nothing_found")}</p>
      )}
      <div className="request-grid">
        {(found.answer ?? []).map((one) => (
          <Answer key={`${one.catalogue}:${one.tmdb_id}`} found={one} onAsk={() => setAsking(one)} />
        ))}
      </div>
      {!words && <Suggestions />}
      {asking && <AskDialog found={asking} onClose={() => setAsking(null)} />}
    </>
  );
}

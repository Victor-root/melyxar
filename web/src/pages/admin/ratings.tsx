/*
 * The ratings from elsewhere: IMDb's, which need nothing, and Rotten
 * Tomatoes', which need a key the administrator asks OMDb for, with the steps
 * to get one written out beside the field.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import type { RatingsSettings } from "../../api";
import { refusalOf, useAsked, useTold } from "../../asking";
import { Panel, Setting } from "../../components/panel";
import { refusalKey } from "../../i18n";
import { DeleteIcon, StarIcon } from "../../icons";
import { howLongSince } from "../../readable";
import { useRunning } from "../../running";
import { useSettings } from "../../settings";

/** Where OMDb hands out its keys. */
const OMDB_KEYS = "https://www.omdbapi.com/apikey.aspx";

export function RatingsPanel() {
  const { t } = useSettings();
  const { finished, watch } = useRunning();
  // Asked again when a task ends, which is when a new file of IMDb ratings
  // has come in.
  const asked = useAsked((signal) => api.ratingsSettings(signal), [finished]);
  const [kept, setKept] = useState<RatingsSettings | null>(null);
  const [typed, setTyped] = useState("");
  useEffect(() => setKept(null), [asked.answer]);
  const shown = kept ?? asked.answer;

  const told = useTold(async (act: () => Promise<RatingsSettings>) => {
    const answer = await act();
    setKept(answer);
    if (answer.tried === "kept") {
      setTyped("");
      watch();
    }
  });

  const said =
    shown?.tried === "kept"
      ? "admin.omdb_set"
      : shown?.tried === "refused"
        ? "admin.omdb_refused"
        : shown?.tried === "unreachable"
          ? "admin.omdb_unreachable"
          : null;

  return (
    <Panel icon={StarIcon} title={t("admin.ratings")} lead={t("admin.ratings_lead")}>
      <Setting label={t("admin.imdb")} why={t("admin.imdb_why")}>
        {shown && (
          <span className={`state-pill ${shown.imdb_fetched_at ? "state-ok" : "state-attention"}`}>
            <span className="state-dot" aria-hidden="true" />
            {shown.imdb_fetched_at
              ? t("admin.imdb_fetched", { since: howLongSince(shown.imdb_fetched_at, Date.now(), t) })
              : t("admin.imdb_never")}
          </span>
        )}
      </Setting>

      <Setting label={t("admin.omdb")} why={t("admin.omdb_why")}>
        {shown && (
          <form
            className="name-choice"
            onSubmit={(event) => {
              event.preventDefault();
              told.tell(() => api.setOmdbKey(typed));
            }}
          >
            <span className="name-field">
              <input
                type="text"
                className="field-line"
                aria-label={t("admin.omdb_key")}
                placeholder={shown.has_omdb_key ? "••••••••" : t("admin.omdb_placeholder")}
                autoComplete="off"
                spellCheck={false}
                value={typed}
                onChange={(event) => setTyped(event.target.value)}
              />
            </span>
            <button
              type="submit"
              className="button button-small button-accent"
              disabled={told.busy || typed.trim() === ""}
            >
              {t("admin.omdb_keep")}
            </button>
            {shown.has_omdb_key && (
              <button
                type="button"
                className="button button-small button-quiet"
                disabled={told.busy}
                onClick={() => told.tell(api.forgetOmdbKey)}
              >
                <DeleteIcon size={15} />
                {t("admin.omdb_forget")}
              </button>
            )}
          </form>
        )}
      </Setting>

      {said && (
        <p className={`panel-notice${shown?.tried === "kept" ? " panel-notice-ok" : " panel-notice-trouble"}`}>
          {t(said)}
        </p>
      )}
      {told.failure && (
        <p className="panel-notice panel-notice-trouble">{t(refusalKey(refusalOf(told.failure)))}</p>
      )}

      <details className="ratings-guide" open={shown !== null && !shown.has_omdb_key}>
        <summary>{t("admin.omdb_guide")}</summary>
        <ol>
          <li>
            {t("admin.omdb_step_open")}{" "}
            <a href={OMDB_KEYS} target="_blank" rel="noopener noreferrer">
              omdbapi.com/apikey.aspx
            </a>
          </li>
          <li>{t("admin.omdb_step_form")}</li>
          <li>{t("admin.omdb_step_activate")}</li>
          <li>{t("admin.omdb_step_paste")}</li>
        </ol>
        <p>{t("admin.omdb_limit")}</p>
      </details>
    </Panel>
  );
}

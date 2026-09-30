/*
 * What the server asks OpenSubtitles with: the key of the administrator's
 * application and their account, with the steps to get both written out
 * beside the fields.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import type { OpenSubtitlesSettings } from "../../api";
import { refusalOf, useAsked, useTold } from "../../asking";
import { Panel, Setting } from "../../components/panel";
import { refusalKey } from "../../i18n";
import { DeleteIcon, SubtitlesIcon } from "../../icons";
import { useSettings } from "../../settings";

/** Where an account is made, and where its application gets its key. */
const SIGN_UP = "https://www.opensubtitles.com/newuser";
const CONSUMERS = "https://www.opensubtitles.com/consumers";

export function OnlineSubtitlesPanel() {
  const { t } = useSettings();
  const asked = useAsked((signal) => api.openSubtitles(signal), []);
  const [kept, setKept] = useState<OpenSubtitlesSettings | null>(null);
  const [key, setKey] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  useEffect(() => setKept(null), [asked.answer]);
  const shown = kept ?? asked.answer;

  const told = useTold(async (act: () => Promise<OpenSubtitlesSettings>) => {
    const answer = await act();
    setKept(answer);
    if (answer.tried === "kept") {
      setKey("");
      setUsername("");
      setPassword("");
    }
  });

  const said =
    shown?.tried === "kept"
      ? "admin.opensubtitles_set"
      : shown?.tried === "refused"
        ? "admin.opensubtitles_refused"
        : shown?.tried === "unreachable"
          ? "admin.omdb_unreachable"
          : null;

  return (
    <Panel
      icon={SubtitlesIcon}
      title={t("admin.opensubtitles")}
      lead={t("admin.opensubtitles_lead")}
    >
      {shown && (
        <Setting label={t("admin.opensubtitles_state")}>
          <span
            className={`state-pill ${shown.has_key ? "state-ok" : "state-attention"}`}
          >
            <span className="state-dot" aria-hidden="true" />
            {t(
              shown.signed_in
                ? "admin.opensubtitles_signed_in"
                : shown.has_key
                  ? "admin.opensubtitles_key_only"
                  : "admin.opensubtitles_none",
            )}
          </span>
        </Setting>
      )}
      <form
        className="opensubtitles-form"
        onSubmit={(event) => {
          event.preventDefault();
          told.tell(() => api.setOpenSubtitles(key, username, password));
        }}
      >
        <input
          type="text"
          className="field-line"
          aria-label={t("admin.opensubtitles_key")}
          placeholder={t("admin.opensubtitles_key")}
          autoComplete="off"
          spellCheck={false}
          value={key}
          onChange={(event) => setKey(event.target.value)}
        />
        <input
          type="text"
          className="field-line"
          aria-label={t("admin.opensubtitles_username")}
          placeholder={t("admin.opensubtitles_username")}
          autoComplete="off"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
        />
        <input
          type="password"
          className="field-line"
          aria-label={t("admin.opensubtitles_password")}
          placeholder={t("admin.opensubtitles_password")}
          autoComplete="new-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        <span className="opensubtitles-actions">
          <button
            type="submit"
            className="button button-small button-accent"
            disabled={told.busy || key.trim() === ""}
          >
            {t("admin.omdb_keep")}
          </button>
          {shown?.has_key && (
            <button
              type="button"
              className="button button-small button-quiet"
              disabled={told.busy}
              onClick={() => told.tell(api.forgetOpenSubtitles)}
            >
              <DeleteIcon size={15} />
              {t("admin.omdb_forget")}
            </button>
          )}
        </span>
      </form>

      {said && (
        <p
          className={`panel-notice${shown?.tried === "kept" ? " panel-notice-ok" : " panel-notice-trouble"}`}
        >
          {t(said)}
        </p>
      )}
      {told.failure && (
        <p className="panel-notice panel-notice-trouble">
          {t(refusalKey(refusalOf(told.failure)))}
        </p>
      )}

      <details
        className="ratings-guide"
        open={shown !== null && !shown.has_key}
      >
        <summary>{t("admin.omdb_guide")}</summary>
        <ol>
          <li>
            {t("admin.opensubtitles_step_account")}{" "}
            <a href={SIGN_UP} target="_blank" rel="noopener noreferrer">
              opensubtitles.com/newuser
            </a>
          </li>
          <li>
            {t("admin.opensubtitles_step_consumer")}{" "}
            <a href={CONSUMERS} target="_blank" rel="noopener noreferrer">
              opensubtitles.com/consumers
            </a>
          </li>
          <li>{t("admin.opensubtitles_step_new")}</li>
          <li>{t("admin.opensubtitles_step_paste")}</li>
        </ol>
        <p>{t("admin.opensubtitles_limit")}</p>
      </details>
    </Panel>
  );
}

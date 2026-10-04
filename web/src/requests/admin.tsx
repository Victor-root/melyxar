/*
 * The requests, for the administrator: whether accounts may make them, who
 * may, and the titles waiting, each with every account that asked for it.
 * A decision is shown at once and put back if the server refuses; what
 * moves elsewhere arrives by the live line.
 */

import { useEffect, useState } from "react";
import { refusalAbout, useAsked } from "../asking";
import { PageHead, Panel, Setting, Toggle } from "../components/panel";
import { PeopleIcon, RequestIcon } from "../icons";
import { useRequestsNews } from "../live";
import { useToast } from "../notifications/toasts";
import { readableDate, seasonsNamed } from "../readable";
import { useSettings } from "../settings";
import { requestsApi } from "./api";
import type { Decision, RequestsAdministration, Waiting } from "./api";
import { TitleCard } from "./card";

/** The longest word a refusal keeps. */
const LONGEST_ANSWER = 500;

function WaitingTitle({
  title,
  onDecide,
}: {
  title: Waiting;
  onDecide: (decision: Decision, answer: string) => void;
}) {
  const { t, language } = useSettings();
  const [refusing, setRefusing] = useState(false);
  const [answer, setAnswer] = useState("");
  const first = title.requests[0];
  return (
    <TitleCard
      catalogue={title.catalogue}
      title={first.title}
      year={first.year}
      poster={first.poster}
      overview={first.overview}
    >
      <div className="request-standing">
        <span className={`state-pill ${title.accepted ? "state-ok" : "state-news"}`}>
          {t(title.accepted ? "requests.state.accepted" : "requests.state.pending")}
        </span>
        <span className="request-others">
          {title.requests.length === 1
            ? t("requests.askers_one")
            : t("requests.askers", { count: title.requests.length })}
        </span>
      </div>
      <ul className="request-askers">
        {title.requests.map((request) => (
          <li key={request.id}>
            <span className="request-asker">{request.user_name}</span>
            <span className="request-when">{readableDate(request.created_at, language)}</span>
            {request.seasons.length > 0 && <span>{seasonsNamed(request.seasons, t)}</span>}
            {request.note && <span className="request-quoted">{request.note}</span>}
          </li>
        ))}
      </ul>
      {refusing ? (
        <div className="request-refusal">
          <textarea
            className="field-line"
            rows={2}
            maxLength={LONGEST_ANSWER}
            autoFocus
            placeholder={t("requests.answer_placeholder")}
            aria-label={t("requests.answer")}
            value={answer}
            onChange={(event) => setAnswer(event.target.value)}
          />
          <div className="request-actions">
            <button type="button" className="button button-accent" onClick={() => onDecide("refused", answer)}>
              {t("requests.refuse")}
            </button>
            <button type="button" className="button" onClick={() => setRefusing(false)}>
              {t("requests.back")}
            </button>
          </div>
        </div>
      ) : (
        <div className="request-actions">
          {!title.accepted && (
            <button type="button" className="button button-accent" onClick={() => onDecide("accepted", "")}>
              {t("requests.accept")}
            </button>
          )}
          <button type="button" className="button" onClick={() => onDecide("added", "")}>
            {t("requests.mark_added")}
          </button>
          <button type="button" className="button" onClick={() => setRefusing(true)}>
            {t("requests.refuse")}
          </button>
        </div>
      )}
    </TitleCard>
  );
}

export function AdminRequests() {
  const { t } = useSettings();
  const toast = useToast();
  const asked = useAsked((signal) => requestsApi.administration(signal), []);
  useRequestsNews(asked.look);
  const [held, setHeld] = useState<RequestsAdministration | null>(null);
  const shown = held ?? asked.answer;
  /* What the server says next replaces what was shown ahead of it. */
  useEffect(() => setHeld(null), [asked.answer]);

  /** Shows a change at once, and puts it back if the server refuses. */
  const change = (next: RequestsAdministration, save: () => Promise<unknown>) => {
    const before = shown;
    setHeld(next);
    save().catch((error: unknown) => {
      setHeld(before);
      toast({ state: "trouble", title: t("requests.failed"), detail: t(refusalAbout(error, "requests")) });
    });
  };

  if (!shown) {
    return <PageHead lead={t("admin.requests_lead")} />;
  }
  const decide = (title: Waiting, decision: Decision, answer: string) =>
    change(
      {
        ...shown,
        waiting:
          decision === "accepted"
            ? shown.waiting.map((one) => (one === title ? { ...one, accepted: true } : one))
            : shown.waiting.filter((one) => one !== title),
      },
      () => requestsApi.decide(title.catalogue, title.tmdb_id, decision, answer),
    );

  return (
    <>
      <PageHead lead={t("admin.requests_lead")} />
      <Panel icon={RequestIcon} title={t("requests.switch")} lead={t("requests.switch_why")}>
        <Setting label={t("requests.switch_on")}>
          <Toggle
            label={t("requests.switch_on")}
            checked={shown.enabled}
            onChange={(enabled) => change({ ...shown, enabled }, () => requestsApi.switch(enabled))}
          />
        </Setting>
      </Panel>
      {shown.enabled && (
        <Panel icon={PeopleIcon} title={t("requests.who")} lead={t("requests.who_why")}>
          {shown.askers.map((asker) => (
            <Setting
              key={asker.id}
              label={asker.name}
              why={asker.administrator ? t("requests.always_administrator") : undefined}
            >
              <Toggle
                label={asker.name}
                checked={asker.may_ask}
                disabled={asker.administrator}
                onChange={(may_ask) =>
                  change(
                    {
                      ...shown,
                      askers: shown.askers.map((one) => (one.id === asker.id ? { ...one, may_ask } : one)),
                    },
                    () => requestsApi.allow(asker.id, may_ask),
                  )
                }
              />
            </Setting>
          ))}
        </Panel>
      )}
      <div className="section-head">
        <h2>{t("requests.waiting")}</h2>
      </div>
      {shown.waiting.length === 0 ? (
        <p className="notice">{t("requests.waiting_none")}</p>
      ) : (
        <div className="request-grid">
          {shown.waiting.map((title) => (
            <WaitingTitle
              key={`${title.catalogue}:${title.tmdb_id}`}
              title={title}
              onDecide={(decision, answer) => decide(title, decision, answer)}
            />
          ))}
        </div>
      )}
    </>
  );
}

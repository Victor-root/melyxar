/*
 * The models that listen to personal videos: which one is in use, which are
 * on the disk, and the buttons that fetch and forget them.
 */

import { useEffect } from "react";
import { api } from "../../api";
import type { SpeechEffort, SpeechModel, SpeechStatus } from "../../api";
import { refusalAbout, useAsked, useTold } from "../../asking";
import { Panel, Picker, Setting } from "../../components/panel";
import { DeleteIcon, SubtitlesIcon } from "../../icons";
import { asSize } from "../../player/describe";
import { useSettings } from "../../settings";

/** Every effort listening may take, as it is kept. */
const EFFORTS: SpeechEffort[] = ["quiet", "balanced", "maximum"];

/** How often a download is looked at again while one is under way. */
const WHILE_DOWNLOADING_MS = 3_000;

export function SpeechPanel() {
  const { t } = useSettings();
  const asked = useAsked((signal) => api.speech(signal), []);
  const shown = asked.answer;
  const downloading = shown?.models.some((model) => model.downloading) ?? false;

  useEffect(() => {
    if (!downloading) return;
    const beat = window.setInterval(asked.look, WHILE_DOWNLOADING_MS);
    return () => window.clearInterval(beat);
  }, [downloading, asked.look]);

  const told = useTold(async (act: () => Promise<unknown>) => {
    await act();
    asked.look();
  });

  return (
    <Panel
      icon={SubtitlesIcon}
      title={t("admin.speech")}
      lead={t("admin.speech_lead")}
    >
      {shown && !shown.tool_found && (
        <p className="panel-notice panel-notice-trouble">
          {t("admin.speech_no_tool")}
        </p>
      )}
      {shown && shown.tool_found && (
        <>
          {shown.models.map((model) => (
            <ModelLine
              key={model.id}
              model={model}
              status={shown}
              busy={told.busy}
              tell={told.tell}
            />
          ))}
          <Setting
            label={t("admin.speech_effort")}
            why={t("admin.speech_effort_why")}
          >
            <Picker
              label={t("admin.speech_effort")}
              value={shown.effort}
              options={EFFORTS.map(
                (effort) => [effort, t(`admin.speech_effort.${effort}`)] as const,
              )}
              disabled={told.busy}
              onPick={(effort) => told.tell(() => api.setSpeechEffort(effort))}
            />
          </Setting>
          {shown.chosen === null && (
            <p className="panel-notice">{t("admin.speech_none_chosen_note")}</p>
          )}
        </>
      )}
      {told.failure && (
        <p className="panel-notice panel-notice-trouble">
          {t(refusalAbout(told.failure, "speech"))}
        </p>
      )}
    </Panel>
  );
}

function ModelLine({
  model,
  status,
  busy,
  tell,
}: {
  model: SpeechModel;
  status: SpeechStatus;
  busy: boolean;
  tell: (act: () => Promise<unknown>) => Promise<void>;
}) {
  const { t } = useSettings();
  const inUse = status.chosen === model.id;
  const state = inUse
    ? "admin.speech_in_use"
    : model.downloading
      ? "admin.speech_downloading"
      : model.downloaded
        ? "admin.speech_ready"
        : "admin.speech_absent";
  return (
    <Setting
      label={`${t(`admin.speech_model.${model.id}`)} (${asSize(model.bytes)})`}
      why={t(`admin.speech_model.${model.id}_why`)}
    >
      <span className="opensubtitles-actions">
        <span
          className={`state-pill ${inUse ? "state-ok" : model.downloaded ? "" : "state-attention"}`}
        >
          <span className="state-dot" aria-hidden="true" />
          {t(state)}
        </span>
        {model.downloaded && !inUse && (
          <button
            type="button"
            className="button button-small button-accent"
            disabled={busy}
            onClick={() => tell(() => api.chooseSpeechModel(model.id))}
          >
            {t("admin.speech_use")}
          </button>
        )}
        {inUse && (
          <button
            type="button"
            className="button button-small button-quiet"
            disabled={busy}
            onClick={() => tell(() => api.chooseSpeechModel(null))}
          >
            {t("admin.speech_stop")}
          </button>
        )}
        {!model.downloaded && !model.downloading && (
          <button
            type="button"
            className="button button-small button-accent"
            disabled={busy}
            onClick={() => tell(() => api.downloadSpeechModel(model.id))}
          >
            {t("admin.speech_download")}
          </button>
        )}
        {model.downloaded && (
          <button
            type="button"
            className="button button-small button-quiet"
            disabled={busy}
            onClick={() => tell(() => api.forgetSpeechModel(model.id))}
          >
            <DeleteIcon size={15} />
            {t("admin.speech_forget")}
          </button>
        )}
      </span>
    </Setting>
  );
}

/*
 * How a film is made into what a screen can play: the card that does it, the
 * pictures of the playback bar, and the colours of a wide gamut film.
 */

import { NumberField, PageHead, Panel, Setting, Stat, Toggle } from "../../components/panel";
import {
  GraphicsCardIcon,
  ImageIcon,
  PlaybackIcon,
  TranscodeIcon,
} from "../../icons";
import { useLibraryWork, usePlaybackSettings } from "../../screens/settings";
import { useSettings } from "../../settings";
import { useOverview } from "./layout";

export function AdminTranscoding() {
  const { t } = useSettings();
  const overview = useOverview().answer;
  const work = useLibraryWork();
  const playback = usePlaybackSettings();
  const failed = work.failed ?? playback.failed;

  return (
    <>
      <PageHead lead={t("admin.transcoding_lead")} />

      {failed && (
        <p className="panel-notice panel-notice-trouble">
          {t(failed === "not_kept" ? "settings.not_kept" : "error.unreachable")}
        </p>
      )}

      <div className="panels">
        <Panel icon={GraphicsCardIcon} title={t("admin.card")} lead={t("admin.card_lead")}>
          <div className="stats">
            <Stat
              icon={TranscodeIcon}
              label={t("admin.media_tools")}
              value={overview ? t(overview.media_tools.found ? "admin.found" : "admin.missing") : "–"}
              note={overview?.media_tools.version ?? undefined}
              state={overview ? (overview.media_tools.found ? "ok" : "trouble") : undefined}
            />
            <Stat
              icon={GraphicsCardIcon}
              label={t("admin.card")}
              value={
                overview
                  ? overview.media_tools.card
                    ? overview.media_tools.card.toUpperCase()
                    : t("admin.card_unused")
                  : "–"
              }
              state={overview ? (overview.media_tools.card ? "ok" : "attention") : undefined}
            />
          </div>
        </Panel>

        <Panel icon={PlaybackIcon} title={t("admin.limits")} lead={t("admin.limits_lead")} soon>
          <Setting label={t("admin.limit_sessions")} why={t("admin.limit_sessions_why")} soon>
            <input className="field-line field-number" disabled value="–" readOnly />
          </Setting>
          <Setting label={t("admin.limit_room")} why={t("admin.limit_room_why")} soon>
            <input className="field-line field-number" disabled value="–" readOnly />
          </Setting>
        </Panel>
      </div>

      {playback.kept && (
        <Panel icon={ImageIcon} title={t("settings.picture")}>
          <Setting
            label={t("settings.tone_mapping_disabled")}
            why={t("settings.tone_mapping_disabled_why")}
          >
            <Toggle
              label={t("settings.tone_mapping_disabled")}
              checked={playback.kept.tone_mapping_disabled}
              onChange={(tone_mapping_disabled) => playback.setTo({ tone_mapping_disabled })}
            />
          </Setting>
        </Panel>
      )}

      {work.kept && (
        <Panel icon={ImageIcon} title={t("settings.thumbnails")} lead={t("settings.thumbnails_why")}>
          <Setting label={t("settings.thumbnails_on")}>
            <Toggle
              label={t("settings.thumbnails_on")}
              checked={work.kept.thumbnails_enabled}
              onChange={(thumbnails_enabled) => work.setTo({ thumbnails_enabled })}
            />
          </Setting>
          <Setting label={t("settings.thumbnails_every")}>
            <NumberField
              label={t("settings.thumbnails_every")}
              value={work.kept.thumbnails_every_seconds}
              min={1}
              max={600}
              disabled={!work.kept.thumbnails_enabled}
              onPick={(thumbnails_every_seconds) => work.setTo({ thumbnails_every_seconds })}
            />
          </Setting>
          <Setting label={t("settings.thumbnails_height")}>
            <NumberField
              label={t("settings.thumbnails_height")}
              value={work.kept.thumbnails_height}
              min={1}
              max={1080}
              disabled={!work.kept.thumbnails_enabled}
              onPick={(thumbnails_height) => work.setTo({ thumbnails_height })}
            />
          </Setting>
          {/* Said before the change and not after it: changing the shape puts
              every film back in front of the upkeep. */}
          <Setting
            label={t("admin.thumbnails_grid")}
            why={t("settings.thumbnails_shape_why")}
          >
            <NumberField
              label={t("settings.thumbnails_columns")}
              value={work.kept.thumbnails_columns}
              min={1}
              max={20}
              disabled={!work.kept.thumbnails_enabled}
              onPick={(thumbnails_columns) => work.setTo({ thumbnails_columns })}
            />
            <span className="setting-times" aria-hidden="true">×</span>
            <NumberField
              label={t("settings.thumbnails_rows")}
              value={work.kept.thumbnails_rows}
              min={1}
              max={20}
              disabled={!work.kept.thumbnails_enabled}
              onPick={(thumbnails_rows) => work.setTo({ thumbnails_rows })}
            />
          </Setting>
        </Panel>
      )}
    </>
  );
}

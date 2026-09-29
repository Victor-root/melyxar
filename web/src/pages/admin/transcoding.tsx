/*
 * How a film is made into what a screen can play: the card that does it, the
 * pictures of the playback bar, the colours of a wide gamut film, and what the
 * films it converts are allowed: how many at once, how much of the disk, and
 * the codecs they come out in.
 */

import type { VideoCodec } from "../../api";
import { NumberField, PageHead, Panel, Setting, Stat, Toggle } from "../../components/panel";
import {
  FfmpegIcon,
  GraphicsCardIcon,
  ImageIcon,
  PlaybackIcon,
} from "../../icons";
import { useLibraryWork, usePlaybackSettings } from "../../screens/settings";
import { useSettings } from "../../settings";
import { useOverview } from "./layout";

/** Every codec a converted film may come out in, best first, as the server
    keeps them. */
const EVERY_CODEC: VideoCodec[] = ["av1", "hevc", "h264"];

/** The ceiling a server is given when one is first switched on. */
const A_FIRST_CEILING = 2;
/** The room the cache is given when a ceiling is first switched on, in
    gigabytes. */
const A_FIRST_ROOM_GB = 8;
const MEGABYTES_IN_A_GB = 1024;

export function AdminTranscoding() {
  const { t } = useSettings();
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
        <CardPanel />

        <Panel icon={PlaybackIcon} title={t("admin.limits")} lead={t("admin.limits_lead")}>
          {playback.kept && (
            <Setting label={t("admin.limit_sessions")} why={t("admin.limit_sessions_why")}>
              <Toggle
                label={t("admin.limit_sessions")}
                checked={playback.kept.max_transcoding_sessions !== null}
                onChange={(limited) =>
                  playback.setTo({
                    max_transcoding_sessions: limited ? A_FIRST_CEILING : null,
                  })
                }
              />
              {/* Shown greyed while there is no ceiling, so what switching it
                  on would set can be read beforehand. */}
              <NumberField
                label={t("admin.limit_sessions_most")}
                value={playback.kept.max_transcoding_sessions ?? A_FIRST_CEILING}
                min={1}
                max={32}
                disabled={playback.kept.max_transcoding_sessions === null}
                onPick={(max_transcoding_sessions) => playback.setTo({ max_transcoding_sessions })}
              />
            </Setting>
          )}
          {playback.kept && (
            <>
              <Setting label={t("admin.limit_room")} why={t("admin.limit_room_why")}>
                <Toggle
                  label={t("admin.limit_room")}
                  checked={playback.kept.transcode_cache_megabytes !== null}
                  onChange={(limited) =>
                    playback.setTo({
                      transcode_cache_megabytes: limited ? A_FIRST_ROOM_GB * MEGABYTES_IN_A_GB : null,
                    })
                  }
                />
                <NumberField
                  label={t("admin.limit_room_most")}
                  value={Math.round(
                    (playback.kept.transcode_cache_megabytes ?? A_FIRST_ROOM_GB * MEGABYTES_IN_A_GB) /
                      MEGABYTES_IN_A_GB,
                  )}
                  min={1}
                  max={1000}
                  disabled={playback.kept.transcode_cache_megabytes === null}
                  onPick={(gigabytes) =>
                    playback.setTo({ transcode_cache_megabytes: gigabytes * MEGABYTES_IN_A_GB })
                  }
                />
                <span className="setting-unit">{t("admin.gigabytes")}</span>
              </Setting>
              {/* Asked only once there is a ceiling to keep under: without
                  one, nothing is ever given up. */}
              <Setting label={t("admin.limit_kept_behind")} why={t("admin.limit_kept_behind_why")}>
                <NumberField
                  label={t("admin.limit_kept_behind")}
                  value={Math.round(playback.kept.transcode_kept_behind_seconds / 60)}
                  min={3}
                  max={30}
                  disabled={playback.kept.transcode_cache_megabytes === null}
                  onPick={(minutes) => playback.setTo({ transcode_kept_behind_seconds: minutes * 60 })}
                />
                <span className="setting-unit">{t("admin.minutes")}</span>
              </Setting>
            </>
          )}
        </Panel>

        {playback.kept && (
          <Panel icon={FfmpegIcon} title={t("admin.codecs")} lead={t("admin.codecs_why")}>
            {EVERY_CODEC.map((codec) => {
              const chosen = playback.kept?.transcode_video_codecs ?? [];
              const allowed = chosen.includes(codec);
              return (
                <Setting key={codec} label={t(`admin.codec.${codec}`)}>
                  <Toggle
                    label={t(`admin.codec.${codec}`)}
                    checked={allowed}
                    // The last one left stays: a server allowed no codec could
                    // convert nothing.
                    disabled={allowed && chosen.length === 1}
                    onChange={(on) =>
                      playback.setTo({
                        transcode_video_codecs: EVERY_CODEC.filter((one) =>
                          one === codec ? on : chosen.includes(one),
                        ),
                      })
                    }
                  />
                </Setting>
              );
            })}
          </Panel>
        )}

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
            <Setting label={t("settings.thumbnails_every")}>
              <NumberField
                label={t("settings.thumbnails_every")}
                value={work.kept.thumbnails_every_seconds}
                min={1}
                max={600}
                onPick={(thumbnails_every_seconds) => work.setTo({ thumbnails_every_seconds })}
              />
            </Setting>
            <Setting label={t("settings.thumbnails_height")}>
              <NumberField
                label={t("settings.thumbnails_height")}
                value={work.kept.thumbnails_height}
                min={1}
                max={1080}
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
                onPick={(thumbnails_columns) => work.setTo({ thumbnails_columns })}
              />
              <span className="setting-times" aria-hidden="true">×</span>
              <NumberField
                label={t("settings.thumbnails_rows")}
                value={work.kept.thumbnails_rows}
                min={1}
                max={20}
                onPick={(thumbnails_rows) => work.setTo({ thumbnails_rows })}
              />
            </Setting>
          </Panel>
        )}
      </div>
    </>
  );
}

/** What converts the films: the media tool and the graphics card it found. */
export function CardPanel() {
  const { t } = useSettings();
  const overview = useOverview().answer;
  return (
    <Panel icon={GraphicsCardIcon} title={t("admin.card")} lead={t("admin.card_lead")}>
      <div className="stats">
        <Stat
          icon={FfmpegIcon}
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
  );
}

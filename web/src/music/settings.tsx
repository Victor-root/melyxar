/*
 * What an account chose for its music: what a film does to it, how songs
 * are levelled and how one follows the next, whether the queue of the last
 * visit comes back, and how heavy a song may be on its way. Changed at once in the player, and put back if the server refuses.
 */

import { PageHead, Panel, Picker, Setting, Toggle } from "../components/panel";
import { useAccount } from "../account";
import { LengthPicker } from "../pages/settings/length";
import { MusicIcon, NetworkIcon, PlaybackIcon, TagIcon } from "../icons";
import { useSettings } from "../settings";
import type { MusicPreferences } from "./api";
import { MUSIC_TABS } from "./tabs";
import { useMusic } from "./player/player";

/** The ceilings offered, heaviest first, in kilobits a second. The server
 *  holds any other between the lowest and the highest of them. */
export const CEILINGS = [320, 256, 192, 160, 128, 96, 64] as const;

/** The crossfades offered, in seconds: none, and up to the longest the
 *  server keeps. */
export const CROSSFADES = [0, 2, 4, 6, 8, 10, 12] as const;

/** A ceiling as the picker keeps it: none is every song as it is. */
export function ceilingValue(kbps: number | null): string {
  return kbps === null ? "none" : String(kbps);
}

export function ceilingOf(value: string): number | null {
  return value === "none" ? null : Number(value);
}

export function MyMusic() {
  const { t } = useSettings();
  const { preferences, setPreferences } = useMusic();
  const { account } = useAccount();
  const change = (changes: Partial<MusicPreferences>) => {
    void setPreferences({ ...preferences, ...changes }).catch(() => {});
  };

  return (
    <>
      <PageHead lead={t("me.music_lead")} />
      <Panel icon={MusicIcon} title={t("settings.music_listening")} lead={t("settings.music_listening_why")}>
        <Setting label={t("settings.music_film")} why={t("settings.music_film_why")}>
          <Picker
            label={t("settings.music_film")}
            value={preferences.film_on_screen}
            options={[
              ["stop", t("settings.music_film.stop")],
              ["pause", t("settings.music_film.pause")],
            ]}
            onPick={(film_on_screen) => change({ film_on_screen })}
          />
        </Setting>
        <Setting label={t("settings.music_volume_mode")} why={t("settings.music_volume_mode_why")}>
          <Picker
            label={t("settings.music_volume_mode")}
            value={preferences.volume_mode}
            options={[
              ["track", t("settings.music_volume_mode.track")],
              ["album", t("settings.music_volume_mode.album")],
              ["off", t("settings.music_volume_mode.off")],
            ]}
            onPick={(volume_mode) => change({ volume_mode })}
          />
        </Setting>
        <Setting label={t("settings.music_crossfade")} why={t("settings.music_crossfade_why")}>
          <Picker
            label={t("settings.music_crossfade")}
            value={String(preferences.crossfade_seconds)}
            options={CROSSFADES.map(
              (seconds) =>
                [
                  String(seconds),
                  seconds === 0 ? t("settings.music_crossfade.none") : t("settings.music_crossfade.seconds", { seconds }),
                ] as const,
            )}
            onPick={(value) => change({ crossfade_seconds: Number(value) })}
          />
        </Setting>
        <Setting label={t("settings.music_resume_queue")} why={t("settings.music_resume_queue_why")}>
          <Toggle
            label={t("settings.music_resume_queue")}
            checked={preferences.resume_queue}
            onChange={(resume_queue) => change({ resume_queue })}
          />
        </Setting>
        <Setting label={t("settings.music_spectrum")} why={t("settings.music_spectrum_why")}>
          <Toggle
            label={t("settings.music_spectrum")}
            checked={preferences.spectrum}
            onChange={(spectrum) => change({ spectrum })}
          />
        </Setting>
      </Panel>
      <Panel icon={PlaybackIcon} title={t("settings.music_skips")} lead={t("settings.music_skips_why")}>
        <Setting label={t("settings.music_skip_on")}>
          <LengthPicker
            label={t("settings.music_skip_on")}
            seconds={preferences.skip_on_seconds}
            longest={preferences.longest_skip_seconds}
            onPick={(skip_on_seconds) => change({ skip_on_seconds })}
          />
        </Setting>
        <Setting label={t("settings.music_skip_back")}>
          <LengthPicker
            label={t("settings.music_skip_back")}
            seconds={preferences.skip_back_seconds}
            longest={preferences.longest_skip_seconds}
            onPick={(skip_back_seconds) => change({ skip_back_seconds })}
          />
        </Setting>
      </Panel>
      <Panel icon={MusicIcon} title={t("settings.music_tabs")} lead={t("settings.music_tabs_why")}>
        {MUSIC_TABS.map((tab) => {
          const hidden = preferences.hidden_tabs.includes(tab);
          const lastShown = !hidden && MUSIC_TABS.length - preferences.hidden_tabs.length === 1;
          return (
            <Setting key={tab} label={t(`music.tab.${tab}`)}>
              <Toggle
                label={t(`music.tab.${tab}`)}
                checked={!hidden}
                disabled={lastShown}
                onChange={(shown) =>
                  change({
                    hidden_tabs: shown
                      ? preferences.hidden_tabs.filter((name) => name !== tab)
                      : [...preferences.hidden_tabs, tab],
                  })
                }
              />
            </Setting>
          );
        })}
      </Panel>
      <Panel icon={NetworkIcon} title={t("settings.music_network")} lead={t("settings.music_network_why")}>
        <Setting label={t("settings.music_max_bitrate")} why={t("settings.music_max_bitrate_why")}>
          <Picker
            label={t("settings.music_max_bitrate")}
            value={ceilingValue(preferences.max_bitrate_kbps)}
            options={[
              ["none", t("settings.music_max_bitrate.none")],
              ...CEILINGS.map((kbps) => [String(kbps), t("settings.music_max_bitrate.kbps", { kbps })] as const),
            ]}
            onPick={(value) => change({ max_bitrate_kbps: ceilingOf(value) })}
          />
        </Setting>
      </Panel>
      {account?.may_edit_tags && (
        <Panel icon={TagIcon} title={t("settings.music_tags")} lead={t("settings.music_tags_why")}>
          <Setting label={t("settings.music_tag_preview")} why={t("settings.music_tag_preview_why")}>
            <Toggle
              label={t("settings.music_tag_preview")}
              checked={preferences.tag_preview}
              onChange={(tag_preview) => change({ tag_preview })}
            />
          </Setting>
        </Panel>
      )}
    </>
  );
}

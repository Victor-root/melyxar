/*
 * What an account chose for its music: what a film does to it, how songs
 * are levelled, whether the queue of the last visit comes back, and how
 * heavy a song may be on its way. Changed at once in the player, and put back if the server refuses.
 */

import { PageHead, Panel, Picker, Setting, Toggle } from "../components/panel";
import { MusicIcon, NetworkIcon } from "../icons";
import { useSettings } from "../settings";
import type { MusicPreferences } from "./api";
import { useMusic } from "./player/player";

/** The ceilings offered, heaviest first, in kilobits a second. The server
 *  holds any other between the lowest and the highest of them. */
export const CEILINGS = [320, 256, 192, 160, 128, 96, 64] as const;

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
        <Setting label={t("settings.music_resume_queue")} why={t("settings.music_resume_queue_why")}>
          <Toggle
            label={t("settings.music_resume_queue")}
            checked={preferences.resume_queue}
            onChange={(resume_queue) => change({ resume_queue })}
          />
        </Setting>
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
    </>
  );
}

/*
 * Subtitles, on a page of their own: when a film starts with them, in which
 * language, and how they are dressed.
 *
 * The first two are kept by the server, because they decide which track a
 * film starts with. How subtitles look is kept by this browser, since it only
 * changes what is drawn here.
 */

import { PageHead, Panel, Picker, Setting } from "../../components/panel";
import { LanguagesIcon, SubtitlesIcon } from "../../icons";
import {
  appearanceClasses,
  BACKGROUNDS,
  COLOURS,
  EDGES,
  HEIGHTS,
  SIZES,
} from "../../player/appearance";
import { languagesAmong } from "../../languages";
import type { Wording } from "../../readable";
import { usePreferences, useSubtitleLook } from "../../screens/settings";
import { useSettings } from "../../settings";

export function MySubtitles() {
  const { t, language } = useSettings();
  const { kept, change, failed } = usePreferences();
  const [look, setLook] = useSubtitleLook();

  return (
    <>
      <PageHead lead={t("me.subtitles_lead")} />

      {failed && (
        <p className="panel-notice panel-notice-trouble">
          {t(failed === "not_kept" ? "settings.not_kept" : "error.unreachable")}
        </p>
      )}

      {kept && (
        <Panel
          icon={LanguagesIcon}
          title={t("settings.subtitle_start")}
          lead={t("settings.subtitle_start_why")}
        >
          <Setting
            label={t("settings.subtitle_mode")}
            why={t(`subtitle_mode.${kept.subtitle_mode}_why`)}
          >
            <Picker
              label={t("settings.subtitle_mode")}
              value={kept.subtitle_mode}
              options={kept.subtitle_modes.map((mode) => [mode, t(`subtitle_mode.${mode}`)] as const)}
              onPick={(subtitle_mode) => change({ subtitle_mode })}
            />
          </Setting>
          <Setting label={t("settings.subtitle_language")} why={t("settings.subtitle_language_why")}>
            <Picker
              label={t("settings.subtitle_language")}
              value={kept.preferred_subtitle_language ?? ""}
              options={languagesAmong(kept.subtitle_languages, language, t("settings.no_preference"))}
              onPick={(picked) => change({ preferred_subtitle_language: picked })}
            />
          </Setting>
          {kept.subtitle_languages.length === 0 && (
            <p className="panel-say">{t("settings.no_languages_yet")}</p>
          )}
        </Panel>
      )}

      <Panel
        icon={SubtitlesIcon}
        title={t("settings.subtitles")}
        lead={t("settings.subtitles_why")}
        className={appearanceClasses(look)}
      >
        {/* Shown rather than described: a size named "very large" means
            nothing until it is seen against a picture. */}
        <div className="subtitle-sample" aria-hidden="true">
          <span className="subtitle-sample-words">{t("settings.subtitles_sample")}</span>
        </div>
        <Setting label={t("player.subtitle_size")}>
          <Picker
            label={t("player.subtitle_size")}
            value={look.size}
            options={worded(SIZES, "subtitle_size", t)}
            onPick={(size) => setLook({ size })}
          />
        </Setting>
        <Setting label={t("player.subtitle_colour")}>
          <Picker
            label={t("player.subtitle_colour")}
            value={look.colour}
            options={worded(COLOURS, "subtitle_colour", t)}
            onPick={(colour) => setLook({ colour })}
          />
        </Setting>
        <Setting label={t("player.subtitle_edge")}>
          <Picker
            label={t("player.subtitle_edge")}
            value={look.edge}
            options={worded(EDGES, "subtitle_edge", t)}
            onPick={(edge) => setLook({ edge })}
          />
        </Setting>
        <Setting label={t("player.subtitle_background")}>
          <Picker
            label={t("player.subtitle_background")}
            value={look.background}
            options={worded(BACKGROUNDS, "subtitle_background", t)}
            onPick={(background) => setLook({ background })}
          />
        </Setting>
        {/* The fourth is a hand on a slider, which only the player itself
            offers a way to move. */}
        <Setting label={t("player.subtitle_height")}>
          <Picker
            label={t("player.subtitle_height")}
            value={look.height}
            options={worded(
              HEIGHTS.filter((one) => one !== "custom"),
              "subtitle_height",
              t,
            )}
            onPick={(height) => setLook({ height })}
          />
        </Setting>
      </Panel>
    </>
  );
}

/** A closed list of choices, each under the wording the interface has for it. */
function worded<T extends string>(among: readonly T[], naming: string, t: Wording): [T, string][] {
  return among.map((one): [T, string] => [one, t(`player.${naming}.${one}`)]);
}

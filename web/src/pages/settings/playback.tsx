/*
 * How films reach this viewer: the sound folded for their speakers, the
 * languages they would rather hear and read, how far the player's buttons
 * step, and how subtitles are dressed.
 *
 * The first two are kept by the server, because they change what it does: the
 * fold to stereo turns a copy into a rebuild, and a preferred language decides
 * which soundtrack starts. How subtitles look is kept by this browser, since it
 * only changes what is drawn here.
 */

import { PageHead, Panel, Picker, Setting, Slider } from "../../components/panel";
import { LanguagesIcon, PlaybackIcon, SoundIcon, SubtitlesIcon } from "../../icons";
import { languageName } from "../../languages";
import {
  appearanceClasses,
  BACKGROUNDS,
  COLOURS,
  EDGES,
  HEIGHTS,
  SIZES,
} from "../../player/appearance";
import { DeviceOptimization } from "../../player/DeviceOptimization";
import { STEP_LENGTHS } from "../../player/steps";
import type { Wording } from "../../readable";
import { usePreferences, useSubtitleLook } from "../../screens/settings";
import { useSettings } from "../../settings";

export function MyPlayback() {
  const { t, language, stepBack, setStepBack, stepOn, setStepOn } = useSettings();
  const { kept, change, failed } = usePreferences();
  const lengths = STEP_LENGTHS.map(
    (seconds) => [String(seconds), t("settings.step_seconds", { seconds })] as const,
  );
  const [look, setLook] = useSubtitleLook();

  return (
    <>
      <PageHead lead={t("me.playback_lead")} />

      {failed && (
        <p className="panel-notice panel-notice-trouble">
          {t(failed === "not_kept" ? "settings.not_kept" : "error.unreachable")}
        </p>
      )}

      {kept && (
        <div className="panels">
          <Panel icon={SoundIcon} title={t("settings.sound")} lead={t("settings.sound_why")}>
            <Setting
              label={t("settings.downmix")}
              why={t(`downmix.${kept.downmix_method}_why`)}
            >
              <Picker
                label={t("settings.downmix")}
                value={kept.downmix_method}
                options={kept.downmix_methods.map((method) => [method, t(`downmix.${method}`)] as const)}
                onPick={(downmix_method) => change({ downmix_method })}
              />
            </Setting>
            <Setting label={t("settings.downmix_gain")}>
              <Slider
                label={t("settings.downmix_gain")}
                min={kept.downmix_gain_range[0]}
                max={kept.downmix_gain_range[1]}
                step={0.1}
                value={kept.downmix_gain}
                shown={kept.downmix_gain.toFixed(1)}
                onChange={(downmix_gain) => change({ downmix_gain })}
              />
            </Setting>
          </Panel>

          <Panel icon={LanguagesIcon} title={t("settings.languages")} lead={t("settings.languages_why")}>
            <Setting label={t("work.audio")}>
              <Picker
                label={t("work.audio")}
                value={kept.preferred_audio_language ?? ""}
                options={languagesAmong(kept.audio_languages, language, t("settings.no_preference"))}
                onPick={(picked) => change({ preferred_audio_language: picked })}
              />
            </Setting>
            <Setting label={t("work.subtitles")}>
              <Picker
                label={t("work.subtitles")}
                value={kept.preferred_subtitle_language ?? ""}
                options={languagesAmong(kept.subtitle_languages, language, t("settings.no_preference"))}
                onPick={(picked) => change({ preferred_subtitle_language: picked })}
              />
            </Setting>
            {kept.audio_languages.length === 0 && (
              <p className="panel-say">{t("settings.no_languages_yet")}</p>
            )}
          </Panel>
        </div>
      )}

      <Panel icon={PlaybackIcon} title={t("settings.steps")} lead={t("settings.steps_why")}>
        <Setting label={t("settings.step_back")}>
          <Picker
            label={t("settings.step_back")}
            value={String(stepBack)}
            options={lengths}
            onPick={(seconds) => setStepBack(Number(seconds))}
          />
        </Setting>
        <Setting label={t("settings.step_on")}>
          <Picker
            label={t("settings.step_on")}
            value={String(stepOn)}
            options={lengths}
            onPick={(seconds) => setStepOn(Number(seconds))}
          />
        </Setting>
      </Panel>

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

      <DeviceOptimization />
    </>
  );
}

/**
 * The languages a library really holds, each under its own name, with "no
 * preference" in front. Only the ones there are: a picker offering a language
 * the collection does not carry is a picker that leads nowhere.
 */
function languagesAmong(among: string[], speaking: string, none: string): [string, string][] {
  return [["", none], ...among.map((code): [string, string] => [code, languageName(code, speaking)])];
}

/** A closed list of choices, each under the wording the interface has for it. */
function worded<T extends string>(among: readonly T[], naming: string, t: Wording): [T, string][] {
  return among.map((one): [T, string] => [one, t(`player.${naming}.${one}`)]);
}

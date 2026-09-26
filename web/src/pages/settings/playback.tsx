/*
 * How films reach this viewer: the sound folded for their speakers, what is
 * done with HDR, the language they would rather hear, and how far the
 * player's buttons step. Subtitles have a page of their own.
 *
 * The first three are kept by the server, because they change what it does:
 * the fold to stereo turns a copy into a rebuild, so does converting HDR, and
 * a preferred language decides which soundtrack starts.
 */

import { PageHead, Panel, Picker, Setting, Slider } from "../../components/panel";
import { ImageIcon, LanguagesIcon, PlaybackIcon, SoundIcon } from "../../icons";
import { languagesAmong } from "../../languages";
import { DeviceOptimization } from "../../player/DeviceOptimization";
import { STEP_LENGTHS } from "../../player/steps";
import { usePreferences } from "../../screens/settings";
import { useSettings } from "../../settings";

export function MyPlayback() {
  const { t, language, stepBack, setStepBack, stepOn, setStepOn } = useSettings();
  const { kept, change, failed } = usePreferences();
  const lengths = STEP_LENGTHS.map(
    (seconds) => [String(seconds), t("settings.step_seconds", { seconds })] as const,
  );

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
            {kept.audio_languages.length === 0 && (
              <p className="panel-say">{t("settings.no_languages_yet")}</p>
            )}
          </Panel>
        </div>
      )}

      {kept && (
        <Panel icon={ImageIcon} title={t("settings.picture")} lead={t("settings.picture_why")}>
          <Setting label={t("settings.wide_gamut")} why={t(`wide_gamut.${kept.wide_gamut}_why`)}>
            <Picker
              label={t("settings.wide_gamut")}
              value={kept.wide_gamut}
              options={kept.wide_gamut_choices.map((choice) => [choice, t(`wide_gamut.${choice}`)] as const)}
              onPick={(wide_gamut) => change({ wide_gamut })}
            />
          </Setting>
        </Panel>
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

      <DeviceOptimization />
    </>
  );
}

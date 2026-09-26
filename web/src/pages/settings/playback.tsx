/*
 * How films reach this viewer: the sound folded for their speakers, what is
 * done with HDR, the language they would rather hear, how far the player's
 * buttons step, and how a film left partway is picked up again. Subtitles
 * have a page of their own.
 *
 * All of it is kept by the server. The fold to stereo turns a copy into a
 * rebuild, so does converting HDR, a preferred language decides which
 * soundtrack starts, and where a film starts again is worked out there.
 */

import type { ResumeRules } from "../../api";
import { NumberField, PageHead, Panel, Picker, Setting, Slider, Toggle } from "../../components/panel";
import {
  HistoryIcon,
  ImageIcon,
  KindIcon,
  LanguagesIcon,
  PlaybackIcon,
  SoundIcon,
} from "../../icons";
import { languagesAmong } from "../../languages";
import { kindsOnTheHomePage, nameOfKind, useLibraries } from "../../libraries";
import { DeviceOptimization } from "../../player/DeviceOptimization";
import { rulesOf, withRulesOf } from "../../resuming";
import { usePreferences } from "../../screens/settings";
import type { Preferences } from "../../screens/settings";
import { useSettings } from "../../settings";
import { LengthPicker } from "./length";

export function MyPlayback() {
  const { t, language, stepBack, setStepBack, stepOn, setStepOn } = useSettings();
  const preferences = usePreferences();
  const { kept, change, failed } = preferences;

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

      {kept && (
        <Panel icon={PlaybackIcon} title={t("settings.steps")} lead={t("settings.steps_why")}>
          <Setting label={t("settings.step_on")}>
            <LengthPicker
              label={t("settings.step_on")}
              seconds={stepOn}
              longest={kept.longest_step}
              onPick={setStepOn}
            />
          </Setting>
          <Setting label={t("settings.step_back")}>
            <LengthPicker
              label={t("settings.step_back")}
              seconds={stepBack}
              longest={kept.longest_step}
              onPick={setStepBack}
            />
          </Setting>
        </Panel>
      )}

      <Resuming preferences={preferences} />

      <DeviceOptimization />
    </>
  );
}

/**
 * How a film left partway is picked up again: how far back it starts, and
 * the three rules that say when it counts as started, as watched, or as too
 * short to come back to. For every kind of library at once, or kind by kind
 * once the switch under them says so.
 */
function Resuming({ preferences }: { preferences: Preferences }) {
  const { t } = useSettings();
  const libraries = useLibraries();
  const { kept, change } = preferences;
  if (!kept) {
    return null;
  }
  const kinds = kindsOnTheHomePage(kept.home_order, libraries.all);

  return (
    <Panel icon={HistoryIcon} title={t("settings.resuming")} lead={t("settings.resuming_why")}>
      <Setting label={t("settings.resume_rewind")} why={t("settings.resume_rewind_why")}>
        <LengthPicker
          label={t("settings.resume_rewind")}
          seconds={kept.resume_rewind_seconds}
          longest={kept.longest_step}
          noneOffered
          onPick={(resume_rewind_seconds) => change({ resume_rewind_seconds })}
        />
      </Setting>

      {kept.resume_rules_per_kind ? (
        kinds.map((kind) => (
          <div className="rules-of-a-kind" key={kind}>
            <h3 className="rules-of-a-kind-name">
              <KindIcon kind={kind} size={18} />
              {nameOfKind(kind, libraries.all, t)}
            </h3>
            <Rules
              rules={rulesOf(kept.resume_rules_by_kind, kind, kept.resume_rules)}
              bounds={kept.resume_bounds}
              onChange={(rules) =>
                change({ resume_rules_by_kind: withRulesOf(kept.resume_rules_by_kind, kind, rules) })
              }
            />
          </div>
        ))
      ) : (
        <Rules
          rules={kept.resume_rules}
          bounds={kept.resume_bounds}
          onChange={(resume_rules) => change({ resume_rules })}
        />
      )}

      <Setting label={t("settings.resume_per_kind")} why={t("settings.resume_per_kind_why")}>
        <Toggle
          label={t("settings.resume_per_kind")}
          checked={kept.resume_rules_per_kind}
          onChange={(resume_rules_per_kind) => change({ resume_rules_per_kind })}
        />
      </Setting>
    </Panel>
  );
}

/** The three rules of a film left partway, each with its unit beside it. */
function Rules({
  rules,
  bounds,
  onChange,
}: {
  rules: ResumeRules;
  bounds: ResumeRules;
  onChange: (rules: ResumeRules) => void;
}) {
  const { t } = useSettings();
  const field = (name: keyof ResumeRules, min: number, max: number, unit: string) => (
    <Setting label={t(`settings.resume_${name}`)} why={t(`settings.resume_${name}_why`)}>
      <span className="field-with-unit">
        <NumberField
          label={t(`settings.resume_${name}`)}
          value={rules[name]}
          min={min}
          max={max}
          onPick={(value) => onChange({ ...rules, [name]: value })}
        />
        <span className="field-unit" aria-hidden="true">
          {unit}
        </span>
      </span>
    </Setting>
  );
  return (
    <>
      {field("min_percent", 0, bounds.min_percent, t("settings.percent_unit"))}
      {field("max_percent", bounds.max_percent, 100, t("settings.percent_unit"))}
      {field("min_seconds", 0, bounds.min_seconds, t("settings.seconds_unit"))}
    </>
  );
}

/*
 * What the viewer has decided about how films reach them.
 *
 * Two kinds of setting sit here, and they are told apart on purpose. Some are
 * kept by the server and change what it does: the fold to stereo turns a copy
 * into a rebuild, and a preferred language decides which soundtrack starts.
 * The others are kept by this browser and only change what is drawn, which is
 * why they work without asking anyone.
 *
 * Every change is sent as it is made rather than gathered behind a save
 * button: there is nothing here that is only half true while being typed, and
 * a page of settings with an unsaved state is a page people leave without
 * saving.
 */

import { useEffect, useState } from "react";
import { api, ApiError } from "../api";
import type { ViewerPreferences } from "../api";
import {
  appearanceClasses,
  BACKGROUNDS,
  COLOURS,
  EDGES,
  HEIGHTS,
  rememberAppearance,
  SIZES,
  storedAppearance,
} from "../player/appearance";
import type { Appearance } from "../player/appearance";
import { languageName } from "../player/languages";
import { DeviceOptimization } from "../player/DeviceOptimization";
import { useSettings } from "../settings";

export function SettingsPage() {
  const { t, language } = useSettings();
  const [kept, setKept] = useState<ViewerPreferences | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [appearance, setAppearanceState] = useState<Appearance>(storedAppearance);

  useEffect(() => {
    const controller = new AbortController();
    api
      .preferences(controller.signal)
      .then((answer) => {
        setKept(answer);
        setFailed(null);
      })
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed("error.unreachable");
        }
      });
    return () => controller.abort();
  }, []);

  /* Sent as it is made, and the answer is what the page then shows: the server
     brings a value back into range rather than refusing the lot, so what it
     kept is not always what was asked for. */
  const change = (changes: Partial<ViewerPreferences>) => {
    setKept((before) => (before ? { ...before, ...changes } : before));
    api
      .savePreferences(changes)
      .then((answer) => {
        setKept(answer);
        setFailed(null);
      })
      .catch((error) => {
        setFailed(error instanceof ApiError ? "settings.not_kept" : "error.unreachable");
      });
  };

  const look = (changes: Partial<Appearance>) => {
    const next = { ...appearance, ...changes };
    setAppearanceState(next);
    rememberAppearance(next);
  };

  return (
    <main className="page settings">
      <div className="section-head">
        <h1>{t("settings.title")}</h1>
      </div>

      {failed && <p className="notice">{t(failed)}</p>}

      <DeviceOptimization />

      <section className="settings-block">
        <h2>{t("settings.sound")}</h2>
        <p className="settings-why">{t("settings.sound_why")}</p>

        {kept && (
          <div className="controls">
            <label className="choice">
              <span className="choice-label">{t("settings.downmix")}</span>
              <select
                value={kept.downmix_method}
                onChange={(event) => change({ downmix_method: event.target.value })}
              >
                {kept.downmix_methods.map((method) => (
                  <option key={method} value={method}>
                    {t(`downmix.${method}`)}
                  </option>
                ))}
              </select>
            </label>

            <label className="choice">
              <span className="choice-label">
                {t("settings.downmix_gain")}
                <span className="settings-value">{kept.downmix_gain.toFixed(1)}</span>
              </span>
              <input
                type="range"
                min={kept.downmix_gain_range[0]}
                max={kept.downmix_gain_range[1]}
                step={0.1}
                value={kept.downmix_gain}
                onChange={(event) => change({ downmix_gain: Number(event.target.value) })}
              />
            </label>
          </div>
        )}
        <p className="settings-why">{t(`downmix.${kept?.downmix_method ?? "broadcast_standard"}_why`)}</p>
      </section>

      <section className="settings-block">
        <h2>{t("settings.languages")}</h2>
        <p className="settings-why">{t("settings.languages_why")}</p>

        {kept && (
          <div className="controls">
            <LanguageChoice
              label={t("work.audio")}
              value={kept.preferred_audio_language}
              among={kept.audio_languages}
              speaking={language}
              none={t("settings.no_preference")}
              onPick={(picked) => change({ preferred_audio_language: picked })}
            />
            <LanguageChoice
              label={t("work.subtitles")}
              value={kept.preferred_subtitle_language}
              among={kept.subtitle_languages}
              speaking={language}
              none={t("settings.no_preference")}
              onPick={(picked) => change({ preferred_subtitle_language: picked })}
            />
          </div>
        )}
        {kept && kept.audio_languages.length === 0 && (
          <p className="settings-why">{t("settings.no_languages_yet")}</p>
        )}
      </section>

      <section className={`settings-block ${appearanceClasses(appearance)}`}>
        <h2>{t("settings.subtitles")}</h2>
        <p className="settings-why">{t("settings.subtitles_why")}</p>

        <div className="controls">
          <Look
            label={t("player.subtitle_size")}
            value={appearance.size}
            among={SIZES}
            naming="subtitle_size"
            onPick={(size) => look({ size })}
            t={t}
          />
          <Look
            label={t("player.subtitle_colour")}
            value={appearance.colour}
            among={COLOURS}
            naming="subtitle_colour"
            onPick={(colour) => look({ colour })}
            t={t}
          />
          <Look
            label={t("player.subtitle_edge")}
            value={appearance.edge}
            among={EDGES}
            naming="subtitle_edge"
            onPick={(edge) => look({ edge })}
            t={t}
          />
          <Look
            label={t("player.subtitle_background")}
            value={appearance.background}
            among={BACKGROUNDS}
            naming="subtitle_background"
            onPick={(background) => look({ background })}
            t={t}
          />
          <Look
            label={t("player.subtitle_height")}
            value={appearance.height}
            among={HEIGHTS}
            naming="subtitle_height"
            onPick={(height) => look({ height })}
            t={t}
          />
        </div>

        {/* Shown rather than described: a size named "very large" means
            nothing until it is seen against a picture. */}
        <div className="subtitle-sample" aria-hidden="true">
          <span className="subtitle-sample-words">{t("settings.subtitles_sample")}</span>
        </div>
      </section>
    </main>
  );
}

function LanguageChoice({
  label,
  value,
  among,
  speaking,
  none,
  onPick,
}: {
  label: string;
  value: string | null;
  among: string[];
  speaking: string;
  none: string;
  onPick: (value: string) => void;
}) {
  return (
    <label className="choice">
      <span className="choice-label">{label}</span>
      <select value={value ?? ""} onChange={(event) => onPick(event.target.value)}>
        <option value="">{none}</option>
        {among.map((code) => (
          <option key={code} value={code}>
            {languageName(code, speaking)}
          </option>
        ))}
      </select>
    </label>
  );
}

function Look<T extends string>({
  label,
  value,
  among,
  naming,
  onPick,
  t,
}: {
  label: string;
  value: T;
  among: readonly T[];
  naming: string;
  onPick: (value: T) => void;
  t: (key: string) => string;
}) {
  return (
    <label className="choice">
      <span className="choice-label">{label}</span>
      <select value={value} onChange={(event) => onPick(event.target.value as T)}>
        {among.map((one) => (
          <option key={one} value={one}>
            {t(`player.${naming}.${one}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

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

import {
  appearanceClasses,
  BACKGROUNDS,
  COLOURS,
  EDGES,
  HEIGHTS,
  SIZES,
} from "../player/appearance";
import { LibraryEditor } from "../components/libraries";
import { languageName } from "../languages";
import { DeviceOptimization } from "../player/DeviceOptimization";
import { asLocalTime, asUtcMinutes, insideTheRange } from "../readable";
import { useSettingsScreen } from "../screens/settings";
import { useSettings } from "../settings";

export function SettingsPage() {
  const { t, language } = useSettings();
  const {
    kept,
    change,
    work,
    setWorkTo,
    playback,
    setPlaybackTo,
    appearance,
    look,
    libraries,
    readLibraries,
    failed,
  } = useSettingsScreen();

  return (
    <main className="page settings">
      <div className="section-head">
        <h1>{t("settings.title")}</h1>
      </div>

      {failed && <p className="notice">{t(failed === "not_kept" ? "settings.not_kept" : "error.unreachable")}</p>}

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

      {work && (
        <section className="settings-block">
          <h2>{t("settings.upkeep")}</h2>
          <p className="settings-why">{t("settings.upkeep_why")}</p>

          <div className="controls">
            <button
              className={`button button-small${work.upkeep_nightly ? " button-on" : ""}`}
              aria-pressed={work.upkeep_nightly}
              onClick={() => setWorkTo({ upkeep_nightly: !work.upkeep_nightly })}
            >
              {t("settings.upkeep_nightly")}
            </button>
            {/* Chosen and shown in the time of this browser. The server keeps
                it in universal time, which is the only clock it can read with
                certainty, so the hour shown here shifts by one when the clocks
                change until somebody sets it again. */}
            <label className="choice">
              <span className="choice-label">{t("settings.upkeep_at")}</span>
              <input
                type="time"
                value={asLocalTime(work.upkeep_at_utc_minutes)}
                disabled={!work.upkeep_nightly}
                onChange={(event) =>
                  setWorkTo({ upkeep_at_utc_minutes: asUtcMinutes(event.target.value) })
                }
              />
            </label>
          </div>
          <p className="settings-why">{t("settings.upkeep_at_why")}</p>
        </section>
      )}

      {work && (
        <section className="settings-block">
          <h2>{t("settings.thumbnails")}</h2>
          <p className="settings-why">{t("settings.thumbnails_why")}</p>

          <div className="controls">
            <button
              className={`button button-small${work.thumbnails_enabled ? " button-on" : ""}`}
              aria-pressed={work.thumbnails_enabled}
              onClick={() => setWorkTo({ thumbnails_enabled: !work.thumbnails_enabled })}
            >
              {t("settings.thumbnails_on")}
            </button>
            <NumberChoice
              label={t("settings.thumbnails_every")}
              value={work.thumbnails_every_seconds}
              min={1}
              max={600}
              disabled={!work.thumbnails_enabled}
              onPick={(thumbnails_every_seconds) => setWorkTo({ thumbnails_every_seconds })}
            />
            <NumberChoice
              label={t("settings.thumbnails_height")}
              value={work.thumbnails_height}
              min={1}
              max={1080}
              disabled={!work.thumbnails_enabled}
              onPick={(thumbnails_height) => setWorkTo({ thumbnails_height })}
            />
            <NumberChoice
              label={t("settings.thumbnails_columns")}
              value={work.thumbnails_columns}
              min={1}
              max={20}
              disabled={!work.thumbnails_enabled}
              onPick={(thumbnails_columns) => setWorkTo({ thumbnails_columns })}
            />
            <NumberChoice
              label={t("settings.thumbnails_rows")}
              value={work.thumbnails_rows}
              min={1}
              max={20}
              disabled={!work.thumbnails_enabled}
              onPick={(thumbnails_rows) => setWorkTo({ thumbnails_rows })}
            />
          </div>
          {/* Said before the change and not after it: changing the shape puts
              every film back in front of the upkeep. */}
          <p className="settings-why">{t("settings.thumbnails_shape_why")}</p>
        </section>
      )}

      {work && (
        <section className="settings-block">
          <h2>{t("settings.companion_files")}</h2>
          <p className="settings-why">{t("settings.companion_files_why")}</p>

          <div className="controls">
            <button
              className={`button button-small${work.read_companion_files ? " button-on" : ""}`}
              aria-pressed={work.read_companion_files}
              onClick={() => setWorkTo({ read_companion_files: !work.read_companion_files })}
            >
              {t("settings.read_companion_files")}
            </button>
          </div>
        </section>
      )}

      {playback && (
        <section className="settings-block">
          <h2>{t("settings.picture")}</h2>
          <p className="settings-why">{t("settings.tone_mapping_disabled_why")}</p>

          <div className="controls">
            <button
              className={`button button-small${playback.tone_mapping_disabled ? " button-on" : ""}`}
              aria-pressed={playback.tone_mapping_disabled}
              onClick={() =>
                setPlaybackTo({ tone_mapping_disabled: !playback.tone_mapping_disabled })
              }
            >
              {t("settings.tone_mapping_disabled")}
            </button>
          </div>
        </section>
      )}

      <section className="settings-block">
        <h2>{t("settings.libraries")}</h2>
        <p className="settings-why">{t("settings.libraries_why")}</p>
        <p className="settings-why">{t("settings.metadata_language_why")}</p>

        <LibraryEditor libraries={libraries} onChanged={readLibraries} />
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

/**
 * One number of a setting, with the range the server will keep it inside.
 *
 * The bounds are on the field as well as on the server, so somebody dragging
 * the arrows is stopped where the server would have stopped them rather than
 * being silently corrected afterwards.
 */
function NumberChoice({
  label,
  value,
  min,
  max,
  disabled,
  onPick,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  disabled?: boolean;
  onPick: (value: number) => void;
}) {
  return (
    <label className="choice">
      <span className="choice-label">{label}</span>
      <input
        type="number"
        className="choice-number"
        value={value}
        min={min}
        max={max}
        disabled={disabled}
        onChange={(event) => {
          const asked = insideTheRange(Number(event.target.value), min, max);
          if (asked !== null) {
            onPick(asked);
          }
        }}
      />
    </label>
  );
}

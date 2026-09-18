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
import { Choice, NumberChoice } from "../components/choice";
import { LibraryEditor } from "../components/libraries";
import { languageName } from "../languages";
import { DeviceOptimization } from "../player/DeviceOptimization";
import { asLocalTime, asUtcMinutes } from "../readable";
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
            <Choice
              label={t("work.audio")}
              value={kept.preferred_audio_language ?? ""}
              options={languagesAmong(kept.audio_languages, language, t("settings.no_preference"))}
              onPick={(picked) => change({ preferred_audio_language: picked })}
            />
            <Choice
              label={t("work.subtitles")}
              value={kept.preferred_subtitle_language ?? ""}
              options={languagesAmong(
                kept.subtitle_languages,
                language,
                t("settings.no_preference"),
              )}
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
          <Choice
            label={t("player.subtitle_size")}
            value={appearance.size}
            options={worded(SIZES, "subtitle_size", t)}
            onPick={(size) => look({ size })}
          />
          <Choice
            label={t("player.subtitle_colour")}
            value={appearance.colour}
            options={worded(COLOURS, "subtitle_colour", t)}
            onPick={(colour) => look({ colour })}
          />
          <Choice
            label={t("player.subtitle_edge")}
            value={appearance.edge}
            options={worded(EDGES, "subtitle_edge", t)}
            onPick={(edge) => look({ edge })}
          />
          <Choice
            label={t("player.subtitle_background")}
            value={appearance.background}
            options={worded(BACKGROUNDS, "subtitle_background", t)}
            onPick={(background) => look({ background })}
          />
          <Choice
            label={t("player.subtitle_height")}
            value={appearance.height}
            options={worded(HEIGHTS, "subtitle_height", t)}
            onPick={(height) => look({ height })}
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

/**
 * The languages a library really holds, each under its own name, with "no
 * preference" in front. Only the ones there are: a picker offering a language
 * the collection does not carry is a picker that leads nowhere.
 */
function languagesAmong(among: string[], speaking: string, none: string): [string, string][] {
  return [["", none], ...among.map((code): [string, string] => [code, languageName(code, speaking)])];
}

/** A closed list of choices, each under the wording the interface has for it. */
function worded<T extends string>(
  among: readonly T[],
  naming: string,
  t: (key: string) => string,
): [T, string][] {
  return among.map((one): [T, string] => [one, t(`player.${naming}.${one}`)]);
}

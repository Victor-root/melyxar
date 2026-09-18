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

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../api";
import type { Library, LibraryWork, ViewerPreferences } from "../api";
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
import { LibraryEditor } from "../components/libraries";
import { languageName } from "../player/languages";
import { DeviceOptimization } from "../player/DeviceOptimization";
import { useSettings } from "../settings";

export function SettingsPage() {
  const { t, language } = useSettings();
  const [kept, setKept] = useState<ViewerPreferences | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [appearance, setAppearanceState] = useState<Appearance>(storedAppearance);
  /* Fetched here rather than taken from the shell, which holds the same
     libraries for the navigation: this is the one screen that changes them,
     so it is the one screen that has to be looking at what it changed. */
  const [libraries, setLibraries] = useState<Library[]>([]);
  /* What the server does with every library: the shape of the thumbnails, the
     description files, and when the upkeep runs. All three were lines of the
     configuration file until now. */
  const [work, setWork] = useState<LibraryWork | null>(null);

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

  /* Read again whenever the editor has changed something, so what is on the
     screen is what the server kept rather than what the screen hoped for. */
  const readLibraries = useCallback((signal?: AbortSignal) => {
    api
      .libraries(signal)
      .then(setLibraries)
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed("error.unreachable");
        }
      });
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    readLibraries(controller.signal);
    return () => controller.abort();
  }, [readLibraries]);

  useEffect(() => {
    const controller = new AbortController();
    api
      .libraryWork(controller.signal)
      .then(setWork)
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed("error.unreachable");
        }
      });
    return () => controller.abort();
  }, []);

  /* Shown straight away and sent at once, and what the server kept is what the
     page then shows: a shape that cannot hold a thumbnail comes back brought
     into range rather than refused. */
  const setWorkTo = (changes: Partial<LibraryWork>) => {
    if (!work) {
      return;
    }
    const before = work;
    const wanted = { ...work, ...changes };
    setWork(wanted);
    api
      .setLibraryWork(wanted)
      .then((kept) => {
        setWork(kept);
        setFailed(null);
      })
      .catch((error) => {
        // Put back what the server still holds, rather than showing a setting
        // next to a server that never heard of it.
        setWork(before);
        setFailed(error instanceof ApiError ? "settings.not_kept" : "error.unreachable");
      });
  };

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

/*
 * A time of day the server keeps in universal time, in the time of this
 * browser.
 *
 * The server keeps one clock and it is UTC, which is the only one it can read
 * with certainty. Nobody should have to do that conversion in their head, so
 * it is done here, where the browser knows its own offset. What this cannot do
 * is follow the clocks changing: a time set in winter shows an hour later in
 * summer until somebody sets it again. That is said on the screen rather than
 * hidden, and it is a nightly piece of upkeep, so an hour either way costs
 * nothing.
 */
function asLocalTime(utcMinutes: number): string {
  const local = wrapIntoADay(utcMinutes - new Date().getTimezoneOffset());
  const hours = String(Math.floor(local / 60)).padStart(2, "0");
  const minutes = String(local % 60).padStart(2, "0");
  return `${hours}:${minutes}`;
}

/** The other way, for what a time field hands back. */
function asUtcMinutes(localTime: string): number {
  const [hours, minutes] = localTime.split(":").map(Number);
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) {
    return 0;
  }
  return wrapIntoADay(hours * 60 + minutes + new Date().getTimezoneOffset());
}

const MINUTES_IN_A_DAY = 24 * 60;

function wrapIntoADay(minutes: number): number {
  return ((minutes % MINUTES_IN_A_DAY) + MINUTES_IN_A_DAY) % MINUTES_IN_A_DAY;
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
          const asked = Number(event.target.value);
          if (Number.isFinite(asked)) {
            onPick(Math.min(max, Math.max(min, Math.round(asked))));
          }
        }}
      />
    </label>
  );
}

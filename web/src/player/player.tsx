/*
 * The player: what a viewer sees and touches.
 *
 * Everything that plays the film lives next door in the engine, and this knows
 * none of it. It is handed what is being played, what went wrong, and what to
 * call when a hand lands on the bar, and its whole job is where that goes on
 * screen. The two were one file for a long time, which meant that moving a
 * button was a change to the thing that plays the film; now this one can be
 * rewritten from nothing without the film noticing.
 *
 * What is still decided here is what belongs to looking rather than to
 * playing: how the words are dressed and where they sit, what the keyboard
 * does, and which of the panels is open.
 *
 * Nothing here touches the film. Not the bar, not the sound, not the button
 * that starts it: every one of those asks the engine, which is the only thing
 * holding the element. The one exception is where a subtitle line sits on the
 * picture, which is written on the cue itself and reachable from nowhere else.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { PlaybackTrack } from "../api";
import { useSettings } from "../settings";
import {
  appearanceClasses,
  BACKGROUNDS,
  COLOURS,
  EDGES,
  HEIGHTS,
  lineFor,
  rememberAppearance,
  SIZES,
  storedAppearance,
} from "./appearance";
import type { Appearance } from "./appearance";
import { Controls } from "./controls";
import { A_STEP, canBePlayedAsItIs, SPEEDS, usePlayback } from "./engine";
import { PlaybackFacts } from "./facts";
import { languageName } from "./languages";
import { QUALITIES, qualityName } from "./quality";

/**
 * What to call a track in a picker.
 *
 * The language first, since that is what a viewer is looking for, then what
 * the file itself calls it when it says something, and the number of channels
 * when there is more than a pair.
 */
function trackName(
  track: PlaybackTrack,
  t: (key: string) => string,
  speaking: string,
): string {
  const parts = [
    track.language ? languageName(track.language, speaking) : t("player.unknown_language"),
  ];
  if (track.title) {
    parts.push(track.title);
  }
  if (track.channels && track.channels > 2) {
    parts.push(`${track.channels}`);
  }
  if (track.burns_in) {
    parts.push(t("player.burns_in_short"));
  }
  return parts.join(" · ");
}

/**
 * One picker among a short list of named choices.
 *
 * Five of these sit side by side for the subtitles alone, and writing each of
 * them out would be the same twenty lines five times over.
 */
function Choice<T extends string>({
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
  /** What the wording of each choice is keyed on. */
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

export function Player({
  sourceId,
  workId,
  title,
  fromTheStart,
  onClose,
}: {
  sourceId: string;
  workId: string;
  title: string;
  /** Set when the viewer asked to start again rather than carry on. */
  fromTheStart?: boolean;
  onClose: () => void;
}) {
  const { t, language } = useSettings();
  const playback = usePlayback({ sourceId, workId, fromTheStart });
  const {
    video,
    plan,
    stream,
    rebuilt,
    failed,
    refusal,
    preparing,
    pictureKey,
    readyPicture,
    audioId,
    subtitleId,
    quality,
    speed,
    stepBy,
    words,
    holdTheWords,
    onWordsRead,
    playOrPause,
    intoTheCorner,
  } = playback;

  /* What is sent fullscreen. The bar is ours now, so it has to come with the
     picture: a video element sent fullscreen on its own leaves every control
     behind it on a page nobody can see. */
  const stage = useRef<HTMLDivElement>(null);
  /* Whether the panel saying what is happening to this film is open. Shut
     between films: it is opened to look at one film in particular. */
  const [factsOpen, setFactsOpen] = useState(false);
  const [appearance, setAppearanceState] = useState<Appearance>(storedAppearance);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      switch (event.key) {
        case "Escape":
          onClose();
          break;
        case " ":
        case "k":
          event.preventDefault();
          playOrPause();
          break;
        case "ArrowLeft":
        case "ArrowRight":
          // Kept from the page: the bar answers to the arrow keys as a slider
          // and would scroll what is behind it otherwise.
          event.preventDefault();
          stepBy(event.key === "ArrowLeft" ? -A_STEP : A_STEP);
          break;
        case "f":
          if (document.fullscreenElement) {
            void document.exitFullscreen();
          } else {
            void stage.current?.requestFullscreen?.();
          }
          break;
        default:
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, stepBy, playOrPause]);

  /* The subtitle being shown, when there is one, it is words rather than
     pictures, and there is a picture ready to hang it on.

     Waiting for the picture is not tidiness. A film being rebuilt is fed in by
     a library, and setting that up resets the element: words hung on it before
     that are wiped without a word, and the viewer is left with a track that is
     switched on and empty. */
  const shownSubtitle =
    readyPicture !== null && readyPicture === pictureKey
      ? plan?.subtitles.find((track) => track.id === plan.chosen_subtitle_id && track.url)
      : undefined;

  const setAppearance = (change: Partial<Appearance>) => {
    const next = { ...appearance, ...change };
    setAppearanceState(next);
    rememberAppearance(next);
  };

  /* How high the words sit belongs to each cue rather than to a stylesheet,
     so it is applied to them as they are read, and again whenever the viewer
     moves them. Counted from the bottom, which keeps them in the same place
     whatever the size of the picture.

     The one place looking reaches into the element, and it has to: where a
     cue sits is written on the cue and nowhere a stylesheet can get at it.
     Whether the words are there at all is the engine's, and this asks it
     nothing about that. */
  const placeCues = useCallback(() => {
    const tracks = video.current?.textTracks;
    if (!tracks) {
      return;
    }
    const line = lineFor(appearance.height);
    for (const track of Array.from(tracks)) {
      for (const cue of Array.from(track.cues ?? [])) {
        (cue as VTTCue).line = line;
      }
    }
  }, [appearance.height, video]);

  /* Placed again whenever the viewer moves them, and handed to the engine so
     that it can place them the instant the words are read. Waiting for a
     render instead would show one frame of words wherever the browser felt
     like putting them. */
  useEffect(() => {
    onWordsRead(placeCues);
    placeCues();
  }, [placeCues, onWordsRead]);

  return (
    <div
      className={`player ${appearanceClasses(appearance)}`}
      role="dialog"
      aria-label={title}
    >
      <div className="player-bar">
        <button className="button" onClick={onClose}>
          {t("player.close")}
        </button>
        <span className="player-title">{title}</span>
        {rebuilt && <span className="fact">{t("player.rebuilt")}</span>}
      </div>

      {failed && (
        <p className="notice">
          {t(failed)}
          {refusal && <span className="player-reasons">{refusal}</span>}
        </p>
      )}

      {/* What the server is doing while the picture is not there yet. Named
          steps rather than a bar alone: a bar filling at an unknown rate says
          only that something is happening, while "reading the film" says which
          part is slow when one of them is. */}
      {rebuilt && readyPicture !== pictureKey && !failed && (
        <p className="notice">
          {t(`player.step.${preparing?.step ?? "starting"}`)}
          {preparing && preparing.wanted > 0 && (
            <span className="player-reasons">
              {t("player.segments_ready", {
                ready: preparing.ready,
                wanted: preparing.wanted,
              })}
            </span>
          )}
        </p>
      )}

      {/* The words take as long as reading the film takes, and until they are
          there the picture plays with nothing on it, which is exactly what a
          subtitle that does not work looks like. */}
      {words && !failed && (
        <p className="notice notice-faint">
          {t(words === "coming" ? "player.words_coming" : "player.words_refused")}
        </p>
      )}

      {/* One element for both, told apart by its key: a film handed over as a
          file carries its address, a rebuilt one is fed by the library, and
          switching between the two has to start from a fresh element rather
          than from one still holding the other's address. */}
      {plan && !failed && (canBePlayedAsItIs(plan) || stream) && (
        <div className="player-stage" ref={stage}>
        <video
          key={pictureKey ?? undefined}
          ref={video}
          className="player-video"
          src={canBePlayedAsItIs(plan) ? plan.url : undefined}
          autoPlay
          /* The picture itself starts and stops the film, the way every player
             does it: the button is a long way from where the eyes are. */
          onClick={playOrPause}
        >
          {shownSubtitle?.url && (
            <track
              key={shownSubtitle.id}
              ref={holdTheWords}
              kind="subtitles"
              src={shownSubtitle.url}
              srcLang={shownSubtitle.language ?? undefined}
              label={trackName(shownSubtitle, t, language)}
              default
            />
          )}
        </video>

        {factsOpen && (
          <PlaybackFacts
            plan={plan}
            video={video}
            session={stream?.id ?? null}
            t={t}
            onClose={() => setFactsOpen(false)}
          />
        )}

        {/* Ours rather than the browser's, because showing the picture of the
            moment under the cursor means knowing where the cursor is on the
            bar, and the browser's bar says nothing about that. */}
        <Controls
          playback={playback}
          stage={stage}
          factsOpen={factsOpen}
          onFactsTurned={() => setFactsOpen((open) => !open)}
          t={t}
        />
        </div>
      )}

      {/* Shown even when the film cannot be played as it is: choosing a track
          is what caused that, and the way back is the same picker. Hiding it
          would leave a viewer stuck with a message. */}
      {plan && !failed && (
        <div className="player-choices">
          {plan.audio.length > 1 && (
            <label className="choice">
              <span className="choice-label">{t("work.audio")}</span>
              <select
                value={plan.chosen_audio_id ?? ""}
                onChange={(event) => playback.choose(event.target.value || null, subtitleId)}
              >
                {plan.audio.map((track) => (
                  <option key={track.id} value={track.id}>
                    {trackName(track, t, language)}
                  </option>
                ))}
              </select>
            </label>
          )}

          {plan.subtitles.length > 0 && (
            <label className="choice">
              <span className="choice-label">{t("work.subtitles")}</span>
              <select
                value={plan.chosen_subtitle_id ?? ""}
                onChange={(event) => playback.choose(audioId, event.target.value || null)}
              >
                <option value="">{t("player.no_subtitle")}</option>
                {plan.subtitles.map((track) => (
                  <option key={track.id} value={track.id}>
                    {trackName(track, t, language)}
                  </option>
                ))}
              </select>
            </label>
          )}

          {/* Offered on every film, not only on one being rebuilt: asking for
              a lighter stream is exactly what turns a film that was handed
              over whole into one that is rebuilt, so hiding the picker until
              then would hide the way in. */}
          <label className="choice">
            <span className="choice-label">{t("player.quality")}</span>
            <select
              value={quality.key}
              onChange={(event) => playback.setQuality(event.target.value)}
            >
              {QUALITIES.map((one) => (
                <option key={one.key} value={one.key}>
                  {qualityName(one, t("player.quality.as_it_is"))}
                </option>
              ))}
            </select>
          </label>

          <label className="choice">
            <span className="choice-label">{t("player.speed")}</span>
            <select
              value={speed}
              onChange={(event) => playback.setSpeed(Number(event.target.value))}
            >
              {SPEEDS.map((value) => (
                <option key={value} value={value}>
                  {value}&times;
                </option>
              ))}
            </select>
          </label>

          {/* Only while subtitles are actually showing: offering to restyle
              words that are not on screen is a row of pickers that do
              nothing. */}
          {shownSubtitle && (
            <>
              <Choice
                label={t("player.subtitle_size")}
                value={appearance.size}
                among={SIZES}
                naming="subtitle_size"
                onPick={(size) => setAppearance({ size })}
                t={t}
              />
              <Choice
                label={t("player.subtitle_colour")}
                value={appearance.colour}
                among={COLOURS}
                naming="subtitle_colour"
                onPick={(colour) => setAppearance({ colour })}
                t={t}
              />
              <Choice
                label={t("player.subtitle_edge")}
                value={appearance.edge}
                among={EDGES}
                naming="subtitle_edge"
                onPick={(edge) => setAppearance({ edge })}
                t={t}
              />
              <Choice
                label={t("player.subtitle_background")}
                value={appearance.background}
                among={BACKGROUNDS}
                naming="subtitle_background"
                onPick={(background) => setAppearance({ background })}
                t={t}
              />
              <Choice
                label={t("player.subtitle_height")}
                value={appearance.height}
                among={HEIGHTS}
                naming="subtitle_height"
                onPick={(height) => setAppearance({ height })}
                t={t}
              />
            </>
          )}

          {/* The picture in a corner while the viewer does something else.
              Not every browser offers it, so the button only appears where
              it works. */}
          {"requestPictureInPicture" in HTMLVideoElement.prototype && (
            <button className="toggle" onClick={intoTheCorner}>
              {t("player.corner")}
            </button>
          )}
        </div>
      )}

      {plan && (
        <p className="player-why">
          {t(`playback.${plan.method}`)}
          {plan.reasons.length > 0 && (
            <span className="player-reasons">
              {plan.reasons.map((reason) => t(`reason.${reason.code}`)).join(" · ")}
            </span>
          )}
          {/* Who is rebuilding the picture, and into what. This is the only
              question anybody asks about a film that stutters, and the answer
              used to be somewhere between a process listing and a guess. */}
          {plan.rebuild && (
            <span className="player-reasons">
              {t(`player.rebuilt_by.${plan.rebuild.by}`)}
              {` · ${plan.rebuild.codec.toUpperCase()}`}
              {plan.rebuild.height !== null && ` · ${plan.rebuild.height}p`}
              {plan.rebuild.bitrate !== null &&
                ` · ${Math.round(plan.rebuild.bitrate / 100_000) / 10} Mb/s`}
            </span>
          )}
        </p>
      )}
    </div>
  );
}

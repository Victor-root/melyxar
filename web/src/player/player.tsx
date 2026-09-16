/*
 * The player: the picture, and what is drawn over it.
 *
 * Everything that plays the film lives next door in the engine, and this knows
 * none of it. It is handed what is being played and what went wrong, and its
 * whole job is what appears on screen.
 *
 * What is left here is the picture itself, the few notices that stand in front
 * of it before there is anything to watch, how the words are dressed, and
 * which panel is open. Everything a hand touches is in the overlay beside
 * this, laid out from an arrangement rather than written into either file.
 *
 * Nothing here touches the film. Not the bar, not the sound, not the button
 * that starts it: every one of those asks the engine, which is the only thing
 * holding the element. The one exception is where a subtitle line sits on the
 * picture, which is written on the cue itself and reachable from nowhere else.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { PlaybackTrack, Work } from "../api";
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
import { storedArrangement } from "./arrangement";
import { canBePlayedAsItIs, usePlayback } from "./engine";
import { PlaybackFacts } from "./facts";
import { useFullscreen } from "./fullscreen";
import { languageName } from "./languages";
import { markFor, useBranding } from "./logo";
import { Overlay } from "./overlay";
import type { Panel, Shape } from "./overlay";
import { rememberSettings, storedSettings } from "./settings";
import { Spinner } from "./spinner";
import type { PlayerSettings } from "./settings";
import "./player.css";

/**
 * What to call a track in a list.
 *
 * The language first, since that is what a viewer is looking for, then what
 * the file itself calls it when it says something, and the number of channels
 * when there is more than a pair.
 */
function trackName(track: PlaybackTrack, t: (key: string) => string, speaking: string): string {
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
    <label className="player-choice">
      <span className="player-choice-label">{label}</span>
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
  work,
  fromTheStart,
  onClose,
}: {
  sourceId: string;
  /** The film as the library describes it, handed down by the screen that
   *  opened the player: it had the whole description in hand already, and the
   *  drawer would otherwise fetch it a second time. */
  work: Work;
  /** Set when the viewer asked to start again rather than carry on. */
  fromTheStart?: boolean;
  onClose: () => void;
}) {
  const { t, language } = useSettings();
  const title = work.title;
  const playback = usePlayback({ sourceId, workId: work.id, fromTheStart });
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
    words,
    holdTheWords,
    onWordsRead,
  } = playback;

  /* What is sent fullscreen: the picture and everything drawn over it. */
  const stage = useRef<HTMLDivElement>(null);
  const fullscreen = useFullscreen(stage);
  /* Which panel is open, if any. Shut between films: one is opened to look at
     one film in particular. */
  const [panel, setPanel] = useState<Panel | null>(null);
  const [appearance, setAppearanceState] = useState<Appearance>(storedAppearance);
  const [settings, setSettingsState] = useState<PlayerSettings>(storedSettings);
  /* How the picture is fitted. Not remembered between films on purpose: it
     answers one film that was mastered oddly, not a standing preference. */
  const [shape, setShape] = useState<Shape>("auto");
  /* Where each control sits. Read once: nothing writes one yet, and the day a
     settings screen does, this is the line that starts listening. */
  const [arrangement] = useState(storedArrangement);
  const branding = useBranding();

  /* The film's own wordmark, once there is one to fetch. Nothing fetches them
     yet, so this falls through to the server's mark, and to the title when
     the server has none either. */
  const mark = markFor(title, null, branding);

  const setAppearance = (change: Partial<Appearance>) => {
    const next = { ...appearance, ...change };
    setAppearanceState(next);
    rememberAppearance(next);
  };

  const setSettings = (change: Partial<PlayerSettings>) => {
    const next = { ...settings, ...change };
    setSettingsState(next);
    rememberSettings(next);
  };

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

  /* How high the words sit belongs to each cue rather than to a stylesheet, so
     it is applied to them as they are read, and again whenever the viewer
     moves them. Counted from the bottom, which keeps them in the same place
     whatever the size of the picture.

     The one place looking reaches into the element, and it has to: where a cue
     sits is written on the cue and nowhere a stylesheet can get at it. */
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

  const naming = useCallback(
    (track: PlaybackTrack) => trackName(track, t, language),
    [t, language],
  );

  return (
    <div className={`player ${appearanceClasses(appearance)}`} role="dialog" aria-label={title}>
      <div className="player-stage" ref={stage}>
        {/* One element for both, told apart by its key: a film handed over as
            a file carries its address, a rebuilt one is fed by the library,
            and switching between the two has to start from a fresh element
            rather than from one still holding the other's address. */}
        {plan && !failed && (canBePlayedAsItIs(plan) || stream) && (
          <video
            key={pictureKey ?? undefined}
            ref={video}
            className={`player-video player-video-${shape}`}
            src={canBePlayedAsItIs(plan) ? plan.url : undefined}
            autoPlay
            /* The picture itself starts and stops the film, the way every
               player does it: the button is a long way from where the eyes
               are. Twice puts it fullscreen, which is the other thing every
               player does and the reason a single click is held for a moment
               before it acts. */
            onClick={() =>
              /* A click on the picture with a panel open shuts the panel and
                 does nothing else. It is what a hand reaching away from a
                 menu means, and it is what stands in for the cross the panels
                 used to carry in their corner. */
              panel ? setPanel(null) : playback.pictureClicked(false)
            }
            onDoubleClick={() => {
              playback.pictureClicked(true);
              fullscreen.toggle();
            }}
          >
            {shownSubtitle?.url && (
              <track
                key={shownSubtitle.id}
                ref={holdTheWords}
                kind="subtitles"
                src={shownSubtitle.url}
                srcLang={shownSubtitle.language ?? undefined}
                label={naming(shownSubtitle)}
                default
              />
            )}
          </video>
        )}

        {/* What stands in front of the picture before there is one to watch,
            and what is wrong when something is. Over the picture like
            everything else: a notice that pushes the film down the page is a
            film that jumps when the notice goes. */}
        <div className="player-notices">
          {failed && (
            <p className="player-notice">
              {t(failed)}
              {refusal && <span className="player-notice-why">{refusal}</span>}
            </p>
          )}

          {/* A ring turning, and under it what the server is actually doing.
              The ring says something is happening, which a line of text on a
              black screen does badly; the words say which part is slow when
              one of them is, which a ring cannot say at all. */}
          {rebuilt && readyPicture !== pictureKey && !failed && (
            <div className="player-working">
              <Spinner />
              <p className="player-notice player-notice-bare">
                {t(`player.step.${preparing?.step ?? "starting"}`)}
                {/* In seconds of film, which the server works out: how long a
                    piece of a rebuilt film is, is its business, and a viewer
                    waiting knows what ten seconds of a film is and not what
                    two sixths of one is.

                    Only while the server is still short of what it wanted:
                    once it has caught up, it goes on producing ahead of where
                    the viewer is, and a count that keeps climbing past the
                    number it was supposedly out of says nothing a viewer can
                    make sense of. The step above already says enough once
                    that line stops being true. */}
                {preparing && preparing.ready_seconds < preparing.wanted_seconds && (
                  <span className="player-notice-why">
                    {t("player.seconds_ready", {
                      ready: Math.round(preparing.ready_seconds),
                      wanted: Math.round(preparing.wanted_seconds),
                    })}
                  </span>
                )}
              </p>
            </div>
          )}

          {/* The words take as long as reading the film takes, and until they
              are there the picture plays with nothing on it, which is exactly
              what a subtitle that does not work looks like. */}
          {words && !failed && (
            <p className="player-notice player-notice-faint">
              {t(words === "coming" ? "player.words_coming" : "player.words_refused")}
            </p>
          )}
        </div>

        <Overlay
          playback={playback}
          arrangement={arrangement}
          settings={settings}
          onSettings={setSettings}
          work={work}
          mark={mark}
          shape={shape}
          onShape={setShape}
          stage={stage}
          fullscreen={fullscreen}
          panel={panel}
          onPanel={setPanel}
          onClose={onClose}
          naming={naming}
          language={language}
          t={t}
        />

        {panel === "facts" && plan && (
          <PlaybackFacts
            plan={plan}
            video={video}
            session={stream?.id ?? null}
            t={t}
            onClose={() => setPanel(null)}
          />
        )}

        {/* Only while subtitles are actually showing: offering to restyle
            words that are not on screen is a row of pickers that do nothing. */}
        {panel === "subtitles" && shownSubtitle && (
          <div className="player-dressing">
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
          </div>
        )}
      </div>
    </div>
  );
}

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
 * holding the element. Subtitles are the one exception in the other
 * direction: read from the element by the engine, but drawn here rather than
 * by the browser, because where they sit answers to the strip of controls
 * standing over the same part of the picture.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { PlaybackTrack, Work } from "../api";
import { useToast } from "../components/toasts";
import { nameOfPlayed } from "../readable";
import { showPlaying } from "../tab";
import { useSettings } from "../settings";
import { appearanceClasses, rememberAppearance, storedAppearance } from "./appearance";
import type { Appearance } from "./appearance";
import { storedArrangement } from "./arrangement";
import { trackName } from "./describe";
import { canBePlayedAsItIs, usePlayback } from "./engine";
import { PlaybackFacts } from "./facts";
import { useFullscreen } from "./fullscreen";
import { markFor, useBranding } from "./logo";
import { Overlay, SkipStretch } from "./overlay";
import type { Panel, Shape } from "./overlay";
import { rememberSettings, storedSettings } from "./settings";
import { Spinner } from "./spinner";
import type { PlayerSettings } from "./settings";
import "./player.css";

export function Player({
  sourceId,
  work,
  fromTheStart,
  startAt,
  onClose,
  onEnded,
  onNextEpisode,
  onPreviousEpisode,
  onSelectEpisode,
}: {
  sourceId: string;
  /** The film as the library describes it, handed down by the screen that
   *  opened the player: it had the whole description in hand already, and the
   *  drawer would otherwise fetch it a second time. */
  work: Work;
  /** Set when the viewer asked to start again rather than carry on. */
  fromTheStart?: boolean;
  /** Where to start instead, in seconds, when a moment was chosen by hand. */
  startAt?: number;
  onClose: () => void;
  /** Told when the film reaches its end on its own, so an episode can be
   *  followed by the one after it. */
  onEnded?: () => void;
  /** Steps to the episode after or before this one. Absent for anything that
   *  is not an episode, and at either end of a series, so the button that
   *  asks for one is drawn only when there is somewhere to go. */
  onNextEpisode?: () => void;
  onPreviousEpisode?: () => void;
  /** Steps straight to any episode of the series, chosen by hand from the
   *  "up next" sheet rather than by ending or by the buttons beside play.
   *  Absent for anything that is not an episode. */
  onSelectEpisode?: (episode: { id: string; source_id: string | null }) => void;
}) {
  const { t, language, stepBack, stepOn } = useSettings();
  const title = work.title;
  const toast = useToast();
  /* Left the way the viewer would leave it, and said why: a film that simply
     vanishes looks like a fault. */
  const stoppedByAnAdministrator = () => {
    toast({ state: "attention", title: t("player.stopped_by_administrator") });
    onClose();
  };
  const playback = usePlayback({
    sourceId,
    workId: work.id,
    fromTheStart,
    startAt,
    onEnded,
    onStopped: stoppedByAnAdministrator,
  });
  const {
    video,
    plan,
    stream,
    rebuilt,
    failed,
    refusal,
    loadingPercent,
    pictureKey,
    readyPicture,
    words,
    shownWords,
    holdTheWords,
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

  /* The film's own title drawn as the film draws it, falling through to the
     series' mark for an episode (which draws none of its own), then to the
     server's mark, and then to the title written out. The whole description
     is already in hand, so this costs nothing to ask for. */
  const series = work.ancestry.find((up) => up.kind === "series") ?? null;
  const mark =
    work.logo.length > 0
      ? markFor(title, work.logo, branding)
      : markFor(series?.title ?? title, series?.logo ?? [], branding);

  /* The tab says what is playing, and gives its name back once it stops. */
  const playing = nameOfPlayed(work, t);
  useEffect(() => {
    showPlaying(playing);
    return () => showPlaying(null);
  }, [playing]);

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

          {/* A ring turning, and a single number under it, climbing from
              nought to a hundred and reaching it the same instant the picture
              does. Built in the engine from the real moments on the way to a
              playing film, not from how far the server alone has got: the
              server can be finished with its own part and the film still be
              seconds away on a slow connection, and a number that stopped
              climbing there would be a number lying about what is left. */}
          {rebuilt && readyPicture !== pictureKey && !failed && (
            <div className="player-working">
              <Spinner />
              <p className="player-notice player-notice-bare">
                {t("player.preparing_percent", { percent: Math.round(loadingPercent) })}
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

        {/* Beside the controls and not inside them: they fade out when
            nobody touches anything, and this is wanted exactly then. */}
        <SkipStretch playback={playback} t={t} />

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
          onNextEpisode={onNextEpisode}
          onPreviousEpisode={onPreviousEpisode}
          onSelectEpisode={onSelectEpisode}
          naming={naming}
          language={language}
          t={t}
          steps={{ back: stepBack, on: stepOn }}
          appearance={appearance}
          onAppearance={setAppearance}
        />

        {/* Drawn here rather than left to the browser: where these sit has to
            answer to the strip of controls above, and a stylesheet reaches a
            cue nowhere near as far as it reaches an element of its own. */}
        {shownWords.length > 0 && (
          <div
            className="player-subtitle-words"
            // Set only for the slider, and only here: the three named places
            // already answer to a class the stylesheet reads, and a value
            // written on the element itself would outrank that class the
            // moment a viewer went back to one of them.
            style={
              appearance.height === "custom"
                ? { ["--subtitle-height" as string]: `${appearance.heightCustom}%` }
                : undefined
            }
            aria-live="polite"
          >
            {shownWords.map((line, index) => (
              <span key={index} className="player-subtitle-line">
                {line}
              </span>
            ))}
          </div>
        )}

        {panel === "facts" && plan && (
          <PlaybackFacts
            plan={plan}
            video={video}
            session={stream?.id ?? null}
            t={t}
            onClose={() => setPanel(null)}
          />
        )}
      </div>
    </div>
  );
}

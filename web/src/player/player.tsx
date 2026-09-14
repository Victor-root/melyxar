/*
 * The player.
 *
 * A film the browser opens by itself is handed to a plain video element: the
 * browser already knows how to fetch the stretch it needs, draw the frames and
 * let someone scrub, and nothing here improves on that.
 *
 * A film it cannot open is rebuilt by the server as it is watched, and arrives
 * cut into segments listed in a playlist. Only Apple's browsers read that on
 * their own, so everywhere else a library feeds the segments in. That is the
 * only reason it is here.
 *
 * The position is reported while watching, so closing the tab in the middle of
 * a film does not lose it. It is reported on a timer rather than on every
 * frame, and once more when the tab goes away, which is the moment that
 * actually matters.
 */

import type Hls from "hls.js";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "../api";
import type { PlaybackPlan, PlaybackSession, PlaybackTrack, Preparation } from "../api";
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
import { languageName } from "./languages";
import { clientProfile } from "./profile";
import {
  QUALITIES,
  qualityCalled,
  qualityName,
  rememberQuality,
  storedQuality,
} from "./quality";

/** The speeds offered. Whole steps: nobody asks for 1.17 times. */
const SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2];

/** How often the position is sent while a film plays. */
const REPORT_EVERY = 10_000;

/** Below this, a film counts as not started, so leaving at once loses nothing. */
const WORTH_REPORTING = 5;

/**
 * How long a segment may take to arrive before the library gives up on it.
 *
 * Set above the patience the server itself has: a segment being rebuilt right
 * now is worth waiting for, and asking again would only start the same work
 * twice.
 */
const SEGMENT_PATIENCE = 40_000;

/**
 * How often the server is asked where the preparation has got to.
 *
 * A segment lasts four seconds and is produced faster than that, so a look
 * every second is often enough to move and rare enough to cost nothing.
 */
const PREPARATION_EVERY = 1_000;

/** Whether the film can reach this browser without being rebuilt. */
function canBePlayedAsItIs(plan: PlaybackPlan): boolean {
  return plan.method === "direct_play";
}

/** What went wrong, in a word this interface knows how to say. */
function wording(error: unknown): string {
  if (!(error instanceof ApiError)) {
    return "error.unreachable";
  }
  switch (error.code) {
    case "root_unavailable":
      return "player.missing";
    case "conflict":
      return "player.too_busy";
    case "dependency_missing":
      return "player.no_conversion";
    case "not_described":
      return "player.not_described";
    default:
      return "error.unreachable";
  }
}

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
  const video = useRef<HTMLVideoElement>(null);
  const [plan, setPlan] = useState<PlaybackPlan | null>(null);
  const [stream, setStream] = useState<PlaybackSession | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  /* What the browser itself said when it refused, word for word. The message
     above is this interface's wording and says only that something went
     wrong; this is the sentence that says which thing, and without it the
     answer lives in a console nobody opens. */
  const [refusal, setRefusal] = useState<string | null>(null);
  const [audioId, setAudioId] = useState<string | null>(null);
  const [subtitleId, setSubtitleId] = useState<string | null>(null);
  const [speed, setSpeed] = useState(1);
  /* What the viewer asked the picture to be held to. Kept across films rather
     than per film: somebody watching on a thin connection is on a thin
     connection for the next one too. */
  const [quality, setQualityState] = useState(storedQuality);
  const [appearance, setAppearanceState] = useState<Appearance>(storedAppearance);
  /* Which picture the browser has actually opened. Null until it has: the
     words are hung on the picture, and only once it is there. */
  const [readyPicture, setReadyPicture] = useState<string | null>(null);
  /* How far the server has got, asked for only while the picture is not there
     yet: once the film is playing, the answer is a request a second for
     something nobody is looking at. */
  const [preparing, setPreparing] = useState<Preparation | null>(null);
  /* The last position seen, kept apart from the element. On the way out the
     element is already gone, and that is exactly the moment the position is
     worth sending. */
  const lastPosition = useRef(0);
  /* Where the picture picks up once it is ready, and null when it starts
     where it is. Applied on loadedmetadata: set any earlier it is ignored
     without a word, and the film starts from the beginning. */
  const resumeAt = useRef<number | null>(null);
  /* Whether the first answer has been seen. The resume point comes from that
     one and no other: later answers carry the same number, and applying it
     again would drag a viewer back to where they were an hour ago. */
  const opened = useRef(false);
  /* The session being watched, kept apart from the element for the same
     reason as the position: on the way out there is nothing left to read it
     from, and that is exactly when it has to be closed. */
  const session = useRef<string | null>(null);
  /* The element carrying the words, so the moment they finish being read can
     be waited for. */
  const subtitleTrack = useRef<HTMLTrackElement | null>(null);
  /* Where the session about to be watched was opened at, told to the server
     and to the library alike. Both used to be left to find out for
     themselves, and both began at the opening of the film: the server built a
     segment nobody would see, then started again where the viewer was. */
  const openedAt = useRef(0);

  /* Asked again whenever a track changes: which tracks are wanted is part of
     the question, and the answer can change with it. A film played as it is
     becomes one that has to be rebuilt the moment someone picks a subtitle
     that can only be drawn into the picture. */
  useEffect(() => {
    const controller = new AbortController();
    api
      .plan(
        sourceId,
        {
          profile: clientProfile(quality),
          audio_track_id: audioId,
          subtitle_track_id: subtitleId,
        },
        controller.signal,
      )
      .then((answer) => {
        if (!opened.current) {
          opened.current = true;
          resumeAt.current = fromTheStart ? null : answer.resume_from_seconds;
        }
        setPlan(answer);
        setFailed(null);
      })
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(wording(error));
        }
      });
    return () => controller.abort();
  }, [sourceId, audioId, subtitleId, fromTheStart, quality]);

  const rebuilt = plan !== null && !canBePlayedAsItIs(plan);
  /* A subtitle made of pictures has to be painted into the picture, so the one
     chosen is part of what the server produces. Named here rather than read
     off the method, which stays the same either way: choosing another one used
     to change nothing at all, and the words only appeared once something else
     forced a new session, which is a very long way of saying they never
     appeared. */
  const paintedIn =
    plan?.subtitles.find((track) => track.id === subtitleId)?.burns_in === true
      ? subtitleId
      : null;
  /* What the server would actually be asked to produce. A subtitle handed
     over alongside the picture changes none of it, so turning subtitles on
     must not throw away a conversion already under way and make the viewer
     wait through it again. */
  const beingProduced = rebuilt
    ? `${plan.method}:${audioId ?? ""}:${paintedIn ?? ""}:${quality.key}`
    : null;
  /* Which picture is on screen: the file itself, or one session of segments.
     A change here means a fresh element rather than a new address on the old
     one, because the two are fed in ways that cannot be swapped. */
  const pictureKey = plan === null ? null : canBePlayedAsItIs(plan) ? "file" : (stream?.id ?? null);

  /* A film this browser cannot open is rebuilt by the server, which needs a
     session to rebuild it into. The session is closed on the way out, and by
     the server itself if that never arrives: a viewer who closes a tab says
     nothing, and a tool left running is a core spent on nobody. */
  useEffect(() => {
    if (!beingProduced) {
      setStream(null);
      return;
    }

    /* Picking another soundtrack starts a new session from nothing, and its
       picture would open at the beginning. Carrying the position over is what
       makes changing a track feel like changing a track. */
    if (lastPosition.current > 0) {
      resumeAt.current = lastPosition.current;
    }
    openedAt.current = Math.max(0, resumeAt.current ?? 0);

    const controller = new AbortController();
    let gone = false;

    api
      .openSession(
        sourceId,
        {
          // A subtitle is named only when it has to be painted into the
          // picture. One made of words travels on its own beside it, and
          // naming it here would rebuild the film for nothing.
          profile: clientProfile(quality),
          audio_track_id: audioId,
          subtitle_track_id: paintedIn,
          start_at_seconds: openedAt.current,
        },
        controller.signal,
      )
      .then((opening) => {
        if (gone) {
          api.closeSession(opening.id);
          return;
        }
        session.current = opening.id;
        setStream(opening);
      })
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(wording(error));
        }
      });

    return () => {
      gone = true;
      controller.abort();
      if (session.current) {
        api.closeSession(session.current);
        session.current = null;
      }
    };
  }, [beingProduced, sourceId, audioId, paintedIn, quality]);

  /* Asked for while the picture is not there yet, and not a moment longer:
     once the film is playing this would be a request a second for something
     nobody is looking at. */
  useEffect(() => {
    const name = stream?.id;
    if (!name || readyPicture === pictureKey) {
      setPreparing(null);
      return;
    }
    const controller = new AbortController();
    const look = () => {
      api
        .preparation(name, controller.signal)
        .then(setPreparing)
        .catch(() => {
          // A step that could not be read is not worth troubling a viewer
          // with: the film is on its way either way, and the next look
          // carries the same news.
        });
    };
    look();
    const timer = window.setInterval(look, PREPARATION_EVERY);
    return () => {
      window.clearInterval(timer);
      controller.abort();
    };
  }, [stream?.id, readyPicture, pictureKey]);

  /* Feeding the segments in. Apple's browsers read a playlist on their own,
     so there the address goes straight to the element and nothing else is
     needed. */
  useEffect(() => {
    const element = video.current;
    if (!element || !stream) {
      return;
    }

    if (element.canPlayType("application/vnd.apple.mpegurl")) {
      // The starting point is said in the address, which is the only way to
      // say it to a browser reading the playlist on its own. Without it the
      // server is asked for the opening of the film before the jump, and
      // produces a segment nobody will ever see.
      element.src =
        openedAt.current > 0
          ? `${stream.playlist_url}#t=${openedAt.current}`
          : stream.playlist_url;
      return;
    }

    let feed: Hls | null = null;
    let gone = false;

    /* Fetched only now, and only by the viewer who needs it. Most films play
       as they are and never touch this, so carrying it in every page would
       slow the whole interface for the few that do. */
    void import("hls.js").then(({ default: Library }) => {
      if (gone) {
        return;
      }
      if (!Library.isSupported()) {
        setFailed("player.cannot_play");
        return;
      }
      feed = new Library({
        fragLoadingTimeOut: SEGMENT_PATIENCE,
        // Where to begin. Left unsaid, the library asks for the opening of
        // the film and only jumps once the picture has started, which has the
        // server produce a segment nobody will ever see first.
        startPosition: openedAt.current,
        // The segments carry no words, so the library has no business
        // touching the subtitles on the picture: left to itself it takes
        // charge of every one it finds there and empties ours as it goes.
        renderTextTracksNatively: false,
      });
      feed.subtitleDisplay = false;
      feed.on(Library.Events.ERROR, (_event, trouble) => {
        // Anything short of fatal is retried on its own, and saying so would
        // turn an invisible hiccup into an error the viewer has to read.
        if (trouble.fatal) {
          setFailed("player.cannot_play");
          setRefusal(
            [trouble.type, trouble.details, trouble.error?.message, trouble.reason]
              .filter(Boolean)
              .join(" · "),
          );
        }
      });
      feed.loadSource(stream.playlist_url);
      feed.attachMedia(element);
    });

    return () => {
      gone = true;
      feed?.destroy();
    };
  }, [stream]);

  /* A choice is remembered once it has been made, never on the way in: the
     tracks a page opens with are what the rules already decided, and writing
     them back would turn a habit into a decision nobody made. */
  const choose = (audio: string | null, subtitle: string | null) => {
    setAudioId(audio);
    setSubtitleId(subtitle);
    api
      .rememberTracks({
        work_id: workId,
        source_id: sourceId,
        audio_track_id: audio,
        subtitle_track_id: subtitle,
      })
      .catch(() => {
        // A choice that could not be written down still applies to this
        // sitting, which is the part the viewer is watching.
      });
  };

  const report = useCallback(() => {
    const seconds = lastPosition.current;
    if (seconds < WORTH_REPORTING) {
      return;
    }
    api.reportPosition(workId, seconds).catch(() => {
      // A position that could not be sent is not worth troubling a viewer
      // with: the next one carries the same news, and the film keeps playing.
    });
  }, [workId]);

  /* Closing the tab is how most films are left, and a request started then is
     usually dropped. Both of these are handed to the browser to deliver on its
     own behalf, which is what that is for.

     The session goes with the position: React tears nothing down when a tab
     closes, so without this the server would keep converting a film nobody is
     watching until it notices on its own, two minutes later. */
  const onTheWayOut = useCallback(() => {
    const seconds = lastPosition.current;
    if (seconds >= WORTH_REPORTING) {
      api.reportPositionOnTheWayOut(workId, seconds);
    }
    if (session.current) {
      api.closeSession(session.current);
    }
  }, [workId]);

  // While playing, and once more when the tab goes away. The second is the
  // one that matters: closing a tab is how most films are left.
  useEffect(() => {
    const timer = window.setInterval(report, REPORT_EVERY);
    const onHidden = () => {
      if (document.visibilityState === "hidden") {
        report();
      }
    };
    document.addEventListener("visibilitychange", onHidden);
    window.addEventListener("pagehide", onTheWayOut);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onHidden);
      window.removeEventListener("pagehide", onTheWayOut);
      report();
    };
  }, [report, onTheWayOut]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const element = video.current;
      if (!element) {
        return;
      }
      switch (event.key) {
        case "Escape":
          onClose();
          break;
        case " ":
        case "k":
          event.preventDefault();
          if (element.paused) {
            void element.play();
          } else {
            element.pause();
          }
          break;
        case "ArrowLeft":
          element.currentTime = Math.max(0, element.currentTime - 10);
          break;
        case "ArrowRight":
          element.currentTime += 10;
          break;
        case "f":
          void element.requestFullscreen?.();
          break;
        default:
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

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

  /* Remembered as it is chosen, not on the way in: what a player opens with
     is what the viewer left it on, and writing that back would be a decision
     nobody made. */
  const setQuality = (key: string) => {
    const chosen = qualityCalled(key);
    setQualityState(chosen);
    rememberQuality(chosen);
  };

  const setAppearance = (change: Partial<Appearance>) => {
    const next = { ...appearance, ...change };
    setAppearanceState(next);
    rememberAppearance(next);
  };

  /* How high the words sit belongs to each cue rather than to a stylesheet,
     so it is applied to them as they are read, and again whenever the viewer
     moves them. Counted from the bottom, which keeps them in the same place
     whatever the size of the picture. */
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
  }, [appearance.height]);

  /* Placed again whenever the viewer moves them, and kept where the words
     themselves can reach it: the listener below is attached once per element
     and must not have to be replaced every time a choice changes. */
  const placing = useRef(placeCues);
  useEffect(() => {
    placing.current = placeCues;
    placeCues();
  }, [placeCues]);

  const whenTheWordsAreRead = useCallback(() => placing.current(), []);

  /* Waited for on the element itself, and re-attached whenever the element is
     a different one: a film being rebuilt gets a fresh picture whenever the
     soundtrack changes, and the words come with it. Before the words are read
     there are no cues to place, and the browser goes on putting them wherever
     it likes. */
  const holdTheWords = useCallback(
    (element: HTMLTrackElement | null) => {
      subtitleTrack.current?.removeEventListener("load", whenTheWordsAreRead);
      subtitleTrack.current = element;
      if (!element) {
        return;
      }
      element.addEventListener("load", whenTheWordsAreRead);
      // Said outright rather than left to the default mark: that mark is read
      // when the picture itself is first read, and words added to a picture
      // already playing would simply stay switched off.
      element.track.mode = "showing";
    },
    [whenTheWordsAreRead],
  );

  /* Where the viewer stopped, applied once the browser knows how long the
     film is. Setting it earlier is ignored, silently, and the film starts
     from the beginning as though nothing had been watched. */
  const onReady = () => {
    const element = video.current;
    if (!element) {
      return;
    }
    element.playbackRate = speed;
    const target = resumeAt.current;
    resumeAt.current = null;
    if (target !== null && target > 0) {
      element.currentTime = target;
    }
    setReadyPicture(pictureKey);
  };

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

      {/* One element for both, told apart by its key: a film handed over as a
          file carries its address, a rebuilt one is fed by the library, and
          switching between the two has to start from a fresh element rather
          than from one still holding the other's address. */}
      {plan && !failed && (canBePlayedAsItIs(plan) || stream) && (
        <video
          key={pictureKey ?? undefined}
          ref={video}
          className="player-video"
          src={canBePlayedAsItIs(plan) ? plan.url : undefined}
          controls
          autoPlay
          onLoadedMetadata={onReady}
          onTimeUpdate={(event) => {
            lastPosition.current = event.currentTarget.currentTime;
          }}
          onPause={report}
          onEnded={report}
          onError={(event) => {
            setFailed("player.cannot_play");
            const refused = event.currentTarget.error;
            setRefusal(
              refused ? `${refused.code} · ${refused.message || "no reason given"}` : null,
            );
          }}
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
                onChange={(event) => choose(event.target.value || null, subtitleId)}
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
                onChange={(event) => choose(audioId, event.target.value || null)}
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
            <select value={quality.key} onChange={(event) => setQuality(event.target.value)}>
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
              onChange={(event) => {
                const chosen = Number(event.target.value);
                setSpeed(chosen);
                if (video.current) {
                  video.current.playbackRate = chosen;
                }
              }}
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
            <button
              className="toggle"
              onClick={() => {
                void video.current?.requestPictureInPicture?.();
              }}
            >
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

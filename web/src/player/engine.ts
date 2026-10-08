/*
 * Playing a film: the part that has nothing to do with how it looks.
 *
 * Everything here answers one question, "what is being played and how is it
 * getting here", and none of it knows where a button sits. Asking the server
 * what to do with this film, opening the session it is rebuilt into, feeding
 * the pieces in, keeping the session alive, remembering where the viewer got
 * to, and picking the film up again when something gives way.
 *
 * Kept apart from the player's own layout on purpose. The two used to live in
 * one file, which meant that changing where a button sits was a change to the
 * thing that plays the film: every new look risked a film that no longer
 * played, and the only way to know was to watch one. Now the look can be
 * rewritten from nothing without this being opened.
 *
 * The element itself belongs here rather than to the layout, because the
 * library that feeds a rebuilt film has to be attached to it and the resume
 * point has to be applied to it. Whoever draws the player is handed the
 * reference and puts it on the element they draw, and that is the whole of
 * what the drawing has to know: nothing out there moves the film, sets the
 * sound, starts it or stops it, or listens to anything it says. It asks here
 * and reads back what comes out, so a player drawn another way tomorrow
 * cannot leave half of playing a film behind by forgetting to wire it up.
 */

import type Hls from "hls.js";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

import { api, ApiError } from "../api";
import { wasAbandoned } from "../asking";
import type { HowItMoved, PlaybackPlan, PlaybackSession, Reading, WideGamutChoice } from "../api";
import { qualityCalled, rememberQuality, storedQuality } from "./quality";
import type { Quality } from "./quality";
import { codecCalled, rememberCodec, requestedCodec, storedCodec } from "./codec";
import type { Codec } from "./codec";
import { rememberLoudness, storedLoudness } from "./loudness";
import { clientProfile } from "./profile";
import { markTheOpening } from "./opening";
import { watchTheReading } from "./watch";
import { sayHowItDecodes } from "./card";
import { useLoading } from "./loading";
import { useWords } from "./words";
import type { Watching } from "./watch";

/** The library itself, as the dynamic import hands it over. */
type HlsLibrary = Awaited<typeof import("hls.js")>["default"];

/** The speeds offered. Whole steps: nobody asks for 1.17 times. */
export const SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2];

/** How often the position is sent while a film plays. */
const REPORT_EVERY = 10_000;

/** The longest the picture is kept covered after its first picture. */
const COVERED_AT_MOST_MS = 1000;
/** Two pictures further apart than this in the film are a jump rather than
 *  the film running from one to the next. */
const ONE_STEP_OF_A_FILM_S = 0.25;

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
 * Its answer moves the number a viewer watches, and a segment is often
 * written in about a second: four looks a second let the number climb
 * through it rather than jump. Each costs the server a few file checks.
 */
const PREPARATION_EVERY = 250;

/**
 * How often the server is told somebody still has the film open.
 *
 * A film playing says so every few seconds by asking for its next segment. A
 * film paused says nothing at all, and the server sweeps away a session
 * nobody has asked anything of after two minutes. Thirty seconds leaves room
 * for a browser that slows its clocks down while the tab is in the
 * background, which they all do.
 */
const STILL_HERE_EVERY = 30_000;

/**
 * How wide a gap in the film the library hops over rather than waiting on.
 *
 * Two readings of the same film do not join on the sound: the picture is
 * carried over untouched and lines up to the millisecond, the sound is rebuilt
 * and lands up to one of its own frames away, a little over twenty
 * thousandths. A jump back into film produced by an earlier reading therefore
 * leaves a gap of a few hundredths where the two meet.
 *
 * The library's own figure is a tenth of a second, and it is measured from
 * where the film has got to rather than across the gap itself, so a gap of six
 * hundredths reached with a twentieth still to play counts as more than a
 * tenth: measured, and it cost two seconds of nothing at all before the
 * library hopped over it. Half a second covers the seam with room to spare and
 * is still far too short to hop over anything a viewer would miss.
 */
const A_SEAM_IS_WORTH_HOPPING = 0.5;

/** Whether the browser is already holding the film around one moment. */
function isHeldAround(element: HTMLVideoElement, moment: number): boolean {
  const held = element.buffered;
  for (let index = 0; index < held.length; index += 1) {
    if (held.start(index) <= moment && moment < held.end(index)) {
      return true;
    }
  }
  return false;
}

/** Whether the film can reach this browser without being rebuilt. */
export function canBePlayedAsItIs(plan: PlaybackPlan): boolean {
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
    case "too_many_streams":
      return "player.too_many_streams";
    case "no_room_left":
      return "player.no_room";
    default:
      return "error.unreachable";
  }
}

/** Everything the player is handed to draw itself and to be driven by. */
export interface Playback {
  /** The element the film plays in. Put on the one the player draws.
   *
   *  Empty until React has drawn it, which the type now says out loud: a
   *  holder made with nothing in it answers with nothing until the element
   *  exists, and React only started admitting that in its nineteenth
   *  version. Nothing here changes, every reader already looked before
   *  touching it. */
  video: React.RefObject<HTMLVideoElement | null>;
  /** What the server decided, once it has answered. */
  plan: PlaybackPlan | null;
  /** The session a rebuilt film is fed from, when there is one. */
  stream: PlaybackSession | null;
  /** Whether this film has to be rebuilt rather than handed over as it is. */
  rebuilt: boolean;
  /** What went wrong, as a word this interface knows how to say. */
  failed: string | null;
  /** What the browser itself said when it refused, word for word. */
  refusal: string | null;
  /** How far there is left to wait, from nought to a hundred, while the
   *  picture is not there yet. Reaches a hundred at the same moment the
   *  picture does, never before. */
  loadingPercent: number;
  /** Which picture is on screen: the file itself, or one session of segments. */
  pictureKey: string | null;
  /** Which picture the browser has actually opened. */
  readyPicture: string | null;
  /** Which picture is on screen and moving: kept covered until then. */
  movingPicture: string | null;
  audioId: string | null;
  subtitleId: string | null;
  quality: Quality;
  /** Which codec a transcode is asked to come out in. "auto" leaves the
   *  choice to the usual negotiation. */
  codec: Codec;
  /** What is done with the film's wide gamut colour for this film alone, and
   *  null when that is left to the account's own choice. */
  wideGamut: WideGamutChoice | null;
  speed: number;
  /** Chooses a soundtrack and a subtitle, and remembers the choice. */
  choose: (audio: string | null, subtitle: string | null) => void;
  setQuality: (key: string) => void;
  setCodec: (key: string) => void;
  setWideGamut: (choice: WideGamutChoice | null) => void;
  setSpeed: (value: number) => void;
  /** A hand landing on the bar, and coming off it, saying what it did. */
  viewerMoving: () => void;
  viewerMoved: (how: HowItMoved) => void;
  /** A fixed step back or on, from a button or from the keyboard. */
  stepBy: (seconds: number) => void;
  /** Where the film is put, in seconds from its beginning. */
  goTo: (seconds: number) => void;
  /** Starts the film, or stops it. */
  playOrPause: () => void;
  /**
   * A click on the picture itself, which may turn out to be half of two.
   *
   * One starts or stops the film, two put it fullscreen, and a browser sends
   * the first of the two before it knows there is a second. So the first is
   * held for the moment it takes to find out: a film that flickers to a stop
   * and back on its way to fullscreen is what acting on it at once looks
   * like.
   */
  pictureClicked: (twice: boolean) => void;
  /** How loud, from nought to one, and whether the sound is off. */
  setLoudness: (volume: number) => void;
  setMuted: (off: boolean) => void;
  /** Where the film has got to, how long it is, and how far it is held. */
  at: number;
  length: number;
  loaded: number;
  playing: boolean;
  muted: boolean;
  loudness: number;
  /** Whether the words are on their way, and whether they never came. */
  words: "coming" | "refused" | null;
  /** The words on screen at this instant, one entry per line, stripped of any
   *  tag a subtitle file carried and no browser here is drawing. */
  shownWords: string[];
  /** Whether the film starts again by itself when it ends. */
  repeat: boolean;
  setRepeat: (on: boolean) => void;
  /** How far the words are shifted, in seconds. Positive is later. */
  wordsOffset: number;
  setWordsOffset: (seconds: number) => void;
  /** Put on the element carrying the words, whenever there is one. */
  holdTheWords: (element: HTMLTrackElement | null) => void;
  /** Puts the picture in a corner of the screen, where the browser allows it. */
  intoTheCorner: () => void;
}

/**
 * Plays one film, from asking the server what to do with it to leaving it.
 *
 * `fromTheStart` is set when the viewer asked to begin again rather than to
 * carry on, and `startAt` when they chose a moment of their own, a chapter.
 */
export function usePlayback({
  sourceId,
  workId,
  fromTheStart,
  startAt,
  onEnded,
  onStopped,
}: {
  sourceId: string;
  workId: string;
  fromTheStart?: boolean;
  startAt?: number;
  /** Told when the film reaches its end on its own, for whoever wants to put
   *  something else on after it. Never told when the viewer asked for it to
   *  repeat, because the browser then never reaches an end at all. */
  onEnded?: () => void;
  /** Told once when an administrator asked for this film to stop. */
  onStopped?: () => void;
}): Playback {
  const video = useRef<HTMLVideoElement>(null);
  const [plan, setPlan] = useState<PlaybackPlan | null>(null);
  /* The film the server was last told is being watched on this device, which
     it only learns from a plan asked for by the player. */
  const [watchedOnTheServer, setWatchedOnTheServer] = useState<string | null>(null);
  /* Which rung of the ladder the plan in hand answers for.
     The plan and the quality are two halves of one question and they do not
     arrive together: asked for a lighter picture, the answer for the old one
     stays in hand for as long as the new one takes to come back. Read as a
     pair in between, they describe a film nobody ever asked for, and the
     server is told to produce it. Measured: one change of quality opened two
     sessions, and the first was abandoned mid request, so the page never
     learnt its name and the server rebuilt a film for nobody until it swept
     it away. */
  const [planFor, setPlanFor] = useState<string | null>(null);
  /* Which codec the plan in hand answers for, for the same reason as planFor
     and read the same way: together with it, never apart. */
  const [codecFor, setCodecFor] = useState<string | null>(null);
  const [stream, setStream] = useState<PlaybackSession | null>(null);
  /* What to do to the library feeding the film in pieces while the viewer is
     moving the bar, and once they have finished. Held here rather than passed
     down, because it only exists while a session is being fed in pieces: a
     film played as it is has nothing of the sort. */
  const whileTheViewerMoves = useRef<{ hold: () => void; letGo: () => void } | null>(null);
  /* Who is following this film, so that the controls can say what moved it.
     A click and a drag are the same thing to the element and two different
     things to the library, and only the bar knows which one happened. */
  const watching = useRef<Watching | null>(null);
  /* Where the film has got to, how long it is, how far the browser holds it,
     and how it sounds. Read off the element rather than remembered alongside
     it: the film is what moves, and a copy of where it has got to is a copy
     that goes wrong the moment anything else moves it. Kept here rather than
     in whatever draws the bar, so that a bar drawn another way tomorrow is
     handed the same numbers instead of going and fetching them again. */
  const [at, setAt] = useState(0);
  const [length, setLength] = useState(0);
  const [loaded, setLoaded] = useState(0);
  const [playing, setPlaying] = useState(false);
  /* What `playing` said before the element was replaced, read when the new one
     is set up. */
  const wasPlaying = useRef(false);
  wasPlaying.current = playing;
  const [muted, setMutedState] = useState(false);
  const [loudness, setLoudnessState] = useState(1);
  const { words, shownWords, holdTheWords, wordsOffset, setWordsOffset } = useWords(video);
  /* Whether the film starts itself again when it ends. */
  const [repeat, setRepeatState] = useState(false);
  /* How many times the session has been opened again from nothing. Counted
     rather than flagged because it is what makes the film reopen at all: the
     session is opened by an effect, and an effect only runs again when
     something it watches has changed. */
  const [afresh, setAfresh] = useState(0);
  const [failed, setFailed] = useState<string | null>(null);
  /* What the browser itself said when it refused, word for word. The message
     above is this interface's wording and says only that something went
     wrong; this is the sentence that says which thing, and without it the
     answer lives in a console nobody opens. */
  const [refusal, setRefusal] = useState<string | null>(null);
  const [audioId, setAudioId] = useState<string | null>(null);
  const [subtitleId, setSubtitleId] = useState<string | null>(null);
  /* Counts how many times a choice has actually been made, so that asking
     again for what this already holds still asks the server again.
     Both begin at nothing, which is also what "no subtitle" is asked with:
     a viewer's very first choice can be turning subtitles off on a track the
     server picked for itself without ever being told to, and asking for
     nothing when nothing is already held is a choice React sees no change
     in and, left to it, silently drops. */
  const [choiceSerial, setChoiceSerial] = useState(0);
  /* Said to the server in so many words: naming no subtitle only says
     nothing, and left to itself the server picks the one the account's mode
     asks for, which is the one the viewer just turned off. */
  const subtitlesOff = choiceSerial > 0 && subtitleId === null;
  const [speed, setSpeedState] = useState(1);
  /* What the viewer asked the picture to be held to. Kept across films rather
     than per film: somebody watching on a thin connection is on a thin
     connection for the next one too. */
  const [quality, setQualityState] = useState(storedQuality);
  /* Which codec a transcode is asked to come out in. "auto" leaves the choice
     to the usual negotiation, which is what nearly everyone wants nearly
     always; forcing one is for chasing a stutter. */
  const [codec, setCodecState] = useState(storedCodec);
  /* What to do with the film's wide gamut colour, over the account's own
     choice. For this film alone and never remembered: it answers a doubt
     about one screen on one evening, and the account's choice is where a
     lasting answer goes. */
  const [wideGamut, setWideGamut] = useState<WideGamutChoice | null>(null);
  /* Which picture the browser has actually opened. Null until it has: the
     words are hung on the picture, and only once it is there. */
  const [readyPicture, setReadyPicture] = useState<string | null>(null);
  /* Which picture has been seen moving. A browser puts up the first picture
     of a film the moment it has it and sets the film going a beat later, so
     that picture stood still on the screen for up to a fifth of a second
     every time a film started: measured on every start in the maintainer's
     journal, and felt as a stutter before the film ran smoothly. */
  const [movingPicture, setMovingPicture] = useState<string | null>(null);
  /* The last position seen, kept apart from the element. On the way out the
     element is already gone, and that is exactly the moment the position is
     worth sending. */
  const lastPosition = useRef(0);
  /* Kept in a hand rather than named in the listener's dependencies: what to
     do at the end can change while a film plays, and rebuilding the listeners
     for it would tear the element's own down and put them back mid film. */
  const reachedTheEnd = useRef(onEnded);
  reachedTheEnd.current = onEnded;
  /* Kept in a hand for the same reason, and emptied once told: the answer
     saying so can come back more than once before the player is gone. */
  const toldToStop = useRef(onStopped);
  toldToStop.current = onStopped;
  const stopHeard = useRef(false);
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
  /* Where the session about to be watched is opened at, told to the server
     when it is opened. The server then puts it in the playlist it writes, and
     every player reads it from there: left to find out for itself, one asks
     for the opening of a film nobody is at and the server builds a segment
     that will never be seen. */
  const openedAt = useRef(0);
  /* Which picture the listeners below were last hung on, to tell a fresh
     element from the same one listened to again. */
  const listenedTo = useRef<string | null>(null);

  /* Asked again whenever a track changes: which tracks are wanted is part of
     the question, and the answer can change with it. A film played as it is
     becomes one that has to be rebuilt the moment someone picks a subtitle
     that can only be drawn into the picture. */
  useEffect(() => {
    const controller = new AbortController();
    /* The profile is measured rather than read off a list, which takes a
       moment the first time and nothing at all afterwards. */
    clientProfile(quality)
      .then((profile) =>
        api.plan(
          sourceId,
          {
            profile,
            audio_track_id: audioId,
            subtitle_track_id: subtitleId,
            subtitles_off: subtitlesOff,
            preferred_video_codec: requestedCodec(codec),
            wide_gamut: wideGamut,
            // This is the player about to show the film, which is what makes
            // it one being watched.
            watching: true,
          },
          controller.signal,
        ),
      )
      .then((answer) => {
        if (!opened.current) {
          opened.current = true;
          resumeAt.current =
            startAt ?? (fromTheStart ? null : answer.resume_from_seconds);
        }
        // Together, always: what is produced is decided on the three of them.
        setPlan(answer);
        setWatchedOnTheServer(workId);
        setPlanFor(quality.key);
        setCodecFor(codec.key);
        setFailed(null);
      })
      .catch((error) => {
        // Walking away from the question is not the server failing to answer
        // it, and that is one rule for the whole interface rather than one
        // this file keeps its own copy of.
        if (!wasAbandoned(error)) {
          setFailed(wording(error));
        }
      });
    return () => controller.abort();
  }, [sourceId, workId, audioId, subtitleId, fromTheStart, startAt, quality, codec, wideGamut, choiceSerial]);

  const rebuilt = plan !== null && !canBePlayedAsItIs(plan);
  /* A subtitle made of pictures has to be painted into the picture, so the one
     chosen is part of what the server produces. Named here rather than read
     off the method, which stays the same either way: choosing another one used
     to change nothing at all, and the words only appeared once something else
     forced a new session, which is a very long way of saying they never
     appeared. Read off the answer rather than off what the viewer picked,
     since the server picks one on its own too, and turning that one off has
     to change what is produced. */
  const paintedIn =
    plan?.subtitles.find((track) => track.id === plan.chosen_subtitle_id)?.burns_in === true
      ? plan.chosen_subtitle_id
      : null;
  /* What the server would actually be asked to produce. A subtitle handed
     over alongside the picture changes none of it, so turning subtitles on
     must not throw away a conversion already under way and make the viewer
     wait through it again. */
  const beingProduced = rebuilt
    ? `${plan.method}:${audioId ?? ""}:${paintedIn ?? ""}:${plan.wide_gamut?.converted ?? ""}:${planFor ?? ""}:${codecFor ?? ""}:${afresh}`
    : null;
  /* Which picture is on screen: the file itself, or one session of segments.
     A change here means a fresh element rather than a new address on the old
     one, because the two are fed in ways that cannot be swapped. */
  const pictureKey = plan === null ? null : canBePlayedAsItIs(plan) ? "file" : (stream?.id ?? null);
  const { loadingPercent, resetLoadingStage, enterLoadingStage, noteWritten, watchCarrying } = useLoading(
    stream?.id ?? null,
    readyPicture,
    pictureKey,
  );

  /* A film this browser cannot open is rebuilt by the server, which needs a
     session to rebuild it into. The session is closed on the way out, and by
     the server itself if that never arrives: a viewer who closes a tab says
     nothing, and a tool left running is a core spent on nobody. */
  useEffect(() => {
    if (!beingProduced) {
      setStream(null);
      return;
    }
    // Starting over: a session opened from nothing owes nobody the tail end
    // of an earlier one's progress, whether this is the first ever or the
    // fourth after a viewer kept changing the quality.
    resetLoadingStage();

    /* Picking another soundtrack starts a new session from nothing, and its
       picture would open at the beginning. Carrying the position over is what
       makes changing a track feel like changing a track. */
    if (lastPosition.current > 0) {
      resumeAt.current = lastPosition.current;
    }
    openedAt.current = Math.max(0, resumeAt.current ?? 0);

    /* Never given up on once it is out, unlike every other request here. The
       server opens the session while the asking is in flight, and giving up
       throws away the only copy of its name: the answer never arrives, the
       page has nothing to close, and a media tool rebuilds a film for nobody
       until the server sweeps the session away on its own. Waited for and
       closed instead, which costs one round trip nobody is watching. */
    let gone = false;

    clientProfile(quality)
      .then((profile) =>
        api.openSession(sourceId, {
          // A subtitle is named only when it has to be painted into the
          // picture. One made of words travels on its own beside it, and
          // naming it here would rebuild the film for nothing.
          profile,
          audio_track_id: audioId,
          subtitle_track_id: paintedIn,
          subtitles_off: paintedIn === null && subtitlesOff,
          preferred_video_codec: requestedCodec(codec),
          wide_gamut: wideGamut,
          start_at_seconds: openedAt.current,
        }),
      )
      .then((opening) => {
        if (gone) {
          api.closeSession(opening.id);
          return;
        }
        session.current = opening.id;
        setStream(opening);
        enterLoadingStage(opening.id, "session_opened");
      })
      .catch((error) => {
        // A viewer who has moved on is not told about a film they left.
        if (!gone) {
          setFailed(wording(error));
        }
      });

    return () => {
      gone = true;
      if (session.current) {
        api.closeSession(session.current);
        session.current = null;
      }
    };
    /* What is being produced, and which film. Nothing else, and the other
       four that used to sit here were the other half of the double opening:
       every one of them is already inside what is being produced, except the
       quality, which must not set this going on its own. A rung is picked,
       the server is asked again what to do with the film at that rung, and
       the answer is what says whether a new session is needed at all. Woken
       by the rung itself, this ran once on the old answer and once on the
       new, and the film played by neither. */
  }, [beingProduced, sourceId]);

  /* A session is kept alive by being asked for something, which a film playing
     does every few seconds and a film paused never does. Without this, pausing
     long enough has the server sweep the session away with the viewer sitting
     in front of it: they press play and every segment answers that the session
     is over, which reaches them as a browser that cannot read the film.

     The refusal is an answer worth acting on rather than a failure: there is
     nothing to go back to, so another session is opened where the viewer
     stands. That also covers a server restarted in the middle of a film. */
  useEffect(() => {
    const name = stream?.id;
    if (!name) {
      return;
    }
    let gone = false;
    const sayWeAreStillHere = () => {
      api
        .stillWatching(name)
        .then((stillThere) => {
          if (!gone && !stillThere) {
            setAfresh((times) => times + 1);
          }
        })
        .catch(() => {
          // One that did not arrive says nothing: the next carries the same
          // news, and a film playing is not worth interrupting for it.
        });
    };
    const beating = window.setInterval(sayWeAreStillHere, STILL_HERE_EVERY);
    // A tab coming back to the front is the moment a viewer is about to press
    // play, and the moment a browser that slowed its clocks right down has
    // missed a beat or two.
    const cameBack = () => {
      if (document.visibilityState === "visible") {
        sayWeAreStillHere();
      }
    };
    document.addEventListener("visibilitychange", cameBack);
    return () => {
      gone = true;
      window.clearInterval(beating);
      document.removeEventListener("visibilitychange", cameBack);
    };
  }, [stream]);

  /* Asked for while the picture is not there yet, and not a moment longer:
     once the film is playing this would be a request a second for something
     nobody is looking at. Nor once the film has failed, which leaves the
     picture never there: asked on behind the words saying so, it was four
     requests a second for as long as the player stayed open. */
  useEffect(() => {
    const name = stream?.id;
    if (!name || readyPicture === pictureKey || failed !== null) {
      return;
    }
    const controller = new AbortController();
    const look = () => {
      api
        .preparation(name, controller.signal)
        .then((seen) => {
          noteWritten(seen.first_written);
          if (seen.first_written >= 1) {
            // The piece the browser needs first exists on the server now. What
            // is left of the wait is getting it here, which the browser counts.
            enterLoadingStage(name, "produced");
          } else if (seen.step !== "starting") {
            enterLoadingStage(name, "producing");
          }
        })
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
  }, [stream?.id, readyPicture, pictureKey, failed, noteWritten, enterLoadingStage]);

  /* Feeding the segments in.

     The library first, and the browser's own reader only where there is no
     library to run: that is the one case it is better at, and it is the one
     browser that gives us no choice.

     It used to be the other way round, on the assumption that a browser saying
     it reads a playlist is Apple's. That stopped being true: a browser that
     says so and is handed the address reads the playlist itself, and then none
     of what is decided here applies to it. It ignored where the playlist said
     to begin, so it asked for the opening of the film and only jumped once the
     picture had started, which had the server build three segments nobody
     would ever see; it said nothing about what it was doing, since everything
     the page knows it knows through the library; and nothing on this side
     could tell, because it plays perfectly well. */
  useEffect(() => {
    const element = video.current;
    if (!element || !stream) {
      return;
    }

    let feed: Hls | null = null;
    let gone = false;
    /* What only this side can see: the film stopping in the middle of itself,
       the picture standing still while the sound runs on, and where a viewer
       jumped from. The server knows what it produced and when it handed it
       over, never whether any of it reached a screen. */
    watching.current = watchTheReading(element, { session: stream.id });
    const stopWatching = watching.current.stop;
    /* Whether the library has already given up. One failure comes back as
       three: it cannot make room for the film, then it cannot put anything in
       the room it did not make. Only the first says anything. */
    let gaveUp = false;

    /* Fetched only now, and only by the viewer who needs it. Most films play
       as they are and never touch this, so carrying it in every page would
       slow the whole interface for the few that do. */
    void import("hls.js").then(({ default: Library }) => {
      if (gone) {
        return;
      }
      if (!Library.isSupported()) {
        // No library here, which is a browser without the parts it is built
        // on. Those read a playlist themselves, and this is what that is for.
        if (element.canPlayType("application/vnd.apple.mpegurl")) {
          element.src = stream.playlist_url;
          return;
        }
        setFailed("player.cannot_play");
        return;
      }
      feed = new Library({
        fragLoadingTimeOut: SEGMENT_PATIENCE,
        maxBufferHole: A_SEAM_IS_WORTH_HOPPING,
        // The segments carry no words, so the library has no business
        // touching the subtitles on the picture: left to itself it takes
        // charge of every one it finds there and empties ours as it goes.
        renderTextTracksNatively: false,
      });
      feed.subtitleDisplay = false;
      /* Everything, not only what lies ahead: what the browser is holding was
         produced by one reading of the film, and a jump starts another. The
         two do not join. Measured on the maintainer's library: the picture
         copied over untouched lines up to the millisecond between two
         readings, and the sound, which is rebuilt, lands up to one of its own
         frames away. The browser holds the two pieces with a gap of a few
         hundredths between them, waits two seconds on it, hops over it, and
         then shows nothing for three seconds while the picture catches up
         with a sound that never stopped. That is exactly what a viewer
         reports as the picture freezing with the sound going on.

         The cost is that a jump is fetched again rather than played from
         what is held, which is what a jump costs anyway. */
      // Whether the library was stopped and is owed a start.
      let stopped = false;
      whileTheViewerMoves.current = {
        // Nothing fetched while a hand is on the bar. Left to itself the
        // library chases every twitch of it, fetching pieces of film nobody
        // will ever see. Only for a hand on the bar: a step of ten seconds has
        // no in between, and stopping the library only to start it again in
        // the same breath is two orders it never needed.
        hold: () => {
          stopped = true;
          feed?.stopLoad();
        },
        letGo: () => {
          // Thrown away only when the viewer has landed somewhere the browser
          // was not already holding.
          //
          // It used to be thrown away on every jump, and that was measured to
          // be worse than what it cured. Landing inside what is held, a
          // browser seeks in it and shows the picture at once, which is what
          // every player in the world relies on. Emptied first, it has to be
          // handed the same film again and start cold in the middle of a group
          // of pictures, and it then shows nothing at all until the next whole
          // picture comes round: measured four times over at two and a half to
          // five seconds, with the browser holding an unbroken stretch from
          // before that whole picture to thirty five seconds past the viewer,
          // and saying it had everything it needed.
          //
          // Landing where nothing is held, there is nothing to lose and the
          // stale film further on is worth being rid of.
          const emptied = !isHeldAround(element, element.currentTime);
          if (emptied) {
            feed?.trigger(Library.Events.BUFFER_FLUSHING, {
              startOffset: 0,
              endOffset: Number.POSITIVE_INFINITY,
              type: null,
            });
          }
          // Only when something was actually disturbed. A jump that landed in
          // film already held and never stopped the library is a jump the
          // browser handles on its own, as it does in every other player, and
          // the less said to the library the fewer ways it has of ending up
          // somewhere nobody can describe.
          if (emptied || stopped) {
            feed?.startLoad(element.currentTime);
          }
          stopped = false;
        },
      };
      sayWhereItBegan(Library, feed, stream.id);
      sayWhereSegmentsLand(Library, feed, stream.id);
      // The playlist has been read, and the library now knows what to ask
      // for. Whatever is slow from here on is either the server producing it
      // or the network carrying it, not this.
      feed.on(Library.Events.MANIFEST_PARSED, () =>
        enterLoadingStage(stream.id, "manifest_parsed"),
      );
      // The very first piece of film has reached this browser and been read.
      // What is left of the wait is the browser noticing it has a film, which
      // is `loadedmetadata` below and is usually the shortest part of all of
      // it. A later piece loading is not this moment and says nothing new.
      //
      // The header is fetched as a fragment too and is numbered by name
      // rather than by place, exactly as it is below in `sayWhereItBegan`: it
      // is a few hundred bytes and loads in no time on any connection, and
      // counting it as the first piece is what made this climb to ninety on
      // a slow link and then sit there for as long as the real first piece,
      // holding several seconds of picture and sound, took to follow it.
      let firstPieceArrived = false;
      // Its bytes counted as they arrive, which is the last real wait before
      // the picture.
      let firstPieceAsked = false;
      feed.on(Library.Events.FRAG_LOADING, (_event, loading) => {
        if (firstPieceAsked || typeof loading.frag.sn !== "number") {
          return;
        }
        firstPieceAsked = true;
        const stats = loading.frag.stats;
        watchCarrying(() => (stats.total > 0 ? stats.loaded / stats.total : 0));
      });
      // Each piece of picture handed to the browser, so the opening seconds
      // say whether a lost picture follows one.
      feed.on(Library.Events.BUFFER_APPENDED, (_event, appended) => {
        if (appended.type === "video" || appended.type === "audiovideo") {
          markTheOpening(element, "piece_added");
        }
      });
      feed.on(Library.Events.FRAG_LOADED, (_event, loaded) => {
        if (firstPieceArrived || typeof loaded.frag.sn !== "number") {
          return;
        }
        firstPieceArrived = true;
        enterLoadingStage(stream.id, "first_fragment_loaded");
      });
      let hiccupsSaid = 0;
      feed.on(Library.Events.ERROR, (_event, trouble) => {
        const because = [
          trouble.type,
          trouble.details,
          trouble.error?.message,
          trouble.reason,
        ]
          .filter(Boolean)
          .join(" · ");
        // Anything short of fatal is retried on its own, and saying so to the
        // viewer would turn an invisible hiccup into an error to read. The
        // journal hears of the first few, since a segment fetched again and
        // again is only explained there.
        if (!trouble.fatal) {
          if (hiccupsSaid < HICCUPS_SAID_PER_FILM) {
            hiccupsSaid += 1;
            const segment = trouble.frag?.sn;
            api
              .tellTheJournal({
                session: stream.id,
                saw: "library_hiccup",
                because,
                segment: typeof segment === "number" ? segment : null,
              })
              .catch(() => {});
          }
          return;
        }
        if (gaveUp) {
          return;
        }
        gaveUp = true;

        /* A browser that reads a playlist on its own is handed the address
           rather than the viewer being handed an error. It has its own way
           through a film, which is not this one: measured on a film rebuilt
           into AV1, the library could not even make room for it while the
           browser played it. A film that plays the plain way beats a film
           that does not play. */
        const browserTakesOver = element.canPlayType("application/vnd.apple.mpegurl") !== "";
        sayItGaveUp({ session: stream.id }, because, browserTakesOver);
        if (browserTakesOver) {
          feed?.destroy();
          feed = null;
          element.src = stream.playlist_url;
          return;
        }

        setFailed("player.cannot_play");
        setRefusal(because);
      });
      feed.loadSource(stream.playlist_url);
      feed.attachMedia(element);
    });

    return () => {
      gone = true;
      whileTheViewerMoves.current = null;
      watching.current = null;
      stopWatching();
      feed?.destroy();
    };
  }, [stream]);

  /* The same watch over a film handed over as it lies on the disk. The server
     hears nothing from such a film once it has handed it over, so whatever
     went wrong with it on the screen reached nobody: a browser that took the
     whole film and never moved it left an empty journal behind. */
  useEffect(() => {
    const element = video.current;
    if (!element || pictureKey !== "file") {
      return;
    }
    watching.current = watchTheReading(element, { source: sourceId });
    const stopWatching = watching.current.stop;
    return () => {
      watching.current = null;
      stopWatching();
    };
  }, [pictureKey, sourceId]);

  /* What decodes the picture now on screen, said once for every picture: a
     film that stutters here and plays elsewhere is most of the time a
     browser decoding it without the graphics card, which nothing on the
     server can see. */
  useEffect(() => {
    const element = video.current;
    if (!element || !plan || readyPicture === null || readyPicture !== pictureKey) {
      return;
    }
    const held = plan.film.picture;
    const codec = plan.rebuild?.codec ?? held?.codec;
    if (!codec) {
      return;
    }
    sayHowItDecodes(stream ? { session: stream.id } : { source: sourceId }, {
      codec,
      width: element.videoWidth,
      height: element.videoHeight,
      frameRate: held?.frame_rate ?? null,
      bitrate: plan.rebuild ? plan.rebuild.bitrate : (held?.bitrate ?? null),
      handedOver: canBePlayedAsItIs(plan) ? "file" : "media-source",
    });
    // Once per picture: the plan and the session move on their own without
    // the picture on screen changing.
  }, [readyPicture]);

  /* A choice is remembered once it has been made, never on the way in: the
     tracks a page opens with are what the rules already decided, and writing
     them back would turn a habit into a decision nobody made. */
  const choose = useCallback(
    (audio: string | null, subtitle: string | null) => {
      setAudioId(audio);
      setSubtitleId(subtitle);
      // Counted rather than read back off the two above: a choice that
      // repeats what they already hold, such as turning off a subtitle the
      // server had picked on its own, is still a choice this made and the
      // server has not yet heard.
      setChoiceSerial((serial) => serial + 1);
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
    },
    [workId, sourceId],
  );

  /* A hand on the bar: the library stops fetching until it comes off. What it
     would fetch in between is a piece of film nobody will see, and it would
     land underneath what is about to be thrown away. */
  const viewerMoving = useCallback(() => {
    whileTheViewerMoves.current?.hold();
  }, []);

  /* The hand off the bar. What the browser holds came out of one reading of
     the film and what comes next comes out of another, so it is thrown away
     and the library is set going again where the viewer landed. A film played
     as it is has no such thing and both of these do nothing. */
  const viewerMoved = useCallback((how: HowItMoved) => {
    /* Said before the library is set going again, so that the line about the
       jump carries the gesture that caused it whichever of the two lands
       first. */
    watching.current?.movedBy(how);
    whileTheViewerMoves.current?.letGo();
  }, []);

  /* Where the film is put, and the only place anything puts it. Everything
     that moves a film comes through here: the bar, the steps, the resume
     point. Held inside the film at both ends, because a step past the end is
     the film over and a bar dragged past the left is a negative second.

     What moved it is said by whoever asked, not here: a hand dragging the bar
     moves the film at every twitch and is one gesture, and this is called on
     every one of those twitches. */
  const goTo = useCallback((seconds: number) => {
    const element = video.current;
    if (!element || !Number.isFinite(seconds)) {
      return;
    }
    const furthest = element.duration;
    const landing = Math.max(
      0,
      Number.isFinite(furthest) ? Math.min(furthest, seconds) : seconds,
    );
    element.currentTime = landing;
    // Said here as well as read back off the element, so that a bar follows
    // the hand rather than the next word out of the browser.
    setAt(landing);
  }, []);

  /* A step back or on, wherever it was asked for: one way of moving the film
     means one place for it to be wrong and one place that says what moved it. */
  const stepBy = useCallback(
    (seconds: number) => {
      const element = video.current;
      if (!element) {
        return;
      }
      goTo(element.currentTime + seconds);
      // A step is one move with nothing in between, so there is nothing to
      // hold back first.
      viewerMoved("a_step");
    },
    [goTo, viewerMoved],
  );

  /* Starts the film, or stops it. Here rather than on the button, because the
     picture itself answers to a click too, and two places deciding what a
     click means is two places for them to disagree. */
  const playOrPause = useCallback(() => {
    const element = video.current;
    if (!element) {
      return;
    }
    if (element.paused) {
      void element.play();
    } else {
      element.pause();
    }
  }, []);

  /* A click on the picture, held long enough to find out whether a second one
     is coming. Two hundred thousandths is under what anybody notices on a play
     button and over what a browser takes to deliver the second click. */
  const aSecondClick = useRef(0);
  const pictureClicked = useCallback(
    (twice: boolean) => {
      window.clearTimeout(aSecondClick.current);
      if (twice) {
        // The first of the two was held and is now known to have been half of
        // a gesture that means something else. Whoever drew the picture deals
        // with what two clicks mean; all this does is not act on the first.
        return;
      }
      aSecondClick.current = window.setTimeout(playOrPause, 200);
    },
    [playOrPause],
  );

  useEffect(() => () => window.clearTimeout(aSecondClick.current), []);

  /* The sound is set on the element and read back off it, never held here as
     a number of its own: a headset and a key the browser answers by itself
     move it too, and a copy kept alongside would be wrong from then on. */
  const setLoudness = useCallback((volume: number) => {
    const element = video.current;
    if (!element) {
      return;
    }
    element.volume = volume;
    element.muted = volume === 0;
  }, []);

  const setMuted = useCallback((off: boolean) => {
    const element = video.current;
    if (element) {
      element.muted = off;
    }
  }, []);

  /* An administrator asked for this film to stop, heard on the live line or
     in an answer, whichever came first. Told to whoever opened the player
     once. */
  const heardToStop = useCallback(() => {
    if (!stopHeard.current) {
      stopHeard.current = true;
      toldToStop.current?.();
    }
  }, []);

  /* Held open for as long as the film is: its end is how the server knows
     the player is gone, however it went, and it is how the server says stop
     the moment it is asked. A browser ties it again by itself when the
     network drops it, but not when the server refused it.

     Opened once the plan is back, since asking for it is what tells the
     server this film is on: opened alongside, the line reaches the server
     first and is refused for a film this device left a moment ago, which is
     every film played again soon after it was closed, or for the episode
     before, still the one on record. */
  const lineOpen = watchedOnTheServer === workId;
  useEffect(() => {
    if (!lineOpen) {
      return;
    }
    const line = api.playerLine(workId);
    line.addEventListener("stop", heardToStop);
    return () => line.close();
  }, [workId, lineOpen, heardToStop]);

  /* Sent whatever the position, because it is also how the server knows
     where the film is and whether it stands still: a position worth
     remembering, or below that only that the player is still here, getting
     the film ready or in its first seconds. The answer says whether an
     administrator asked it to stop. */
  const tellTheServer = useCallback(
    (leaving: boolean) => {
      const seconds = lastPosition.current;
      const paused = video.current?.paused ?? true;
      const said =
        seconds >= WORTH_REPORTING
          ? api.reportPosition(workId, seconds, paused, leaving)
          : api.stillPlaying(workId, seconds > 0 ? seconds : null, paused, leaving);
      said
        .then((answer) => {
          if (answer.stop) {
            heardToStop();
          }
        })
        .catch(() => {
          // A position that could not be sent is not worth troubling a viewer
          // with: the next one carries the same news, and the film keeps
          // playing.
        });
    },
    [workId, heardToStop],
  );
  const report = useCallback(() => tellTheServer(false), [tellTheServer]);

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
    } else {
      api.stillPlayingOnTheWayOut(workId, seconds > 0 ? seconds : null);
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
      tellTheServer(true);
    };
  }, [report, tellTheServer, onTheWayOut]);

  /* Remembered as it is chosen, not on the way in: what a player opens with
     is what the viewer left it on, and writing that back would be a decision
     nobody made. */
  const setQuality = useCallback((key: string) => {
    const chosen = qualityCalled(key);
    setQualityState(chosen);
    rememberQuality(chosen);
  }, []);

  const setCodec = useCallback((key: string) => {
    const chosen = codecCalled(key);
    setCodecState(chosen);
    rememberCodec(chosen);
  }, []);

  const setSpeed = useCallback((value: number) => {
    setSpeedState(value);
    if (video.current) {
      video.current.playbackRate = value;
    }
  }, []);

  /* Where the viewer stopped, applied once the browser knows how long the
     film is. Setting it earlier is ignored, silently, and the film starts
     from the beginning as though nothing had been watched. */
  const onPictureReady = useCallback(() => {
    const element = video.current;
    if (!element) {
      return;
    }
    element.playbackRate = speed;
    const target = resumeAt.current;
    resumeAt.current = null;
    if (target !== null && target > 0) {
      // Said before the film is moved, so that the jump it causes is not read
      // as the browser moving the film on its own.
      watching.current?.movedBy("picked_up_where_it_was_left");
      goTo(target);
    }
    // The same instant the notice comes down: a number left behind at
    // whatever it last crept to would be a number that was still climbing
    // the moment the picture no longer needed it to. Absent for a film
    // played as it is, which never had a session to report against.
    enterLoadingStage(stream?.id ?? null, "done");
    setReadyPicture(pictureKey);
  }, [speed, pictureKey, goTo, enterLoadingStage, stream]);

  const setRepeat = useCallback((on: boolean) => {
    setRepeatState(on);
    const element = video.current;
    if (element) {
      element.loop = on;
    }
  }, []);

  const notePictureRefused = useCallback((refused: MediaError | null) => {
    setFailed("player.cannot_play");
    const because = refused ? `${refused.code} · ${refused.message || "no reason given"}` : null;
    setRefusal(because);
    // The one other way a film stops playing outright, beside the library
    // giving up on it: the browser's own decoder failing beneath it, with
    // hls.js none the wiser. Left unreported, a fault this real left nothing
    // behind but a screenshot. A film with no session is the file itself.
    if (because) {
      sayItGaveUp(session.current ? { session: session.current } : { source: sourceId }, because, false);
    }
  }, [sourceId]);

  /* The picture in a corner of the screen while the viewer does something
     else. Offered by the browser rather than by us, so whoever draws the
     button asks first whether there is one to draw. */
  const intoTheCorner = useCallback(() => {
    void video.current?.requestPictureInPicture?.();
  }, []);

  /* Everything the element says about itself, listened to from the moment it
     exists and again whenever it is a different one.

     All of it here rather than hung on the element by whoever draws it. Half
     of these are not about how a film looks at all: where the viewer got to,
     the resume point applied the moment the browser knows how long the film
     is, and the browser refusing the picture outright. Left to the drawing,
     a player drawn another way tomorrow has to know to wire them up, and a
     film that quietly starts from the beginning again is what forgetting one
     of them looks like.

     Before the browser is let anywhere near the element, rather than after:
     the picture is read as soon as it exists, and a listener attached a beat
     later is a listener that missed the reading. That beat is the whole of
     the resume point. */
  useLayoutEffect(() => {
    const element = video.current;
    if (!element) {
      return;
    }
    /* The element is a new one for every film and starts at full volume, so
       the setting is put back on it before anything is heard. */
    const wanted = storedLoudness();
    element.volume = wanted.volume;
    element.muted = wanted.muted;
    // A fresh element does not carry what the viewer asked of the last one.
    element.loop = repeat;
    /* Nor where the last one had got to. Going from a rebuilt film back to
       the file itself, a lighter picture given up or its colours kept after
       all, has no session to carry the position over, and the file would
       open at its beginning. Only when nothing already says where to pick
       up, and read before this element's own zero overwrites it. */
    if (listenedTo.current !== null && listenedTo.current !== pictureKey) {
      if (resumeAt.current === null && lastPosition.current > 0) {
        resumeAt.current = lastPosition.current;
      }
      // A film paused stays paused on its new element: it would otherwise
      // start by itself, as a fresh element does, after a restart of the
      // server or a change of track or quality.
      if (!wasPlaying.current) {
        element.autoplay = false;
      }
    }
    listenedTo.current = pictureKey;
    const tell = () => {
      setAt(element.currentTime);
      setLength(Number.isFinite(element.duration) ? element.duration : 0);
      setPlaying(!element.paused && !element.ended);
      setMutedState(element.muted);
      setLoudnessState(element.volume);
      const buffered = element.buffered;
      setLoaded(buffered.length > 0 ? buffered.end(buffered.length - 1) : 0);
      // Where the viewer is, kept in a hand rather than in the drawing: it is
      // what is sent to the server and what a fresh session picks up from,
      // and it has to survive the element going away.
      lastPosition.current = element.currentTime;
    };
    tell();
    const events = [
      "timeupdate",
      "durationchange",
      "loadedmetadata",
      "play",
      "pause",
      "ended",
      "progress",
      "volumechange",
      "seeking",
      "seeked",
    ];
    for (const name of events) {
      element.addEventListener(name, tell);
    }
    /* Whatever moves the sound, wherever from: the buttons on the bar, a
       keyboard key the browser answers on its own, a headset. */
    const remember = () => rememberLoudness({ volume: element.volume, muted: element.muted });
    const refused = () => notePictureRefused(element.error);
    element.addEventListener("volumechange", remember);
    element.addEventListener("loadedmetadata", onPictureReady);
    const finished = () => {
      report();
      reachedTheEnd.current?.();
    };
    // Said the moment it happens rather than on the next beat: somebody
    // following the film elsewhere sees it pause, start again or jump at once.
    element.addEventListener("pause", report);
    element.addEventListener("playing", report);
    element.addEventListener("seeked", report);
    element.addEventListener("ended", finished);
    element.addEventListener("error", refused);
    return () => {
      for (const name of events) {
        element.removeEventListener(name, tell);
      }
      element.removeEventListener("volumechange", remember);
      element.removeEventListener("loadedmetadata", onPictureReady);
      element.removeEventListener("pause", report);
      element.removeEventListener("playing", report);
      element.removeEventListener("seeked", report);
      element.removeEventListener("ended", finished);
      element.removeEventListener("error", refused);
    };
  }, [pictureKey, onPictureReady, report, notePictureRefused, repeat]);

  /* The picture is uncovered once two pictures in a row show the film
     running, never on the first alone. Uncovered anyway a moment after the
     first picture, for a film the browser left paused or a browser that
     cannot say when each picture reaches the screen: a covered picture that
     never comes out is worse than one that started with a hitch. */
  useEffect(() => {
    const element = video.current;
    if (!element || pictureKey === null) {
      return;
    }
    const key = pictureKey;
    let previous: number | null = null;
    let pending: number | null = null;
    let atMost = 0;
    const uncover = () => {
      window.clearTimeout(atMost);
      if (pending !== null) {
        element.cancelVideoFrameCallback?.(pending);
        pending = null;
      }
      markTheOpening(element, "uncovered");
      setMovingPicture(key);
    };
    if (!element.requestVideoFrameCallback) {
      element.addEventListener("playing", uncover, { once: true });
      return () => element.removeEventListener("playing", uncover);
    }
    const onPicture = (_now: number, picture: VideoFrameCallbackMetadata) => {
      pending = null;
      if (previous === null) {
        atMost = window.setTimeout(uncover, COVERED_AT_MOST_MS);
      } else if (
        !element.paused &&
        !element.seeking &&
        picture.mediaTime > previous &&
        picture.mediaTime - previous < ONE_STEP_OF_A_FILM_S
      ) {
        uncover();
        return;
      }
      previous = picture.mediaTime;
      pending = element.requestVideoFrameCallback(onPicture);
    };
    pending = element.requestVideoFrameCallback(onPicture);
    return () => {
      window.clearTimeout(atMost);
      if (pending !== null) {
        element.cancelVideoFrameCallback?.(pending);
      }
    };
  }, [pictureKey]);

  return {
    video,
    plan,
    stream,
    rebuilt,
    failed,
    refusal,
    loadingPercent,
    pictureKey,
    readyPicture,
    movingPicture,
    audioId,
    subtitleId,
    quality,
    codec,
    wideGamut,
    speed,
    choose,
    setQuality,
    setCodec,
    setWideGamut,
    setSpeed,
    viewerMoving,
    viewerMoved,
    stepBy,
    goTo,
    playOrPause,
    pictureClicked,
    setLoudness,
    setMuted,
    at,
    length,
    loaded,
    playing,
    muted,
    loudness,
    words,
    shownWords,
    holdTheWords,
    repeat,
    setRepeat,
    wordsOffset,
    setWordsOffset,
    intoTheCorner,
  };
}

/**
 * Tells the journal where the library decided to begin the film.
 *
 * Half of what happens when a film starts happens here rather than on the
 * server, and the journal showed none of it: the server could say it had been
 * asked for the seventeenth minute while this asked for the opening of the
 * film, with nothing anywhere saying which of the two was wrong.
 *
 * Three numbers, and only those three: what the playlist told the library, the
 * second it settled on, and the segment it asked for first. The server names
 * the facts it accepts and refuses anything else, so this is never a way of
 * putting a page's own words in somebody's journal.
 */
function sayWhereItBegan(Library: HlsLibrary, feed: Hls, session: string) {
  let playlistSaid: number | null = null;
  let told = false;

  feed.on(Library.Events.LEVEL_LOADED, (_event, level) => {
    playlistSaid = level.details.startTimeOffset;
  });

  feed.on(Library.Events.FRAG_LOADING, (_event, loading) => {
    // The header is fetched as a fragment too and is numbered by name rather
    // than by place, which is exactly what is not being asked here.
    if (told || typeof loading.frag.sn !== "number") {
      return;
    }
    told = true;
    api
      .tellTheJournal({
        session,
        saw: "playback_began",
        playlist_said_second: playlistSaid,
        began_at_second: loading.frag.start,
        first_segment: loading.frag.sn,
      })
      // Nothing waits on this: it is a line in a journal, and a film that
      // plays matters more than knowing where it started.
      .catch(() => {});
  });
}

/** How many segments of each run of the tool are said to the journal. */
const SEGMENTS_SAID_PER_RUN = 3;

/** How many troubles the library got over are said to the journal per film. */
const HICCUPS_SAID_PER_FILM = 20;

/**
 * Tells the journal where the first segments of each run of the tool were
 * placed, picture and sound apart.
 *
 * Each run begins where somebody jumped to, and a picture and a sound the
 * tool began at different moments are placed apart on the film's clock: seen
 * from here only, since the server knows what it wrote and never where the
 * browser put it.
 */
function sayWhereSegmentsLand(Library: HlsLibrary, feed: Hls, session: string) {
  let shift: number | null = null;
  let last = -1;
  let saidOfThisRun = 0;

  feed.on(Library.Events.INIT_PTS_FOUND, (_event, found) => {
    shift = found.initPTS / found.timescale;
  });

  feed.on(Library.Events.FRAG_BUFFERED, (_event, buffered) => {
    const { sn, start } = buffered.frag;
    if (typeof sn !== "number") {
      return;
    }
    saidOfThisRun = sn === last + 1 ? saidOfThisRun : 0;
    last = sn;
    if (saidOfThisRun >= SEGMENTS_SAID_PER_RUN) {
      return;
    }
    saidOfThisRun += 1;
    const streams = buffered.frag.elementaryStreams as Record<string, { startPTS: number } | null>;
    const together = streams.audiovideo?.startPTS ?? null;
    api
      .tellTheJournal({
        session,
        saw: "segment_placed",
        segment: sn,
        playlist_second: start,
        video_starts_second: streams.video?.startPTS ?? together,
        audio_starts_second: streams.audio?.startPTS ?? together,
        library_shift_second: shift,
      })
      .catch(() => {});
  });
}

/**
 * Tells the journal that the library gave up on a film, in its own words.
 *
 * The one thing the page reports that is not a number, because the wording is
 * the answer: the library naming what it could not do with a film this server
 * produced. Without it, a film that plays the plain way looks like a film that
 * plays, and the reason it had to is nowhere.
 */
function sayItGaveUp(reading: Reading, because: string, browserTookOver: boolean) {
  api
    .tellTheJournal({ ...reading, saw: "playback_refused", because, browser_took_over: browserTookOver })
    .catch(() => {});
}

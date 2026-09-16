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
 * reference and puts it on the element they draw.
 */

import type Hls from "hls.js";
import { useCallback, useEffect, useRef, useState } from "react";

import { api, ApiError } from "../api";
import type { HowItMoved, PlaybackPlan, PlaybackSession, Preparation } from "../api";
import { qualityCalled, rememberQuality, storedQuality } from "./quality";
import type { Quality } from "./quality";
import { clientProfile } from "./profile";
import { watchTheReading } from "./watch";
import type { Watching } from "./watch";

/** The library itself, as the dynamic import hands it over. */
type HlsLibrary = Awaited<typeof import("hls.js")>["default"];

/** The speeds offered. Whole steps: nobody asks for 1.17 times. */
export const SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2];

/**
 * How far one step of the film goes, in seconds.
 *
 * Ten, which is what every player uses and what the thing is for: missing a
 * line of dialogue, not choosing a scene. The bar is there for choosing a
 * scene.
 */
export const A_STEP = 10;

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
    default:
      return "error.unreachable";
  }
}

/** Everything the player is handed to draw itself and to be driven by. */
export interface Playback {
  /** The element the film plays in. Put on the one the player draws. */
  video: React.RefObject<HTMLVideoElement>;
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
  /** How far the server has got, while the picture is not there yet. */
  preparing: Preparation | null;
  /** Which picture is on screen: the file itself, or one session of segments. */
  pictureKey: string | null;
  /** Which picture the browser has actually opened. */
  readyPicture: string | null;
  audioId: string | null;
  subtitleId: string | null;
  quality: Quality;
  speed: number;
  /** Chooses a soundtrack and a subtitle, and remembers the choice. */
  choose: (audio: string | null, subtitle: string | null) => void;
  setQuality: (key: string) => void;
  setSpeed: (value: number) => void;
  /** A hand landing on the bar, and coming off it, saying what it did. */
  viewerMoving: () => void;
  viewerMoved: (how: HowItMoved) => void;
  /** A fixed step back or on, from a button or from the keyboard. */
  stepBy: (seconds: number) => void;
  /** Handed to the element: it is on these that the film picks itself up. */
  onPictureReady: () => void;
  notePosition: (seconds: number) => void;
  report: () => void;
  notePictureRefused: (refused: MediaError | null) => void;
}

/**
 * Plays one film, from asking the server what to do with it to leaving it.
 *
 * `fromTheStart` is set when the viewer asked to begin again rather than to
 * carry on.
 */
export function usePlayback({
  sourceId,
  workId,
  fromTheStart,
}: {
  sourceId: string;
  workId: string;
  fromTheStart?: boolean;
}): Playback {
  const video = useRef<HTMLVideoElement>(null);
  const [plan, setPlan] = useState<PlaybackPlan | null>(null);
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
  const [speed, setSpeedState] = useState(1);
  /* What the viewer asked the picture to be held to. Kept across films rather
     than per film: somebody watching on a thin connection is on a thin
     connection for the next one too. */
  const [quality, setQualityState] = useState(storedQuality);
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
  /* Where the session about to be watched is opened at, told to the server
     when it is opened. The server then puts it in the playlist it writes, and
     every player reads it from there: left to find out for itself, one asks
     for the opening of a film nobody is at and the server builds a segment
     that will never be seen. */
  const openedAt = useRef(0);

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
          },
          controller.signal,
        ),
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
    ? `${plan.method}:${audioId ?? ""}:${paintedIn ?? ""}:${quality.key}:${afresh}`
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

    clientProfile(quality)
      .then((profile) =>
        api.openSession(
          sourceId,
          {
            // A subtitle is named only when it has to be painted into the
            // picture. One made of words travels on its own beside it, and
            // naming it here would rebuild the film for nothing.
            profile,
            audio_track_id: audioId,
            subtitle_track_id: paintedIn,
            start_at_seconds: openedAt.current,
          },
          controller.signal,
        ),
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
    watching.current = watchTheReading(element, stream.id);
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
      feed.on(Library.Events.ERROR, (_event, trouble) => {
        // Anything short of fatal is retried on its own, and saying so would
        // turn an invisible hiccup into an error the viewer has to read.
        if (!trouble.fatal || gaveUp) {
          return;
        }
        gaveUp = true;
        const because = [
          trouble.type,
          trouble.details,
          trouble.error?.message,
          trouble.reason,
        ]
          .filter(Boolean)
          .join(" · ");

        /* A browser that reads a playlist on its own is handed the address
           rather than the viewer being handed an error. It has its own way
           through a film, which is not this one: measured on a film rebuilt
           into AV1, the library could not even make room for it while the
           browser played it. A film that plays the plain way beats a film
           that does not play. */
        const browserTakesOver = element.canPlayType("application/vnd.apple.mpegurl") !== "";
        sayItGaveUp(stream.id, because, browserTakesOver);
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

  /* A choice is remembered once it has been made, never on the way in: the
     tracks a page opens with are what the rules already decided, and writing
     them back would turn a habit into a decision nobody made. */
  const choose = useCallback(
    (audio: string | null, subtitle: string | null) => {
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

  /* A step back or on, wherever it was asked for: one way of moving the film
     means one place for it to be wrong and one place that says what moved it.
     Held inside the film at both ends, because a step past the end is the
     film over. */
  const stepBy = useCallback(
    (seconds: number) => {
      const element = video.current;
      if (!element) {
        return;
      }
      const furthest = element.duration;
      const wanted = element.currentTime + seconds;
      element.currentTime = Math.max(
        0,
        Number.isFinite(furthest) ? Math.min(furthest, wanted) : wanted,
      );
      // A step is one move with nothing in between, so there is nothing to
      // hold back first.
      viewerMoved("a_step");
    },
    [viewerMoved],
  );

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

  /* Remembered as it is chosen, not on the way in: what a player opens with
     is what the viewer left it on, and writing that back would be a decision
     nobody made. */
  const setQuality = useCallback((key: string) => {
    const chosen = qualityCalled(key);
    setQualityState(chosen);
    rememberQuality(chosen);
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
      element.currentTime = target;
    }
    setReadyPicture(pictureKey);
  }, [speed, pictureKey]);

  const notePosition = useCallback((seconds: number) => {
    lastPosition.current = seconds;
  }, []);

  const notePictureRefused = useCallback((refused: MediaError | null) => {
    setFailed("player.cannot_play");
    setRefusal(refused ? `${refused.code} · ${refused.message || "no reason given"}` : null);
  }, []);

  return {
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
    choose,
    setQuality,
    setSpeed,
    viewerMoving,
    viewerMoved,
    stepBy,
    onPictureReady,
    notePosition,
    report,
    notePictureRefused,
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

/**
 * Tells the journal that the library gave up on a film, in its own words.
 *
 * The one thing the page reports that is not a number, because the wording is
 * the answer: the library naming what it could not do with a film this server
 * produced. Without it, a film that plays the plain way looks like a film that
 * plays, and the reason it had to is nowhere.
 */
function sayItGaveUp(session: string, because: string, browserTookOver: boolean) {
  api
    .tellTheJournal({ session, saw: "playback_refused", because, browser_took_over: browserTookOver })
    .catch(() => {});
}

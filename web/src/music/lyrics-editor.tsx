/*
 * The lyrics of a song written again by hand, for an administrator, while
 * listening to it: words without moments given one line by line, and lines
 * that have them put back one by one where they are sung.
 *
 * Laid out from the song down: what plays it, its wave with a mark where
 * each line starts, then the lines, written one way of three (one by one,
 * tapping along, or as text). What is kept goes to the server for every
 * account, before any other lyrics the song has, and the originals stay
 * where they are, one press away.
 *
 * The song is heard on its own, apart from the player, which is paused: it
 * can be slowed down, and nothing is counted as listened to. Where it has got
 * to moves the playhead, the clock and which line is lit without drawing the
 * editor again; only a change of the line lit does.
 */

import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { refusalOf } from "../asking";
import { asPreciseClock } from "../clock";
import { Modal } from "../components/modal";
import { DeleteIcon, PlayIcon, RefreshIcon, ResetIcon } from "../icons";
import { useToast } from "../notifications/toasts";
import type { Wording } from "../readable";
import { useSettings } from "../settings";
import { music as server } from "./api";
import type { Song, SongLyrics } from "./api";
import { HeardSong } from "./heard-song";
import { Scrubber } from "./scrubbing";
import {
  NOTHING_EDITED,
  aLine,
  edited,
  endedAt,
  inSungOrder,
  linesOf,
  nextToStamp,
  outOfOrder,
  previousToStamp,
  sameLines,
  shifted,
  sungAt,
  toHundredth,
  toKeep,
  waitsForAMoment,
} from "./lyrics-editing";
import type { EditedLine } from "./lyrics-editing";
import { LyricsLines, NUDGE_MS } from "./lyrics-lines";
import type { LineActs } from "./lyrics-lines";
import { AsText, TapAlong } from "./lyrics-modes";
import { LyricsTimeline } from "./lyrics-timeline";
import type { TimelineActs, TimelineHandle } from "./lyrics-timeline";
import { useMusicMarks } from "./marks";
import { PauseIcon } from "./player/icons";
import { LIT_AHEAD_MS } from "./player/lyrics";
import { currentMusicClock, useMusic } from "./player/player";
import { useSongWave } from "./wave-view";

type Mode = "lines" | "tap" | "text";

/** How fast the song may be heard. */
const RATES = [0.5, 0.75, 1] as const;

/** How far the arrows and the buttons beside the clock move the song. */
const SKIP_SECONDS = 2;

/** How much is heard before a line listened to from where it starts. */
const LEAD_IN_SECONDS = 1.5;

/** How much before the line awaited tapping along starts again. */
const AGAIN_BEFORE_SECONDS = 3;

export function LyricsEditor({
  song,
  onClose,
  onChanged,
}: {
  song: Song;
  onClose: () => void;
  /** Told once the lyrics the song shows have changed. */
  onChanged?: () => void;
}) {
  const { t, language } = useSettings();
  const toast = useToast();
  const { lyricsHaveMoved } = useMusicMarks();
  const player = useMusic();
  const wave = useSongWave(song.id);
  const length =
    (song.duration_ms ?? 0) / 1000 || (wave && wave.momentsASecond > 0 ? wave.levels.length / wave.momentsASecond : 0);

  const [editing, change] = useReducer(edited, NOTHING_EDITED);
  const lines = editing.lines;
  const linesNow = useRef(lines);
  linesNow.current = lines;
  /** The lines as they were read or last kept, to tell what changed since. */
  const [kept, setKept] = useState<EditedLine[] | null>(null);
  const [byHand, setByHand] = useState(false);
  const [mode, setMode] = useState<Mode>("lines");
  const [selected, setSelected] = useState<number | null>(null);
  const selectedNow = useRef(selected);
  selectedNow.current = selected;
  const [sung, setSung] = useState(-1);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [asking, setAsking] = useState<"leave" | "revert" | null>(null);
  const [shifting, setShifting] = useState(false);

  /* What the song has is taken as the lines each time it is read: on the
     way in, and once the originals are brought back. */
  const take = useCallback((lyrics: SongLyrics | null) => {
    const found = linesOf(lyrics);
    change({ kind: "start", lines: found });
    setKept(found);
    setByHand(lyrics?.source === "by_hand");
    setSelected(found[0]?.key ?? null);
    setMode(found.length === 0 ? "text" : "lines");
    setLoading(false);
  }, []);
  useEffect(() => {
    const stop = new AbortController();
    server.lyrics(song.id, stop.signal).then(take, (error: unknown) => {
      if (!stop.signal.aborted) {
        setRefused(`lyrics_dialog.refused.${refusalOf(error)}`);
        take(null);
      }
    });
    return () => stop.abort();
  }, [song.id, take]);

  /* The song, heard on its own. */
  const [playing, setPlaying] = useState(false);
  const [rate, setRate] = useState<number>(1);
  const hearing = useMemo(() => {
    const heard: HeardSong = new HeardSong(song.id, () => setPlaying(heard.playing));
    return heard;
  }, [song.id]);
  const timeline = useRef<TimelineHandle>(null);
  const clock = useRef<HTMLSpanElement>(null);
  const shown = useCallback(
    (seconds: number, followed: boolean) => {
      timeline.current?.follow(seconds, followed);
      if (clock.current) {
        clock.current.textContent = asPreciseClock(seconds);
      }
      setSung(sungAt(linesNow.current, seconds * 1000 + LIT_AHEAD_MS));
    },
    [],
  );

  /* The player stops for the editor, and the song starts where the player
     had it when it is the one playing. */
  const opened = useRef({ player, song: song.id, done: false });
  useEffect(() => {
    const was = opened.current;
    if (!was.done) {
      was.done = true;
      if (was.player.song?.id === was.song) {
        hearing.seek(currentMusicClock().position);
      }
      if (was.player.playing) {
        was.player.toggle();
      }
    }
    shown(hearing.position(), false);
    return () => hearing.stop();
  }, [hearing, shown]);

  useEffect(() => {
    if (!playing) {
      shown(hearing.position(), false);
      return;
    }
    let frame = 0;
    const follow = () => {
      shown(hearing.position(), true);
      frame = requestAnimationFrame(follow);
    };
    frame = requestAnimationFrame(follow);
    return () => cancelAnimationFrame(frame);
  }, [playing, hearing, shown]);

  // The line lit follows the lines, moved or typed, while the song stands.
  useEffect(() => {
    setSung(sungAt(lines, hearing.position() * 1000 + LIT_AHEAD_MS));
  }, [lines, hearing]);

  const seek = useCallback(
    (seconds: number) => {
      hearing.seek(Math.min(Math.max(seconds, 0), length || Infinity));
      shown(hearing.position(), false);
    },
    [hearing, length, shown],
  );
  const toggle = useCallback(() => (hearing.playing ? hearing.pause() : hearing.play()), [hearing]);

  /* The song heard at the pace of the hand dragging the playhead, read into
     memory as the editor opens. A song that played when the playhead was
     taken plays on from where it is let go. */
  const scrubber = useRef<Scrubber | null>(null);
  useEffect(() => {
    const one = new Scrubber(song.id);
    one.read();
    scrubber.current = one;
    return () => {
      one.close();
      scrubber.current = null;
    };
  }, [song.id]);
  const playOn = useRef(false);
  const scrub = useCallback(
    (phase: "take" | "drag" | "let go", seconds: number) => {
      const at = Math.min(Math.max(seconds, 0), length || Infinity);
      if (phase === "take") {
        playOn.current = hearing.playing;
        if (hearing.playing) {
          hearing.pause();
        }
        scrubber.current?.begin(at);
        shown(at, false);
      } else if (phase === "drag") {
        scrubber.current?.to(at);
        shown(at, false);
      } else {
        seek(at);
        if (playOn.current) {
          hearing.play();
        }
      }
    },
    [hearing, length, seek, shown],
  );
  const changeRate = (next: number) => {
    setRate(next);
    hearing.setRate(next);
  };

  /* What is done to the lines, the same whichever part of the editor it is
     done from. */
  const edit = useCallback(
    (doing: (lines: EditedLine[]) => EditedLine[], merging?: string) => change({ kind: "edit", edit: doing, merging }),
    [],
  );
  const setAt = useCallback(
    (key: number, at: number | null, merging?: string) =>
      edit(
        (all) => all.map((line) => (line.key === key ? { ...line, at: at === null ? null : toHundredth(at) } : line)),
        merging,
      ),
    [edit],
  );
  const placeAndGoOn = useCallback(() => {
    const all = linesNow.current;
    const index = all.findIndex((line) => line.key === selectedNow.current);
    if (index < 0) {
      return;
    }
    setAt(all[index].key, hearing.position() * 1000);
    const next = nextToStamp(all, index);
    setSelected(next >= 0 ? all[next].key : null);
  }, [hearing, setAt]);
  const undo = useCallback(() => change({ kind: "undo" }), []);
  const redo = useCallback(() => change({ kind: "redo" }), []);
  const undoTap = useCallback(() => {
    const all = linesNow.current;
    const index = all.findIndex((line) => line.key === selectedNow.current);
    const back = previousToStamp(all, index < 0 ? all.length : index);
    undo();
    if (back >= 0) {
      setSelected(all[back].key);
    }
  }, [undo]);

  const lineActs = useMemo<LineActs>(
    () => ({
      select: setSelected,
      setText: (key, text) =>
        edit((all) => all.map((line) => (line.key === key ? { ...line, text } : line)), `text:${key}`),
      setAt: (key, at) => setAt(key, at),
      nudge: (key, by) =>
        edit(
          (all) =>
            all.map((line) => (line.key === key && line.at !== null ? { ...line, at: toHundredth(line.at + by) } : line)),
          `nudge:${key}`,
        ),
      stamp: (key) => {
        setSelected(key);
        setAt(key, hearing.position() * 1000);
      },
      playFrom: (key) => {
        const line = linesNow.current.find((one) => one.key === key);
        if (line?.at != null) {
          hearing.play(Math.max(0, line.at / 1000 - LEAD_IN_SECONDS));
        }
      },
      remove: (key) => edit((all) => all.filter((line) => line.key !== key)),
      settle: () => change({ kind: "settle" }),
    }),
    [edit, setAt, hearing],
  );
  const timelineActs = useMemo<TimelineActs>(
    () => ({
      select: setSelected,
      move: (key, at) => setAt(key, at, `move:${key}`),
      end: (key, at) => edit((all) => endedAt(all, key, at, length * 1000), `end:${key}`),
      settle: () => change({ kind: "settle" }),
      scrub,
    }),
    [setAt, edit, scrub, length],
  );

  const chosenIndex = lines.findIndex((line) => line.key === selected);
  /** A line after the one chosen, or at the end, with words or a break. */
  const addLine = (at: number | null) => {
    const line = aLine(at, "");
    edit((all) => {
      const after = all.findIndex((one) => one.key === selectedNow.current);
      return after < 0 ? [...all, line] : [...all.slice(0, after + 1), line, ...all.slice(after + 1)];
    });
    setSelected(line.key);
  };

  /* The keys, everywhere in the editor but where something is typed. Space
     and Enter are the editor's even on a button that was pressed, which
     otherwise keeps them for itself: only a button reached with the Tab key
     is pressed by them. */
  useEffect(() => {
    let tabbed = false;
    const pointed = () => {
      tabbed = false;
    };
    const keys = (event: KeyboardEvent) => {
      if (event.key === "Tab") {
        tabbed = true;
        return;
      }
      const target = event.target as HTMLElement | null;
      const typing = !!target?.closest("input, textarea, select, [contenteditable='true']");
      if (asking !== null || event.defaultPrevented || typing) {
        return;
      }
      const onAButton = tabbed && !!target?.closest("button");
      const command = event.ctrlKey || event.metaKey;
      if (command && event.key.toLowerCase() === "z") {
        event.preventDefault();
        (event.shiftKey ? redo : undo)();
        return;
      }
      if (command && event.key.toLowerCase() === "y") {
        event.preventDefault();
        redo();
        return;
      }
      if (command) {
        return;
      }
      const all = linesNow.current;
      const index = all.findIndex((line) => line.key === selectedNow.current);
      switch (event.key) {
        case " ":
          if (onAButton) {
            return;
          }
          toggle();
          break;
        case "Enter":
          if (onAButton) {
            return;
          }
          placeAndGoOn();
          break;
        case "Backspace":
          if (mode !== "tap") {
            return;
          }
          undoTap();
          break;
        case "ArrowLeft":
        case "ArrowRight": {
          const forward = event.key === "ArrowRight";
          if (event.altKey && index >= 0) {
            lineActs.nudge(all[index].key, forward ? NUDGE_MS : -NUDGE_MS);
          } else {
            seek(hearing.position() + (forward ? SKIP_SECONDS : -SKIP_SECONDS));
          }
          break;
        }
        case "ArrowUp":
        case "ArrowDown": {
          const next = index + (event.key === "ArrowDown" ? 1 : -1);
          if (next < 0 || next >= all.length) {
            return;
          }
          setSelected(all[next].key);
          break;
        }
        default:
          return;
      }
      event.preventDefault();
    };
    window.addEventListener("keydown", keys);
    window.addEventListener("pointerdown", pointed);
    return () => {
      window.removeEventListener("keydown", keys);
      window.removeEventListener("pointerdown", pointed);
    };
  }, [asking, mode, toggle, placeAndGoOn, undo, redo, undoTap, seek, hearing, lineActs]);

  const disorder = useMemo(() => outOfOrder(lines), [lines]);
  const stamped = lines.filter((line) => line.at !== null).length;
  const waiting = lines.filter(waitsForAMoment).length;
  const worded = lines.filter((line) => line.text.trim() !== "" || line.at !== null).length;
  const toBeKept = toKeep(lines);
  const changed = kept !== null && !sameLines(lines, kept);

  const save = async () => {
    if (!toBeKept) {
      return;
    }
    setBusy(true);
    setRefused(null);
    try {
      await server.writeLyrics(song.id, toBeKept);
      // Shown as they were kept, in the order they are sung.
      const sorted = inSungOrder(lines);
      if (!sameLines(sorted, lines)) {
        edit(() => sorted);
      }
      setKept(sorted);
      setByHand(true);
      lyricsHaveMoved();
      onChanged?.();
      toast({ state: "ok", title: t("lyrics_editor.saved", { title: song.title }) });
    } catch (error) {
      setRefused(`lyrics_dialog.refused.${refusalOf(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const revert = async () => {
    setAsking(null);
    setBusy(true);
    setRefused(null);
    try {
      await server.forgetLyricsByHand(song.id);
      lyricsHaveMoved();
      onChanged?.();
      take(await server.lyrics(song.id));
      toast({ state: "ok", title: t("lyrics_editor.reverted", { title: song.title }) });
    } catch (error) {
      setRefused(`lyrics_dialog.refused.${refusalOf(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const askToLeave = () => (changed ? setAsking("leave") : onClose());
  const status =
    lines.length === 0
      ? t("lyrics_editor.status_empty")
      : stamped === 0
        ? t("lyrics_editor.status_plain", { count: worded })
        : waiting > 0
          ? t("lyrics_editor.status_waiting", { count: waiting })
          : t("lyrics_editor.status_placed", { count: stamped });
  const rateSaid = (one: number) => `${one.toLocaleString(language)}×`;

  return (
    <Modal
      title={t("lyrics_editor.title", { title: song.title })}
      onClose={askToLeave}
      className="lyrics-editor"
      footer={
        <>
          <span className={`lyrics-editor-status${waiting > 0 && stamped > 0 ? " lyrics-editor-status-waiting" : ""}`}>
            {status}
            {disorder.size > 0 && (
              <span className="lyrics-editor-status-out">{t("lyrics_editor.status_out_of_turn", { count: disorder.size })}</span>
            )}
          </span>
          {byHand && (
            <button type="button" className="button button-quiet" disabled={busy} onClick={() => setAsking("revert")}>
              <RefreshIcon size={16} />
              {t("lyrics_editor.revert")}
            </button>
          )}
          <button
            type="button"
            className="button button-accent"
            disabled={busy || !changed || toBeKept === null}
            title={toBeKept === null && lines.length > 0 ? t("lyrics_editor.save_waiting") : undefined}
            onClick={() => void save()}
          >
            {t("lyrics_editor.save")}
          </button>
        </>
      }
    >
      <div className="lyrics-editor-transport">
        <button
          type="button"
          className="lyrics-editor-play"
          title={t(playing ? "music.pause" : "music.play")}
          aria-label={t(playing ? "music.pause" : "music.play")}
          onClick={toggle}
        >
          {playing ? <PauseIcon size={22} /> : <PlayIcon size={22} />}
        </button>
        <button
          type="button"
          className="button button-small lyrics-editor-skip"
          title={t("lyrics_editor.back", { seconds: SKIP_SECONDS })}
          onClick={() => seek(hearing.position() - SKIP_SECONDS)}
        >
          −{SKIP_SECONDS} s
        </button>
        <button
          type="button"
          className="button button-small lyrics-editor-skip"
          title={t("lyrics_editor.ahead", { seconds: SKIP_SECONDS })}
          onClick={() => seek(hearing.position() + SKIP_SECONDS)}
        >
          +{SKIP_SECONDS} s
        </button>
        <span className="lyrics-editor-clock">
          <span ref={clock}>{asPreciseClock(0)}</span>
          <span className="lyrics-editor-length"> / {asPreciseClock(length)}</span>
        </span>
        <span className="lyrics-editor-choice" role="group" aria-label={t("lyrics_editor.speed")}>
          {RATES.map((one) => (
            <button
              key={one}
              type="button"
              aria-pressed={rate === one}
              title={t("lyrics_editor.speed")}
              onClick={() => changeRate(one)}
            >
              {rateSaid(one)}
            </button>
          ))}
        </span>
        <div className="lyrics-editor-history">
          <button
            type="button"
            className="lyrics-line-button"
            disabled={editing.past.length === 0}
            title={t("lyrics_editor.undo")}
            aria-label={t("lyrics_editor.undo")}
            onClick={undo}
          >
            <ResetIcon size={17} />
          </button>
          <button
            type="button"
            className="lyrics-line-button lyrics-editor-redo"
            disabled={editing.future.length === 0}
            title={t("lyrics_editor.redo")}
            aria-label={t("lyrics_editor.redo")}
            onClick={redo}
          >
            <ResetIcon size={17} />
          </button>
        </div>
      </div>

      {length > 0 ? (
        <LyricsTimeline
          wave={wave}
          length={length}
          lines={lines}
          selected={selected}
          acts={timelineActs}
          handle={timeline}
          t={t}
        />
      ) : (
        <p className="lyrics-editor-note">{t("lyrics_editor.no_length")}</p>
      )}

      <div className="lyrics-editor-modes">
        <span className="lyrics-editor-choice lyrics-editor-tabs" role="tablist" aria-label={t("lyrics_editor.modes")}>
          {(["lines", "tap", "text"] as const).map((one) => (
            <button
              key={one}
              type="button"
              role="tab"
              aria-selected={mode === one}
              aria-pressed={mode === one}
              onClick={() => setMode(one)}
            >
              {t(`lyrics_editor.mode.${one}`)}
            </button>
          ))}
        </span>
        {mode === "lines" && (
          <span className="lyrics-editor-tools">
            <button type="button" className="button button-small" onClick={() => addLine(null)}>
              {t("lyrics_editor.add_line")}
            </button>
            <button
              type="button"
              className="button button-small"
              title={t("lyrics_editor.add_break_why")}
              onClick={() => addLine(toHundredth(hearing.position() * 1000))}
            >
              {t("lyrics_editor.add_break")}
            </button>
            <button
              type="button"
              className="button button-small"
              aria-expanded={shifting}
              disabled={stamped === 0}
              onClick={() => setShifting((was) => !was)}
            >
              {t("lyrics_editor.shift")}
            </button>
            <button
              type="button"
              className="button button-small"
              title={t("lyrics_editor.clear_times_why")}
              disabled={stamped === 0}
              onClick={() => edit((all) => all.map((line) => (line.at === null ? line : { ...line, at: null })))}
            >
              <DeleteIcon size={14} />
              {t("lyrics_editor.clear_times")}
            </button>
          </span>
        )}
      </div>

      {mode === "lines" && shifting && stamped > 0 && (
        <ShiftLines
          chosen={chosenIndex}
          shift={(from, by) => {
            edit((all) => shifted(all, from, by));
            setShifting(false);
          }}
          t={t}
        />
      )}

      {refused && <p className="notice">{t(refused)}</p>}

      {loading ? (
        <p className="lyrics-editor-note">{t("library.loading")}</p>
      ) : mode === "lines" ? (
        lines.length > 0 ? (
          <LyricsLines lines={lines} selected={selected} sung={sung} disorder={disorder} acts={lineActs} t={t} />
        ) : (
          <p className="lyrics-editor-note">{t("lyrics_editor.empty")}</p>
        )
      ) : mode === "tap" ? (
        <TapAlong
          lines={lines}
          awaited={chosenIndex}
          canUndo={editing.past.length > 0}
          place={placeAndGoOn}
          undo={undoTap}
          againBefore={() => {
            const line = lines[chosenIndex];
            const before = lines[previousToStamp(lines, chosenIndex)];
            const from = line?.at ?? before?.at ?? hearing.position() * 1000 + AGAIN_BEFORE_SECONDS * 1000;
            hearing.play(Math.max(0, from / 1000 - AGAIN_BEFORE_SECONDS));
          }}
          t={t}
        />
      ) : (
        <AsText lines={lines} apply={(read) => edit(() => read)} t={t} />
      )}

      {mode !== "text" && (
        <p className="lyrics-editor-keys">{t(mode === "tap" ? "lyrics_editor.keys_tap" : "lyrics_editor.keys")}</p>
      )}

      {asking === "leave" && (
        <Modal
          title={t("lyrics_editor.leave_title")}
          onClose={() => setAsking(null)}
          className="lyrics-editor-ask"
          footer={
            <>
              <button type="button" className="button button-quiet" onClick={() => setAsking(null)}>
                {t("lyrics_editor.leave_stay")}
              </button>
              <button type="button" className="button button-accent" onClick={onClose}>
                {t("lyrics_editor.leave_go")}
              </button>
            </>
          }
        >
          <p>{t("lyrics_editor.leave_why")}</p>
        </Modal>
      )}
      {asking === "revert" && (
        <Modal
          title={t("lyrics_editor.revert_title")}
          onClose={() => setAsking(null)}
          className="lyrics-editor-ask"
          footer={
            <>
              <button type="button" className="button button-quiet" onClick={() => setAsking(null)}>
                {t("lyrics_editor.leave_stay")}
              </button>
              <button type="button" className="button button-accent" onClick={() => void revert()}>
                {t("lyrics_editor.revert_go")}
              </button>
            </>
          }
        >
          <p>{t("lyrics_editor.revert_why")}</p>
        </Modal>
      )}
    </Modal>
  );
}

/** Every line, or the lines from the one chosen on, moved by a number of
 *  seconds typed. */
function ShiftLines({
  chosen,
  shift,
  t,
}: {
  /** The index of the line chosen, -1 for none. */
  chosen: number;
  shift: (from: number, by: number) => void;
  t: Wording;
}) {
  const [typed, setTyped] = useState("");
  const [from, setFrom] = useState<"all" | "chosen">("all");
  const seconds = Number(typed.trim().replace(",", "."));
  const by = typed.trim() !== "" && Number.isFinite(seconds) ? Math.round(seconds * 1000) : null;
  return (
    <form
      className="lyrics-shift"
      onSubmit={(event) => {
        event.preventDefault();
        if (by) {
          shift(from === "all" || chosen < 0 ? 0 : chosen, by);
        }
      }}
    >
      <label className="lyrics-shift-by">
        <span>{t("lyrics_editor.shift_by")}</span>
        <input
          value={typed}
          inputMode="decimal"
          placeholder="+0.5"
          autoFocus
          onChange={(event) => setTyped(event.target.value)}
        />
      </label>
      <span className="lyrics-editor-choice" role="group">
        <button type="button" aria-pressed={from === "all"} onClick={() => setFrom("all")}>
          {t("lyrics_editor.shift_all")}
        </button>
        <button type="button" aria-pressed={from === "chosen"} disabled={chosen < 0} onClick={() => setFrom("chosen")}>
          {t("lyrics_editor.shift_from", { number: chosen + 1 })}
        </button>
      </span>
      <button type="submit" className="button button-small button-accent" disabled={!by}>
        {t("lyrics_editor.shift_apply")}
      </button>
    </form>
  );
}

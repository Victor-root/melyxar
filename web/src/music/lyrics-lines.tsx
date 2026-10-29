/*
 * The lines of the lyrics editor one under the other, each with the moment
 * it starts, its words, and what moves it: a tenth of a second either way,
 * onto what is playing, or typed. The line being sung is lit as the player
 * would light it, and a line out of turn says so.
 *
 * Each line is drawn again only when it changes, or when it is the one
 * chosen or sung and stops or starts being so.
 */

import { memo, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { ChevronLeftIcon, ChevronRightIcon, ClockIcon, DeleteIcon, PlayIcon, WarningIcon } from "../icons";
import type { Wording } from "../readable";
import { readStamp, stampOf, waitsForAMoment } from "./lyrics-editing";
import type { EditedLine } from "./lyrics-editing";

/** A tenth of a second, which is as far as a press of the arrows moves a
 *  line. */
export const NUDGE_MS = 100;

export interface LineActs {
  select: (key: number) => void;
  setText: (key: number, text: string) => void;
  /** Nothing takes the line's moment away. */
  setAt: (key: number, at: number | null) => void;
  nudge: (key: number, by: number) => void;
  /** Puts the line on what is playing. */
  stamp: (key: number) => void;
  playFrom: (key: number) => void;
  remove: (key: number) => void;
  /** Ends a step of typing. */
  settle: () => void;
}

export const LyricsLines = memo(function LyricsLines({
  lines,
  selected,
  sung,
  disorder,
  acts,
  t,
}: {
  lines: EditedLine[];
  selected: number | null;
  /** The index of the line being sung, -1 for none. */
  sung: number;
  /** The keys of the lines out of turn. */
  disorder: Set<number>;
  acts: LineActs;
  t: Wording;
}) {
  const list = useRef<HTMLOListElement>(null);

  /* The line chosen, then the one being sung while nobody is writing in the
     list, are kept in sight: only the list moves, never the page. */
  const chosenIndex = lines.findIndex((line) => line.key === selected);
  useEffect(() => bringIntoSight(list.current, chosenIndex), [chosenIndex]);
  useEffect(() => {
    if (!list.current?.contains(document.activeElement)) {
      bringIntoSight(list.current, sung);
    }
  }, [sung]);

  return (
    <ol className="lyrics-lines" ref={list}>
      {lines.map((line, index) => (
        <LineRow
          key={line.key}
          line={line}
          number={index + 1}
          chosen={line.key === selected}
          sung={index === sung}
          outOfTurn={disorder.has(line.key)}
          acts={acts}
          t={t}
        />
      ))}
    </ol>
  );
});

function bringIntoSight(list: HTMLOListElement | null, index: number) {
  const row = list?.children[index] as HTMLElement | undefined;
  if (!list || !row) {
    return;
  }
  const top = row.offsetTop - list.offsetTop;
  if (top < list.scrollTop || top + row.offsetHeight > list.scrollTop + list.clientHeight) {
    list.scrollTo({ top: top - (list.clientHeight - row.offsetHeight) / 2, behavior: "smooth" });
  }
}

const LineRow = memo(function LineRow({
  line,
  number,
  chosen,
  sung,
  outOfTurn,
  acts,
  t,
}: {
  line: EditedLine;
  number: number;
  chosen: boolean;
  sung: boolean;
  outOfTurn: boolean;
  acts: LineActs;
  t: Wording;
}) {
  const waiting = waitsForAMoment(line);
  const select = () => acts.select(line.key);
  return (
    <li
      className={`lyrics-line${chosen ? " lyrics-line-chosen" : ""}${sung ? " lyrics-line-sung" : ""}`}
      onPointerDown={select}
    >
      <span className="lyrics-line-number">{number}</span>
      <span className="lyrics-line-moment">
        <button
          type="button"
          className="lyrics-line-button"
          disabled={line.at === null}
          title={t("lyrics_editor.earlier")}
          aria-label={t("lyrics_editor.earlier")}
          onClick={() => acts.nudge(line.key, -NUDGE_MS)}
        >
          <ChevronLeftIcon size={15} />
        </button>
        <MomentField line={line} number={number} waiting={waiting} outOfTurn={outOfTurn} acts={acts} t={t} />
        <button
          type="button"
          className="lyrics-line-button"
          disabled={line.at === null}
          title={t("lyrics_editor.later")}
          aria-label={t("lyrics_editor.later")}
          onClick={() => acts.nudge(line.key, NUDGE_MS)}
        >
          <ChevronRightIcon size={15} />
        </button>
      </span>
      <button
        type="button"
        className="lyrics-line-button lyrics-line-stamp"
        title={t("lyrics_editor.stamp")}
        aria-label={t("lyrics_editor.stamp")}
        onClick={() => acts.stamp(line.key)}
      >
        <ClockIcon size={15} />
      </button>
      <button
        type="button"
        className="lyrics-line-button"
        disabled={line.at === null}
        title={t("lyrics_editor.play_from")}
        aria-label={t("lyrics_editor.play_from")}
        onClick={() => acts.playFrom(line.key)}
      >
        <PlayIcon size={14} />
      </button>
      <input
        className="lyrics-line-words"
        value={line.text}
        placeholder={t("lyrics_editor.break")}
        aria-label={t("lyrics_editor.words", { number })}
        onFocus={select}
        onBlur={acts.settle}
        onChange={(event) => acts.setText(line.key, event.target.value)}
      />
      {outOfTurn && (
        <span className="lyrics-line-warning" title={t("lyrics_editor.out_of_turn")}>
          <WarningIcon size={15} />
        </span>
      )}
      <button
        type="button"
        className="lyrics-line-button lyrics-line-remove"
        title={t("lyrics_editor.remove")}
        aria-label={t("lyrics_editor.remove")}
        onClick={() => acts.remove(line.key)}
      >
        <DeleteIcon size={15} />
      </button>
    </li>
  );
});

/** The moment of a line, typed: read when the field is left or on Enter,
 *  put back as it was when it reads as nothing, taken away when emptied. */
function MomentField({
  line,
  number,
  waiting,
  outOfTurn,
  acts,
  t,
}: {
  line: EditedLine;
  number: number;
  waiting: boolean;
  outOfTurn: boolean;
  acts: LineActs;
  t: Wording;
}) {
  const written = line.at === null ? "" : stampOf(line.at);
  const [typed, setTyped] = useState<string | null>(null);
  const done = () => {
    if (typed === null) {
      return;
    }
    setTyped(null);
    if (typed.trim() === "") {
      acts.setAt(line.key, null);
      return;
    }
    const at = readStamp(typed);
    if (at !== null && at !== line.at) {
      acts.setAt(line.key, at);
    }
  };
  const keys = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      event.currentTarget.blur();
    } else if (event.key === "Escape" && typed !== null) {
      event.stopPropagation();
      setTyped(null);
    }
  };
  return (
    <input
      className={`lyrics-line-time${waiting ? " lyrics-line-time-waiting" : ""}${outOfTurn ? " lyrics-line-time-out" : ""}`}
      value={typed ?? written}
      placeholder="--:--.--"
      inputMode="decimal"
      aria-label={t("lyrics_editor.moment", { number })}
      onFocus={() => acts.select(line.key)}
      onChange={(event) => setTyped(event.target.value)}
      onBlur={done}
      onKeyDown={keys}
    />
  );
}

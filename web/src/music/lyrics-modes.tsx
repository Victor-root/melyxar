/*
 * The two other ways the lyrics editor writes lines than one by one: tapping
 * along with the song, each press placing the line awaited on what is
 * playing, and the whole of the words as text, to paste lyrics in or tidy
 * them up at a stroke.
 */

import { useEffect, useRef, useState } from "react";
import { ClockIcon, ResetIcon } from "../icons";
import type { Wording } from "../readable";
import { asText, fromText, nextToStamp, previousToStamp, stampOf } from "./lyrics-editing";
import type { EditedLine } from "./lyrics-editing";

/** The line awaited, large, the one placed before it and the two after,
 *  the gaps between verses passed over as they are when tapping. */
export function TapAlong({
  lines,
  awaited,
  canUndo,
  place,
  undo,
  againBefore,
  t,
}: {
  lines: EditedLine[];
  /** The index of the line the next press places, -1 once all are. */
  awaited: number;
  canUndo: boolean;
  place: () => void;
  undo: () => void;
  /** Plays again from a little before the line awaited. */
  againBefore: () => void;
  t: Wording;
}) {
  const box = useRef<HTMLDivElement>(null);
  /* The line awaited stays in the middle of what there is room for. */
  useEffect(() => {
    const element = box.current;
    const shown = element?.querySelector<HTMLElement>(".lyrics-tap-awaited");
    if (element && shown) {
      element.scrollTop = shown.offsetTop - element.offsetTop - (element.clientHeight - shown.offsetHeight) / 2;
    }
  }, [awaited]);
  const line = lines[awaited];
  const before = line ? [lines[previousToStamp(lines, awaited)]].filter(Boolean) : [];
  const after: EditedLine[] = [];
  for (let next = nextToStamp(lines, awaited); line && next >= 0 && after.length < 2; next = nextToStamp(lines, next)) {
    after.push(lines[next]);
  }
  return (
    <section className="lyrics-tap" aria-label={t("lyrics_editor.mode.tap")}>
      <p className="lyrics-tap-why">{t("lyrics_editor.tap_why")}</p>
      {line ? (
        <div className="lyrics-tap-lines" ref={box}>
          {before.map((one) => (
            <TapLine key={one.key} line={one} kind="before" t={t} />
          ))}
          <span className="lyrics-tap-count">
            {t("lyrics_editor.tap_count", { number: awaited + 1, count: lines.length })}
          </span>
          <TapLine line={line} kind="awaited" t={t} />
          {after.map((one) => (
            <TapLine key={one.key} line={one} kind="after" t={t} />
          ))}
        </div>
      ) : (
        <p className="lyrics-tap-done">{t("lyrics_editor.tap_done")}</p>
      )}
      <div className="lyrics-tap-actions">
        <button type="button" className="button" disabled={!canUndo} onClick={undo}>
          <ResetIcon size={16} />
          {t("lyrics_editor.undo")}
        </button>
        <button type="button" className="button button-accent button-large lyrics-tap-place" disabled={!line} onClick={place}>
          <ClockIcon size={18} />
          {t("lyrics_editor.tap_place")}
        </button>
        <button type="button" className="button" disabled={!line} onClick={againBefore}>
          {t("lyrics_editor.tap_again")}
        </button>
      </div>
    </section>
  );
}

function TapLine({ line, kind, t }: { line: EditedLine; kind: "before" | "awaited" | "after"; t: Wording }) {
  return (
    <span className={`lyrics-tap-line lyrics-tap-${kind}`}>
      <span className="lyrics-tap-moment">{line.at === null ? "--:--.--" : stampOf(line.at)}</span>
      <span className="lyrics-tap-words">{line.text || t("lyrics_editor.break")}</span>
    </span>
  );
}

/** The words as text, a moment in front of each line that has one. What is
 *  typed is taken as the lines once the field is left, the text closed
 *  included. */
export function AsText({
  lines,
  apply,
  t,
}: {
  lines: EditedLine[];
  apply: (lines: EditedLine[]) => void;
  t: Wording;
}) {
  const [typed, setTyped] = useState(() => asText(lines));
  /** What the lines say as text, to tell what was typed since. */
  const taken = useRef(typed);
  const now = useRef({ typed, apply });
  now.current = { typed, apply };
  const take = () => {
    if (now.current.typed !== taken.current) {
      taken.current = now.current.typed;
      now.current.apply(fromText(now.current.typed));
    }
  };
  useEffect(() => take, []);
  return (
    <section className="lyrics-text" aria-label={t("lyrics_editor.mode.text")}>
      <p className="lyrics-text-why">{t("lyrics_editor.text_why")}</p>
      <textarea
        className="lyrics-text-field"
        value={typed}
        spellCheck={false}
        aria-label={t("lyrics_editor.mode.text")}
        placeholder={t("lyrics_editor.text_placeholder")}
        onChange={(event) => setTyped(event.target.value)}
        onBlur={take}
      />
    </section>
  );
}

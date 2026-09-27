/*
 * Where the opening and closing titles of this file really are, said by an
 * administrator while it plays.
 *
 * Found by watching: pause where the titles start, take that moment, play on
 * to where they end, take that one. So it stands on the picture as a window
 * that stays open while the film is moved about, and every moment in it can
 * be taken off the screen rather than typed.
 */

import { useState } from "react";

import { api } from "../api";
import type { PlaybackSegment, SegmentCorrection, Stretches } from "../api";
import { asClock, fromClock } from "./clock";
import { PlayerWindow } from "./window";

/** The stretches a person corrects here. Advertisements are only ever read
 *  off a recording's chapters, and nobody asked to move them. */
const KINDS = ["recap", "intro", "outro"] as const;

type Say = (key: string, values?: Record<string, string | number>) => string;

interface Props {
  source: string;
  stretches: Stretches;
  /** Where the film is now, which a "here" button takes. */
  at: number;
  t: Say;
  onChanged: (stretches: Stretches) => void;
  onClose: () => void;
}

export function SegmentsWindow({ source, stretches, at, t, onChanged, onClose }: Props) {
  return (
    <PlayerWindow
      title={t("segments.title")}
      closeLabel={t("segments.close")}
      className="segments"
      onClose={onClose}
    >
      <div className="segments-rows">
        {KINDS.map((kind) => {
          const current = stretches.segments.find((segment) => segment.kind === kind) ?? null;
          return (
            <SegmentRow
              // Drawn afresh from what the server answers, so a field left
              // half typed never outlives the stretch it was typed against.
              key={`${kind}:${current?.from_second}:${current?.to_second}`}
              source={source}
              kind={kind}
              current={current}
              corrected={stretches.corrected_segments.includes(kind)}
              at={at}
              t={t}
              onChanged={onChanged}
            />
          );
        })}
      </div>
    </PlayerWindow>
  );
}

function SegmentRow({
  source,
  kind,
  current,
  corrected,
  at,
  t,
  onChanged,
}: {
  source: string;
  kind: string;
  current: PlaybackSegment | null;
  corrected: boolean;
  at: number;
  t: Say;
  onChanged: (stretches: Stretches) => void;
}) {
  const [from, setFrom] = useState(current ? asClock(current.from_second) : "");
  const [to, setTo] = useState(current ? asClock(current.to_second) : "");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState(false);

  const start = fromClock(from);
  const end = fromClock(to);
  const readable = start !== null && end !== null && start < end;

  const settle = (asking: Promise<Stretches>) => {
    setBusy(true);
    setRefused(false);
    asking
      .then(onChanged)
      .catch(() => setRefused(true))
      .finally(() => setBusy(false));
  };
  const say = (correction: SegmentCorrection) =>
    settle(api.correctSegment(source, kind, correction));

  const origin = current
    ? t(`segments.origin.${current.origin}`)
    : t(corrected ? "segments.origin.none_said" : "segments.origin.nothing");

  return (
    <section className="segments-row">
      <div className="segments-row-head">
        <h3>{t(`segments.kind.${kind}`)}</h3>
        <span className={current?.origin === "manual" || corrected ? "segments-origin segments-origin-manual" : "segments-origin"}>
          {origin}
        </span>
      </div>

      <div className="segments-times">
        <Moment label={t("segments.from")} value={from} onChange={setFrom} onHere={() => setFrom(asClock(at))} t={t} />
        <Moment label={t("segments.to")} value={to} onChange={setTo} onHere={() => setTo(asClock(at))} t={t} />
      </div>

      <div className="segments-actions">
        <button
          type="button"
          className="button button-small button-accent"
          disabled={busy || !readable}
          onClick={() =>
            start !== null &&
            end !== null &&
            say({ said: "stretch", from_second: start, to_second: end })
          }
        >
          {t("segments.save")}
        </button>
        <button
          type="button"
          className="button button-small"
          disabled={busy}
          onClick={() => say({ said: "none" })}
        >
          {t("segments.none")}
        </button>
        {corrected && (
          <button
            type="button"
            className="button button-small button-quiet"
            disabled={busy}
            title={t("segments.automatic_why")}
            onClick={() => settle(api.takeBackSegment(source, kind))}
          >
            {t("segments.automatic")}
          </button>
        )}
      </div>

      {refused && <p className="segments-refused">{t("segments.refused")}</p>}
    </section>
  );
}

/** One moment of a stretch: typed, or taken off the screen. */
function Moment({
  label,
  value,
  onChange,
  onHere,
  t,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onHere: () => void;
  t: Say;
}) {
  const unreadable = value.trim() !== "" && fromClock(value) === null;
  return (
    <label className="segments-moment">
      <span>{label}</span>
      <input
        className="field-line segments-field"
        value={value}
        placeholder="0:00"
        inputMode="numeric"
        aria-invalid={unreadable}
        onChange={(event) => onChange(event.target.value)}
      />
      <button type="button" className="button button-small" title={t("segments.here_why")} onClick={onHere}>
        {t("segments.here")}
      </button>
    </label>
  );
}

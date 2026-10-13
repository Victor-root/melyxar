/*
 * The paintings an account without a picture wears.
 *
 * Eight of them, each a piece of the night sky or of the things a media
 * server holds, drawn rather than fetched so they cost no request and stay
 * sharp at any size, from the corner of the header to a tile on the door.
 * Not one colour is written here: every stop is one of the four tints the
 * stylesheet works out from the accent and the account's turn round the
 * wheel, so the paintings follow whatever accent somebody chose.
 *
 * Gradients and shapes only, and nothing blurred or filtered: a filter is
 * worked out again whenever the face moves, and these move under the pointer.
 */

import { useId } from "react";
import type { FaceDrawing } from "../faces";

/** The tints, as the stylesheet names them. */
const NIGHT = "var(--art-night)";
const DEEP = "var(--art-deep)";
const BASE = "var(--art-base)";
const LIGHT = "var(--art-light)";
const GLOW = "var(--art-glow)";

type Stop = [offset: number, colour: string, opacity?: number];

function stops(list: Stop[]) {
  return list.map(([offset, colour, opacity = 1], at) => (
    <stop key={at} offset={offset} style={{ stopColor: colour, stopOpacity: opacity }} />
  ));
}

/** A few points of light, the same handful wherever a painting has a sky. */
function Stars({ at }: { at: [number, number, number][] }) {
  return (
    <g style={{ fill: LIGHT }}>
      {at.map(([x, y, size], index) => (
        <circle key={index} cx={x} cy={y} r={size} opacity={0.35 + size * 0.5} />
      ))}
    </g>
  );
}

const SKY: [number, number, number][] = [
  [14, 18, 0.7],
  [31, 9, 0.45],
  [46, 24, 0.5],
  [9, 41, 0.4],
  [62, 12, 0.6],
  [24, 31, 0.35],
  [83, 21, 0.4],
];

export function FaceArt({ drawing }: { drawing: FaceDrawing }) {
  // One set of gradients per face: two faces sharing an identifier would both
  // be painted in the tints of whichever was drawn first.
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const ref = (name: string) => `url(#${id}${name})`;
  const def = (name: string) => `${id}${name}`;

  return (
    <svg viewBox="0 0 100 100" preserveAspectRatio="xMidYMid slice" focusable="false">
      <defs>
        {/* The ground every painting starts from: the night, a little lighter
            where the light comes from. */}
        <radialGradient id={def("ground")} cx="0.3" cy="0.2" r="1">
          {stops([
            [0, DEEP, 0.55],
            [0.55, NIGHT],
          ])}
        </radialGradient>
        {/* The edges darkened, which is what gives each one its depth. */}
        <radialGradient id={def("vignette")} cx="0.5" cy="0.5" r="0.75">
          {stops([
            [0.6, NIGHT, 0],
            [1, NIGHT, 0.7],
          ])}
        </radialGradient>
        {DRAWINGS[drawing].defs(def)}
      </defs>
      <rect width="100" height="100" style={{ fill: NIGHT }} />
      <rect width="100" height="100" fill={ref("ground")} />
      {DRAWINGS[drawing].ink(ref)}
      <rect width="100" height="100" fill={ref("vignette")} />
    </svg>
  );
}

type Name = (name: string) => string;

const DRAWINGS: Record<FaceDrawing, { defs: (def: Name) => React.ReactNode; ink: (ref: Name) => React.ReactNode }> = {
  /* A planet seen from its night side: a thin crescent of daylight along one
     edge, the air above it lit, the rest of it lost in the dark. */
  planet: {
    defs: (def) => (
      <>
        <radialGradient id={def("body")} cx="0.7" cy="0.72" r="0.74">
          {stops([
            [0, NIGHT],
            [0.74, NIGHT],
            [0.88, DEEP],
            [0.96, BASE],
            [1, LIGHT],
          ])}
        </radialGradient>
        <radialGradient id={def("air")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.84, GLOW, 0.5],
            [0.9, GLOW, 0.18],
            [1, GLOW, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("dawn")} cx="0.18" cy="0.2" r="0.6">
          {stops([
            [0, GLOW, 0.4],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => (
      <>
        <rect width="100" height="100" fill={ref("dawn")} />
        <Stars at={SKY.slice(0, 5)} />
        <circle cx="76" cy="80" r="74" fill={ref("air")} />
        <circle cx="76" cy="80" r="62" fill={ref("body")} />
      </>
    ),
  },

  /* A disc held against a light, which shows only as the ring around it and
     the one bright point where it breaks through. */
  eclipse: {
    defs: (def) => (
      <>
        <radialGradient id={def("corona")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.5, GLOW, 0.9],
            [0.62, BASE, 0.45],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <linearGradient id={def("rim")} x1="0" y1="1" x2="1" y2="0">
          {stops([
            [0, LIGHT, 0.15],
            [0.7, LIGHT, 0.9],
            [1, LIGHT, 1],
          ])}
        </linearGradient>
        <radialGradient id={def("spark")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, LIGHT],
            [0.25, LIGHT, 0.7],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => (
      <>
        <Stars at={SKY.slice(2)} />
        <circle cx="50" cy="52" r="46" fill={ref("corona")} />
        <circle cx="50" cy="52" r="23" style={{ fill: NIGHT }} />
        <circle cx="50" cy="52" r="23" fill="none" stroke={ref("rim")} strokeWidth="0.9" />
        <circle cx="65.5" cy="35.5" r="9" fill={ref("spark")} />
      </>
    ),
  },

  /* A ringed world, the rings passing behind it and in front. */
  rings: {
    defs: (def) => (
      <>
        <radialGradient id={def("body")} cx="0.32" cy="0.28" r="0.85">
          {stops([
            [0, LIGHT],
            [0.25, BASE],
            [0.6, DEEP],
            [1, NIGHT],
          ])}
        </radialGradient>
        <linearGradient id={def("ring")} x1="0" y1="0" x2="1" y2="0">
          {stops([
            [0, BASE, 0.1],
            [0.3, LIGHT, 0.85],
            [0.7, BASE, 0.6],
            [1, BASE, 0.1],
          ])}
        </linearGradient>
        <clipPath id={def("front")} clipPathUnits="userSpaceOnUse">
          <rect x="0" y="54" width="100" height="46" />
        </clipPath>
        <radialGradient id={def("halo")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.4, GLOW, 0.35],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => {
      const band = (
        <>
          <ellipse cx="50" cy="54" rx="43" ry="10.5" fill="none" stroke={ref("ring")} strokeWidth="3.2" />
          <ellipse cx="50" cy="54" rx="35" ry="8" fill="none" stroke={ref("ring")} strokeWidth="1" opacity="0.6" />
        </>
      );
      return (
        <>
          <Stars at={SKY} />
          <circle cx="50" cy="54" r="38" fill={ref("halo")} />
          <g transform="rotate(-16 50 54)">{band}</g>
          <circle cx="50" cy="54" r="21" fill={ref("body")} />
          <g transform="rotate(-16 50 54)">
            <g clipPath={ref("front")}>{band}</g>
          </g>
        </>
      );
    },
  },

  /* Light falling through the dark in long folds, like a curtain of silk or
     the aurora: three ribbons, each brightest where it turns. */
  silk: {
    defs: (def) => (
      <>
        <linearGradient id={def("one")} x1="0" y1="0" x2="1" y2="0.3">
          {stops([
            [0, BASE, 0],
            [0.45, LIGHT, 0.9],
            [1, BASE, 0],
          ])}
        </linearGradient>
        <linearGradient id={def("two")} x1="0" y1="0.4" x2="1" y2="0">
          {stops([
            [0, GLOW, 0],
            [0.6, GLOW, 0.7],
            [1, DEEP, 0],
          ])}
        </linearGradient>
        <linearGradient id={def("three")} x1="0" y1="0" x2="1" y2="0">
          {stops([
            [0, DEEP, 0],
            [0.35, BASE, 0.55],
            [1, DEEP, 0],
          ])}
        </linearGradient>
      </>
    ),
    ink: (ref) => (
      <>
        <path d="M-5 78 C 20 50, 48 96, 105 46 L105 60 C 50 104, 22 62, -5 92 Z" fill={ref("three")} />
        <path d="M-5 62 C 26 30, 52 82, 105 28 L105 36 C 54 88, 26 44, -5 70 Z" fill={ref("two")} />
        <path d="M-5 50 C 30 22, 50 66, 105 16 L105 20 C 52 70, 30 30, -5 54 Z" fill={ref("one")} />
        <Stars at={[[18, 14, 0.5], [70, 74, 0.4], [86, 86, 0.6], [40, 8, 0.35]]} />
      </>
    ),
  },

  /* Daybreak over the curve of a world: the horizon lit along its edge and
     the light rising off it into the sky. */
  horizon: {
    defs: (def) => (
      <>
        <radialGradient id={def("sun")} cx="0.5" cy="0.66" r="0.62">
          {stops([
            [0, LIGHT, 0.95],
            [0.18, GLOW, 0.7],
            [0.55, BASE, 0.2],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <linearGradient id={def("land")} x1="0" y1="0" x2="0" y2="1">
          {stops([
            [0, DEEP, 0.85],
            [0.12, NIGHT],
          ])}
        </linearGradient>
        <linearGradient id={def("edge")} x1="0" y1="0" x2="1" y2="0">
          {stops([
            [0, LIGHT, 0],
            [0.5, LIGHT, 1],
            [1, LIGHT, 0],
          ])}
        </linearGradient>
      </>
    ),
    ink: (ref) => (
      <>
        <Stars at={SKY} />
        <rect width="100" height="100" fill={ref("sun")} />
        <circle cx="50" cy="168" r="102" fill={ref("land")} />
        <circle cx="50" cy="168" r="102" fill="none" stroke={ref("edge")} strokeWidth="1.1" />
      </>
    ),
  },

  /* A record, turning: the grooves catching the light in two opposite
     wedges, the label in the account's own colour. */
  record: {
    defs: (def) => (
      <>
        <radialGradient id={def("disc")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, DEEP],
            [0.4, NIGHT],
            [1, NIGHT],
          ])}
        </radialGradient>
        <linearGradient id={def("sheen")} x1="0" y1="0" x2="1" y2="1">
          {stops([
            [0, LIGHT, 0],
            [0.5, LIGHT, 0.5],
            [1, LIGHT, 0],
          ])}
        </linearGradient>
        <radialGradient id={def("label")} cx="0.35" cy="0.3" r="0.8">
          {stops([
            [0, LIGHT],
            [0.4, BASE],
            [1, DEEP],
          ])}
        </radialGradient>
        <radialGradient id={def("halo")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.7, GLOW, 0.4],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => (
      <>
        <circle cx="50" cy="50" r="49" fill={ref("halo")} />
        <circle cx="50" cy="50" r="39" fill={ref("disc")} />
        <g fill="none" style={{ stroke: LIGHT }} strokeWidth="0.35" opacity="0.14">
          {[36, 33, 30, 27.5, 25, 22.5, 20].map((r) => (
            <circle key={r} cx="50" cy="50" r={r} />
          ))}
        </g>
        <path d="M50 50 L22 22 A39 39 0 0 1 36 13.6 Z M50 50 L78 78 A39 39 0 0 1 64 86.4 Z" fill={ref("sheen")} opacity="0.55" />
        <circle cx="50" cy="50" r="39" fill="none" style={{ stroke: BASE }} strokeWidth="0.6" opacity="0.6" />
        <circle cx="50" cy="50" r="12" fill={ref("label")} />
        <circle cx="50" cy="50" r="1.6" style={{ fill: NIGHT }} />
      </>
    ),
  },

  /* Clouds of coloured gas around one bright star. */
  nebula: {
    defs: (def) => (
      <>
        <radialGradient id={def("one")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, BASE, 0.85],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("two")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, GLOW, 0.7],
            [1, GLOW, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("three")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, DEEP, 0.9],
            [1, DEEP, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("star")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, LIGHT],
            [0.2, LIGHT, 0.8],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => (
      <>
        <ellipse cx="30" cy="68" rx="46" ry="34" fill={ref("three")} />
        <ellipse cx="66" cy="40" rx="40" ry="30" fill={ref("one")} transform="rotate(-25 66 40)" />
        <ellipse cx="44" cy="54" rx="26" ry="18" fill={ref("two")} transform="rotate(20 44 54)" />
        <Stars at={[...SKY, [78, 70, 0.5], [20, 86, 0.4], [90, 52, 0.35]]} />
        <circle cx="57" cy="47" r="11" fill={ref("star")} />
        <g style={{ fill: LIGHT }}>
          <ellipse cx="57" cy="47" rx="16" ry="0.45" />
          <ellipse cx="57" cy="47" rx="0.45" ry="12" />
        </g>
      </>
    ),
  },

  /* A small sun and the paths of what turns around it, one of them caught
     part way round. */
  orbits: {
    defs: (def) => (
      <>
        <radialGradient id={def("sun")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, LIGHT],
            [0.3, GLOW, 0.9],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <linearGradient id={def("path")} x1="0" y1="0" x2="1" y2="1">
          {stops([
            [0, LIGHT, 0.65],
            [1, BASE, 0.1],
          ])}
        </linearGradient>
        <radialGradient id={def("moon")} cx="0.35" cy="0.3" r="0.8">
          {stops([
            [0, LIGHT],
            [0.5, BASE],
            [1, DEEP],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => (
      <>
        <Stars at={SKY.slice(1)} />
        <g fill="none" stroke={ref("path")}>
          <ellipse cx="50" cy="52" rx="20" ry="12" strokeWidth="0.7" transform="rotate(-20 50 52)" />
          <ellipse cx="50" cy="52" rx="33" ry="20" strokeWidth="0.6" transform="rotate(-20 50 52)" />
          <ellipse cx="50" cy="52" rx="46" ry="28" strokeWidth="0.5" transform="rotate(-20 50 52)" opacity="0.7" />
        </g>
        <circle cx="50" cy="52" r="18" fill={ref("sun")} />
        <circle cx="50" cy="52" r="4.5" style={{ fill: LIGHT }} />
        <circle cx="79.5" cy="38.5" r="4" fill={ref("moon")} />
        <circle cx="25" cy="71" r="2" fill={ref("moon")} />
      </>
    ),
  },
};

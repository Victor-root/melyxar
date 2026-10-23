/*
 * The paintings an account without a picture wears.
 *
 * Eight of them, each a piece of the night sky or of the things a media
 * server holds, drawn rather than fetched so they cost no request and stay
 * sharp at any size, from the corner of the header to a tile on the door.
 * Not one colour is written here: every stop is one of the tints the
 * stylesheet works out from the accent and the account's turn round the
 * wheel, so the paintings follow whatever accent somebody chose.
 *
 * Gradients and shapes only, and nothing blurred or filtered: a filter is
 * worked out again whenever the face moves, and these move under the pointer.
 * What makes them rich is layering instead: light laid over light, fine lines
 * and a sky of many stars.
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

type Point = [x: number, y: number];
type Star = [x: number, y: number, size: number];

/**
 * Numbers that look scattered and are the same every time: a face must not
 * change between two drawings of it.
 */
function scattered(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

/** A sky of many stars, most of them faint and a few bright, as a real one. */
function skyOf(seed: number, count: number): Star[] {
  const next = scattered(seed);
  return Array.from({ length: count }, () => {
    const x = next() * 100;
    const y = next() * 100;
    const bright = next();
    return [x, y, 0.12 + bright * bright * bright * 0.85];
  });
}

const SKY = skyOf(7, 64);
const FAR_SKY = skyOf(29, 90);

/** A point on an ellipse turned by `tilt` degrees about its centre. */
function onEllipse(cx: number, cy: number, rx: number, ry: number, tilt: number, angle: number): Point {
  const x = rx * Math.cos(angle);
  const y = ry * Math.sin(angle);
  const turn = (tilt * Math.PI) / 180;
  return [cx + x * Math.cos(turn) - y * Math.sin(turn), cy + x * Math.sin(turn) + y * Math.cos(turn)];
}

/** Stars, the brightest wearing a soft halo and a glint across them. */
function Stars({ at }: { at: Star[] }) {
  return (
    <g>
      <g style={{ fill: LIGHT }}>
        {at.map(([x, y, size], index) => (
          <circle key={index} cx={x} cy={y} r={size * 0.55} opacity={0.3 + size * 0.7} />
        ))}
      </g>
      <g style={{ fill: GLOW }}>
        {at
          .filter(([, , size]) => size > 0.62)
          .map(([x, y, size], index) => (
            <circle key={index} cx={x} cy={y} r={size * 1.9} opacity={0.16} />
          ))}
      </g>
      <g style={{ fill: LIGHT }}>
        {at
          .filter(([, , size]) => size > 0.78)
          .map(([x, y, size], index) => (
            <g key={index} opacity={0.75}>
              <ellipse cx={x} cy={y} rx={size * 3.2} ry={0.1} />
              <ellipse cx={x} cy={y} rx={0.1} ry={size * 3.2} />
            </g>
          ))}
      </g>
    </g>
  );
}

/** A bright point of light with long spikes, as a lens draws a star. */
function Flare({ x, y, size, fill }: { x: number; y: number; size: number; fill: string }) {
  return (
    <g>
      <circle cx={x} cy={y} r={size * 2.6} fill={fill} />
      <g style={{ fill: LIGHT }}>
        <ellipse cx={x} cy={y} rx={size * 5} ry={size * 0.12} />
        <ellipse cx={x} cy={y} rx={size * 0.12} ry={size * 4} />
        <g transform={`rotate(45 ${x} ${y})`} opacity={0.55}>
          <ellipse cx={x} cy={y} rx={size * 2.2} ry={size * 0.08} />
          <ellipse cx={x} cy={y} rx={size * 0.08} ry={size * 2.2} />
        </g>
        <circle cx={x} cy={y} r={size * 0.55} />
      </g>
    </g>
  );
}

/** Bands across a world, waving a little, the way gas giants are striped. */
function Bands({ from, to, count, tilt, seed }: { from: number; to: number; count: number; tilt: number; seed: number }) {
  const next = scattered(seed);
  const tints = [BASE, DEEP, GLOW, NIGHT];
  const step = (to - from) / count;
  return (
    <g transform={`rotate(${tilt} 50 50)`}>
      {Array.from({ length: count }, (_, index) => {
        const top = from + index * step + next() * step * 0.3;
        const tall = step * (0.35 + next() * 0.6);
        const sway = 1 + next() * 2.5;
        const tint = tints[Math.floor(next() * tints.length)];
        return (
          <path
            key={index}
            d={`M-20 ${top} C 15 ${top - sway}, 55 ${top + sway}, 120 ${top - sway / 2} L120 ${top + tall} C 55 ${top + tall + sway}, 15 ${top + tall - sway}, -20 ${top + tall} Z`}
            style={{ fill: tint }}
            opacity={0.1 + next() * 0.18}
          />
        );
      })}
    </g>
  );
}

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
            [0, DEEP, 0.6],
            [0.55, NIGHT],
          ])}
        </radialGradient>
        {/* The edges darkened, which is what gives each one its depth. */}
        <radialGradient id={def("vignette")} cx="0.5" cy="0.5" r="0.75">
          {stops([
            [0.55, NIGHT, 0],
            [1, NIGHT, 0.75],
          ])}
        </radialGradient>
        {/* A sheen across the top, as light on glass. */}
        <linearGradient id={def("sheen")} x1="0" y1="0" x2="0" y2="1">
          {stops([
            [0, LIGHT, 0.14],
            [0.35, LIGHT, 0],
          ])}
        </linearGradient>
        {DRAWINGS[drawing].defs(def)}
      </defs>
      <rect width="100" height="100" style={{ fill: NIGHT }} />
      <rect width="100" height="100" fill={ref("ground")} />
      {DRAWINGS[drawing].ink(ref)}
      <rect width="100" height="100" fill={ref("vignette")} />
      <rect width="100" height="100" fill={ref("sheen")} />
    </svg>
  );
}

type Name = (name: string) => string;

const DRAWINGS: Record<FaceDrawing, { defs: (def: Name) => React.ReactNode; ink: (ref: Name) => React.ReactNode }> = {
  /* A banded giant rising out of the dark, lit from above on the left, with
     its air glowing along the lit edge and a small moon beside it. */
  planet: {
    defs: (def) => (
      <>
        <radialGradient id={def("body")} cx="0.28" cy="0.22" r="0.95">
          {stops([
            [0, LIGHT],
            [0.14, GLOW],
            [0.38, BASE],
            [0.66, DEEP],
            [0.9, NIGHT],
          ])}
        </radialGradient>
        <linearGradient id={def("night")} x1="0.15" y1="0.1" x2="0.85" y2="0.9">
          {stops([
            [0.35, NIGHT, 0],
            [0.75, NIGHT, 0.8],
            [1, NIGHT, 0.95],
          ])}
        </linearGradient>
        <linearGradient id={def("limb")} x1="0" y1="0" x2="1" y2="1">
          {stops([
            [0, LIGHT, 1],
            [0.35, GLOW, 0.7],
            [0.6, GLOW, 0],
          ])}
        </linearGradient>
        <radialGradient id={def("air")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.86, GLOW, 0.55],
            [0.92, GLOW, 0.2],
            [1, GLOW, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("dawn")} cx="0.12" cy="0.1" r="0.7">
          {stops([
            [0, GLOW, 0.45],
            [1, GLOW, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("moon")} cx="0.3" cy="0.28" r="0.85">
          {stops([
            [0, LIGHT],
            [0.45, BASE],
            [1, NIGHT],
          ])}
        </radialGradient>
        <clipPath id={def("within")}>
          <circle cx="62" cy="66" r="44" />
        </clipPath>
      </>
    ),
    ink: (ref) => (
      <>
        <rect width="100" height="100" fill={ref("dawn")} />
        <Stars at={SKY.filter(([x, y]) => (x - 62) ** 2 + (y - 66) ** 2 > 48 ** 2)} />
        <circle cx="62" cy="66" r="50" fill={ref("air")} />
        <circle cx="62" cy="66" r="44" fill={ref("body")} />
        <g clipPath={ref("within")}>
          <Bands from={18} to={112} count={14} tilt={-14} seed={3} />
          <ellipse cx="46" cy="62" rx="7" ry="3.2" style={{ fill: BASE }} opacity={0.5} transform="rotate(-14 46 62)" />
          <ellipse cx="46" cy="62" rx="4" ry="1.6" style={{ fill: LIGHT }} opacity={0.35} transform="rotate(-14 46 62)" />
          <rect width="100" height="100" fill={ref("night")} />
        </g>
        <circle cx="62" cy="66" r="44" fill="none" stroke={ref("limb")} strokeWidth="0.9" />
        <circle cx="19" cy="27" r="5.5" fill={ref("moon")} />
        <circle cx="19" cy="27" r="5.5" fill="none" style={{ stroke: LIGHT }} strokeWidth="0.25" opacity={0.5} />
      </>
    ),
  },

  /* The sun hidden behind a dark disc: its corona streaming out in long
     rays, a red rim of fire at the edge, and the one bright bead where the
     light breaks through. */
  eclipse: {
    defs: (def) => (
      <>
        <radialGradient id={def("corona")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.44, LIGHT, 0.95],
            [0.5, GLOW, 0.75],
            [0.66, BASE, 0.3],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("ray")} gradientUnits="userSpaceOnUse" cx="50" cy="52" r="50">
          {stops([
            [0.42, LIGHT, 0.55],
            [0.6, GLOW, 0.22],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("disc")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.7, NIGHT],
            [1, DEEP, 0.6],
          ])}
        </radialGradient>
        <linearGradient id={def("rim")} x1="0" y1="1" x2="1" y2="0">
          {stops([
            [0, BASE, 0.4],
            [0.6, GLOW, 0.9],
            [1, LIGHT, 1],
          ])}
        </linearGradient>
        <radialGradient id={def("bead")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, LIGHT],
            [0.3, LIGHT, 0.6],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => {
      const next = scattered(11);
      return (
        <>
          <Stars at={FAR_SKY.filter(([x, y]) => (x - 50) ** 2 + (y - 52) ** 2 > 30 ** 2)} />
          <g fill={ref("ray")}>
            {Array.from({ length: 44 }, (_, index) => {
              const angle = (index / 44) * Math.PI * 2 + next() * 0.15;
              const reach = 30 + next() * 26;
              const wide = 0.015 + next() * 0.035;
              const [ax, ay] = onEllipse(50, 52, 22, 22, 0, angle - wide);
              const [bx, by] = onEllipse(50, 52, 22, 22, 0, angle + wide);
              const [tx, ty] = onEllipse(50, 52, reach, reach, 0, angle);
              return <path key={index} d={`M${ax} ${ay} L${tx} ${ty} L${bx} ${by} Z`} opacity={0.35 + next() * 0.5} />;
            })}
          </g>
          <circle cx="50" cy="52" r="40" fill={ref("corona")} />
          <circle cx="50" cy="52" r="23.4" fill="none" style={{ stroke: BASE }} strokeWidth="1.6" opacity={0.7} />
          <circle cx="50" cy="52" r="23" fill={ref("disc")} />
          <circle cx="50" cy="52" r="23" fill="none" stroke={ref("rim")} strokeWidth="0.7" />
          <g style={{ fill: GLOW }} opacity={0.8}>
            <path d="M30.5 63 q-2.4 1.2 -1.6 -2.6 q1 1.4 1.6 2.6 Z" />
            <path d="M36 70.6 q-1.8 2.6 -2.6 -0.6 q1.4 0.2 2.6 0.6 Z" />
            <path d="M71.2 47 q2.6 -0.4 1.4 2.4 q-0.8 -1.2 -1.4 -2.4 Z" />
          </g>
          <circle cx="66" cy="35.6" r="10" fill={ref("bead")} />
          <Flare x={66} y={35.6} size={1.6} fill={ref("bead")} />
        </>
      );
    },
  },

  /* A ringed world: rings of many bands with a dark gap between them,
     passing behind it and in front, its shadow laid across them and theirs
     across it. */
  rings: {
    defs: (def) => (
      <>
        <radialGradient id={def("body")} cx="0.32" cy="0.26" r="0.9">
          {stops([
            [0, LIGHT],
            [0.2, GLOW],
            [0.45, BASE],
            [0.75, DEEP],
            [1, NIGHT],
          ])}
        </radialGradient>
        <linearGradient id={def("ring")} x1="0" y1="0" x2="1" y2="0">
          {stops([
            [0, BASE, 0.05],
            [0.25, LIGHT, 0.9],
            [0.55, GLOW, 0.8],
            [0.85, BASE, 0.5],
            [1, DEEP, 0.05],
          ])}
        </linearGradient>
        <linearGradient id={def("dusk")} x1="0" y1="0" x2="1" y2="1">
          {stops([
            [0.45, NIGHT, 0],
            [1, NIGHT, 0.85],
          ])}
        </linearGradient>
        <clipPath id={def("front")} clipPathUnits="userSpaceOnUse">
          <rect x="-20" y="54" width="140" height="60" />
        </clipPath>
        <clipPath id={def("within")}>
          <circle cx="50" cy="54" r="21" />
        </clipPath>
        <radialGradient id={def("halo")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.35, GLOW, 0.4],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => {
      // Inner to outer: how far out, how thick, how bright.
      const bands: [number, number, number][] = [
        [27, 0.5, 0.35],
        [29, 1.4, 0.55],
        [31.5, 2.2, 0.95],
        [34, 1.6, 0.85],
        [35.6, 0.5, 0.6],
        [38.4, 1.8, 0.7],
        [40.6, 0.9, 0.5],
        [42.4, 0.35, 0.35],
      ];
      const rings = (
        <g fill="none" stroke={ref("ring")}>
          {bands.map(([reach, thick, bright]) => (
            <ellipse key={reach} cx="50" cy="54" rx={reach} ry={reach * 0.24} strokeWidth={thick} opacity={bright} />
          ))}
        </g>
      );
      return (
        <>
          <Stars at={SKY} />
          <circle cx="50" cy="54" r="40" fill={ref("halo")} />
          <g transform="rotate(-16 50 54)">{rings}</g>
          <circle cx="50" cy="54" r="21" fill={ref("body")} />
          <g clipPath={ref("within")}>
            <Bands from={30} to={80} count={9} tilt={-16} seed={5} />
            {/* The rings' shadow, a dark line across the world. */}
            <ellipse cx="50" cy="51" rx="30" ry="2" transform="rotate(-16 50 54)" style={{ fill: NIGHT }} opacity={0.3} />
            <rect width="100" height="100" fill={ref("dusk")} />
          </g>
          <g transform="rotate(-16 50 54)">
            <g clipPath={ref("front")}>{rings}</g>
          </g>
        </>
      );
    },
  },

  /* The northern lights over a range of mountains: curtains of light
     hanging in long folds, each made of many fine rays, brightest at their
     foot and fading as they rise. */
  silk: {
    defs: (def) => (
      <>
        <linearGradient id={def("ray")} x1="0" y1="1" x2="0" y2="0">
          {stops([
            [0, LIGHT, 0.9],
            [0.12, GLOW, 0.75],
            [0.5, BASE, 0.3],
            [1, DEEP, 0],
          ])}
        </linearGradient>
        <linearGradient id={def("far")} x1="0" y1="1" x2="0" y2="0">
          {stops([
            [0, GLOW, 0.55],
            [0.4, BASE, 0.2],
            [1, BASE, 0],
          ])}
        </linearGradient>
        <linearGradient id={def("land")} x1="0" y1="0" x2="0" y2="1">
          {stops([
            [0, DEEP],
            [0.25, NIGHT],
          ])}
        </linearGradient>
        <linearGradient id={def("ridge")} x1="0" y1="0" x2="1" y2="0">
          {stops([
            [0, LIGHT, 0.1],
            [0.5, LIGHT, 0.75],
            [1, LIGHT, 0.1],
          ])}
        </linearGradient>
      </>
    ),
    ink: (ref) => {
      const farFoot = (x: number) => 60 - 10 * Math.sin((x / 100) * Math.PI * 1.6 + 0.6);
      // Its foot behind the mountains, so the light rises from beyond them.
      const nearFoot = (x: number) => 86 - 6 * Math.sin((x / 100) * Math.PI * 1.3 + 0.2);
      // The glow the rays stand in, rising off their foot.
      const glowAlong = (foot: (x: number) => number, tall: number) => {
        const xs = Array.from({ length: 23 }, (_, index) => -5 + index * 5);
        const base = xs.map((x) => `${x} ${foot(x)}`).join(" L");
        const top = [...xs].reverse().map((x) => `${x} ${foot(x) - tall}`).join(" L");
        return `M${base} L${top} Z`;
      };
      const curtain = (seed: number, count: number, foot: (x: number) => number, tall: number, fill: string) => {
        const next = scattered(seed);
        return (
          <g fill={fill}>
            {Array.from({ length: count }, (_, index) => {
              const x = -4 + (index / count) * 108 + next() * 1.5;
              const y = foot(x);
              const high = tall * (0.6 + next() * 0.4);
              const wide = 0.25 + next() * 0.7;
              return <rect key={index} x={x} y={y - high} width={wide} height={high} opacity={0.15 + next() * 0.45} />;
            })}
          </g>
        );
      };
      return (
        <>
          <Stars at={FAR_SKY.filter(([, y]) => y < 55)} />
          <path d={glowAlong(nearFoot, 40)} fill={ref("far")} />
          {curtain(17, 150, farFoot, 40, ref("far"))}
          {curtain(23, 190, nearFoot, 62, ref("ray"))}
          <path
            d="M-5 100 L-5 84 L8 79 L16 82 L27 73 L35 78 L44 71 L55 80 L63 75 L74 82 L84 76 L95 80 L105 78 L105 100 Z"
            fill={ref("land")}
          />
          <path
            d="M-5 84 L8 79 L16 82 L27 73 L35 78 L44 71 L55 80 L63 75 L74 82 L84 76 L95 80 L105 78"
            fill="none"
            stroke={ref("ridge")}
            strokeWidth="0.45"
          />
          <path d="M-5 100 L-5 92 L12 89 L26 93 L40 88 L58 94 L72 90 L88 95 L105 91 L105 100 Z" style={{ fill: NIGHT }} />
        </>
      );
    },
  },

  /* Sunrise seen from orbit: the curve of a world below with its clouds,
     its air glowing in thin layers along the edge, and the sun just
     breaking over it with its rays and the flare of the lens. */
  horizon: {
    defs: (def) => (
      <>
        <radialGradient id={def("sun")} gradientUnits="userSpaceOnUse" cx="50" cy="56" r="60">
          {stops([
            [0, LIGHT],
            [0.06, LIGHT, 0.9],
            [0.16, GLOW, 0.6],
            [0.45, BASE, 0.22],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <radialGradient id={def("land")} gradientUnits="userSpaceOnUse" cx="50" cy="56" r="70">
          {stops([
            [0, BASE, 0.9],
            [0.25, DEEP],
            [0.7, NIGHT],
          ])}
        </radialGradient>
        <linearGradient id={def("edge")} x1="0" y1="0" x2="1" y2="0">
          {stops([
            [0, GLOW, 0],
            [0.5, LIGHT, 1],
            [1, GLOW, 0],
          ])}
        </linearGradient>
        <linearGradient id={def("streak")} x1="0" y1="0" x2="1" y2="0">
          {stops([
            [0, LIGHT, 0],
            [0.5, LIGHT, 0.9],
            [1, LIGHT, 0],
          ])}
        </linearGradient>
        <clipPath id={def("world")}>
          <circle cx="50" cy="158" r="102" />
        </clipPath>
      </>
    ),
    ink: (ref) => {
      const next = scattered(31);
      return (
        <>
          <Stars at={FAR_SKY.filter(([, y]) => y < 45)} />
          <rect width="100" height="100" fill={ref("sun")} />
          <g style={{ stroke: LIGHT }} strokeWidth="0.15">
            {Array.from({ length: 14 }, (_, index) => {
              const angle = Math.PI + ((index + 0.5) / 14) * Math.PI;
              const reach = 14 + next() * 22;
              const [x, y] = onEllipse(50, 56, reach, reach, 0, angle);
              return <line key={index} x1="50" y1="56" x2={x} y2={y} opacity={0.08 + next() * 0.16} />;
            })}
          </g>
          {[4.6, 3.2, 1.9].map((lift, index) => (
            <circle key={lift} cx="50" cy="158" r={102 + lift} fill="none" style={{ stroke: index === 2 ? LIGHT : GLOW }} strokeWidth={index === 2 ? 0.6 : 1.3} opacity={0.18 + index * 0.25} />
          ))}
          <circle cx="50" cy="158" r="102" fill={ref("land")} />
          {/* Clouds, long and thin as seen this low over the edge. */}
          <g clipPath={ref("world")} style={{ fill: LIGHT }}>
            {Array.from({ length: 16 }, (_, index) => {
              const y = 59 + index * 2.6 + next() * 1.5;
              const from = next() * 70 - 15;
              const span = 14 + next() * 34;
              const thick = 0.4 + (index / 16) * 1.6;
              return (
                <path
                  key={index}
                  d={`M${from} ${y} Q ${from + span / 2} ${y - thick} ${from + span} ${y} Q ${from + span / 2} ${y + thick * 0.4} ${from} ${y} Z`}
                  opacity={0.05 + (1 - index / 16) * 0.16}
                />
              );
            })}
          </g>
          <circle cx="50" cy="158" r="102" fill="none" stroke={ref("edge")} strokeWidth="0.8" />
          <ellipse cx="50" cy="56" rx="46" ry="0.45" fill={ref("streak")} />
          <circle cx="50" cy="56" r="3.2" style={{ fill: LIGHT }} />
          <g style={{ fill: GLOW }}>
            <circle cx="62" cy="43" r="1.4" opacity={0.3} />
            <circle cx="70" cy="34" r="3" opacity={0.12} />
            <circle cx="76" cy="27" r="1" opacity={0.35} />
            <circle cx="85" cy="16" r="5" opacity={0.08} />
          </g>
        </>
      );
    },
  },

  /* A record on its turntable: fine grooves in tracks, the light catching
     them in two opposite wedges, a printed label, and the tone arm resting
     on the edge. */
  record: {
    defs: (def) => (
      <>
        <radialGradient id={def("disc")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, DEEP],
            [0.35, NIGHT],
            [1, NIGHT],
          ])}
        </radialGradient>
        <linearGradient id={def("sheen")} x1="0" y1="0" x2="1" y2="1">
          {stops([
            [0, LIGHT, 0],
            [0.5, LIGHT, 0.55],
            [1, LIGHT, 0],
          ])}
        </linearGradient>
        <linearGradient id={def("faint")} x1="1" y1="0" x2="0" y2="1">
          {stops([
            [0, GLOW, 0],
            [0.5, GLOW, 0.35],
            [1, GLOW, 0],
          ])}
        </linearGradient>
        <radialGradient id={def("label")} cx="0.35" cy="0.3" r="0.8">
          {stops([
            [0, LIGHT],
            [0.35, GLOW],
            [0.7, BASE],
            [1, DEEP],
          ])}
        </radialGradient>
        <radialGradient id={def("halo")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0.72, GLOW, 0.45],
            [1, GLOW, 0],
          ])}
        </radialGradient>
        <linearGradient id={def("metal")} x1="0" y1="0" x2="1" y2="1">
          {stops([
            [0, LIGHT],
            [0.45, GLOW, 0.8],
            [1, DEEP],
          ])}
        </linearGradient>
      </>
    ),
    ink: (ref) => {
      const next = scattered(41);
      // The silent bands between two songs, where the grooves part.
      const between = new Set([5, 12, 20]);
      return (
        <>
          <Stars at={SKY.filter(([x, y]) => (x - 46) ** 2 + (y - 54) ** 2 > 45 ** 2)} />
          <circle cx="46" cy="54" r="46" fill={ref("halo")} />
          <circle cx="46" cy="54" r="40" fill={ref("disc")} />
          <g fill="none" style={{ stroke: LIGHT }}>
            {Array.from({ length: 28 }, (_, index) => (
              <circle
                key={index}
                cx="46"
                cy="54"
                r={14 + index * 0.92}
                strokeWidth={between.has(index) ? 0.12 : 0.22}
                opacity={between.has(index) ? 0.02 : 0.06 + next() * 0.1}
              />
            ))}
          </g>
          <path d="M46 54 L17.7 25.7 A40 40 0 0 1 32.3 16.4 Z M46 54 L74.3 82.3 A40 40 0 0 1 59.7 91.6 Z" fill={ref("sheen")} opacity={0.5} />
          <path d="M46 54 L86 54 A40 40 0 0 1 84.6 64.4 Z M46 54 L6 54 A40 40 0 0 1 7.4 43.6 Z" fill={ref("faint")} opacity={0.6} />
          <circle cx="46" cy="54" r="40" fill="none" style={{ stroke: GLOW }} strokeWidth="0.5" opacity={0.6} />
          <circle cx="46" cy="54" r="12.5" fill={ref("label")} />
          <circle cx="46" cy="54" r="11.2" fill="none" style={{ stroke: LIGHT }} strokeWidth="0.25" opacity={0.6} />
          <g style={{ fill: NIGHT }} opacity={0.35}>
            <rect x="39" y="46.5" width="14" height="1.1" rx="0.55" />
            <rect x="41" y="48.6" width="10" height="0.8" rx="0.4" />
            <rect x="40" y="59" width="12" height="0.8" rx="0.4" />
            <rect x="42.5" y="60.8" width="7" height="0.7" rx="0.35" />
          </g>
          <circle cx="46" cy="54" r="1.4" style={{ fill: LIGHT }} opacity={0.9} />
          <circle cx="46" cy="54" r="0.7" style={{ fill: NIGHT }} />
          {/* The tone arm, from its pivot to the head resting in the grooves. */}
          <circle cx="82" cy="20" r="5.6" fill={ref("metal")} />
          <circle cx="82" cy="20" r="3.2" style={{ fill: NIGHT }} opacity={0.6} />
          <circle cx="82" cy="20" r="1.4" fill={ref("metal")} />
          <path d="M82 20 L83 44 Q83.5 50 79 54.5" fill="none" stroke={ref("metal")} strokeWidth="1.3" strokeLinecap="round" />
          <path d="M82 20 L83 44 Q83.5 50 79 54.5" fill="none" style={{ stroke: LIGHT }} strokeWidth="0.3" opacity={0.7} />
          <rect x="74.5" y="53.5" width="7" height="3.6" rx="0.8" fill={ref("metal")} transform="rotate(38 78 55.3)" />
        </>
      );
    },
  },

  /* A cloud of gas and dust lit from within: soft clouds of several
     colours laid over each other, dark lanes of dust across them, fine
     bright threads, and a young star at the heart with its spikes. */
  nebula: {
    defs: (def) => (
      <>
        {(
          [
            ["base", BASE, 0.75],
            ["glow", GLOW, 0.6],
            ["deep", DEEP, 0.95],
            ["light", LIGHT, 0.45],
          ] as const
        ).map(([name, tint, strength]) => (
          <radialGradient key={name} id={def(name)} cx="0.5" cy="0.5" r="0.5">
            {stops([
              [0, tint, strength],
              [0.55, tint, strength * 0.4],
              [1, tint, 0],
            ])}
          </radialGradient>
        ))}
        <radialGradient id={def("star")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, LIGHT],
            [0.2, LIGHT, 0.8],
            [1, GLOW, 0],
          ])}
        </radialGradient>
      </>
    ),
    ink: (ref) => {
      // Where each cloud sits, how wide and tall, how it leans, and its tint.
      const clouds: [number, number, number, number, number, string][] = [
        [28, 70, 50, 34, 10, "deep"],
        [70, 34, 44, 30, -25, "deep"],
        [62, 42, 36, 24, -25, "base"],
        [36, 60, 30, 20, 25, "base"],
        [50, 50, 26, 16, 15, "glow"],
        [74, 62, 22, 14, -40, "glow"],
        [24, 34, 20, 13, 30, "base"],
        [56, 47, 14, 9, -10, "light"],
        [40, 56, 10, 6, 30, "light"],
      ];
      return (
        <>
          <Stars at={FAR_SKY} />
          {clouds.map(([x, y, wide, tall, lean, tint], index) => (
            <ellipse key={index} cx={x} cy={y} rx={wide} ry={tall} fill={ref(tint)} transform={`rotate(${lean} ${x} ${y})`} />
          ))}
          {/* Each lane laid three times, wide and faint to narrow and dark,
              so its edges fade without anything blurred. */}
          <g fill="none" style={{ stroke: NIGHT }} strokeLinecap="round">
            {(
              [
                ["M8 58 C 24 50, 34 66, 52 58 S 76 64, 96 50", 6],
                ["M18 76 C 30 70, 44 80, 60 72", 4],
                ["M44 22 C 54 30, 62 24, 78 30", 3],
              ] as const
            ).map(([d, wide]) =>
              [1, 0.6, 0.3].map((share) => (
                <path key={`${d}${share}`} d={d} strokeWidth={wide * share} opacity={0.16} />
              )),
            )}
          </g>
          <g fill="none" style={{ stroke: LIGHT }} strokeLinecap="round">
            <path d="M14 44 C 30 36, 40 52, 58 44 S 82 40, 92 30" strokeWidth="0.35" opacity={0.3} />
            <path d="M22 64 C 34 58, 46 68, 66 60" strokeWidth="0.25" opacity={0.25} />
            <path d="M48 70 C 58 66, 70 74, 86 66" strokeWidth="0.3" opacity={0.2} />
          </g>
          <Stars at={SKY.filter(([, , size]) => size > 0.5)} />
          <Flare x={56} y={47} size={2} fill={ref("star")} />
          <Flare x={24} y={30} size={0.7} fill={ref("star")} />
          <Flare x={80} y={74} size={0.8} fill={ref("star")} />
        </>
      );
    },
  },

  /* A small solar system: a bright sun, the paths of its worlds drawn
     finely, each world lit on the side that faces the sun, a ringed one
     among them, a belt of stones, and a comet passing. */
  orbits: {
    defs: (def) => (
      <>
        <radialGradient id={def("sun")} cx="0.5" cy="0.5" r="0.5">
          {stops([
            [0, LIGHT],
            [0.14, LIGHT, 0.9],
            [0.32, GLOW, 0.45],
            [1, BASE, 0],
          ])}
        </radialGradient>
        <linearGradient id={def("path")} x1="0" y1="0" x2="0" y2="1">
          {stops([
            [0, BASE, 0.25],
            [1, LIGHT, 0.7],
          ])}
        </linearGradient>
        {/* A world lit from wherever the sun stands from it. */}
        {(
          [
            ["right", 0.75, 0.4],
            ["left", 0.2, 0.3],
            ["up", 0.45, 0.15],
            ["down", 0.55, 0.8],
          ] as const
        ).map(([name, x, y]) => (
          <radialGradient key={name} id={def(name)} cx={x} cy={y} r="0.85">
            {stops([
              [0, LIGHT],
              [0.35, GLOW],
              [0.65, BASE],
              [1, NIGHT],
            ])}
          </radialGradient>
        ))}
        <linearGradient id={def("tail")} x1="1" y1="0" x2="0" y2="1">
          {stops([
            [0, LIGHT, 0.9],
            [0.3, GLOW, 0.5],
            [1, BASE, 0],
          ])}
        </linearGradient>
      </>
    ),
    ink: (ref) => {
      const tilt = -18;
      const at = (rx: number, ry: number, angle: number) => onEllipse(50, 54, rx, ry, tilt, angle);
      const next = scattered(53);
      const [mx, my] = at(16, 8, 0.4);
      const [vx, vy] = at(25, 13, 3.6);
      const [rx, ry] = at(35, 18.5, 5.4);
      const [ox, oy] = at(46, 24.5, 2.2);
      return (
        <>
          <Stars at={SKY} />
          {/* A comet crossing high above, its tail away from the sun. */}
          <path d="M18 14 L 2 2.5 L 0 5.5 Z" fill={ref("tail")} />
          <circle cx="18" cy="14" r="0.9" style={{ fill: LIGHT }} />
          <g fill="none" stroke={ref("path")}>
            {[
              [16, 8],
              [25, 13],
              [35, 18.5],
              [46, 24.5],
            ].map(([wide, tall]) => (
              <ellipse key={wide} cx="50" cy="54" rx={wide} ry={tall} strokeWidth="0.3" transform={`rotate(${tilt} 50 54)`} />
            ))}
          </g>
          <g style={{ fill: LIGHT }}>
            {Array.from({ length: 70 }, (_, index) => {
              const angle = (index / 70) * Math.PI * 2 + next() * 0.08;
              const reach = 29.5 + next() * 2.6;
              const [x, y] = at(reach, reach * 0.53, angle);
              return <circle key={index} cx={x} cy={y} r={0.12 + next() * 0.22} opacity={0.25 + next() * 0.45} />;
            })}
          </g>
          <circle cx="50" cy="54" r="13" fill={ref("sun")} />
          <circle cx="50" cy="54" r="3" style={{ fill: LIGHT }} />
          <Flare x={50} y={54} size={1.4} fill={ref("sun")} />
          <circle cx={mx} cy={my} r="1.6" fill={ref("left")} />
          <circle cx={vx} cy={vy} r="2.6" fill={ref("right")} />
          <g transform={`rotate(${tilt} ${rx} ${ry})`}>
            <ellipse cx={rx} cy={ry} rx="6.4" ry="1.6" fill="none" style={{ stroke: GLOW }} strokeWidth="0.6" opacity={0.75} />
            <circle cx={rx} cy={ry} r="3.2" fill={ref("up")} />
            <path d={`M${rx - 6.4} ${ry} A6.4 1.6 0 0 0 ${rx + 6.4} ${ry}`} fill="none" style={{ stroke: LIGHT }} strokeWidth="0.6" opacity={0.85} />
          </g>
          <circle cx={ox} cy={oy} r="4.2" fill={ref("down")} />
          <circle cx={ox + 6.5} cy={oy - 2} r="0.9" fill={ref("left")} />
        </>
      );
    },
  },
};

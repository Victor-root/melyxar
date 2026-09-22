/*
 * What is drawn behind the door.
 *
 * Three answers, and which one is used belongs to the administrator rather
 * than to this file. A picture they put there wins over everything: there is
 * nothing left to choose once somebody has said what they want behind their
 * own door. Failing that, one of the two drawn below, whichever the server
 * says.
 *
 * Every one of them is drawn from the accent colour and the surfaces of the
 * theme in force, so a server given another accent keeps its own door rather
 * than a red one. Not one of them holds a picture of a film: what is behind
 * the box somebody types into should not be a thing to look at.
 */

import type { Branding } from "../api";

export function DoorBackground({ branding }: { branding: Branding }) {
  if (branding.login_background_path) {
    return (
      <div className="door-picture" aria-hidden="true">
        {/* What is stored is what the browser asks for: the day an
            administrator can upload one, the address it is served at is the
            address written down. */}
        <img src={branding.login_background_path} alt="" />
        {/* A wash over it, or the card and the words would be read against
            whatever somebody happened to upload. */}
        <div className="door-picture-wash" />
      </div>
    );
  }

  return branding.login_background_style === "library" ? <DrawnLibrary /> : <Drift />;
}

/*
 * The one a server wears out of the box: light, and dust caught in it.
 *
 * Nothing here is a shape anybody has to read, which is the point. It is the
 * accent behind a sheet of glass, and it says the screen is alive without
 * asking for a moment of anybody's attention.
 *
 * One box, and the stylesheet paints the lot: nothing is computed here and
 * nothing is fetched, since a background that costs a request is a
 * background somebody waits for. It used to be seven boxes each moving at
 * its own pace, which cost a graphics card more than the whole rest of the
 * interface put together; the stylesheet has that sum, and the home page is
 * set on the same painting.
 */
function Drift() {
  return <div className="door-drift drift" aria-hidden="true" />;
}

/*
 * The other one: the shelf of things a media server holds.
 *
 * What this screen wore before the drift above, kept because it says what
 * this server is for without a word, and offered to an administrator who
 * would rather have it.
 *
 * A row of identical rectangles said nothing: a shape with one line on it is
 * not a film, it is a box. What a media server holds is films, series,
 * episodes, records and sound, so that is what is drawn: a poster with a title
 * on three lines, a sleeve with a disc, an episode card, a reel of film, a
 * player bar part way through, a note, a waveform, a clapperboard.
 *
 * All of it is drawn rather than typed, in the same square-cornered style as
 * the player's icons, so nine different objects read as one set. The stroke
 * does not scale with the object: a bar drawn twice the size of a note would
 * otherwise carry a line twice as heavy, and the set would fall apart.
 */
type Shape =
  | "poster"
  | "album"
  | "episode"
  | "strip"
  | "badge"
  | "bar"
  | "note"
  | "wave"
  | "clapper";

/** The drawing of each one, and the square it is drawn in. */
const DRAWN: Record<Shape, { box: [number, number]; ink: React.ReactNode }> = {
  /* A film poster: artwork, then a title long enough to be a title. */
  poster: {
    box: [60, 90],
    ink: (
      <>
        <rect x="0.9" y="0.9" width="58.2" height="88.2" rx="4.5" />
        <rect x="6" y="6" width="48" height="52" rx="2.6" />
        <circle cx="41" cy="19" r="5" />
        <path
          d="M6 58 L21 33 L33 49 L41 40 L54 58 Z"
          fill="currentColor"
          stroke="none"
          opacity="0.5"
        />
        <g fill="currentColor" stroke="none">
          <rect x="6" y="66" width="42" height="3" rx="1.5" />
          <rect x="6" y="74" width="30" height="3" rx="1.5" opacity="0.7" />
          <rect x="6" y="82" width="19" height="3" rx="1.5" opacity="0.7" />
        </g>
      </>
    ),
  },

  /* A record sleeve, the disc sitting in it. */
  album: {
    box: [64, 64],
    ink: (
      <>
        <rect x="0.9" y="0.9" width="62.2" height="62.2" rx="4.5" />
        <circle cx="32" cy="27" r="15" />
        <circle cx="32" cy="27" r="5.4" />
        <circle cx="32" cy="27" r="1.2" fill="currentColor" stroke="none" />
        <g fill="currentColor" stroke="none">
          <rect x="11" y="50" width="30" height="3" rx="1.5" />
          <rect x="11" y="57" width="18" height="3" rx="1.5" opacity="0.7" />
        </g>
      </>
    ),
  },

  /* An episode: the wide card, the play mark, the name under it. */
  episode: {
    box: [96, 74],
    ink: (
      <>
        <rect x="0.9" y="0.9" width="94.2" height="52.2" rx="4.5" />
        <circle cx="48" cy="27" r="10.5" />
        <path d="M44.8 21.8 L54 27 L44.8 32.2 Z" fill="currentColor" stroke="none" />
        <g fill="currentColor" stroke="none">
          <rect x="1" y="60" width="56" height="3.2" rx="1.6" />
          <rect x="1" y="68" width="33" height="3.2" rx="1.6" opacity="0.7" />
        </g>
      </>
    ),
  },

  /* A length of film, perforated down both edges. */
  strip: {
    box: [36, 100],
    ink: (
      <>
        <rect x="0.9" y="0.9" width="34.2" height="98.2" rx="3" />
        {[6, 18, 30, 42, 54, 66, 78, 90].map((y) => (
          <g key={y} fill="currentColor" stroke="none" opacity="0.6">
            <rect x="3.4" y={y} width="4.2" height="5" rx="1.2" />
            <rect x="28.4" y={y} width="4.2" height="5" rx="1.2" />
          </g>
        ))}
        {[5, 29, 53, 77].map((y) => (
          <rect key={y} x="10.5" y={y} width="15" height="18" rx="1.6" />
        ))}
      </>
    ),
  },

  /* The round play mark, on its own. */
  badge: {
    box: [48, 48],
    ink: (
      <>
        <circle cx="24" cy="24" r="22.6" />
        <path d="M19 15.4 L34 24 L19 32.6 Z" fill="currentColor" stroke="none" />
      </>
    ),
  },

  /* A player bar, a third of the way through something. */
  bar: {
    box: [168, 30],
    ink: (
      <>
        <rect x="0.9" y="0.9" width="166.2" height="28.2" rx="14.1" />
        <path d="M13 9.6 L23 15 L13 20.4 Z" fill="currentColor" stroke="none" />
        <rect
          x="32"
          y="13.8"
          width="104"
          height="2.4"
          rx="1.2"
          fill="currentColor"
          stroke="none"
          opacity="0.4"
        />
        <rect
          x="32"
          y="13.8"
          width="44"
          height="2.4"
          rx="1.2"
          fill="currentColor"
          stroke="none"
        />
        <circle cx="76" cy="15" r="4.2" fill="currentColor" stroke="none" />
        <path
          d="M145 11.4h3l4-3.4v14l-4-3.4h-3Z"
          fill="currentColor"
          stroke="none"
          opacity="0.8"
        />
        <path d="M155.6 11.8a3 3 0 0 1 0 6.4" />
      </>
    ),
  },

  /* Two notes under one beam, the shape sound has had for four hundred
     years. */
  note: {
    box: [40, 48],
    ink: (
      <>
        <ellipse
          cx="9"
          cy="38.5"
          rx="6.2"
          ry="4.8"
          transform="rotate(-18 9 38.5)"
          fill="currentColor"
          stroke="none"
        />
        <ellipse
          cx="30"
          cy="33.5"
          rx="6.2"
          ry="4.8"
          transform="rotate(-18 30 33.5)"
          fill="currentColor"
          stroke="none"
        />
        <path d="M15 38.5V8M36 33.5V3" />
        <path d="M15 8 L36 3 L36 9.6 L15 14.6 Z" fill="currentColor" stroke="none" />
      </>
    ),
  },

  /* Sound seen rather than heard. */
  wave: {
    box: [96, 40],
    ink: (
      <g fill="currentColor" stroke="none">
        {[10, 22, 34, 16, 28, 38, 24, 12, 30, 20, 34, 14].map((tall, at) => (
          <rect
            key={at}
            x={at * 8.2}
            y={20 - tall / 2}
            width="4.2"
            height={tall}
            rx="2.1"
            opacity={0.5 + (tall / 38) * 0.5}
          />
        ))}
      </g>
    ),
  },

  /* The board that opens a take. */
  clapper: {
    box: [84, 64],
    ink: (
      <>
        <rect x="1.4" y="24" width="81.2" height="38.6" rx="3.4" />
        <g fill="currentColor" stroke="none">
          <rect x="11" y="36" width="44" height="3.2" rx="1.6" opacity="0.7" />
          <rect x="11" y="45" width="27" height="3.2" rx="1.6" opacity="0.5" />
        </g>
        <g transform="rotate(-6 42 14)">
          <rect x="3" y="7" width="78" height="13" rx="2.4" />
          {[10, 30, 50, 70].map((x) => (
            <path
              key={x}
              d={`M${x} 7 h6 l-5 13 h-6 Z`}
              fill="currentColor"
              stroke="none"
              opacity="0.6"
            />
          ))}
        </g>
      </>
    ),
  },
};

/** One object, where it sits and how slowly it moves. */
interface Piece {
  what: Shape;
  x: string;
  y: string;
  /** Its width on screen; its height follows from the square it is drawn in. */
  wide: number;
  turn: number;
  /** How far it rises before coming back down. */
  lift: number;
  /** How long one rise and fall takes, and how far into it this one starts. */
  over: number;
  from: number;
  /** The width the screen has to reach before this one is drawn at all: see
   *  the stylesheet. Left out, it is drawn as soon as there is a background. */
  needs?: 900 | 1100;
  /** The few that carry the accent rather than the faint grey. */
  accent?: boolean;
}

/*
 * Where each one goes.
 *
 * Two rules hold the whole arrangement together. Nothing sits where the card
 * does, which is the middle of the screen, so the shelf keeps to the two
 * margins: a poster behind the box somebody is typing into is a poster in the
 * way. And no two of the same kind sit near each other, so the eye reads a
 * shelf of different things rather than a pattern.
 *
 * Each one also says how much screen it needs before it is drawn at all. The
 * card takes the same width whatever is around it, so the narrower the window
 * the less margin there is, and the ones nearest the middle are the first to
 * go.
 */
const SHELF: Piece[] = [
  { what: "poster", x: "1%", y: "13%", wide: 112, turn: -7, lift: -22, over: 15, from: 0 },
  { what: "album", x: "3%", y: "70%", wide: 92, turn: -5, lift: -18, over: 17, from: -2 },
  { what: "episode", x: "2%", y: "52%", wide: 170, turn: 4, lift: -15, over: 18, from: -5, needs: 900 },
  { what: "note", x: "23%", y: "12%", wide: 40, turn: 6, lift: -20, over: 12, from: -7, needs: 900 },
  { what: "strip", x: "20%", y: "72%", wide: 62, turn: 9, lift: -26, over: 21, from: -9, needs: 900 },
  { what: "wave", x: "19%", y: "33%", wide: 124, turn: -2, lift: -13, over: 13, from: -3, needs: 1100, accent: true },
  { what: "badge", x: "26%", y: "51%", wide: 52, turn: 0, lift: -20, over: 15, from: -10, needs: 1100, accent: true },

  { what: "poster", x: "81%", y: "10%", wide: 120, turn: 6, lift: -24, over: 19, from: -11 },
  { what: "clapper", x: "82%", y: "62%", wide: 96, turn: -8, lift: -18, over: 14, from: -4 },
  { what: "album", x: "72%", y: "18%", wide: 88, turn: -4, lift: -16, over: 20, from: -8, needs: 1100 },
  { what: "episode", x: "74%", y: "44%", wide: 150, turn: 5, lift: -22, over: 22, from: -13, needs: 900 },
  { what: "note", x: "94%", y: "76%", wide: 36, turn: -10, lift: -18, over: 11, from: -1, needs: 900 },
  { what: "wave", x: "90%", y: "33%", wide: 104, turn: 3, lift: -12, over: 13, from: -5, needs: 1100 },
  { what: "bar", x: "69%", y: "82%", wide: 236, turn: -3, lift: -15, over: 16, from: -6, needs: 1100 },
];

function DrawnLibrary() {
  return (
    <div className="door-shelf" aria-hidden="true">
      {SHELF.map((piece, at) => {
        const [wide, tall] = DRAWN[piece.what].box;
        return (
          <svg
            key={`${piece.what}-${at}`}
            className={`door-piece${piece.needs ? ` door-piece-from-${piece.needs}` : ""}${
              piece.accent ? " door-piece-accent" : ""
            }`}
            viewBox={`0 0 ${wide} ${tall}`}
            fill="none"
            stroke="currentColor"
            strokeWidth={1.4}
            strokeLinecap="round"
            strokeLinejoin="round"
            focusable="false"
            style={
              {
                "--at-x": piece.x,
                "--at-y": piece.y,
                "--wide": `${piece.wide}px`,
                "--turn": `${piece.turn}deg`,
                "--lift": `${piece.lift}px`,
                "--over": `${piece.over}s`,
                "--from": `${piece.from}s`,
              } as React.CSSProperties
            }
          >
            {DRAWN[piece.what].ink}
          </svg>
        );
      })}
    </div>
  );
}

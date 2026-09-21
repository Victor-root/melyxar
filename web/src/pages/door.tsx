/*
 * The door: the one screen somebody meets before this server knows them.
 *
 * It is the first thing anybody sees of Melyxar, and for a while it will be
 * the only finished thing, so it is drawn properly rather than as a form on a
 * grey page.
 *
 * One screen doing two jobs. A brand new server asks for the account it does
 * not have yet, with the password typed twice, because a typo there is a
 * server nobody can get into without a terminal. A server that has been set up
 * asks for a password, once.
 */

import { useEffect, useRef, useState } from "react";
import type { Account, Branding } from "../api";
import { useDoorScreen } from "../screens/door";
import { useSettings } from "../settings";
import type { ThemeChoice } from "../settings";

export function Door({
  branding,
  cameIn,
}: {
  branding: Branding;
  cameIn: (who: Account) => void;
}) {
  const { t, theme, setTheme } = useSettings();
  const door = useDoorScreen(branding, cameIn);

  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  /* Said here rather than by the server: the two boxes never leave this
     screen, so there is nothing to ask about. */
  const [twiceDiffers, setTwiceDiffers] = useState(false);
  const first = useRef<HTMLInputElement>(null);

  // The cursor where the first word goes, on a screen whose only purpose is to
  // be typed into.
  useEffect(() => first.current?.focus(), []);

  const refusal = twiceDiffers
    ? { key: "door.refused.not_the_same", values: {} }
    : door.refused;

  const send = (event: React.FormEvent) => {
    event.preventDefault();
    if (door.brandNew && password !== again) {
      setTwiceDiffers(true);
      return;
    }
    setTwiceDiffers(false);
    void door.knock(name.trim(), password);
  };

  // Typing again is somebody answering the refusal, so it goes.
  const typing = <T,>(set: (value: T) => void) => (value: T) => {
    setTwiceDiffers(false);
    door.forget();
    set(value);
  };

  return (
    <main className="door">
      <div className="door-glow" aria-hidden="true" />
      <DriftingMedia />

      <form className="door-card" onSubmit={send}>
        {/* The mark this server wears. A server given one of its own shows
            that instead, once there is a screen to give it one. */}
        <img className="door-mark" src="/melyxar-512.png" alt="" aria-hidden="true" />

        <h1 className="door-name">{branding.server_name}</h1>
        <p className="door-invitation">
          {t(door.brandNew ? "door.first.invitation" : "door.invitation")}
        </p>

        <label className="door-label" htmlFor="door-name">
          {t("door.name")}
        </label>
        <input
          id="door-name"
          ref={first}
          className="door-field"
          value={name}
          onChange={(event) => typing(setName)(event.target.value)}
          autoComplete="username"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          required
        />

        <label className="door-label" htmlFor="door-password">
          {t("door.password")}
        </label>
        <input
          id="door-password"
          className="door-field"
          type="password"
          value={password}
          onChange={(event) => typing(setPassword)(event.target.value)}
          autoComplete={door.brandNew ? "new-password" : "current-password"}
          required
        />

        {door.brandNew && (
          <>
            <label className="door-label" htmlFor="door-again">
              {t("door.password_again")}
            </label>
            <input
              id="door-again"
              className="door-field"
              type="password"
              value={again}
              onChange={(event) => typing(setAgain)(event.target.value)}
              autoComplete="new-password"
              required
            />
            {/* Deliberately without the number: the shortest a password may
                be is the server's rule, and it travels with the refusal that
                names it. Written here as well, the two would drift apart and
                the wrong one would be the one on screen. */}
            <p className="door-rule">{t("door.rule")}</p>
          </>
        )}

        {/* The refusal takes the place under the fields rather than appearing
            between them: a message that pushes the button down as it arrives
            is a message somebody clicks through by accident. */}
        <div className="door-answer" role="alert" aria-live="polite">
          {refusal && (
            <span className="door-refused" key={refusal.key}>
              {t(refusal.key, refusal.values)}
            </span>
          )}
        </div>

        <button
          className="button button-accent button-large door-go"
          type="submit"
          disabled={door.asking || name.trim() === "" || password === ""}
        >
          {t(
            door.asking
              ? "door.asking"
              : door.brandNew
                ? "door.first.go"
                : "door.go",
          )}
        </button>
      </form>

      {/* The language is read from the browser and never asked about here:
          somebody who cannot read the door cannot get through it to change
          it, so there is nothing to gain by offering a choice before one is
          needed. The theme is the one door setting a viewer can still turn,
          since automatic does not always land on the one their eyes want. */}
      <button
        type="button"
        className="door-theme"
        onClick={() => setTheme(nextTheme(theme))}
      >
        <ThemeIcon theme={theme} />
        {t("door.theme_toggle")}
      </button>

      <p className="door-slogan">{t("door.slogan")}</p>
      <p className="door-footer">{t("door.footer")}</p>
    </main>
  );
}

/** What the theme becomes the next time this is pressed. Cycling rather than
 *  a plain switch, because there are three of them: automatic first, since
 *  that is what a server nobody has told otherwise stays at. */
const THEME_ORDER: readonly ThemeChoice[] = ["system", "light", "dark"];

function nextTheme(current: ThemeChoice): ThemeChoice {
  return THEME_ORDER[(THEME_ORDER.indexOf(current) + 1) % THEME_ORDER.length];
}

/** The theme in force, drawn rather than named: a sun, a moon, or the two
 *  halved for whichever one automatic currently means. */
function ThemeIcon({ theme }: { theme: ThemeChoice }) {
  if (theme === "light") {
    return (
      <svg
        className="door-theme-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.8}
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="4.3" />
        <path d="M12 2.6v2.7M12 18.7v2.7M4.1 12H1.4M22.6 12h-2.7M5.4 5.4l1.9 1.9M16.7 16.7l1.9 1.9M18.6 5.4l-1.9 1.9M7.3 16.7l-1.9 1.9" />
      </svg>
    );
  }
  if (theme === "dark") {
    return (
      <svg
        className="door-theme-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.8}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M20.2 13.4A8.4 8.4 0 1 1 10.6 3.8a6.7 6.7 0 0 0 9.6 9.6Z" />
      </svg>
    );
  }
  return (
    <svg
      className="door-theme-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="8.4" />
      <path d="M12 3.6a8.4 8.4 0 0 1 0 16.8Z" fill="currentColor" stroke="none" />
    </svg>
  );
}

/*
 * What drifts behind the card.
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
 * does, at any width the background is shown at, which is why each one says
 * how much screen it needs before it appears at all: a poster behind the box
 * somebody is typing into is a poster in the way, and the card takes the same
 * three hundred and eighty pixels whatever is around it, so the narrower the
 * screen the less there is left at the sides. And no two of the same kind sit
 * near each other, so the eye reads a shelf of different things rather than a
 * pattern.
 */
const DRIFT: Piece[] = [
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

function DriftingMedia() {
  return (
    <div className="door-media" aria-hidden="true">
      {DRIFT.map((piece, at) => {
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

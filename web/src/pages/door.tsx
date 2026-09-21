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

export function Door({
  branding,
  cameIn,
}: {
  branding: Branding;
  cameIn: (who: Account) => void;
}) {
  const { t, language, setLanguage } = useSettings();
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
      <DriftingPosters />

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

      {/* The language belongs to whoever is reading, before this server knows
          who that is: somebody who cannot read the door cannot get through it
          to change it. */}
      <div className="door-languages">
        {(["en", "fr"] as const).map((one) => (
          <button
            key={one}
            type="button"
            className={`door-language${language === one ? " door-language-on" : ""}`}
            onClick={() => setLanguage(one)}
          >
            {one === "en" ? "English" : "Français"}
          </button>
        ))}
      </div>
    </main>
  );
}

/**
 * One poster, as the background drifts it.
 *
 * Nothing but where it sits, how big it is, how far round it is turned, and
 * how long it takes to rise and fall. The drawing itself is in the
 * stylesheet, because every one of them is drawn the same way.
 */
interface Drifting {
  /** Across and down, as a share of the screen. */
  x: string;
  y: string;
  /** How wide, which is also how near it reads as being. */
  wide: number;
  turn: string;
  /** How long one rise and fall takes. */
  over: string;
  /** Where in that it starts, so no two of them move together. Negative, so
   *  none of them is waiting to begin when the screen arrives. */
  from: string;
  /** Left out on a narrow screen, where they would sit under the card. */
  onlyWide?: boolean;
}

/**
 * Where each one sits.
 *
 * Down both sides and never in the middle, which is where the card is: a
 * poster behind the box somebody is typing into is a poster in the way. The
 * four marked as wide only are the ones that fill the gap on a big screen and
 * would land on the card on a small one.
 *
 * Nearer ones are bigger and are drawn a little stronger, further ones smaller
 * and fainter, which is the whole of the depth here and costs nothing.
 */
const POSTERS: Drifting[] = [
  { x: "7%", y: "12%", wide: 104, turn: "-9deg", over: "13s", from: "-1s" },
  { x: "4%", y: "52%", wide: 132, turn: "6deg", over: "17s", from: "-6s" },
  { x: "11%", y: "78%", wide: 88, turn: "-4deg", over: "15s", from: "-3s" },
  { x: "83%", y: "9%", wide: 96, turn: "8deg", over: "16s", from: "-9s" },
  { x: "88%", y: "46%", wide: 124, turn: "-7deg", over: "14s", from: "-4s" },
  { x: "79%", y: "80%", wide: 80, turn: "11deg", over: "18s", from: "-12s" },
  { x: "22%", y: "24%", wide: 72, turn: "5deg", over: "19s", from: "-7s", onlyWide: true },
  { x: "19%", y: "68%", wide: 92, turn: "-12deg", over: "12s", from: "-2s", onlyWide: true },
  { x: "70%", y: "22%", wide: 84, turn: "-6deg", over: "20s", from: "-15s", onlyWide: true },
  { x: "73%", y: "66%", wide: 68, turn: "9deg", over: "16s", from: "-10s", onlyWide: true },
];

/**
 * Posters drifting behind the door.
 *
 * What this server is for, said without a word, on the one screen that has
 * nothing of the library on it yet.
 *
 * Two things learnt from doing this before and worth not learning again. The
 * movement is a turn and a rise and nothing else: a transform is handed to the
 * card that composes the page and costs nothing per frame, where a shadow or a
 * colour that moves is drawn again every frame. And none of them is given a
 * blur: a blurred thing that moves makes the screen behind it be blurred again
 * sixty times a second, which is how an idle page comes to hold a graphics
 * card at full tilt.
 *
 * They stand still for anybody who asked their system for less movement, which
 * the theme sees to for every animation at once.
 */
function DriftingPosters() {
  return (
    <div className="door-posters" aria-hidden="true">
      {POSTERS.map((poster) => (
        <span
          key={`${poster.x} ${poster.y}`}
          className={`door-poster${poster.onlyWide ? " door-poster-roomy" : ""}`}
          style={
            {
              "--at-x": poster.x,
              "--at-y": poster.y,
              "--wide": `${poster.wide}px`,
              "--turn": poster.turn,
              "--over": poster.over,
              "--from": poster.from,
            } as React.CSSProperties
          }
        />
      ))}
    </div>
  );
}

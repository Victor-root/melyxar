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

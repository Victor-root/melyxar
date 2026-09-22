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
 *
 * And it opens on the accounts rather than on the form. A household signs in
 * by pressing a face and typing a password, so the name field is not the first
 * thing on the screen; it is behind one button, for whoever asked to be left
 * off the list and for whoever would rather type. A server that offers no
 * names has nothing to open on and shows the form straight away.
 *
 * Nothing of Melyxar is written into this screen. The mark, the name and what
 * is behind the card all come from what the server says it wears, and what
 * this project ships with is the fallback rather than the rule: an
 * administrator is meant to be able to replace the lot without a line of this
 * file changing.
 */

import { useEffect, useRef, useState } from "react";
import type { Account, Branding } from "../api";
import { DoorBackground } from "../components/door-background";
import {
  AccountAddIcon,
  AccountIcon,
  BackIcon,
  EnterIcon,
  EyeIcon,
  EyeOffIcon,
  LockIcon,
} from "../icons";
import { useDoorScreen } from "../screens/door";
import { useSettings } from "../settings";
import type { ThemeChoice } from "../settings";

/** The letter a name is shown by, where there is no picture of anybody yet.
 *
 *  Taken with the whole of what the browser calls a character, so a name
 *  starting with an accented letter or with something outside the alphabet is
 *  not cut in half. */
function firstLetterOf(name: string): string {
  return [...name][0]?.toUpperCase() ?? "?";
}

/** The mark this project ships with, worn by a server that was given none. */
const THE_MARK_WE_SHIP = "/melyxar-512.png";

/**
 * What the card is showing.
 *
 * The accounts on this server, or the form. On the form, the name is either
 * already settled, because a card was pressed, or still to be typed.
 */
type Showing = { the: "accounts" } | { the: "form"; forWhom: string | null };

/**
 * The most account cards the card puts across before wrapping to a second row.
 *
 * It is what the card's own width is worked out from, so it is a count rather
 * than a width: four of them and the card comes out a little wider than it
 * stands at on its own, which is as wide as a sign in card should ever get.
 * A fifth account starts a second row instead of widening the screen further.
 */
const MOST_ACROSS = 4;

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
  /* Ticked to begin with: a media server is mostly met on somebody's own
     machine, and asking a household to sign in every morning to protect them
     from a case that is not theirs is the wrong way round. */
  const [remember, setRemember] = useState(true);
  const [shown, setShown] = useState(false);
  /* Said here rather than by the server: the two boxes never leave this
     screen, so there is nothing to ask about. */
  const [twiceDiffers, setTwiceDiffers] = useState(false);
  const first = useRef<HTMLInputElement>(null);
  const passwordField = useRef<HTMLInputElement>(null);

  /* Nothing until something is pressed. Until then the card shows the accounts
     when the server offers any and the form when it does not, which is the
     only way a list that arrives a moment after the screen can be the thing
     the screen opens on. */
  const [turnedTo, setTurnedTo] = useState<Showing | null>(null);
  const showing: Showing =
    turnedTo ??
    (door.names.length > 0
      ? { the: "accounts" }
      : { the: "form", forWhom: null });
  const onTheForm = showing.the === "form";
  const forWhom = showing.the === "form" ? showing.forWhom : null;

  // The cursor where the next word goes, every time the card turns to the
  // form: the name for somebody typing their own, the password for somebody
  // whose name was settled by the card they pressed.
  useEffect(() => {
    if (!onTheForm) {
      return;
    }
    (forWhom === null ? first : passwordField).current?.focus();
  }, [onTheForm, forWhom]);

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
    void door.knock(name.trim(), password, remember);
  };

  // Typing again is somebody answering the refusal, so it goes.
  const typing = <T,>(set: (value: T) => void) => (value: T) => {
    setTwiceDiffers(false);
    door.forget();
    set(value);
  };

  /* Turning the card empties it: a password half typed for one account and a
     refusal earned by another both belong to the view being left. The name
     comes from where the card is being turned to, so there is one place it is
     ever decided. */
  const turnTo = (next: Showing) => {
    setTwiceDiffers(false);
    door.forget();
    setName(next.the === "form" ? (next.forWhom ?? "") : "");
    setPassword("");
    setTurnedTo(next);
  };

  return (
    <main className="door">
      <DoorBackground branding={branding} />

      {/* The frame around the card: the halo behind it and the card itself,
          kept in one element so the glow stays exactly the card's size and
          shape without either one having to know the other's dimensions. */}
      <div className="door-card-frame">
        {/* The colour is the theme's accent, wherever an administrator has
            set it: nothing here is a colour of its own. */}
        <div className="door-halo" aria-hidden="true" />

        {/* How many cards go across is what this card is as wide as, so the
            count is handed to the drawing and the arithmetic stays in it.
            The width is the same on both views: a card that shrank the
            moment somebody pressed a face would be a screen that moves under
            a hand already going for the password. */}
        <form
          className="door-card"
          style={
            {
              "--across": Math.min(door.names.length, MOST_ACROSS),
            } as React.CSSProperties
          }
          onSubmit={send}
        >
          {/* Only ever on the form, and only when there is a list to go back
              to: on a server that offers no names it would go nowhere. */}
          {onTheForm && door.names.length > 0 && (
            <button
              type="button"
              className="door-back"
              onClick={() => turnTo({ the: "accounts" })}
            >
              <BackIcon size={16} />
              {t("door.back")}
            </button>
          )}

          <div className="door-crown">
            <img
              className="door-mark"
              src={branding.logo_path ?? THE_MARK_WE_SHIP}
              alt=""
              aria-hidden="true"
            />
            <h1 className="door-name">{branding.server_name}</h1>
          </div>

          {/* Nothing here once a card has been pressed: the face and the name
              under the mark say who this is for, and a line saying it again in
              words is one the eye reads past on the way to the password. */}
          {forWhom === null && (
            <p className="door-invitation">
              {showing.the === "accounts"
                ? t("door.pick")
                : t(door.brandNew ? "door.first.invitation" : "door.invitation")}
            </p>
          )}

          {/* Who is on this server, for whoever is not typing their own name for
              the thousandth time. Pressing one settles the name and turns the
              card to the password, which is the whole of the first half of
              signing in done in one press. */}
          {showing.the === "accounts" && (
            <>
              <div className="door-who">
                {door.names.map((offered) => (
                  <button
                    type="button"
                    key={offered}
                    className="door-who-one"
                    onClick={() => turnTo({ the: "form", forWhom: offered })}
                    /* A name too long for the card is cut on it, so the whole
                       of it has to be readable from somewhere. */
                    title={offered}
                  >
                    <span className="door-face door-who-face" aria-hidden="true">
                      {firstLetterOf(offered)}
                    </span>
                    <span className="door-who-name">{offered}</span>
                  </button>
                ))}
              </div>

              {/* The way in that is not on the list: an account that asked to be
                  left off it signs in by typing its name, and so does anybody
                  who would rather. */}
              <button
                type="button"
                className="door-by-hand"
                onClick={() => turnTo({ the: "form", forWhom: null })}
              >
                <AccountIcon size={18} />
                {t("door.by_hand")}
              </button>
            </>
          )}

          {/* Whose password is being asked for, once a card has settled it. The
              name itself is held rather than drawn as a field, so the one thing
              left on the card is the one thing left to do. */}
          {forWhom !== null && (
            <div className="door-forwhom">
              <span className="door-face door-forwhom-face" aria-hidden="true">
                {firstLetterOf(forWhom)}
              </span>
              <span className="door-forwhom-name">{forWhom}</span>
              {/* Out of sight and genuinely there: a password keeper fills the
                  entry it recognises by the name beside it, and a card with no
                  name on it anywhere offers nothing to recognise. */}
              <input
                className="door-kept-name"
                type="text"
                value={forWhom}
                readOnly
                tabIndex={-1}
                aria-hidden="true"
                autoComplete="username"
              />
            </div>
          )}

          {onTheForm && (
            <>
              {forWhom === null && (
                <>
                  <label className="door-label" htmlFor="door-name">
                    {t("door.name")}
                  </label>
                  <div className="door-box">
                    <AccountIcon className="door-box-mark" size={19} />
                    <input
                      id="door-name"
                      ref={first}
                      className="door-field"
                      value={name}
                      onChange={(event) => typing(setName)(event.target.value)}
                      placeholder={t("door.name")}
                      autoComplete="username"
                      autoCapitalize="off"
                      autoCorrect="off"
                      spellCheck={false}
                      required
                    />
                  </div>
                </>
              )}

              <label className="door-label" htmlFor="door-password">
                {t("door.password")}
              </label>
              <div className="door-box">
                <LockIcon className="door-box-mark" size={19} />
                <input
                  id="door-password"
                  ref={passwordField}
                  className="door-field"
                  type={shown ? "text" : "password"}
                  value={password}
                  onChange={(event) => typing(setPassword)(event.target.value)}
                  placeholder={t("door.password")}
                  autoComplete={
                    door.brandNew ? "new-password" : "current-password"
                  }
                  required
                />
                {/* Reading back what was typed is the one way out of a password
                    mistyped behind dots, and on a door that holds somebody's
                    whole library it is worth the moment it is readable for. */}
                <button
                  type="button"
                  className="door-reveal"
                  onClick={() => setShown(!shown)}
                  aria-label={t(
                    shown ? "door.hide_password" : "door.show_password",
                  )}
                  aria-pressed={shown}
                  title={t(shown ? "door.hide_password" : "door.show_password")}
                >
                  {shown ? <EyeOffIcon size={19} /> : <EyeIcon size={19} />}
                </button>
              </div>

              {door.brandNew && (
                <>
                  <label className="door-label" htmlFor="door-again">
                    {t("door.password_again")}
                  </label>
                  <div className="door-box">
                    <LockIcon className="door-box-mark" size={19} />
                    <input
                      id="door-again"
                      className="door-field"
                      type={shown ? "text" : "password"}
                      value={again}
                      onChange={(event) => typing(setAgain)(event.target.value)}
                      placeholder={t("door.password_again")}
                      autoComplete="new-password"
                      required
                    />
                  </div>
                  {/* Deliberately without the number: the shortest a password
                      may be is the server's rule, and it travels with the
                      refusal that names it. Written here as well, the two would
                      drift apart and the wrong one would be the one on
                      screen. */}
                  <p className="door-rule">{t("door.rule")}</p>
                </>
              )}

              <div className="door-aside">
                {/* Whether the browser holds on to the session once it is
                    closed. Nothing else about it changes: a session lasts
                    exactly as long either way, and what this asks is whether
                    the machine is one to be left signed in. */}
                <label className="door-remember">
                  <input
                    type="checkbox"
                    checked={remember}
                    onChange={(event) => setRemember(event.target.checked)}
                  />
                  {t("door.remember")}
                </label>

                {/* Nothing behind it yet: there is no way to put a password back
                    from this screen, and the one that exists is a command on the
                    server itself. Drawn all the same, and turned off until there
                    is something for it to do. */}
                {!door.brandNew && (
                  <button
                    type="button"
                    className="door-forgot"
                    disabled
                    title={t("door.forgot_why")}
                  >
                    {t("door.forgot")}
                  </button>
                )}
              </div>

              {/* The refusal takes the place under the fields rather than
                  appearing between them: a message that pushes the button down
                  as it arrives is a message somebody clicks through by
                  accident. */}
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
                <EnterIcon size={20} />
                {t(
                  door.asking
                    ? "door.asking"
                    : door.brandNew
                      ? "door.first.go"
                      : "door.go",
                )}
              </button>

              {/* On a brand new server the button above is the one that makes an
                  account, so a second one offering the same thing would be two
                  doors into one room. */}
              {!door.brandNew && (
                <>
                  <div className="door-or">
                    <span>{t("door.or")}</span>
                  </div>
                  {/* Nothing behind this one either: accounts on this server are
                      made by whoever runs it. Turned off rather than left out,
                      so the day the screen for it exists there is a place for
                      it. */}
                  <button
                    type="button"
                    className="button button-large door-make"
                    disabled
                    title={t("door.create_why")}
                  >
                    <AccountAddIcon size={20} />
                    {t("door.create")}
                  </button>
                </>
              )}
            </>
          )}

          {/* What this server is for, at the card's own foot rather than
              floating above it: the last thing read on this screen, once
              there is nothing left to press. */}
          <p className="door-slogan">{t("door.slogan")}</p>
        </form>
      </div>

      {/* The language is read from the browser and never asked about here:
          somebody who cannot read the door cannot get through it to change
          it, so there is nothing to gain by offering a choice before one is
          needed. The theme is the one door setting a viewer can still turn,
          since automatic does not always land on the one their eyes want. */}
      <div className="door-foot">
        <button
          type="button"
          className="door-theme"
          onClick={() => setTheme(nextTheme(theme))}
        >
          <ThemeIcon theme={theme} />
          {t("door.theme_toggle")}
        </button>

        <p className="door-footer">
          {t("door.footer_before")}
          <a
            href="https://github.com/Victor-root"
            target="_blank"
            rel="noopener noreferrer"
            className="door-footer-link"
          >
            Victor-root
          </a>
          {t("door.footer_after")}
        </p>
      </div>
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

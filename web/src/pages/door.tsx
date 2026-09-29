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
import { Face } from "../components/face";
import { LanguagePicker } from "../components/language-picker";
import { ThemeToggle } from "../components/theme-toggle";
import { useDoorScreen } from "../screens/door";
import { useSettings } from "../settings";
import { ServerMark } from "../components/server-mark";

/** The letter a name is shown by, where it has no picture.
 *
 *  Taken with the whole of what the browser calls a character, so a name
 *  starting with an accented letter or with something outside the alphabet is
 *  not cut in half. */
function firstLetterOf(name: string): string {
  return [...name][0]?.toUpperCase() ?? "?";
}

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
  const { t, languageChoice } = useSettings();
  const door = useDoorScreen(branding, cameIn, languageChoice);

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

      {/* How many cards go across is what this card is as wide as, so the
          count is handed to the drawing and the arithmetic stays in it. The
          width is the same on both views: a card that shrank the moment
          somebody pressed a face would be a screen that moves under a hand
          already going for the password. */}
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
          {/* A mark of the administrator's own stays in its own colours; the
              one Melyxar ships takes the accent, like the rest of the page. */}
          <ServerMark branding={branding} size={58} className="door-mark" />
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
                  key={offered.name}
                  className="door-who-one"
                  onClick={() => turnTo({ the: "form", forWhom: offered.name })}
                  /* A name too long for the card is cut on it, so the whole
                     of it has to be readable from somewhere. */
                  title={offered.name}
                >
                  <Face
                    className="door-face door-who-face"
                    name={offered.name}
                    avatar={offered.avatar}
                    letters={firstLetterOf(offered.name)}
                  />
                  <span className="door-who-name">{offered.name}</span>
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
            <Face
              className="door-face door-forwhom-face"
              name={forWhom}
              avatar={door.names.find((offered) => offered.name === forWhom)?.avatar ?? null}
              letters={firstLetterOf(forWhom)}
            />
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
        <p className="door-slogan">{branding.door_slogan ?? t("door.slogan")}</p>
      </form>

      {/* The two door settings a viewer can turn before anybody knows them:
          the language, the browser's to begin with, and the theme, since
          automatic does not always land on the one either wants. On a brand
          new server the language chosen here is the first account's. */}
      <div className="door-foot">
        <LanguagePicker />
        <ThemeToggle />

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

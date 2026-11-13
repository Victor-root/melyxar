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
 * is behind the accounts all come from what the server says it wears, and what
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
  DeviceIcon,
  EnterIcon,
  EyeIcon,
  EyeOffIcon,
  LockIcon,
  PasskeyIcon,
  ServerIcon,
} from "../icons";
import { Face } from "../components/face";
import { LanguagePicker } from "../components/language-picker";
import { ThemeToggle } from "../components/theme-toggle";
import { appServers } from "../app-servers";
import { useDoorScreen } from "../screens/door";
import { useSettings } from "../settings";
import { ServerMark } from "../components/server-mark";
import { useBackClosesIt } from "../components/back-closes";
import { LinkAtTheDoor } from "./door-link";
import { columnsOfFaces } from "../door-columns";
import { COARSE, useMediaQuery } from "../media-query";

/**
 * What the screen is showing.
 *
 * The accounts on this server, the form, or a code to be let in by. On the form, the name is either
 * already settled, because a tile was pressed, or still to be typed.
 */
type Showing =
  | { the: "accounts" }
  | { the: "form"; forWhom: string | null }
  /* A code shown, for an account signed in elsewhere to let this browser in. */
  | { the: "telephone" }
  /* The form that makes an account, where the server lets anybody make one. */
  | { the: "sign_up" };

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

  /* Nothing until something is pressed. Until then the screen shows the accounts
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
  const signingUp = showing.the === "sign_up";
  /* The fields, for signing in or for making an account. */
  const withFields = (onTheForm || signingUp) && !door.awaitingApproval;
  /* An account being made, which takes its password twice: a typo there is an
     account nobody can get into. */
  const making = door.brandNew || signingUp;
  const forWhom = showing.the === "form" ? showing.forWhom : null;
  const chosen = door.names.find((offered) => offered.name === forWhom);

  // The cursor where the next word goes, every time the screen turns to the
  // form: the name for somebody typing their own, the password for somebody
  // whose name was settled by the tile they pressed. Not on a touch screen,
  // where it would raise the keyboard before anybody asked to type.
  const touch = useMediaQuery(COARSE);
  useEffect(() => {
    if (!door.asked || !withFields || touch) {
      return;
    }
    (forWhom === null ? first : passwordField).current?.focus();
  }, [door.asked, withFields, forWhom, touch]);

  const refusal = twiceDiffers
    ? { key: "door.refused.not_the_same", values: {} }
    : door.refused;

  const send = (event: React.FormEvent) => {
    event.preventDefault();
    if (making && password !== again) {
      setTwiceDiffers(true);
      return;
    }
    setTwiceDiffers(false);
    void (signingUp ? door.signUp : door.knock)(name.trim(), password, remember);
  };

  // Typing again is somebody answering the refusal, so it goes.
  const typing = <T,>(set: (value: T) => void) => (value: T) => {
    setTwiceDiffers(false);
    door.forget();
    set(value);
  };

  /* Turning the screen empties it: a password half typed for one account and a
     refusal earned by another both belong to the view being left. The name
     comes from where the screen is being turned to, so there is one place it is
     ever decided. */
  const turnTo = (next: Showing) => {
    setTwiceDiffers(false);
    door.forget();
    setName(next.the === "form" ? (next.forWhom ?? "") : "");
    setPassword("");
    setAgain("");
    setTurnedTo(next);
  };

  /* The form for one account is a step on from the accounts, so the back key
     of a phone takes it back to them rather than out of the interface. */
  const backToTheAccounts = () => turnTo({ the: "accounts" });
  const leaveTheForm = useBackClosesIt(backToTheAccounts, onTheForm && door.names.length > 0);
  /* Where a code is shown, the back key goes back to where it was asked from. */
  const onTheTelephone = showing.the === "telephone";
  const [cameFrom, setCameFrom] = useState<Showing>({ the: "accounts" });
  const leaveTheTelephone = useBackClosesIt(() => turnTo(cameFrom), onTheTelephone);
  const toTheTelephone = () => {
    setCameFrom(showing);
    turnTo({ the: "telephone" });
  };
  /* Making an account is a step on from wherever it was asked from too. */
  const leaveTheSignUp = useBackClosesIt(() => turnTo(cameFrom), signingUp);
  const toTheSignUp = () => {
    setCameFrom(showing);
    turnTo({ the: "sign_up" });
  };

  return (
    <main className="door">
      <DoorBackground branding={branding} />
      {/* The light the accounts stand in, the same over every background: it is
          what this screen is built around rather than a background somebody
          picks. */}
      <div className="door-stage" aria-hidden="true" />

      {/* The way back to the accounts, an arrow alone in the corner. Only ever
          on the form, and only when there is a list to go back to: on a server
          that offers no names it would go nowhere. */}
      {onTheForm && door.names.length > 0 && (
        <button
          type="button"
          className="door-back"
          aria-label={t("door.back")}
          title={t("door.back")}
          onClick={() => leaveTheForm(backToTheAccounts)}
        >
          <BackIcon size={20} />
        </button>
      )}
      {signingUp && (
        <button
          type="button"
          className="door-back"
          aria-label={t("door.back")}
          title={t("door.back")}
          onClick={() => leaveTheSignUp(() => turnTo(cameFrom))}
        >
          <BackIcon size={20} />
        </button>
      )}

      {/* No card around any of it: the accounts stand in the open in the middle
          of the screen, and the form takes their place once one is pressed. */}
      {door.asked && (
        <form className="door-hall" onSubmit={send}>
          {/* Nothing here once a tile has been pressed: the face and the name
              say who this is for, and a line saying it again in words is one
              the eye reads past on the way to the password. */}
          {forWhom === null && (
            <div className="door-greeting">
              {/* A mark of the administrator's own stays in its own colours;
                  the one Melyxar ships takes the accent, like the rest of the
                  page. */}
              <ServerMark branding={branding} size={140} className="door-greeting-mark" />
              <h1 className="door-welcome">
                {t("door.welcome_before")}
                <span className="door-welcome-name">{branding.server_name}</span>
                {t("door.welcome_after")}
              </h1>
              <p className="door-invitation">
                {showing.the === "accounts"
                  ? t("door.pick")
                  : showing.the === "telephone"
                    ? t("door.link.invitation")
                    : signingUp
                      ? t(door.awaitingApproval ? "door.sign_up.waiting" : "door.sign_up.invitation")
                      : t(door.brandNew ? "door.first.invitation" : "door.invitation")}
              </p>
            </div>
          )}

          {/* Made, and waiting for an administrator: nothing left to type
              here, only the way back. */}
          {signingUp && door.awaitingApproval && (
            <button
              type="button"
              className="door-by-hand door-by-hand-accent"
              onClick={() => leaveTheSignUp(() => turnTo(cameFrom))}
            >
              <BackIcon size={18} />
              {t("door.back")}
            </button>
          )}

          {/* Who is on this server, for whoever is not typing their own name for
              the thousandth time. Pressing one settles the name and turns the
              screen to the password, which is the whole of the first half of
              signing in done in one press. */}
          {showing.the === "accounts" && (
            <>
              <div
                className="door-who"
                style={
                  {
                    "--door-count": door.names.length,
                    "--door-columns": columnsOfFaces(door.names.length),
                  } as React.CSSProperties
                }
              >
                {door.names.map((offered, at) => (
                  <button
                    type="button"
                    key={offered.name}
                    className="door-who-one"
                    style={{ "--order": at } as React.CSSProperties}
                    onClick={() => turnTo({ the: "form", forWhom: offered.name })}
                    /* A name too long for the tile is cut on it, so the whole
                       of it has to be readable from somewhere. */
                    title={offered.name}
                  >
                    <Face
                      className="door-who-face"
                      avatar={offered.avatar}
                      picture={offered.face_picture}
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
                className="door-by-hand door-by-hand-accent"
                onClick={() => turnTo({ the: "form", forWhom: null })}
              >
                <AccountIcon size={18} />
                {t("door.by_hand")}
              </button>
              {door.byPasskey && (
                <button
                  type="button"
                  className="door-by-hand"
                  disabled={door.asking}
                  onClick={() => void door.knockWithAKey(remember)}
                >
                  <PasskeyIcon size={18} />
                  {t("door.passkey")}
                </button>
              )}
              <button type="button" className="door-by-hand" onClick={toTheTelephone}>
                <DeviceIcon size={18} />
                {t("door.link.go")}
              </button>
              {door.signUpsOpen && (
                <button type="button" className="door-by-hand" onClick={toTheSignUp}>
                  <AccountAddIcon size={18} />
                  {t("door.create")}
                </button>
              )}
              {/* Where a passkey refused on this view says why: there is no
                  form here for the refusal to sit under. */}
              {door.byPasskey && (
                <div className="door-answer" role="alert" aria-live="polite">
                  {door.refused && (
                    <span className="door-refused" key={door.refused.key}>
                      {t(door.refused.key, door.refused.values)}
                    </span>
                  )}
                </div>
              )}
            </>
          )}

          {onTheTelephone && <LinkAtTheDoor cameIn={cameIn} onBack={() => leaveTheTelephone(() => turnTo(cameFrom))} />}

          {/* Whose password is being asked for, once a tile has settled it. The
              name itself is held rather than drawn as a field, so the one thing
              left on the screen is the one thing left to do. */}
          {forWhom !== null && (
            <div className="door-forwhom">
              <Face
                className="door-forwhom-face"
                avatar={chosen?.avatar ?? null}
                picture={chosen?.face_picture ?? null}
              />
              <span className="door-forwhom-name">{forWhom}</span>
              {/* Out of sight and genuinely there: a password keeper fills the
                  entry it recognises by the name beside it, and a screen with
                  no name on it anywhere offers nothing to recognise. */}
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

          {withFields && (
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
                    making ? "new-password" : "current-password"
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

              {making && (
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
                {!making && (
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
                      : signingUp
                        ? "door.sign_up.go"
                        : "door.go",
                )}
              </button>

              {/* Where an account is being made, the button above is the one
                  that makes it, so a second one offering the same thing would
                  be two doors into one room. Making one's own account is only
                  offered where the administrator lets anybody. */}
              {!making && (
                <>
                  <div className="door-or">
                    <span>{t("door.or")}</span>
                  </div>
                  {door.byPasskey && (
                    <button
                      type="button"
                      className="button button-large door-make"
                      disabled={door.asking}
                      onClick={() => void door.knockWithAKey(remember)}
                    >
                      <PasskeyIcon size={20} />
                      {t("door.passkey")}
                    </button>
                  )}
                  <button type="button" className="button button-large door-make" onClick={toTheTelephone}>
                    <DeviceIcon size={20} />
                    {t("door.link.go")}
                  </button>
                  {door.signUpsOpen && (
                    <button type="button" className="button button-large door-make" onClick={toTheSignUp}>
                      <AccountAddIcon size={20} />
                      {t("door.create")}
                    </button>
                  )}
                </>
              )}
            </>
          )}
        </form>
      )}

      {/* The two door settings a viewer can turn before anybody knows them,
          on one side: the language, the browser's to begin with, and the
          theme, since automatic does not always land on the one either wants.
          On a brand new server the language chosen here is the first
          account's. What this server is for, and whose project it is, on the
          other, the last things read on this screen. */}
      <footer className="door-foot">
        <div className="door-settings">
          <LanguagePicker quiet />
          <ThemeToggle />
          {appServers && (
            <button type="button" className="theme-toggle" onClick={() => appServers?.choose()}>
              <ServerIcon size={16} className="theme-toggle-icon" />
              {t("nav.change_server")}
            </button>
          )}
        </div>

        <div className="door-credits">
          <p className="door-slogan">{branding.door_slogan ?? t("door.slogan")}</p>
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
      </footer>
    </main>
  );
}

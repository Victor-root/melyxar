/*
 * Who this account is: the picture it wears, its name, its password, whether
 * the door offers its name, and the devices it is signed in on.
 */

import { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import type { Account, SignedInDevice } from "../../api";
import { useAccount } from "../../account";
import { refusalAbout, useAsked, useTold } from "../../asking";
import type { Asked } from "../../asking";
import { Cropper } from "../../components/cropper";
import { DeviceLines } from "../../components/device-lines";
import { Face } from "../../components/face";
import { PageHead, Panel, Setting, Toggle } from "../../components/panel";
import { AccountIcon, DeviceIcon, EnterIcon, LockIcon, ProfileIcon } from "../../icons";
import { useJournalNews } from "../../live";
import { usePreferences } from "../../screens/settings";
import { useSettings } from "../../settings";

/** What the server answers a request carrying more than it takes. */
const TOO_LARGE = 413;

/** What the server answers when the current password is not the one typed. */
const NOT_THE_CURRENT_ONE = 401;

/** How often one's own devices are looked at again, for a sign in made on
 *  another one while this page stays open. */
const DEVICES_LOOKED_AT_EVERY_MS = 30_000;

export function MyProfile() {
  const { t } = useSettings();
  const preferences = usePreferences();
  const devices = useMyDevices();

  return (
    <>
      <PageHead lead={t("me.profile_lead")} />
      <ProfilePicture />
      <div className="panels">
        <Name />
        <Password onChanged={devices.look} />
        {preferences.kept && (
          <Panel icon={EnterIcon} title={t("settings.door")} lead={t("settings.door_why")}>
            <Setting label={t("settings.door_hide_me")} why={t("settings.door_hide_me_why")}>
              <Toggle
                label={t("settings.door_hide_me")}
                checked={preferences.kept.hidden_at_the_door}
                onChange={(hidden_at_the_door) => preferences.change({ hidden_at_the_door })}
              />
            </Setting>
          </Panel>
        )}
      </div>
      <MyDevices devices={devices} />
    </>
  );
}

/**
 * The picture this account wears, in the header and on the sign in screen.
 *
 * Chosen from the files of the device in hand, then framed by hand: pulled
 * around and brought closer inside the square it will be shown in.
 */
function ProfilePicture() {
  const { t } = useSettings();
  const { account, cameIn } = useAccount();
  const chooser = useRef<HTMLInputElement>(null);
  const [framing, setFraming] = useState<File | null>(null);
  // The account answered is the one every screen then draws from.
  const told = useTold(async (change: () => Promise<Account>) => cameIn(await change()));

  if (!account) {
    return null;
  }
  const refused = told.failure;

  return (
    <Panel icon={ProfileIcon} title={t("settings.avatar")} lead={t("settings.avatar_why")}>
      <div className="profile-card">
        <Face className="avatar profile-face" name={account.name} avatar={account.avatar} />
        <div className="profile-words">
          <span className="profile-name">{account.name}</span>
          <span className="line-note">
            {t(account.is_administrator ? "me.role_admin" : "me.role_viewer")}
          </span>
          <div className="profile-actions">
            <input
              ref={chooser}
              type="file"
              accept="image/jpeg,image/png,image/webp,image/gif"
              hidden
              onChange={(event) => {
                const image = event.target.files?.[0];
                // Emptied, so choosing the same file again is still a choice.
                event.target.value = "";
                if (image) {
                  setFraming(image);
                }
              }}
            />
            <button
              className="button button-small button-accent"
              disabled={told.busy}
              onClick={() => chooser.current?.click()}
            >
              {t(account.avatar ? "settings.avatar_change" : "settings.avatar_choose")}
            </button>
            {account.avatar && (
              <button
                className="button button-small"
                disabled={told.busy}
                onClick={() => told.tell(api.removeAvatar)}
              >
                {t("settings.avatar_remove")}
              </button>
            )}
            {told.busy && <span className="line-note">{t("settings.avatar_busy")}</span>}
          </div>
        </div>
      </div>
      {refused && (
        <p className="panel-notice panel-notice-trouble">
          {t(refused.status === TOO_LARGE ? "refused.avatar.too_large" : refusalAbout(refused, "avatar"))}
        </p>
      )}
      {framing && (
        <Cropper
          image={framing}
          onClose={() => setFraming(null)}
          onFramed={(square) => {
            setFraming(null);
            told.tell(() => api.setAvatar(square));
          }}
        />
      )}
    </Panel>
  );
}

/**
 * The name this account signs in with. Every device stays signed in: the name
 * guards nothing, and everything of the account hangs off it by something
 * other than its name.
 */
function Name() {
  const { t } = useSettings();
  const { account, cameIn } = useAccount();
  /* What is being typed, and nothing while the field shows the name as it is,
     so a name changed from here is the one the field goes back to. */
  const [wanted, setWanted] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const told = useTold(async (name: string) => {
    cameIn(await api.rename(name));
    setWanted(null);
    setDone(true);
  });

  if (!account) {
    return null;
  }
  const shown = wanted ?? account.name;
  const refused = told.failure;

  return (
    <Panel icon={AccountIcon} title={t("me.name")} lead={t("me.name_lead")}>
      <form
        className="account-form"
        onSubmit={(event) => {
          event.preventDefault();
          setDone(false);
          void told.tell(shown);
        }}
      >
        <input
          type="text"
          className="field-line"
          autoComplete="username"
          aria-label={t("me.name")}
          value={shown}
          onChange={(event) => {
            setDone(false);
            setWanted(event.target.value);
          }}
        />
        {refused && (
          <p className="panel-notice panel-notice-trouble">{t(refusalAbout(refused, "account"))}</p>
        )}
        {done && <p className="panel-notice panel-notice-ok">{t("me.name_changed")}</p>}
        <div className="panel-foot">
          <button
            type="submit"
            className="button button-accent"
            disabled={told.busy || shown.trim() === "" || shown.trim() === account.name}
          >
            {t("me.name_change")}
          </button>
        </div>
      </form>
    </Panel>
  );
}

/**
 * Changing the password, which signs every other device out: somebody changes
 * it because they think somebody else knows it.
 */
function Password({ onChanged }: { onChanged: () => void }) {
  const { t } = useSettings();
  const { cameIn } = useAccount();
  const [current, setCurrent] = useState("");
  const [wanted, setWanted] = useState("");
  const [again, setAgain] = useState("");
  const [done, setDone] = useState(false);
  const told = useTold(async () => {
    cameIn(await api.changePassword(current, wanted));
    onChanged();
    setCurrent("");
    setWanted("");
    setAgain("");
    setDone(true);
  });

  const differ = again !== "" && again !== wanted;
  const refused = told.failure;

  return (
    <Panel icon={LockIcon} title={t("me.password")} lead={t("me.password_lead")}>
      <form
        className="account-form"
        onSubmit={(event) => {
          event.preventDefault();
          setDone(false);
          void told.tell();
        }}
      >
        <input
          type="password"
          className="field-line"
          autoComplete="current-password"
          placeholder={t("me.password_current")}
          aria-label={t("me.password_current")}
          value={current}
          onChange={(event) => setCurrent(event.target.value)}
        />
        <input
          type="password"
          className="field-line"
          autoComplete="new-password"
          placeholder={t("me.password_new")}
          aria-label={t("me.password_new")}
          value={wanted}
          onChange={(event) => setWanted(event.target.value)}
        />
        <input
          type="password"
          className="field-line"
          autoComplete="new-password"
          placeholder={t("me.password_again")}
          aria-label={t("me.password_again")}
          value={again}
          onChange={(event) => setAgain(event.target.value)}
        />
        {differ && <p className="panel-notice">{t("door.refused.not_the_same")}</p>}
        {refused && (
          <p className="panel-notice panel-notice-trouble">
            {t(
              refused.status === NOT_THE_CURRENT_ONE
                ? "me.password_wrong"
                : refusalAbout(refused, "account"),
            )}
          </p>
        )}
        {done && <p className="panel-notice panel-notice-ok">{t("me.password_changed")}</p>}
        <div className="panel-foot">
          <button
            type="submit"
            className="button button-accent"
            disabled={told.busy || !current || !wanted || wanted !== again}
          >
            {t("me.password_change")}
          </button>
        </div>
      </form>
    </Panel>
  );
}

/**
 * The devices this account is signed in on.
 *
 * Looked at again on a beat, since nobody but an administrator hears the
 * journal as it is written, and at once for one who does.
 */
function useMyDevices(): Asked<SignedInDevice[]> {
  const devices = useAsked((signal) => api.myDevices(signal));
  const { look } = devices;
  useJournalNews(look);
  useEffect(() => {
    const beat = window.setInterval(look, DEVICES_LOOKED_AT_EVERY_MS);
    return () => window.clearInterval(beat);
  }, [look]);
  return devices;
}

/** Each device signed out alone: a phone lost, a computer lent, without
 *  asking anybody. */
function MyDevices({ devices }: { devices: Asked<SignedInDevice[]> }) {
  const { t } = useSettings();
  return (
    <Panel icon={DeviceIcon} title={t("me.devices")} lead={t("me.devices_lead")}>
      {devices.failure && <p className="panel-notice panel-notice-trouble">{t("error.unreachable")}</p>}
      {devices.answer && (
        <DeviceLines
          devices={devices.answer}
          withAccount={false}
          signOut={api.signOutMyDevice}
          onSignedOut={devices.look}
        />
      )}
    </Panel>
  );
}

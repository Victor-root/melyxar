/*
 * Who this account is: the picture it wears, its password, and whether the
 * door offers its name.
 */

import { useRef, useState } from "react";
import { api } from "../../api";
import type { Account } from "../../api";
import { useAccount } from "../../account";
import { refusalAbout, useTold } from "../../asking";
import { Cropper } from "../../components/cropper";
import { Face } from "../../components/face";
import { PageHead, Panel, Setting, Toggle } from "../../components/panel";
import { EnterIcon, LockIcon, ProfileIcon } from "../../icons";
import { usePreferences } from "../../screens/settings";
import { useSettings } from "../../settings";

/** What the server answers a request carrying more than it takes. */
const TOO_LARGE = 413;

/** What the server answers when the current password is not the one typed. */
const NOT_THE_CURRENT_ONE = 401;

export function MyProfile() {
  const { t } = useSettings();
  const preferences = usePreferences();

  return (
    <>
      <PageHead lead={t("me.profile_lead")} />
      <ProfilePicture />
      <div className="panels">
        <Password />
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
 * Changing the password, which signs every other device out: somebody changes
 * it because they think somebody else knows it.
 */
function Password() {
  const { t } = useSettings();
  const { cameIn } = useAccount();
  const [current, setCurrent] = useState("");
  const [wanted, setWanted] = useState("");
  const [again, setAgain] = useState("");
  const [done, setDone] = useState(false);
  const told = useTold(async () => {
    cameIn(await api.changePassword(current, wanted));
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
        className="password-form"
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

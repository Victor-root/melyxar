/*
 * The passkeys of this account: made here with its password, then used at
 * the door in place of a name and a password, renamed or taken away.
 */

import { useState } from "react";
import { api } from "../../api";
import type { ApiError, Passkey } from "../../api";
import { figuresOf, heldBackFor, refusalAbout, useTold } from "../../asking";
import type { Asked } from "../../asking";
import { Panel } from "../../components/panel";
import { deviceName } from "../../devices";
import { PasskeyIcon } from "../../icons";
import { DeviceSaidNo, makeAPasskey, passkeysHere } from "../../passkeys";
import type { DeviceRefusal } from "../../passkeys";
import { readableDate } from "../../readable";
import type { Wording } from "../../readable";
import { useSettings } from "../../settings";

/** Asked for by the profile, which looks again once a new password has
 *  taken them all away. */
export function Passkeys({ kept }: { kept: Asked<Passkey[]> }) {
  const { t } = useSettings();
  const here = passkeysHere();
  const [adding, setAdding] = useState(false);

  return (
    <Panel
      icon={PasskeyIcon}
      title={t("me.passkeys")}
      lead={t("me.passkeys_lead")}
      action={
        here &&
        !adding && (
          <button className="button button-small" onClick={() => setAdding(true)}>
            {t("me.passkey_add")}
          </button>
        )
      }
    >
      {!here && <p className="panel-notice">{t("refused.passkey.not_a_site")}</p>}
      {kept.failure && <p className="panel-notice panel-notice-trouble">{t("error.unreachable")}</p>}
      {adding && (
        <NewPasskey
          onDone={() => {
            setAdding(false);
            kept.look();
          }}
          onCancel={() => setAdding(false)}
        />
      )}
      {kept.answer && <PasskeyLines passkeys={kept.answer} onChanged={kept.look} />}
    </Panel>
  );
}

/** The password, and the name the new key goes by, before the device is
 *  asked to make it. */
function NewPasskey({ onDone, onCancel }: { onDone: () => void; onCancel: () => void }) {
  const { t } = useSettings();
  const [name, setName] = useState(() => deviceName(navigator.userAgent, t));
  const [password, setPassword] = useState("");
  const [device, setDevice] = useState<DeviceRefusal | null>(null);
  const told = useTold(async () => {
    setDevice(null);
    try {
      await makeAPasskey(password, name.trim());
    } catch (error) {
      if (error instanceof DeviceSaidNo) {
        setDevice(error.why);
        return;
      }
      throw error;
    }
    onDone();
  });

  return (
    <form
      className="account-form"
      onSubmit={(event) => {
        event.preventDefault();
        void told.tell();
      }}
    >
      <input
        type="text"
        className="field-line"
        autoComplete="off"
        placeholder={t("me.passkey_name")}
        aria-label={t("me.passkey_name")}
        value={name}
        onChange={(event) => setName(event.target.value)}
      />
      <input
        type="password"
        className="field-line"
        autoComplete="current-password"
        placeholder={t("me.password_current")}
        aria-label={t("me.password_current")}
        value={password}
        onChange={(event) => setPassword(event.target.value)}
      />
      {device && device !== "cancelled" && (
        <p className="panel-notice panel-notice-trouble">
          {t(device === "already_here" ? "me.passkey_already_here" : "door.refused.passkey_failed")}
        </p>
      )}
      {told.failure && <p className="panel-notice panel-notice-trouble">{passkeyRefusal(told.failure, t)}</p>}
      <div className="panel-foot">
        <button type="button" className="button" onClick={onCancel}>
          {t("settings.cancel")}
        </button>
        <button
          type="submit"
          className="button button-accent"
          disabled={told.busy || name.trim() === "" || password === ""}
        >
          {t("me.passkey_make")}
        </button>
      </div>
    </form>
  );
}

/** Why a passkey was not made or renamed, as the panel says it. */
function passkeyRefusal(refused: ApiError, t: Wording): string {
  const seconds = heldBackFor(refused);
  if (seconds !== null) {
    return t("door.refused.held_back", { seconds });
  }
  return t(refusalAbout(refused, "passkey"), figuresOf(refused));
}

/** One line a key, renamed in place or taken away after a second press. */
function PasskeyLines({ passkeys, onChanged }: { passkeys: Passkey[]; onChanged: () => void }) {
  const { t, language } = useSettings();
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const rename = useTold(async (id: string, name: string) => {
    await api.renamePasskey(id, name.trim());
    setRenaming(null);
    onChanged();
  });
  const remove = useTold(async (id: string) => {
    await api.removePasskey(id);
    setRemoving(null);
    onChanged();
  });

  if (passkeys.length === 0) {
    return <p className="empty-line">{t("me.passkeys_none")}</p>;
  }

  const failure = rename.failure ?? remove.failure;
  return (
    <>
      {failure && <p className="panel-notice panel-notice-trouble">{passkeyRefusal(failure, t)}</p>}
      <div className="lines">
        {passkeys.map((passkey) => (
          <div className="line" key={passkey.id}>
            <span className="line-mark" aria-hidden="true">
              <PasskeyIcon size={18} />
            </span>
            {renaming?.id === passkey.id ? (
              <form
                className="line-words"
                onSubmit={(event) => {
                  event.preventDefault();
                  void rename.tell(passkey.id, renaming.name);
                }}
              >
                <input
                  type="text"
                  className="field-line"
                  autoComplete="off"
                  aria-label={t("me.passkey_name")}
                  value={renaming.name}
                  onChange={(event) => setRenaming({ id: passkey.id, name: event.target.value })}
                />
              </form>
            ) : (
              <span className="line-words">
                <span className="line-name">{passkey.name}</span>
                <span className="line-note">{aboutPasskey(passkey, language, t)}</span>
              </span>
            )}
            <span className="line-end">
              {renaming?.id === passkey.id ? (
                <>
                  <button className="button button-small" onClick={() => setRenaming(null)}>
                    {t("settings.cancel")}
                  </button>
                  <button
                    className="button button-small button-accent"
                    disabled={rename.busy || renaming.name.trim() === ""}
                    onClick={() => void rename.tell(passkey.id, renaming.name)}
                  >
                    {t("me.passkey_rename_save")}
                  </button>
                </>
              ) : removing === passkey.id ? (
                <>
                  <button className="button button-small" onClick={() => setRemoving(null)}>
                    {t("settings.cancel")}
                  </button>
                  <button
                    className="button button-small button-danger-full"
                    disabled={remove.busy}
                    onClick={() => void remove.tell(passkey.id)}
                  >
                    {t("me.passkey_remove")}
                  </button>
                </>
              ) : (
                <>
                  <button
                    className="button button-small"
                    onClick={() => {
                      setRemoving(null);
                      setRenaming({ id: passkey.id, name: passkey.name });
                    }}
                  >
                    {t("me.passkey_rename")}
                  </button>
                  <button
                    className="button button-small"
                    onClick={() => {
                      setRenaming(null);
                      setRemoving(passkey.id);
                    }}
                  >
                    {t("me.passkey_remove")}
                  </button>
                </>
              )}
            </span>
          </div>
        ))}
      </div>
    </>
  );
}

/** When a key was made and last used, and the site it serves when it is not
 *  this one. */
function aboutPasskey(passkey: Passkey, language: string, t: Wording): string {
  const date = (value: string) => readableDate(value, language) ?? value;
  return [
    t("me.passkey_made_on", { date: date(passkey.created_at) }),
    passkey.used_at ? t("me.passkey_used_on", { date: date(passkey.used_at) }) : t("me.passkey_never_used"),
    passkey.site === window.location.hostname ? null : passkey.site,
  ]
    .filter((part): part is string => part !== null)
    .join(" · ");
}

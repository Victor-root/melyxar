/*
 * The accounts of this server and what each may do and see.
 *
 * One panel per account, as the libraries have one each: who it is, where it
 * is signed in, and every right it holds, each changed the moment it is
 * pressed. The server decides what is kept and what is refused; the page says
 * the refusal beside the account it was about.
 *
 * What is refused on one's own account (stepping down, being taken away,
 * signed out of everywhere, a password put on without the old one) is not
 * offered on it at all: one's own password is changed from one's profile.
 */

import { useState } from "react";
import { api, ApiError } from "../../api";
import type { Library, ManagedAccount, Rights } from "../../api";
import { useAccount } from "../../account";
import { useAsked, useTold } from "../../asking";
import { Face } from "../../components/face";
import { Modal } from "../../components/modal";
import { Editable, PageHead, Panel, Picker, Setting, Toggle } from "../../components/panel";
import { lastSeen } from "../../devices";
import { AccountAddIcon, KindIcon, LockIcon, PeopleIcon } from "../../icons";
import type { IconProps } from "../../icons";
import { useLibraries } from "../../libraries";
import { useJournalNews } from "../../live";
import { howMany } from "../../readable";
import type { Wording } from "../../readable";
import { useSettings } from "../../settings";
import { AN_ORDINARY_ACCOUNT, granting, MOST_STREAMS_OFFERED, reachOf, settled } from "./rights";

export function AdminUsers() {
  const { t } = useSettings();
  const accounts = useAsked((signal) => api.accounts(signal));
  // Where each account is signed in moves with every sign in and sign out,
  // and each of them is a line of the journal.
  useJournalNews(accounts.look);
  const { all: libraries } = useLibraries();
  const [adding, setAdding] = useState(false);
  /* What was just done, said once at the head of the page. */
  const [said, setSaid] = useState<string | null>(null);

  const done = (words: string) => {
    setSaid(words);
    accounts.look();
  };

  return (
    <>
      <PageHead
        lead={t("admin.users_lead")}
        actions={
          !adding && (
            <button
              className="button button-accent"
              onClick={() => {
                setSaid(null);
                setAdding(true);
              }}
            >
              <AccountAddIcon size={18} />
              {t("admin.add_user")}
            </button>
          )
        }
      />

      {accounts.failure && (
        <p className="panel-notice panel-notice-trouble">{t("error.unreachable")}</p>
      )}
      {said && <p className="panel-notice panel-notice-ok">{said}</p>}

      {adding && (
        <NewAccount
          libraries={libraries}
          onDone={(name) => {
            setAdding(false);
            done(t("users.created", { name }));
          }}
          onCancel={() => setAdding(false)}
        />
      )}

      {(accounts.answer ?? []).map((account) => (
        <AccountPanel
          key={account.id}
          account={account}
          libraries={libraries}
          onChanged={accounts.look}
          onDone={done}
        />
      ))}
    </>
  );
}

/** The face of an account, in the shape a panel takes an icon in. */
function faceOf(account: ManagedAccount) {
  return function AccountFace(_: IconProps) {
    return <Face name={account.name} avatar={account.avatar} className="account-face" />;
  };
}

/** A refusal of the server, in words this page can say. */
function refusalOf(failure: ApiError | null, t: Wording): string | null {
  if (!failure) {
    return null;
  }
  if (failure.reason) {
    return t(`refused.account.${failure.reason}`, {
      shortest: Number(failure.details?.shortest ?? 0),
    });
  }
  return t(failure.code === "not_found" ? "users.gone" : "users.failed");
}

/** Who an account is and where it is, in a line: its role, what it reaches,
 *  and where it is signed in. */
function leadOf(account: ManagedAccount, libraries: Library[], t: Wording): string {
  const { rights } = account;
  const role = rights.is_administrator
    ? t("users.role.administrator")
    : `${t("users.role.member")} · ${reachOf(rights, libraries, t)}`;
  const where =
    account.devices === 0 || account.last_seen_at === null
      ? t("users.nowhere")
      : `${howMany(account.devices, "users.devices", t)} · ${lastSeen(account.last_seen_at, Date.now(), t)}`;
  return `${role} · ${where}`;
}

function AccountPanel({
  account,
  libraries,
  onChanged,
  onDone,
}: {
  account: ManagedAccount;
  libraries: Library[];
  onChanged: () => void;
  onDone: (words: string) => void;
}) {
  const { t } = useSettings();
  const { cameIn } = useAccount();
  const [asking, setAsking] = useState<"sign_out" | "remove" | null>(null);
  const [choosingPassword, setChoosingPassword] = useState(false);

  const change = useTold(async (rights: Rights) => {
    await api.setRights(account.id, settled(rights));
    onChanged();
  });
  const rename = useTold(async (name: string) => {
    await api.renameAccount(account.id, name);
    // One's own name is also the one the bar at the top shows.
    if (account.is_you) {
      cameIn(await api.me());
    }
    onChanged();
  });
  const signOut = useTold(async () => {
    await api.signOutEverywhere(account.id);
    setAsking(null);
    onDone(t("users.signed_out_done", { name: account.name }));
  });
  const remove = useTold(async () => {
    await api.removeAccount(account.id);
    setAsking(null);
    onDone(t("users.removed", { name: account.name }));
  });

  const refusal =
    refusalOf(change.failure, t) ??
    refusalOf(rename.failure, t) ??
    refusalOf(signOut.failure, t) ??
    refusalOf(remove.failure, t);

  return (
    <Panel
      icon={faceOf(account)}
      title={account.is_you ? t("users.named_you", { name: account.name }) : account.name}
      lead={leadOf(account, libraries, t)}
      className="account-panel"
      action={
        !account.is_you && (
          <>
            <button className="button button-small" onClick={() => setChoosingPassword(true)}>
              <LockIcon size={15} />
              {t("users.password")}
            </button>
            <button
              className="button button-small"
              disabled={account.devices === 0}
              onClick={() => setAsking(asking === "sign_out" ? null : "sign_out")}
            >
              {t("users.sign_out")}
            </button>
            <button
              className="button button-small button-danger"
              onClick={() => setAsking(asking === "remove" ? null : "remove")}
            >
              {t("users.remove")}
            </button>
          </>
        )
      }
    >
      {refusal && <p className="panel-notice panel-notice-trouble">{refusal}</p>}

      {asking && (
        <div className="removal-box">
          <p className="removal-ask">
            {t(asking === "remove" ? "users.remove_asks" : "users.sign_out_asks", {
              name: account.name,
            })}
          </p>
          <div className="removal-actions">
            <button className="button button-small" onClick={() => setAsking(null)}>
              {t("settings.cancel")}
            </button>
            <button
              className="button button-small button-danger-full"
              disabled={signOut.busy || remove.busy}
              onClick={() => (asking === "remove" ? remove.tell() : signOut.tell())}
            >
              {t(asking === "remove" ? "users.remove" : "users.sign_out")}
            </button>
          </div>
        </div>
      )}

      <div className="settings-lines">
        <Setting label={t("users.name")}>
          <Editable
            value={account.name}
            label={t("users.name")}
            onSettled={(name) => void rename.tell(name)}
          />
        </Setting>
        <RightsLines
          rights={account.rights}
          libraries={libraries}
          you={account.is_you}
          onChange={(rights) => void change.tell(rights)}
        />
      </div>

      {account.is_you && <span className="panel-say">{t("users.not_yourself")}</span>}

      {choosingPassword && (
        <PasswordDialog
          account={account}
          onClose={() => setChoosingPassword(false)}
          onDone={() => {
            setChoosingPassword(false);
            onDone(t("users.password_done", { name: account.name }));
          }}
        />
      )}
    </Panel>
  );
}

/**
 * Every right an account holds, each one a switch.
 *
 * An administrator holds them all, so its lines stop at the one that says so.
 * The two that are drawn and not wired yet say so too.
 */
function RightsLines({
  rights,
  libraries,
  you,
  onChange,
}: {
  rights: Rights;
  libraries: Library[];
  /** One's own account: stepping down is not offered from here. */
  you: boolean;
  onChange: (rights: Rights) => void;
}) {
  const { t } = useSettings();
  const set = (changed: Partial<Rights>) => onChange(settled({ ...rights, ...changed }));

  return (
    <>
      <Setting label={t("users.administrator")} why={t("users.administrator_why")}>
        <Toggle
          label={t("users.administrator")}
          checked={rights.is_administrator}
          disabled={you}
          onChange={(is_administrator) => set({ is_administrator })}
        />
      </Setting>

      {!rights.is_administrator && (
        <>
          <Setting label={t("users.every_library")} why={t("users.every_library_why")}>
            <Toggle
              label={t("users.every_library")}
              checked={rights.sees_every_library}
              onChange={(sees_every_library) => set({ sees_every_library })}
            />
          </Setting>

          {!rights.sees_every_library && (
            <div className="lines account-libraries">
              {libraries.length === 0 && <p className="empty-line">{t("users.no_library_yet")}</p>}
              {libraries.map((library) => {
                const granted = rights.libraries.includes(library.id);
                return (
                  <div className="line" key={library.id}>
                    <span className="line-mark" aria-hidden="true">
                      <KindIcon kind={library.kind} size={18} />
                    </span>
                    <span className="line-words">
                      <span className="line-name">{library.name}</span>
                    </span>
                    <span className="line-end">
                      <Toggle
                        label={library.name}
                        checked={granted}
                        onChange={(wanted) => onChange(granting(rights, library.id, wanted))}
                      />
                    </span>
                  </div>
                );
              })}
              {libraries.length > 0 && rights.libraries.length === 0 && (
                <p className="panel-say">{t("users.no_library")}</p>
              )}
            </div>
          )}

          <Setting label={t("admin.right.may_delete")} why={t("users.may_delete_why")}>
            <Toggle
              label={t("admin.right.may_delete")}
              checked={rights.may_delete}
              onChange={(may_delete) => set({ may_delete })}
            />
          </Setting>
          <Setting
            label={t("admin.right.may_delete_from_disk")}
            why={t("users.may_delete_from_disk_why")}
          >
            <Toggle
              label={t("admin.right.may_delete_from_disk")}
              checked={rights.may_delete_from_disk}
              disabled={!rights.may_delete}
              onChange={(may_delete_from_disk) => set({ may_delete_from_disk })}
            />
          </Setting>
          <Setting label={t("admin.limit_streams")} why={t("users.streams_why")}>
            <Picker
              label={t("admin.limit_streams")}
              value={rights.most_streams === null ? "" : String(rights.most_streams)}
              options={[
                ["", t("users.no_limit")] as const,
                ...Array.from(
                  { length: MOST_STREAMS_OFFERED },
                  (_, index) => [String(index + 1), String(index + 1)] as const,
                ),
              ]}
              onPick={(value) => set({ most_streams: value === "" ? null : Number(value) })}
            />
          </Setting>
          <Setting label={t("admin.right.may_download")} soon>
            <Toggle label={t("admin.right.may_download")} checked={false} onChange={() => {}} disabled />
          </Setting>
          <Setting label={t("admin.limit_age")} soon>
            <Toggle label={t("admin.limit_age")} checked={false} onChange={() => {}} disabled />
          </Setting>
        </>
      )}
    </>
  );
}

/**
 * A password put on another account, typed twice. Every device of that
 * account is signed out when it is kept: whoever held it before is out.
 */
function PasswordDialog({
  account,
  onClose,
  onDone,
}: {
  account: ManagedAccount;
  onClose: () => void;
  onDone: () => void;
}) {
  const { t } = useSettings();
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  const [mismatch, setMismatch] = useState(false);
  const keep = useTold(async () => {
    await api.putPassword(account.id, password);
    onDone();
  });

  const submit = () => {
    if (password !== again) {
      setMismatch(true);
      return;
    }
    setMismatch(false);
    void keep.tell();
  };
  const refusal = mismatch ? t("users.password_mismatch") : refusalOf(keep.failure, t);

  return (
    <Modal
      title={t("users.password_title", { name: account.name })}
      onClose={onClose}
      footer={
        <>
          <button className="button" onClick={onClose}>
            {t("settings.cancel")}
          </button>
          <button
            className="button button-accent"
            disabled={keep.busy || password.length === 0 || again.length === 0}
            onClick={submit}
          >
            {t("users.save")}
          </button>
        </>
      }
    >
      <form
        className="account-password"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <p className="panel-say">{t("users.password_why")}</p>
        <label className="account-field">
          <span>{t("users.password_new")}</span>
          <input
            type="password"
            className="field-line"
            autoComplete="new-password"
            autoFocus
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        <label className="account-field">
          <span>{t("users.password_again")}</span>
          <input
            type="password"
            className="field-line"
            autoComplete="new-password"
            value={again}
            onChange={(event) => setAgain(event.target.value)}
          />
        </label>
        {refusal && <p className="panel-notice panel-notice-trouble">{refusal}</p>}
        {/* Enter in either field keeps the password, as a form does. */}
        <button type="submit" hidden />
      </form>
    </Modal>
  );
}

/**
 * An account being made: its name, its password twice, and its rights.
 *
 * Nothing is sent until the whole of it is there, so an account never exists
 * for a moment with rights nobody chose.
 */
function NewAccount({
  libraries,
  onDone,
  onCancel,
}: {
  libraries: Library[];
  onDone: (name: string) => void;
  onCancel: () => void;
}) {
  const { t } = useSettings();
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [again, setAgain] = useState("");
  const [rights, setRights] = useState<Rights>(AN_ORDINARY_ACCOUNT);
  const [mismatch, setMismatch] = useState(false);
  const create = useTold(async () => {
    const made = await api.createAccount(name.trim(), password, rights);
    onDone(made.name);
  });

  const submit = () => {
    if (password !== again) {
      setMismatch(true);
      return;
    }
    setMismatch(false);
    void create.tell();
  };
  const refusal = mismatch ? t("users.password_mismatch") : refusalOf(create.failure, t);

  return (
    <Panel icon={PeopleIcon} title={t("users.new")} lead={t("users.new_why")} className="account-new">
      <div className="settings-lines">
        <Setting label={t("users.name")}>
          <input
            type="text"
            className="field-line"
            autoFocus
            autoComplete="off"
            aria-label={t("users.name")}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </Setting>
        <Setting label={t("users.password")}>
          <input
            type="password"
            className="field-line"
            autoComplete="new-password"
            aria-label={t("users.password")}
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </Setting>
        <Setting label={t("users.password_again")}>
          <input
            type="password"
            className="field-line"
            autoComplete="new-password"
            aria-label={t("users.password_again")}
            value={again}
            onChange={(event) => setAgain(event.target.value)}
          />
        </Setting>
        <RightsLines rights={rights} libraries={libraries} you={false} onChange={setRights} />
      </div>

      {refusal && <p className="panel-notice panel-notice-trouble">{refusal}</p>}

      <div className="panel-foot">
        <button className="button" onClick={onCancel}>
          {t("settings.cancel")}
        </button>
        <button
          className="button button-accent"
          disabled={create.busy || !name.trim() || password.length === 0 || again.length === 0}
          onClick={submit}
        >
          {t("users.create")}
        </button>
      </div>
    </Panel>
  );
}

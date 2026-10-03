/*
 * Notifications, for the whole server: a message to send, or to try, with
 * everything a notification can be; what every account gets when it has not
 * chosen; and which libraries announce what arrives in them.
 */

import { useState } from "react";
import { api } from "../api";
import { refusalOf, useAsked } from "../asking";
import { PageHead, Panel, Picker, Setting, Toggle } from "../components/panel";
import { BellIcon, FolderIcon, PeopleIcon } from "../icons";
import { refusalKey } from "../i18n";
import { useSettings } from "../settings";
import { notesApi } from "./api";
import type { Audience, Channels, ChoosableKind, ForEveryAccount, Level, Written } from "./api";
import { ChannelSwitches } from "./settings";
import { useToast } from "./toasts";

const LEVELS: Level[] = ["ok", "news", "attention", "trouble"];

/** How long a message may be asked to stay, in seconds; "level" leaves it to
 *  its colour. */
const DURATIONS = ["level", "4", "8", "15", "30", "60"] as const;

type Recipients = Audience["to"];

/** Tomorrow evening, the usual time of a maintenance, as a field holds it. */
function tomorrowEvening(): string {
  const when = new Date();
  when.setDate(when.getDate() + 1);
  when.setHours(20, 0, 0, 0);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${when.getFullYear()}-${pad(when.getMonth() + 1)}-${pad(when.getDate())}T${pad(when.getHours())}:${pad(
    when.getMinutes(),
  )}`;
}

function SendForm() {
  const { t } = useSettings();
  const toast = useToast();
  const accounts = useAsked((signal) => api.accounts(signal), []);
  const [level, setLevel] = useState<Level>("news");
  const [title, setTitle] = useState("");
  const [text, setText] = useState("");
  /* A trial disturbs nobody else until asked to. */
  const [recipients, setRecipients] = useState<Recipients>("administrators");
  const [chosen, setChosen] = useState<string[]>([]);
  const [duration, setDuration] = useState<(typeof DURATIONS)[number]>("level");
  const [sticky, setSticky] = useState(false);
  const [priority, setPriority] = useState(false);
  const [mandatory, setMandatory] = useState(false);
  const [due, setDue] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  const audience: Audience =
    recipients === "accounts" ? { to: "accounts", accounts: chosen } : { to: recipients };
  const ready = title.trim() !== "" && (recipients !== "accounts" || chosen.length > 0) && !sending;

  const send = () => {
    const written: Written = {
      level,
      title: title.trim(),
      text: text.trim(),
      audience,
      shown_for_seconds: duration === "level" ? null : Number(duration),
      sticky,
      priority,
      mandatory,
      due_at: due ? new Date(due).toISOString() : null,
    };
    setSending(true);
    notesApi
      .write(written)
      .then(() => {
        toast({ state: "ok", title: t("notes.sent") });
        setTitle("");
        setText("");
      })
      .catch((error: unknown) =>
        toast({ state: "trouble", title: t("notes.not_sent"), detail: t(refusalKey(refusalOf(error))) }),
      )
      .finally(() => setSending(false));
  };

  return (
    <Panel icon={BellIcon} title={t("notes.send")} lead={t("notes.send_why")} className="notes-form">
      <Setting label={t("notes.level")} why={t("notes.level_why")}>
        <Picker
          label={t("notes.level")}
          value={level}
          options={LEVELS.map((one) => [one, t(`notes.level.${one}`)] as const)}
          onPick={setLevel}
        />
      </Setting>
      <Setting label={t("notes.title")} stacked>
        <input
          type="text"
          className="field-line"
          maxLength={120}
          aria-label={t("notes.title")}
          value={title}
          onChange={(event) => setTitle(event.target.value)}
        />
      </Setting>
      <Setting label={t("notes.text")} stacked>
        <textarea
          className="field-line note-text"
          maxLength={2000}
          rows={3}
          aria-label={t("notes.text")}
          value={text}
          onChange={(event) => setText(event.target.value)}
        />
      </Setting>
      <Setting label={t("notes.recipients")} why={t("notes.recipients_why")}>
        <Picker
          label={t("notes.recipients")}
          value={recipients}
          options={[
            ["administrators", t("notes.recipients.administrators")],
            ["everyone", t("notes.recipients.everyone")],
            ["accounts", t("notes.recipients.accounts")],
          ]}
          onPick={setRecipients}
        />
      </Setting>
      {recipients === "accounts" &&
        (accounts.answer ?? []).map((account) => (
          <Setting key={account.id} label={account.name}>
            <Toggle
              label={account.name}
              checked={chosen.includes(account.id)}
              onChange={(on) =>
                setChosen((was) => (on ? [...was, account.id] : was.filter((id) => id !== account.id)))
              }
            />
          </Setting>
        ))}
      <Setting label={t("notes.duration")} why={t("notes.duration_why")}>
        <Picker
          label={t("notes.duration")}
          value={duration}
          options={DURATIONS.map(
            (one) =>
              [one, one === "level" ? t("notes.duration.level") : t("notes.duration.seconds", { seconds: one })] as const,
          )}
          onPick={setDuration}
        />
      </Setting>
      <Setting label={t("notes.sticky")} why={t("notes.sticky_why")}>
        <Toggle label={t("notes.sticky")} checked={sticky} onChange={setSticky} />
      </Setting>
      <Setting label={t("notes.priority")} why={t("notes.priority_why")}>
        <Toggle label={t("notes.priority")} checked={priority} onChange={setPriority} />
      </Setting>
      <Setting label={t("notes.mandatory")} why={t("notes.mandatory_why")}>
        <Toggle label={t("notes.mandatory")} checked={mandatory || due !== null} disabled={due !== null} onChange={setMandatory} />
      </Setting>
      <Setting label={t("notes.maintenance")} why={t("notes.maintenance_why")}>
        <Toggle
          label={t("notes.maintenance")}
          checked={due !== null}
          onChange={(on) => setDue(on ? tomorrowEvening() : null)}
        />
      </Setting>
      {due !== null && (
        <Setting label={t("notes.maintenance_when")}>
          <input
            type="datetime-local"
            className="field-line"
            aria-label={t("notes.maintenance_when")}
            value={due}
            onChange={(event) => setDue(event.target.value || null)}
          />
        </Setting>
      )}
      <div className="note-send">
        <button type="button" className="button button-accent" disabled={!ready} onClick={send}>
          {t("notes.send_now")}
        </button>
      </div>
    </Panel>
  );
}

export function AdminNotifications() {
  const { t } = useSettings();
  const toast = useToast();
  const asked = useAsked((signal) => notesApi.forEveryAccount(signal), []);
  const [held, setHeld] = useState<ForEveryAccount | null>(null);
  const shown = held ?? asked.answer;

  /** Shows a change at once, and puts it back if the server refuses. */
  const change = (next: ForEveryAccount, save: () => Promise<unknown>) => {
    const before = shown;
    setHeld(next);
    save().catch(() => {
      setHeld(before);
      toast({ state: "trouble", title: t("notes.failed") });
    });
  };
  const setDefault = (kind: ChoosableKind, channels: Channels) => {
    if (!shown) return;
    change(
      { ...shown, defaults: shown.defaults.map((one) => (one.kind === kind ? { ...one, ...channels } : one)) },
      () => notesApi.setDefault(kind, channels),
    );
  };
  const setAnnounces = (library: string, announces: boolean) => {
    if (!shown) return;
    change(
      { ...shown, libraries: shown.libraries.map((one) => (one.id === library ? { ...one, announces } : one)) },
      () => notesApi.setAnnounces(library, announces),
    );
  };

  return (
    <>
      <PageHead lead={t("admin.notifications_lead")} />
      <SendForm />
      {shown && (
        <>
          <Panel icon={PeopleIcon} title={t("notes.defaults")} lead={t("notes.defaults_why")}>
            {shown.defaults.map((one) => (
              <Setting key={one.kind} label={t(`notes.kind.${one.kind}`)}>
                <ChannelSwitches kind={one.kind} channels={one} onChange={(channels) => setDefault(one.kind, channels)} />
              </Setting>
            ))}
          </Panel>
          <Panel icon={FolderIcon} title={t("notes.announcing")} lead={t("notes.announcing_why")}>
            {shown.libraries.length === 0 && <p className="panel-say">{t("notes.announcing_none")}</p>}
            {shown.libraries.map((library) => (
              <Setting key={library.id} label={library.name}>
                <Toggle
                  label={library.name}
                  checked={library.announces}
                  onChange={(announces) => setAnnounces(library.id, announces)}
                />
              </Setting>
            ))}
          </Panel>
        </>
      )}
    </>
  );
}

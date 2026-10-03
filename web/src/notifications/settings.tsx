/*
 * What an account chose about its notifications: kind by kind whether they
 * are kept in the bell and said on the screen, the libraries it hears about,
 * its quiet hours, and on this device whether they are said outside the
 * page. Changed at once, and put back if the server refuses.
 */

import { useState } from "react";
import { PageHead, Panel, Setting, Toggle } from "../components/panel";
import { BellIcon, ClockIcon, DeviceIcon, FolderIcon } from "../icons";
import { useSettings } from "../settings";
import { notesApi } from "./api";
import type { Channels, ChoosableKind, NoteChoices, NoteSettings } from "./api";
import { useNotes } from "./store";
import { systemCanSay, systemRefused } from "./system";
import { clockOf, minuteOfClock } from "./wording";

/** Where quiet hours start when they are first switched on. */
const QUIET_BY_DEFAULT = { quiet_from: 22 * 60, quiet_until: 7 * 60 };

/** One kind, its two switches side by side. */
export function ChannelSwitches({
  kind,
  channels,
  onChange,
}: {
  kind: ChoosableKind;
  channels: Channels;
  onChange: (channels: Channels) => void;
}) {
  const { t } = useSettings();
  const what = t(`notes.kind.${kind}`);
  return (
    <span className="note-channels">
      <span className="note-channel">
        <span className="note-channel-name">{t("notes.bell")}</span>
        <Toggle
          label={`${what}: ${t("notes.bell")}`}
          checked={channels.bell}
          onChange={(bell) => onChange({ ...channels, bell })}
        />
      </span>
      <span className="note-channel">
        <span className="note-channel-name">{t("notes.screen")}</span>
        <Toggle
          label={`${what}: ${t("notes.screen")}`}
          checked={channels.screen}
          onChange={(screen) => onChange({ ...channels, screen })}
        />
      </span>
    </span>
  );
}

export function MyNotifications() {
  const { t } = useSettings();
  const { choices, changeChoices, system, setSystem } = useNotes();
  const [refused, setRefused] = useState(systemRefused);

  if (!choices) {
    return <PageHead lead={t("me.notifications_lead")} />;
  }
  const settle = (settings: NoteSettings) =>
    changeChoices({ ...choices, settings }, () => notesApi.settle(settings));
  const choose = (kind: ChoosableKind, channels: Channels) => {
    const next: NoteChoices = {
      ...choices,
      kinds: choices.kinds.map((one) => (one.kind === kind ? { ...one, ...channels, chosen: true } : one)),
    };
    changeChoices(next, () => notesApi.choose(kind, channels));
  };
  const hear = (library: string, announced: boolean) => {
    const next: NoteChoices = {
      ...choices,
      libraries: choices.libraries.map((one) => (one.id === library ? { ...one, announced } : one)),
    };
    changeChoices(next, () => notesApi.hearAbout(library, announced));
  };
  const { settings } = choices;
  const quiet = settings.quiet_from !== null && settings.quiet_until !== null;
  const atClock = (field: "quiet_from" | "quiet_until", clock: string) => {
    const minute = minuteOfClock(clock);
    if (minute !== null) settle({ ...settings, [field]: minute });
  };

  return (
    <>
      <PageHead lead={t("me.notifications_lead")} />
      <Panel icon={BellIcon} title={t("notes.what")} lead={t("notes.what_why")}>
        {choices.kinds.map((one) => (
          <Setting key={one.kind} label={t(`notes.kind.${one.kind}`)} why={t(`notes.kind.${one.kind}_why`)}>
            <ChannelSwitches kind={one.kind} channels={one} onChange={(channels) => choose(one.kind, channels)} />
          </Setting>
        ))}
        <Setting label={t("notes.kind.maintenance")} why={t("notes.kind.maintenance_why")}>
          <span className="note-always">{t("notes.always")}</span>
        </Setting>
      </Panel>

      {choices.libraries.length > 0 && (
        <Panel icon={FolderIcon} title={t("notes.libraries")} lead={t("notes.libraries_why")}>
          {choices.libraries.map((library) => (
            <Setting key={library.id} label={library.name}>
              <Toggle
                label={library.name}
                checked={library.announced}
                onChange={(announced) => hear(library.id, announced)}
              />
            </Setting>
          ))}
        </Panel>
      )}

      <Panel icon={ClockIcon} title={t("notes.quiet")} lead={t("notes.quiet_why")}>
        <Setting label={t("notes.quiet_on")}>
          <Toggle
            label={t("notes.quiet_on")}
            checked={quiet}
            onChange={(on) =>
              settle({ ...settings, ...(on ? QUIET_BY_DEFAULT : { quiet_from: null, quiet_until: null }) })
            }
          />
        </Setting>
        {quiet && (
          <Setting label={t("notes.quiet_hours")}>
            <span className="note-clocks">
              <input
                type="time"
                className="field-line"
                aria-label={t("notes.quiet_from")}
                value={clockOf(settings.quiet_from ?? 0)}
                onChange={(event) => atClock("quiet_from", event.target.value)}
              />
              <span>{t("notes.quiet_to")}</span>
              <input
                type="time"
                className="field-line"
                aria-label={t("notes.quiet_until")}
                value={clockOf(settings.quiet_until ?? 0)}
                onChange={(event) => atClock("quiet_until", event.target.value)}
              />
            </span>
          </Setting>
        )}
        <Setting label={t("notes.urgent_while_playing")} why={t("notes.urgent_while_playing_why")}>
          <Toggle
            label={t("notes.urgent_while_playing")}
            checked={settings.priority_while_playing}
            onChange={(priority_while_playing) => settle({ ...settings, priority_while_playing })}
          />
        </Setting>
        <Setting label={t("notes.urgent_while_quiet")} why={t("notes.urgent_while_quiet_why")}>
          <Toggle
            label={t("notes.urgent_while_quiet")}
            checked={settings.priority_while_quiet}
            onChange={(priority_while_quiet) => settle({ ...settings, priority_while_quiet })}
          />
        </Setting>
      </Panel>

      <Panel icon={DeviceIcon} title={t("notes.system")} lead={t("notes.system_why")}>
        <Setting
          label={t("notes.system_on")}
          why={!systemCanSay() ? t("notes.system_none") : refused ? t("notes.system_refused") : undefined}
        >
          <Toggle
            label={t("notes.system_on")}
            checked={system}
            disabled={!systemCanSay()}
            onChange={(wanted) => {
              void setSystem(wanted).then(() => setRefused(systemRefused()));
            }}
          />
        </Setting>
      </Panel>
    </>
  );
}

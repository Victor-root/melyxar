/*
 * What the server is, for everybody who comes to it: its name, its face, the
 * screen at the door, and the few switches that change it as a whole. Drawn
 * whole; what has no engine yet says so.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import {
  Editable,
  NumberField,
  PageHead,
  Panel,
  Picker,
  Setting,
  Toggle,
} from "../../components/panel";
import { useToast } from "../../components/toasts";
import { refusalKey } from "../../i18n";
import { refusalAbout, refusalOf } from "../../asking";
import {
  DatabaseIcon,
  EnterIcon,
  HistoryIcon,
  RefreshIcon,
  ServerIcon,
  WarningIcon,
} from "../../icons";
import { serverRenamed } from "../../player/logo";
import { useSettings } from "../../settings";
import { useOverview } from "./layout";

export function AdminSettings() {
  const { t } = useSettings();

  return (
    <>
      <PageHead lead={t("admin.settings_lead")} />
      <div className="panels">
        <ServerPanel />

        <Panel icon={EnterIcon} title={t("admin.door")} lead={t("admin.door_lead")} soon>
          <Setting label={t("admin.door_background")} soon>
            <Picker
              label={t("admin.door_background")}
              value="abstract"
              options={[
                ["abstract", t("admin.door_background.abstract")],
                ["library", t("admin.door_background.library")],
              ]}
              onPick={() => {}}
              disabled
            />
          </Setting>
          <Setting label={t("admin.door_picture")} soon>
            <button className="button button-small" disabled>
              {t("settings.avatar_choose")}
            </button>
          </Setting>
        </Panel>

        <Panel icon={WarningIcon} title={t("admin.maintenance")} lead={t("admin.maintenance_lead")} soon>
          <Setting label={t("admin.maintenance_on")} soon>
            <Toggle label={t("admin.maintenance_on")} checked={false} onChange={() => {}} disabled />
          </Setting>
          <Setting label={t("admin.maintenance_message")} soon>
            <input className="field-line" disabled readOnly value="" />
          </Setting>
        </Panel>

        <Panel icon={RefreshIcon} title={t("admin.updates")} lead={t("admin.updates_lead")} soon>
          <Setting label={t("admin.updates_check")} soon>
            <Toggle label={t("admin.updates_check")} checked={false} onChange={() => {}} disabled />
          </Setting>
        </Panel>

        <JournalPanel />

        <Panel icon={DatabaseIcon} title={t("admin.backups")} lead={t("admin.backups_lead")} soon>
          <Setting label={t("admin.backups_daily")} soon>
            <Toggle label={t("admin.backups_daily")} checked={false} onChange={() => {}} disabled />
          </Setting>
        </Panel>
      </div>
    </>
  );
}

/** What the server is called, and the mark it will be given. */
function ServerPanel() {
  const { t } = useSettings();
  const toast = useToast();
  const overview = useOverview();
  const [name, setName] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    api
      .serverName(controller.signal)
      .then((kept) => setName(kept.server_name))
      .catch(() => {
        // Left empty: the field says nothing rather than a name the server
        // may not hold.
      });
    return () => controller.abort();
  }, []);

  const rename = (wanted: string) => {
    const before = name;
    setName(wanted);
    api
      .renameServer(wanted)
      .then((kept) => {
        setName(kept.server_name);
        serverRenamed(kept.server_name);
        overview.again();
      })
      .catch((error) => {
        setName(before);
        toast({ state: "trouble", title: t("admin.server_name_failed"), detail: t(refusalAbout(error, "server")) });
      });
  };

  return (
    <Panel icon={ServerIcon} title={t("admin.server")} lead={t("admin.server_lead")}>
      <Setting label={t("admin.server_name")}>
        {name !== null && <Editable value={name} label={t("admin.server_name")} onSettled={rename} />}
      </Setting>
      <Setting label={t("admin.logo")} why={t("admin.logo_why")} soon>
        <button className="button button-small" disabled>
          {t("settings.avatar_choose")}
        </button>
      </Setting>
    </Panel>
  );
}

/** The fewest and the most days the activity journal may keep, as the
 *  server holds it to. */
const KEPT_DAYS = { min: 7, max: 3650 };

/** How long the activity journal keeps what it writes. */
function JournalPanel() {
  const { t } = useSettings();
  const toast = useToast();
  const [days, setDays] = useState<number | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    api
      .activityKeptDays(controller.signal)
      .then((kept) => setDays(kept.days))
      .catch(() => {
        // Left empty: the field says nothing rather than a number the server
        // may not hold.
      });
    return () => controller.abort();
  }, []);

  const keep = (wanted: number) => {
    const before = days;
    setDays(wanted);
    api
      .keepActivityDays(wanted)
      .then((kept) => setDays(kept.days))
      .catch((error) => {
        setDays(before);
        toast({ state: "trouble", title: t("activity.kept_failed"), detail: t(refusalKey(refusalOf(error))) });
      });
  };

  return (
    <Panel icon={HistoryIcon} title={t("activity.title")} lead={t("activity.kept_lead")}>
      <Setting label={t("activity.kept")} why={t("activity.kept_why")}>
        {days !== null && (
          <NumberField
            label={t("activity.kept")}
            value={days}
            min={KEPT_DAYS.min}
            max={KEPT_DAYS.max}
            onPick={keep}
          />
        )}
      </Setting>
    </Panel>
  );
}

/*
 * What the server is, for everybody who comes to it: its name, its face, the
 * screen at the door, and the few switches that change it as a whole. Drawn
 * whole; what has no engine yet says so.
 */

import { useEffect, useRef, useState } from "react";
import { api, ApiError } from "../../api";
import type { ServerSettings } from "../../api";
import {
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
  MelyxarMark,
  RefreshIcon,
  ResetIcon,
  ServerIcon,
  WarningIcon,
} from "../../icons";
import { serverChanged } from "../../player/logo";
import { howMany } from "../../readable";
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

/** The longest a server's name may be, as the server holds it to. */
const LONGEST_NAME = 15;

/** What the server answers a request carrying more than it takes. */
const TOO_LARGE = 413;

/** What the server is called and the logo it wears. */
function ServerPanel() {
  const { t } = useSettings();
  const toast = useToast();
  const overview = useOverview();
  const chooser = useRef<HTMLInputElement>(null);
  const [server, setServer] = useState<ServerSettings | null>(null);
  const [typed, setTyped] = useState("");
  const [naming, setNaming] = useState(false);
  const [sending, setSending] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    api
      .server(controller.signal)
      .then((read) => {
        setServer(read);
        setTyped(read.server_name);
      })
      .catch(() => {
        // Left empty: the panel says nothing rather than a name the server
        // may not hold.
      });
    return () => controller.abort();
  }, []);

  /* Whatever changed, the server answers both as it now holds them, and
     every screen showing either is told at once. */
  const kept = (now: ServerSettings) => {
    setServer(now);
    serverChanged(now);
    overview.again();
  };

  /* Sent when somebody says so, and said back once it is kept: a name only
     half typed is never anybody's choice, and a field that saves on its own
     leaves nobody sure it did. */
  const changeName = (change: () => Promise<ServerSettings>, done: string) => {
    setNaming(true);
    change()
      .then((now) => {
        kept(now);
        setTyped(now.server_name);
        toast({ state: "ok", title: t(done, { name: now.server_name }) });
      })
      .catch((error) => {
        toast({ state: "trouble", title: t("admin.server_name_failed"), detail: t(refusalAbout(error, "server")) });
      })
      .finally(() => setNaming(false));
  };

  const changeLogo = (change: () => Promise<ServerSettings>) => {
    setSending(true);
    change()
      .then(kept)
      .catch((error) => {
        const refused =
          error instanceof ApiError && error.status === TOO_LARGE
            ? "refused.server.too_large"
            : refusalAbout(error, "server");
        toast({ state: "trouble", title: t("admin.logo_failed"), detail: t(refused) });
      })
      .finally(() => setSending(false));
  };

  return (
    <Panel icon={ServerIcon} title={t("admin.server")} lead={t("admin.server_lead")}>
      <Setting label={t("admin.server_name")}>
        {server && (
          <form
            className="name-choice"
            onSubmit={(event) => {
              event.preventDefault();
              changeName(() => api.renameServer(typed), "admin.server_name_saved");
            }}
          >
            <span className="name-field">
              <input
                type="text"
                className="field-line"
                aria-label={t("admin.server_name")}
                aria-describedby="name-left"
                value={typed}
                maxLength={LONGEST_NAME}
                onChange={(event) => setTyped(event.target.value)}
              />
              {/* Counted in letters, the way the server counts them. */}
              <span id="name-left" className="name-left" aria-live="polite">
                {howMany(LONGEST_NAME - [...typed].length, "admin.server_name_left", t)}
              </span>
            </span>
            <button
              type="submit"
              className="button button-small button-accent"
              disabled={naming || !typed.trim() || typed.trim() === server.server_name}
            >
              {t("admin.server_name_save")}
            </button>
            <button
              type="button"
              className="button button-small button-quiet"
              disabled={naming || server.server_name === server.default_name}
              onClick={() => changeName(api.forgetServerName, "admin.server_name_given_back")}
              title={t("admin.server_name_default_why", { name: server.default_name })}
            >
              <ResetIcon size={15} />
              {t("admin.server_name_default")}
            </button>
          </form>
        )}
      </Setting>
      <Setting label={t("admin.logo")} why={t("admin.logo_why")}>
        {server && (
          <div className="logo-choice">
            {server.logo ? (
              <img className="logo-choice-picture" src={server.logo} alt="" aria-hidden="true" />
            ) : (
              <MelyxarMark size={32} />
            )}
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
                  changeLogo(() => api.setServerLogo(image));
                }
              }}
            />
            <button
              className="button button-small"
              disabled={sending}
              onClick={() => chooser.current?.click()}
            >
              {t(server.logo ? "admin.logo_change" : "admin.logo_choose")}
            </button>
            {server.logo && (
              <button
                className="button button-small button-quiet"
                disabled={sending}
                onClick={() => changeLogo(api.removeServerLogo)}
              >
                {t("admin.logo_remove")}
              </button>
            )}
            {sending && <span className="line-note">{t("settings.avatar_busy")}</span>}
          </div>
        )}
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

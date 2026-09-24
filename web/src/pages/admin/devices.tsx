/*
 * Every device signed in to this server, whoever's it is, and the way to sign
 * one out without touching the others of the same account.
 *
 * Looked at again whenever a line of the journal is written: every sign in
 * and every sign out is one.
 */

import { api } from "../../api";
import { useAsked } from "../../asking";
import { DeviceLines } from "../../components/device-lines";
import { PageHead, Panel } from "../../components/panel";
import { DeviceIcon, LockIcon } from "../../icons";
import { useJournalNews } from "../../live";
import { useSettings } from "../../settings";

export function AdminDevices() {
  const { t } = useSettings();
  const devices = useAsked((signal) => api.devices(signal));
  useJournalNews(devices.look);

  return (
    <>
      <PageHead lead={t("admin.devices_lead")} />
      <Panel icon={DeviceIcon} title={t("admin.devices_signed_in")} lead={t("admin.devices_signed_in_lead")}>
        {devices.failure && <p className="panel-notice panel-notice-trouble">{t("error.unreachable")}</p>}
        {devices.answer && (
          <DeviceLines
            devices={devices.answer}
            withAccount
            signOut={api.signOutDevice}
            onSignedOut={devices.look}
          />
        )}
      </Panel>
      <Panel icon={LockIcon} title={t("admin.tv_code")} lead={t("admin.tv_code_lead")} soon />
    </>
  );
}

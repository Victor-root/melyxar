/*
 * Every device signed in to this server, and the way to sign one out: drawn
 * now, wired with the lot that manages accounts from the interface.
 */

import { PageHead, Panel } from "../../components/panel";
import { DeviceIcon, LockIcon } from "../../icons";
import { useSettings } from "../../settings";
import { Ghosts } from "./ghosts";

export function AdminDevices() {
  const { t } = useSettings();
  return (
    <>
      <PageHead lead={t("admin.devices_lead")} />
      <Panel icon={DeviceIcon} title={t("admin.devices_signed_in")} lead={t("admin.devices_signed_in_lead")} soon>
        <Ghosts heads={["admin.col.device", "admin.col.who", "admin.col.last_seen", "admin.col.since"]} />
      </Panel>
      <Panel icon={LockIcon} title={t("admin.tv_code")} lead={t("admin.tv_code_lead")} soon />
    </>
  );
}

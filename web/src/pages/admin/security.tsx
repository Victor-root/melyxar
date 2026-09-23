/*
 * Who may come in, and how the way in is protected: drawn now, wired with the
 * lot that follows sign in attempts and, later, with the modes of access.
 */

import { PageHead, Panel, Setting, Toggle } from "../../components/panel";
import { LockIcon, ShieldIcon, WarningIcon } from "../../icons";
import { useSettings } from "../../settings";
import { Ghosts } from "./ghosts";

/** The four ways in this server will offer, said as they are. */
const ACCESS = ["proxy", "self_signed", "provided", "automatic"];

export function AdminSecurity() {
  const { t } = useSettings();
  return (
    <>
      <PageHead lead={t("admin.security_lead")} />
      <div className="panels">
        <Panel icon={WarningIcon} title={t("admin.refused_sign_ins")} lead={t("admin.refused_sign_ins_lead")} soon>
          <Ghosts heads={["admin.col.when", "admin.col.name", "admin.col.address"]} />
        </Panel>
        <Panel icon={LockIcon} title={t("admin.brake")} lead={t("admin.brake_lead")} soon>
          <Setting label={t("admin.brake_after")} soon>
            <input className="field-line field-number" disabled value="10" readOnly />
          </Setting>
        </Panel>
      </div>
      <Panel icon={ShieldIcon} title={t("admin.access")} lead={t("admin.access_lead")} soon>
        {ACCESS.map((way) => (
          <Setting key={way} label={t(`admin.access.${way}`)} why={t(`admin.access.${way}_why`)} soon>
            <Toggle label={t(`admin.access.${way}`)} checked={false} onChange={() => {}} disabled />
          </Setting>
        ))}
      </Panel>
    </>
  );
}

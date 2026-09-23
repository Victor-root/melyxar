/*
 * The accounts of this server and what each may do: drawn now, wired with the
 * lot that manages accounts from the interface.
 */

import { PageHead, Panel, Setting, Toggle } from "../../components/panel";
import { AccountAddIcon, FolderIcon, PeopleIcon } from "../../icons";
import { useSettings } from "../../settings";
import { Ghosts } from "./ghosts";

export function AdminUsers() {
  const { t } = useSettings();
  return (
    <>
      <PageHead
        lead={t("admin.users_lead")}
        actions={
          <button className="button button-accent" disabled title={t("admin.soon")}>
            <AccountAddIcon size={18} />
            {t("admin.add_user")}
          </button>
        }
      />
      <Panel icon={PeopleIcon} title={t("admin.accounts")} lead={t("admin.accounts_lead")} soon>
        <Ghosts heads={["admin.col.who", "admin.col.role", "admin.col.libraries", "admin.col.last_seen"]} />
      </Panel>
      <div className="panels">
        <Panel icon={FolderIcon} title={t("admin.rights")} lead={t("admin.rights_lead")} soon>
          {["may_download", "may_delete", "may_delete_from_disk"].map((right) => (
            <Setting key={right} label={t(`admin.right.${right}`)} soon>
              <Toggle label={t(`admin.right.${right}`)} checked={false} onChange={() => {}} disabled />
            </Setting>
          ))}
        </Panel>
        <Panel icon={PeopleIcon} title={t("admin.limits_people")} lead={t("admin.limits_people_lead")} soon>
          <Setting label={t("admin.limit_streams")} soon>
            <input className="field-line field-number" disabled value="–" readOnly />
          </Setting>
          <Setting label={t("admin.limit_age")} soon>
            <input className="field-line field-number" disabled value="–" readOnly />
          </Setting>
        </Panel>
      </div>
    </>
  );
}

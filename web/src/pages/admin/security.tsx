/*
 * Who may come in, and how the way in is protected: the sign ins refused,
 * how many wrong passwords an account takes, and how the server is reached.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import { refusalOf } from "../../asking";
import { NumberField, PageHead, Panel, Setting } from "../../components/panel";
import { useToast } from "../../components/toasts";
import { refusalKey } from "../../i18n";
import { LockIcon, WarningIcon } from "../../icons";
import { useSettings } from "../../settings";
import { AccessPanel } from "./access-panel";
import { ActivityJournal } from "./activity-list";

/** The fewest and the most wrong passwords an account may take, as the
 *  server holds it to. */
const TRIES = { min: 3, max: 100 };

export function AdminSecurity() {
  const { t } = useSettings();
  return (
    <>
      <PageHead lead={t("admin.security_lead")} />
      <div className="panels">
        <Panel
          icon={WarningIcon}
          title={t("admin.refused_sign_ins")}
          lead={t("admin.refused_sign_ins_lead")}
        >
          <ActivityJournal families={["refused"]} />
        </Panel>
        <BrakePanel />
      </div>
      <AccessPanel />
    </>
  );
}

/** How many wrong passwords in a row an account takes before it waits. */
function BrakePanel() {
  const { t } = useSettings();
  const toast = useToast();
  const [tries, setTries] = useState<number | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    api
      .signInTries(controller.signal)
      .then((kept) => setTries(kept.tries))
      .catch(() => {
        // Left empty: the field says nothing rather than a number the server
        // may not hold.
      });
    return () => controller.abort();
  }, []);

  const keep = (wanted: number) => {
    const before = tries;
    setTries(wanted);
    api
      .setSignInTries(wanted)
      .then((kept) => setTries(kept.tries))
      .catch((error) => {
        setTries(before);
        toast({
          state: "trouble",
          title: t("admin.brake_failed"),
          detail: t(refusalKey(refusalOf(error))),
        });
      });
  };

  return (
    <Panel
      icon={LockIcon}
      title={t("admin.brake")}
      lead={t("admin.brake_lead")}
    >
      <Setting label={t("admin.brake_after")} why={t("admin.brake_after_why")}>
        {tries !== null && (
          <NumberField
            label={t("admin.brake_after")}
            value={tries}
            min={TRIES.min}
            max={TRIES.max}
            onPick={keep}
          />
        )}
      </Setting>
    </Panel>
  );
}

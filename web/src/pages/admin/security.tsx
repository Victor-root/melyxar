/*
 * Who may come in, and how the way in is protected: the sign ins refused,
 * how many wrong passwords an account takes, and how the server is reached.
 */

import { Fragment, useEffect, useState } from "react";
import { api } from "../../api";
import type { AccessMode, AccessStatus } from "../../api";
import { refusalAbout, refusalOf } from "../../asking";
import {
  NumberField,
  PageHead,
  Panel,
  Setting,
  Toggle,
} from "../../components/panel";
import { useToast } from "../../components/toasts";
import { refusalKey } from "../../i18n";
import { LockIcon, ShieldIcon, WarningIcon } from "../../icons";
import { useSettings } from "../../settings";
import { addressAfter, runningOut } from "./access";
import { ActivityJournal } from "./activity-list";

/** The ways in this server offers, said as they are. */
const ACCESS: AccessMode[] = ["proxy", "self_signed", "provided", "automatic"];

/** Not offered by this version yet. */
const NOT_YET: AccessMode = "automatic";

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

/** How the server is reached: one way at a time, the certificate in use,
 *  and why there is none when there should be. */
function AccessPanel() {
  const { t, language } = useSettings();
  const toast = useToast();
  const [status, setStatus] = useState<AccessStatus | null>(null);
  const [certificatePath, setCertificatePath] = useState("");
  const [keyPath, setKeyPath] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    api
      .access(controller.signal)
      .then((shown) => {
        setStatus(shown);
        setCertificatePath(shown.certificate_path ?? "");
        setKeyPath(shown.private_key_path ?? "");
      })
      .catch(() => {
        // Left empty rather than showing a way in the server may not use.
      });
    return () => controller.abort();
  }, []);

  const choose = (mode: AccessMode) => {
    if (!status) {
      return;
    }
    const encrypted = status.certificate !== null;
    setBusy(true);
    api
      .chooseAccess(
        mode,
        certificatePath.trim() || null,
        keyPath.trim() || null,
      )
      .then((chosen) => {
        setStatus(chosen);
        const next =
          chosen.certificate || chosen.mode === "proxy"
            ? addressAfter(chosen.mode, encrypted, window.location)
            : null;
        if (next) {
          window.location.replace(next);
        }
      })
      .catch((error) => {
        toast({
          state: "trouble",
          title: t("admin.access_failed"),
          detail: t(refusalAbout(error, "access")),
        });
      })
      .finally(() => setBusy(false));
  };

  const certificate = status?.certificate ?? null;
  return (
    <Panel
      icon={ShieldIcon}
      title={t("admin.access")}
      lead={t("admin.access_lead")}
    >
      {certificate && (
        <p
          className={`panel-notice${runningOut(certificate.not_after, new Date()) ? " panel-notice-trouble" : " panel-notice-ok"}`}
        >
          {t("admin.access_certificate", {
            names: certificate.names.join(", "),
            until: new Date(certificate.not_after).toLocaleDateString(
              language,
              { dateStyle: "long" },
            ),
          })}
        </p>
      )}
      {status?.problem && (
        <p className="panel-notice panel-notice-trouble">
          {t("admin.access_in_the_clear", {
            why: t(`refused.access.${status.problem}`),
          })}
        </p>
      )}
      {ACCESS.map((way) => (
        <Fragment key={way}>
          <Setting
            label={t(`admin.access.${way}`)}
            why={
              <>
                {t(`admin.access.${way}_why`)}
                {way !== "proxy" && way !== NOT_YET && (
                  <> {t("admin.access_not_behind_a_proxy")}</>
                )}
              </>
            }
            soon={way === NOT_YET}
          >
            <Toggle
              label={t(`admin.access.${way}`)}
              checked={status?.mode === way}
              disabled={
                !status ||
                busy ||
                way === NOT_YET ||
                (way === "proxy" && status.mode === "proxy")
              }
              onChange={(on) => choose(on ? way : "proxy")}
            />
          </Setting>
          {way === "provided" && (
            <form
              className="access-form"
              onSubmit={(event) => {
                event.preventDefault();
                choose("provided");
              }}
            >
              <input
                type="text"
                className="field-line"
                aria-label={t("admin.access_certificate_path")}
                placeholder={t("admin.access_certificate_path")}
                autoComplete="off"
                spellCheck={false}
                value={certificatePath}
                onChange={(event) => setCertificatePath(event.target.value)}
              />
              <input
                type="text"
                className="field-line"
                aria-label={t("admin.access_key_path")}
                placeholder={t("admin.access_key_path")}
                autoComplete="off"
                spellCheck={false}
                value={keyPath}
                onChange={(event) => setKeyPath(event.target.value)}
              />
              <span>
                <button
                  type="submit"
                  className="button button-small button-accent"
                  disabled={
                    !status ||
                    busy ||
                    certificatePath.trim() === "" ||
                    keyPath.trim() === ""
                  }
                >
                  {t("admin.access_use_provided")}
                </button>
              </span>
            </form>
          )}
        </Fragment>
      ))}
    </Panel>
  );
}

/*
 * Who may come in, and how the way in is protected: the sign ins refused,
 * how many wrong passwords an account takes, and how the server is reached.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import type { AccessMode, AccessStatus } from "../../api";
import { refusalAbout, refusalOf } from "../../asking";
import {
  NumberField,
  PageHead,
  Panel,
  Picker,
  Setting,
} from "../../components/panel";
import { useToast } from "../../components/toasts";
import { refusalKey } from "../../i18n";
import { LockIcon, ShieldIcon, WarningIcon } from "../../icons";
import { useSettings } from "../../settings";
import type { Draft } from "./access";
import { addressAfter, changed, draftOf, namesOf, runningOut } from "./access";
import { ActivityJournal } from "./activity-list";

/** The ways in this server offers, said as they are. */
const ACCESS: AccessMode[] = ["proxy", "self_signed", "provided"];

/** What happens to somebody who opens the server without the padlock. */
type Redirect = "on" | "off";
const REDIRECTS: Redirect[] = ["on", "off"];

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
  // What the page shows: nothing is sent before the administrator applies it.
  const [draft, setDraft] = useState<Draft | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    api
      .access(controller.signal)
      .then((shown) => {
        setStatus(shown);
        setDraft(draftOf(shown));
      })
      .catch(() => {
        // Left empty rather than showing a way in the server may not use.
      });
    return () => controller.abort();
  }, []);

  const edit = (changes: Partial<Draft>) =>
    setDraft((now) => (now ? { ...now, ...changes } : now));

  const apply = async () => {
    if (!status || !draft) {
      return;
    }
    const encrypted = status.certificate !== null;
    setBusy(true);
    try {
      // The names first, so a certificate made for this mode carries them.
      let chosen = await api.setAccessOptions(
        draft.redirect,
        namesOf(draft.names),
      );
      chosen = await api.chooseAccess(
        draft.mode,
        draft.certificatePath.trim() || null,
        draft.keyPath.trim() || null,
      );
      setStatus(chosen);
      setDraft(draftOf(chosen));
      const next =
        chosen.certificate || chosen.mode === "proxy"
          ? addressAfter(chosen.mode, encrypted, window.location)
          : null;
      if (next) {
        window.location.replace(next);
      }
    } catch (error) {
      toast({
        state: "trouble",
        title: t("admin.access_failed"),
        detail: t(refusalAbout(error, "access")),
      });
    } finally {
      setBusy(false);
    }
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
      {draft && (
        <>
          <Setting
            stacked
            label={t("admin.access_mode")}
            why={t(`admin.access.${draft.mode}_why`)}
          >
            <Picker<AccessMode>
              label={t("admin.access_mode")}
              value={draft.mode}
              options={ACCESS.map(
                (way) => [way, t(`admin.access.${way}`)] as const,
              )}
              onPick={(mode) => edit({ mode })}
              disabled={busy}
            />
          </Setting>
          {draft.mode === "provided" && (
            <div className="access-form">
              <input
                type="text"
                className="field-line"
                aria-label={t("admin.access_certificate_path")}
                placeholder={t("admin.access_certificate_path")}
                autoComplete="off"
                spellCheck={false}
                value={draft.certificatePath}
                onChange={(event) =>
                  edit({ certificatePath: event.target.value })
                }
              />
              <input
                type="text"
                className="field-line"
                aria-label={t("admin.access_key_path")}
                placeholder={t("admin.access_key_path")}
                autoComplete="off"
                spellCheck={false}
                value={draft.keyPath}
                onChange={(event) => edit({ keyPath: event.target.value })}
              />
            </div>
          )}
          {draft.mode !== "proxy" && (
            <Setting
              stacked
              label={t("admin.access_redirect")}
              why={t(
                `admin.access_redirect.${draft.redirect ? "on" : "off"}_why`,
              )}
            >
              <Picker<Redirect>
                label={t("admin.access_redirect")}
                value={draft.redirect ? "on" : "off"}
                options={REDIRECTS.map(
                  (one) => [one, t(`admin.access_redirect.${one}`)] as const,
                )}
                onPick={(one) => edit({ redirect: one === "on" })}
                disabled={busy}
              />
            </Setting>
          )}
          {draft.mode === "self_signed" && (
            <div className="access-form">
              <input
                type="text"
                className="field-line"
                aria-label={t("admin.access_names")}
                placeholder={t("admin.access_names")}
                autoComplete="off"
                spellCheck={false}
                value={draft.names}
                onChange={(event) => edit({ names: event.target.value })}
              />
            </div>
          )}
          <div className="access-form">
            <span>
              <button
                type="button"
                className="button button-small button-accent"
                disabled={
                  busy ||
                  !status ||
                  !changed(status, draft) ||
                  (draft.mode === "provided" &&
                    (draft.certificatePath.trim() === "" ||
                      draft.keyPath.trim() === ""))
                }
                onClick={apply}
              >
                {t("admin.access_apply")}
              </button>
            </span>
            <p className="setting-why">{t("admin.access_apply_why")}</p>
          </div>
        </>
      )}
    </Panel>
  );
}

/*
 * How the server is reached: what is in force, and a dialog to change it.
 *
 * The page only says where things stand. The choices are made in the dialog
 * and sent together when it is applied, so nothing changes under the
 * administrator's hand.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import type { AccessMode, AccessStatus } from "../../api";
import { refusalAbout } from "../../asking";
import { Modal } from "../../components/modal";
import { Panel, Picker, Setting } from "../../components/panel";
import { useToast } from "../../components/toasts";
import { ShieldIcon } from "../../icons";
import { useSettings } from "../../settings";
import type { Draft } from "./access";
import { addressAfter, changed, draftOf, namesOf, runningOut } from "./access";

/** The ways in this server offers, said as they are. */
const ACCESS: AccessMode[] = ["proxy", "self_signed", "provided"];

/** What happens to somebody who opens the server without the padlock. */
type Redirect = "on" | "off";
const REDIRECTS: Redirect[] = ["on", "off"];

export function AccessPanel() {
  const { t, language } = useSettings();
  const [status, setStatus] = useState<AccessStatus | null>(null);
  const [editing, setEditing] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    api
      .access(controller.signal)
      .then(setStatus)
      .catch(() => {
        // Left empty rather than showing a way in the server may not use.
      });
    return () => controller.abort();
  }, []);

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
      {status && (
        <Setting
          label={t(`admin.access.${status.mode}`)}
          why={t(`admin.access.${status.mode}_why`)}
        >
          <button
            type="button"
            className="button button-small"
            onClick={() => setEditing(true)}
          >
            {t("admin.access_edit")}
          </button>
        </Setting>
      )}
      {status && editing && (
        <AccessDialog
          status={status}
          onClose={() => setEditing(false)}
          onApplied={(chosen) => {
            setStatus(chosen);
            setEditing(false);
          }}
        />
      )}
    </Panel>
  );
}

function AccessDialog({
  status,
  onClose,
  onApplied,
}: {
  status: AccessStatus;
  onClose: () => void;
  onApplied: (chosen: AccessStatus) => void;
}) {
  const { t } = useSettings();
  const toast = useToast();
  const [draft, setDraft] = useState<Draft>(() => draftOf(status));
  const [busy, setBusy] = useState(false);

  const edit = (changes: Partial<Draft>) =>
    setDraft((now) => ({ ...now, ...changes }));

  const apply = async () => {
    const encrypted = status.certificate !== null;
    setBusy(true);
    try {
      // The names first, so a certificate made for this mode carries them.
      await api.setAccessOptions(draft.redirect, namesOf(draft.names));
      const chosen = await api.chooseAccess(
        draft.mode,
        draft.certificatePath.trim() || null,
        draft.keyPath.trim() || null,
      );
      onApplied(chosen);
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
      setBusy(false);
    }
  };

  const incomplete =
    draft.mode === "provided" &&
    (draft.certificatePath.trim() === "" || draft.keyPath.trim() === "");

  return (
    <Modal
      title={t("admin.access")}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t("admin.access_cancel")}
          </button>
          <button
            type="button"
            className="button button-accent"
            disabled={busy || incomplete || !changed(status, draft)}
            onClick={apply}
          >
            {t("admin.access_apply")}
          </button>
        </>
      }
    >
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
            onChange={(event) => edit({ certificatePath: event.target.value })}
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
          why={t(`admin.access_redirect.${draft.redirect ? "on" : "off"}_why`)}
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
      <p className="setting-why">{t("admin.access_apply_why")}</p>
    </Modal>
  );
}

/*
 * The question put before what a provider said about a work is taken away.
 *
 * Asked because it cannot be undone from here: the texts, the cast, the
 * pictures and every field written by hand go with it. The work is named
 * after its files again, and the automatic look up leaves it alone so it is
 * not taken for the same wrong film a second time.
 */

import { api } from "../api";
import { refusalAbout, useTold } from "../asking";
import { useSettings } from "../settings";
import { Modal } from "./modal";
import { useToast } from "./toasts";

export function ForgetIdentityDialog({
  workId,
  title,
  onClose,
  onForgotten,
}: {
  workId: string;
  title: string;
  onClose: () => void;
  /** Said once it is done, so the screens showing the work read it again. */
  onForgotten: () => void;
}) {
  const { t } = useSettings();
  const toast = useToast();
  const told = useTold(async () => {
    try {
      await api.forgetIdentity(workId);
      toast({ state: "ok", title: t("forget.done"), detail: t("forget.done_why") });
      onForgotten();
    } catch (error) {
      toast({ state: "trouble", title: t("forget.failed"), detail: t(refusalAbout(error, "forget")) });
    }
    onClose();
  });

  return (
    <Modal
      title={t("forget.title", { title })}
      onClose={onClose}
      footer={
        <button className="button button-accent" disabled={told.busy} onClick={() => told.tell()}>
          {t("forget.confirm")}
        </button>
      }
    >
      <p>{t("forget.what_goes")}</p>
      <p className="settings-why">{t("forget.after")}</p>
    </Modal>
  );
}

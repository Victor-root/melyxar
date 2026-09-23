/*
 * The question put before a work is deleted: out of the library only, or off
 * the disk as well.
 *
 * What goes is counted by the server the moment the question appears, and
 * the files that would leave the disk are named: a film and a series of two
 * hundred episodes are deleted from the same button, and nobody should find
 * out which after saying yes. Off the disk is offered only to an account
 * allowed to do it, and is never what the panel starts on.
 */

import { useState } from "react";
import { useAccount } from "../account";
import { api } from "../api";
import { refusalAbout, useAsked, useTold } from "../asking";
import { useMarks } from "../marks";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { Modal } from "./modal";

/** How many files are named before the rest are only counted. */
const FILES_NAMED = 8;

export function DeleteDialog({
  workId,
  title,
  onClose,
  onDeleted,
}: {
  workId: string;
  title: string;
  onClose: () => void;
  /** Said once the work is gone, so whoever showed it can move on. */
  onDeleted: () => void;
}) {
  const { t } = useSettings();
  const going = useAsked((signal) => api.whatDeletingTakes(workId, signal), [workId]);
  const [fromDisk, setFromDisk] = useState(false);
  const told = useTold(async () => {
    await api.deleteWork(workId, fromDisk);
    onDeleted();
  });

  const answer = going.answer;
  const refused = told.failure ?? going.failure;
  const named = answer?.files.slice(0, FILES_NAMED) ?? [];
  const unnamed = (answer?.files.length ?? 0) - named.length;

  return (
    <Modal
      title={t("delete.title", { title })}
      onClose={onClose}
      footer={
        <button
          className="button button-accent"
          disabled={answer === null || told.busy}
          onClick={() => told.tell()}
        >
          {told.busy ? t("delete.busy") : t("delete.confirm")}
        </button>
      }
    >
      {answer && <p>{howMany(answer.works, "delete.what_goes", t)}</p>}

      {answer && (
        <div className="delete-choices">
          <label className="delete-choice">
            <input
              type="radio"
              name="delete-how"
              checked={!fromDisk}
              onChange={() => setFromDisk(false)}
            />
            <span>
              <strong>{t("delete.library_only")}</strong>
              <span className="settings-why">{t("delete.library_only_why")}</span>
            </span>
          </label>

          {answer.may_delete_from_disk && (
            <label className="delete-choice">
              <input
                type="radio"
                name="delete-how"
                checked={fromDisk}
                onChange={() => setFromDisk(true)}
              />
              <span>
                <strong>{t("delete.from_disk")}</strong>
                {answer.files.length === 0 ? (
                  <span className="settings-why">{t("delete.no_file")}</span>
                ) : (
                  <>
                    <span className="settings-why">{t("delete.from_disk_why")}</span>
                    <ul className="delete-files">
                      {named.map((file) => (
                        <li key={file.path}>{file.path}</li>
                      ))}
                      {unnamed > 0 && <li>{t("delete.more_files", { count: unnamed })}</li>}
                    </ul>
                  </>
                )}
              </span>
            </label>
          )}
        </div>
      )}

      {refused && <p className="notice">{t(refusalAbout(refused, "deletion"))}</p>}
    </Modal>
  );
}

/** The button that puts the question, on the page of a work, for an account
 *  allowed to delete. Once the work is gone every card of it goes too, and
 *  the page says where to go next. */
export function DeleteButton({
  workId,
  title,
  onDeleted,
}: {
  workId: string;
  title: string;
  onDeleted: () => void;
}) {
  const { t } = useSettings();
  const { account } = useAccount();
  const marks = useMarks();
  const [asking, setAsking] = useState(false);

  if (!account?.may_delete) {
    return null;
  }
  return (
    <>
      <button className="button button-small" onClick={() => setAsking(true)}>
        {t("card.menu.delete")}
      </button>
      {asking && (
        <DeleteDialog
          workId={workId}
          title={title}
          onClose={() => setAsking(false)}
          onDeleted={() => {
            setAsking(false);
            marks.setGone(workId);
            onDeleted();
          }}
        />
      )}
    </>
  );
}

/*
 * The whole history of this account's notifications, on a page of its own:
 * where the bell leads when it sits in the account's menu, and where a long
 * history is read in comfort.
 */

import { Link } from "react-router-dom";
import { PageBackdrop } from "../components/backdrop";
import { TickIcon } from "../icons";
import { useSettings } from "../settings";
import { NoteList } from "./list";
import { useNotes } from "./store";
import { useTabPage } from "../tab-page";

export function NotesPage() {
  const { t } = useSettings();
  const { unread, markRead } = useNotes();
  useTabPage(t("nav.notifications"));
  return (
    <>
      <PageBackdrop />
      <main className="page notes-page">
        <div className="section-head">
          <h1>{t("nav.notifications")}</h1>
          <span className="notes-page-actions">
            {unread > 0 && (
              <button type="button" className="button" onClick={() => markRead()}>
                <TickIcon size={16} />
                {t("notes.mark_all_read")}
              </button>
            )}
            <Link to="/settings/notifications" className="button">
              {t("notes.choose")}
            </Link>
          </span>
        </div>
        <NoteList />
      </main>
    </>
  );
}

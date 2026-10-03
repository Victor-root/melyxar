/*
 * The history of an account's notifications, line by line: the same list in
 * the bell and on the page of the history.
 */

import { useEffect, useState } from "react";
import type { MouseEvent } from "react";
import { Link } from "react-router-dom";
import { useShownPicture } from "../components/picture";
import { CloseIcon, EyeIcon, EyeOffIcon } from "../icons";
import { howLongSince } from "../readable";
import type { Wording } from "../readable";
import { useSettings } from "../settings";
import type { Note } from "./api";
import { useNotes } from "./store";
import { sayNote } from "./wording";

/** How often the ages of the lines are counted again. */
const AGED_EVERY_MS = 60_000;

/** The time now, moved on every minute, for the ages of the lines. */
function useMinute(): number {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), AGED_EVERY_MS);
    return () => window.clearInterval(timer);
  }, []);
  return now;
}

/** How long ago a notification arrived, or that it just did. */
function age(instant: string, now: number, t: Wording): string {
  return now - new Date(instant).getTime() < AGED_EVERY_MS
    ? t("notes.just_now")
    : t("notes.ago", { time: howLongSince(instant, now, t) });
}

/** A press on a button inside a line does what the button says, and neither
 *  opens the line nor closes the list it is in. */
function only(act: () => void) {
  return (event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    act();
  };
}

function NoteLine({ note, now }: { note: Note; now: number }) {
  const { t, language } = useSettings();
  const { markRead, markUnread, remove } = useNotes();
  const said = sayNote(note, t, language);
  const poster = useShownPicture(note.poster);
  const open = () => {
    if (!note.read) markRead([note.id]);
  };

  const words = (
    <>
      {poster.picture ? (
        <img
          className="note-poster"
          src={poster.picture.src}
          srcSet={poster.picture.srcSet}
          sizes="36px"
          alt=""
          loading="lazy"
          onError={poster.itDidNotLoad}
        />
      ) : (
        <span className={`state-dot state-${note.level} note-dot`} aria-hidden="true" />
      )}
      <span className="note-words">
        <span className="note-title">{said.title}</span>
        {said.detail && <span className="note-detail">{said.detail}</span>}
        <span className="note-age">{age(note.created_at, now, t)}</span>
      </span>
    </>
  );

  return (
    <li className={`note note-${note.level}${note.read ? "" : " note-unread"}`}>
      {said.to ? (
        <Link className="note-open" to={said.to} onClick={open}>
          {words}
        </Link>
      ) : (
        <button type="button" className="note-open" onClick={open}>
          {words}
        </button>
      )}
      <span className="note-actions">
        <button
          type="button"
          className="note-action"
          title={t(note.read ? "notes.mark_unread" : "notes.mark_read")}
          aria-label={t(note.read ? "notes.mark_unread" : "notes.mark_read")}
          onClick={only(() => (note.read ? markUnread(note.id) : markRead([note.id])))}
        >
          {note.read ? <EyeOffIcon size={15} /> : <EyeIcon size={15} />}
        </button>
        {!note.mandatory && !note.priority && (
          <button
            type="button"
            className="note-action"
            title={t("notes.remove")}
            aria-label={t("notes.remove")}
            onClick={only(() => remove(note.id))}
          >
            <CloseIcon size={15} />
          </button>
        )}
      </span>
    </li>
  );
}

/** The history held, newest first, with what reads the older pages. */
export function NoteList() {
  const { t } = useSettings();
  const { notes, more, loaded, loadOlder } = useNotes();
  const now = useMinute();

  if (loaded && notes.length === 0) {
    return <p className="notes-none">{t("notes.none")}</p>;
  }
  return (
    <>
      <ul className="notes">
        {notes.map((note) => (
          <NoteLine key={note.id} note={note} now={now} />
        ))}
      </ul>
      {more && (
        <button type="button" className="notes-older" onClick={only(loadOlder)}>
          {t("notes.older")}
        </button>
      )}
    </>
  );
}

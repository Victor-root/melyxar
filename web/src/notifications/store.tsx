/*
 * This account's notifications, held once for the whole interface.
 *
 * The bell, the corner of the screen and the page of settings all show the
 * same history and the same choices, so they are held here, above every
 * screen. What the account does (read, unread, remove, choose) is written
 * here at once and put back if the server refuses; what changes on another
 * device arrives by the live line and is written here the same way. Each
 * notification the server says is for the screen is said in the corner, by
 * the same component as every other word, unless a film is playing or the
 * account's quiet hours hold, and outside the page too when this device
 * asked for it and the page is hidden.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { pictureSet } from "../api";
import { useLineKeptWhileHidden, useNotificationWords } from "../live";
import type { NotificationWord } from "../live";
import { useIsAFilmOnScreen } from "../on-screen";
import { useSettings } from "../settings";
import { notesApi } from "./api";
import type { Note, NoteChoices, NoteSettings } from "./api";
import { arrived, marked, NOTHING_HELD, olderPage, removed, unreadAmong } from "./history";
import type { Held } from "./history";
import { chooseSystem, sayToTheSystem, systemSays } from "./system";
import { useToast } from "./toasts";
import { minuteOfTheDay, posterOf, sayNote, showsNow } from "./wording";

/** What an account that has not been read yet is taken to have chosen. */
const UNTIL_READ: NoteSettings = {
  quiet_from: null,
  quiet_until: null,
  priority_while_playing: true,
  priority_while_quiet: true,
};

export interface Notes extends Held {
  /** Whether the first page has been read. */
  loaded: boolean;
  loadOlder: () => void;
  /** Every one when none is named. */
  markRead: (ids?: string[]) => void;
  markUnread: (id: string) => void;
  remove: (id: string) => void;
  /** Nothing until read. */
  choices: NoteChoices | null;
  /** Shows a change of choices at once, and puts the old ones back if
   *  saving it fails. */
  changeChoices: (next: NoteChoices, save: () => Promise<unknown>) => void;
  /** Whether this device says them outside the page. */
  system: boolean;
  setSystem: (wanted: boolean) => Promise<void>;
}

const NotesContext = createContext<Notes>({
  ...NOTHING_HELD,
  loaded: false,
  loadOlder: () => {},
  markRead: () => {},
  markUnread: () => {},
  remove: () => {},
  choices: null,
  changeChoices: () => {},
  system: false,
  setSystem: async () => {},
});

export function useNotes(): Notes {
  return useContext(NotesContext);
}

export function NotesProvider({ children }: { children: ReactNode }) {
  const { t, language } = useSettings();
  const toast = useToast();
  const navigate = useNavigate();
  const filmOnScreen = useIsAFilmOnScreen();
  const [held, setHeld] = useState<Held>(NOTHING_HELD);
  const [loaded, setLoaded] = useState(false);
  const [choices, setChoices] = useState<NoteChoices | null>(null);
  const [system, setSystemState] = useState(systemSays);
  useLineKeptWhileHidden(system);

  /* What the handlers of the line read, always the latest. */
  const latest = useRef({ held, choices, filmOnScreen, t, language });
  latest.current = { held, choices, filmOnScreen, t, language };
  /* Removed here and not yet heard back from the server: its word about
     them is not news. */
  const removedHere = useRef(new Set<string>());

  const readAfresh = useCallback(() => {
    notesApi
      .page()
      .then((page) => {
        setHeld({ notes: page.notifications, unread: page.unread, more: page.more });
        setLoaded(true);
      })
      .catch(() => {
        // What is held stays: the next opening of the line asks again.
      });
  }, []);

  const readChoices = useCallback(() => {
    notesApi
      .choices()
      .then(setChoices)
      .catch(() => {});
  }, []);

  useEffect(() => {
    readAfresh();
    readChoices();
  }, [readAfresh, readChoices]);

  const failed = useCallback(() => {
    toast({ state: "trouble", title: latest.current.t("notes.failed") });
  }, [toast]);

  const markRead = useCallback(
    (ids?: string[]) => {
      const changing = unreadAmong(latest.current.held, ids);
      setHeld((now) => marked(now, changing, true));
      notesApi.markRead(ids).catch(() => {
        setHeld((now) => marked(now, changing, false));
        failed();
      });
    },
    [failed],
  );

  const markUnread = useCallback(
    (id: string) => {
      setHeld((now) => marked(now, [id], false));
      notesApi.markUnread([id]).catch(() => {
        setHeld((now) => marked(now, [id], true));
        failed();
      });
    },
    [failed],
  );

  const remove = useCallback(
    (id: string) => {
      const before = latest.current.held.notes.find((one) => one.id === id);
      removedHere.current.add(id);
      setHeld((now) => removed(now, [id]).held);
      notesApi.remove(id).catch(() => {
        removedHere.current.delete(id);
        if (before) setHeld((now) => arrived(now, before));
        failed();
      });
    },
    [failed],
  );

  const loadOlder = useCallback(() => {
    const oldest = latest.current.held.notes[latest.current.held.notes.length - 1];
    notesApi
      .page(oldest?.id)
      .then((page) => setHeld((now) => olderPage(now, page)))
      .catch(failed);
  }, [failed]);

  const changeChoices = useCallback(
    (next: NoteChoices, save: () => Promise<unknown>) => {
      const before = latest.current.choices;
      setChoices(next);
      save().catch(() => {
        setChoices(before);
        failed();
      });
    },
    [failed],
  );

  const setSystem = useCallback(async (wanted: boolean) => {
    setSystemState(await chooseSystem(wanted));
  }, []);

  /** Says a notification on the screen, and outside the page when it is
   *  hidden and this device asked for it. */
  const show = useCallback(
    (note: Note, kept: boolean) => {
      const { choices: chosen, filmOnScreen: film, t: wording, language: speaking } = latest.current;
      if (!showsNow(note, chosen?.settings ?? UNTIL_READ, film, minuteOfTheDay(new Date()))) {
        return;
      }
      const said = sayNote(note, wording, speaking);
      const open = () => {
        if (kept && !note.read) markRead([note.id]);
      };
      toast({
        state: note.level,
        title: said.title,
        named: said.named,
        detail: said.detail,
        poster: posterOf(note, said),
        to: said.to,
        sticky: note.sticky,
        shownForMs: note.shown_for_ms,
        onOpen: open,
      });
      if (document.visibilityState === "hidden" && systemSays()) {
        sayToTheSystem(said.title, said.detail, pictureSet(posterOf(note, said))?.src ?? null, () => {
          open();
          if (said.to) navigate(said.to);
        });
      }
    },
    [toast, markRead, navigate],
  );

  useNotificationWords((word: NotificationWord) => {
    switch (word.name) {
      case "open":
      case "notifications_missed":
        readAfresh();
        if (word.name === "open") readChoices();
        return;
      case "notification_choices":
        readChoices();
        return;
      case "notification": {
        const { notification, kept, screen } = word.data as { notification: Note; kept: boolean; screen: boolean };
        if (kept) setHeld((now) => arrived(now, notification));
        if (screen) show(notification, kept);
        return;
      }
      case "notification_recalled": {
        const { notification } = word.data as { notification: Note };
        setHeld((now) => arrived(now, notification));
        show(notification, true);
        return;
      }
      case "notifications_read":
      case "notifications_unread": {
        const { ids } = word.data as { ids: string[] };
        setHeld((now) => marked(now, ids, word.name === "notifications_read"));
        return;
      }
      case "notifications_removed": {
        const { ids } = word.data as { ids: string[] };
        const news = ids.filter((id) => !removedHere.current.delete(id));
        if (news.length === 0) return;
        const after = removed(latest.current.held, news);
        setHeld(after.held);
        if (after.unknown) readAfresh();
        return;
      }
    }
  });

  const notes = useMemo(
    () => ({ ...held, loaded, loadOlder, markRead, markUnread, remove, choices, changeChoices, system, setSystem }),
    [held, loaded, loadOlder, markRead, markUnread, remove, choices, changeChoices, system, setSystem],
  );

  return <NotesContext.Provider value={notes}>{children}</NotesContext.Provider>;
}

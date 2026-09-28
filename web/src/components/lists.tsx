/*
 * What collections and playlists share: the window that puts a title in some
 * of them or takes it out, and makes a new one with it in; the card each one
 * is shown as; and the head of the page of one.
 *
 * Each tick is kept at once and put back as it was if the server refuses:
 * the tick is what the list holds. Every screen showing a list reads it again
 * once the server agrees.
 */

import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { Link, useNavigate } from "react-router-dom";
import type { Card } from "../api";
import { refusalOf, useAsked } from "../asking";
import { refusalKey } from "../i18n";
import { DeleteIcon, EditIcon, PlayAllIcon, TickIcon } from "../icons";
import { useMarks } from "../marks";
import { playInTurn } from "../queue";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { playsOnItsOwn } from "../works";
import { ROOM_FOR_A_PICTURE } from "./card";
import { Modal } from "./modal";
import { useShownPicture } from "./picture";

/** The lists of one kind, as the window asks them. */
export interface Lists {
  every: (signal: AbortSignal) => Promise<{ id: string; name: string }[]>;
  holding: (work: string, signal: AbortSignal) => Promise<string[]>;
  put: (id: string, works: string[], inIt: boolean) => Promise<unknown>;
  create: (name: string, works: string[]) => Promise<{ id: string }>;
}

export function PutInListsDialog({
  workId,
  title,
  lists,
  words,
  onClose,
}: {
  workId: string;
  title: string;
  lists: Lists;
  /** Where the words of the window start: `collect` or `playlist`. */
  words: string;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const { rowsHaveMoved } = useMarks();
  const every = useAsked((signal) => lists.every(signal), []);
  const held = useAsked((signal) => lists.holding(workId, signal), [workId]);
  const [made, setMade] = useState<{ id: string; name: string }[]>([]);
  const [inThem, setInThem] = useState<Set<string> | null>(null);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  useEffect(() => {
    if (held.answer && inThem === null) {
      setInThem(new Set(held.answer));
    }
  }, [held.answer, inThem]);

  const shown = [
    ...(every.answer ?? []),
    ...made.filter((one) => !every.answer?.some((list) => list.id === one.id)),
  ];

  const toggle = async (id: string) => {
    if (!inThem) {
      return;
    }
    const putIn = !inThem.has(id);
    const next = new Set(inThem);
    if (putIn) {
      next.add(id);
    } else {
      next.delete(id);
    }
    setInThem(next);
    setRefused(null);
    try {
      await lists.put(id, [workId], putIn);
      rowsHaveMoved();
    } catch (error) {
      setInThem(inThem);
      setRefused(refusalOf(error));
    }
  };

  const create = async () => {
    const name = typed.trim();
    if (name === "" || !inThem) {
      return;
    }
    setBusy(true);
    setRefused(null);
    try {
      const { id } = await lists.create(name, [workId]);
      setMade((before) => [...before, { id, name }]);
      setInThem(new Set([...inThem, id]));
      setTyped("");
      rowsHaveMoved();
    } catch (error) {
      setRefused(refusalOf(error));
    }
    setBusy(false);
  };

  const failure = every.failure ?? held.failure;

  return (
    <Modal title={t(`${words}.title`, { title })} onClose={onClose}>
      {failure && <p className="notice">{t(refusalKey(refusalOf(failure)))}</p>}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}

      {inThem && (
        <>
          {shown.length === 0 ? (
            <p className="settings-why">{t(`${words}.none_yet`)}</p>
          ) : (
            <ul className="lists-lines">
              {shown.map((list) => {
                const ticked = inThem.has(list.id);
                return (
                  <li key={list.id}>
                    <button
                      type="button"
                      className={`lists-line${ticked ? " lists-line-in" : ""}`}
                      aria-pressed={ticked}
                      onClick={() => toggle(list.id)}
                    >
                      <span className="lists-mark" aria-hidden="true">
                        {ticked && <TickIcon size={14} />}
                      </span>
                      <span className="lists-name">{list.name}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}

          <form
            className="lists-new"
            onSubmit={(event) => {
              event.preventDefault();
              void create();
            }}
          >
            <input
              type="text"
              className="field-line"
              aria-label={t(`${words}.new`)}
              placeholder={t(`${words}.new`)}
              maxLength={80}
              value={typed}
              onChange={(event) => setTyped(event.target.value)}
            />
            <button type="submit" className="button button-small button-accent" disabled={busy || typed.trim() === ""}>
              {t("lists.create")}
            </button>
          </form>
        </>
      )}
    </Modal>
  );
}

/** One list as a card: the poster of its first title, its name, and how many
 *  titles it holds. */
export function ListTile({
  id,
  name,
  count,
  cover,
  to,
}: {
  id: string;
  name: string;
  count: number;
  cover: Card | null;
  to: string;
}) {
  const { t } = useSettings();
  const { picture, itDidNotLoad } = useShownPicture(cover?.poster ?? []);
  return (
    <article
      className="card card-standing"
      data-card={id}
      style={{ ["--card-color" as string]: cover?.color ?? "var(--surface-raised)" }}
    >
      <div className="card-picture">
        {picture ? (
          <img
            src={picture.src}
            srcSet={picture.srcSet}
            sizes={ROOM_FOR_A_PICTURE.standing}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={itDidNotLoad}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {name.slice(0, 1)}
          </span>
        )}
        <Link className="card-open" to={to} title={name} draggable={false}>
          <span className="visually-hidden">{name}</span>
        </Link>
      </div>
      <span className="card-line">
        <span className="card-title">{name}</span>
      </span>
      <span className="card-year">{howMany(count, "lists.count", t)}</span>
    </article>
  );
}

/**
 * The head of the page of one list: its name, how many titles it holds,
 * playing them all in turn, and for whoever may change it, renaming and
 * deleting it.
 */
export function ListHead({
  name,
  cards,
  onRename,
  onDelete,
  words,
}: {
  name: string;
  cards: Card[];
  /** Nothing for somebody who may not change it. */
  onRename: ((name: string) => Promise<void>) | null;
  onDelete: (() => Promise<void>) | null;
  /** Where the words of the question put before deleting start. */
  words: string;
}) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const [renaming, setRenaming] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const playable = cards.filter(playsOnItsOwn).map((card) => card.id);

  const attempt = async (doing: () => Promise<void>) => {
    setRefused(null);
    try {
      await doing();
      return true;
    } catch (error) {
      setRefused(refusalOf(error));
      return false;
    }
  };

  let named: ReactNode = <h1>{name}</h1>;
  if (renaming !== null && onRename) {
    named = (
      <form
        className="lists-new list-rename"
        onSubmit={(event) => {
          event.preventDefault();
          void attempt(() => onRename(renaming)).then((done) => done && setRenaming(null));
        }}
      >
        <input
          type="text"
          className="field-line"
          aria-label={t("lists.name")}
          autoFocus
          maxLength={80}
          value={renaming}
          onChange={(event) => setRenaming(event.target.value)}
        />
        <button type="submit" className="button button-small button-accent" disabled={renaming.trim() === ""}>
          {t("lists.keep")}
        </button>
        <button type="button" className="button button-small button-quiet" onClick={() => setRenaming(null)}>
          {t("lists.cancel")}
        </button>
      </form>
    );
  }

  return (
    <>
      <div className="section-head list-head">
        {named}
        <span className="list-count">{howMany(cards.length, "lists.count", t)}</span>
        <span className="list-actions">
          {playable.length > 0 && (
            <button
              type="button"
              className="button button-small button-accent"
              onClick={() => {
                playInTurn(playable);
                navigate(`/work/${playable[0]}?play`);
              }}
            >
              <PlayAllIcon size={16} />
              {t("lists.play_all")}
            </button>
          )}
          {onRename && renaming === null && (
            <button type="button" className="button button-small" onClick={() => setRenaming(name)}>
              <EditIcon size={15} />
              {t("lists.rename")}
            </button>
          )}
          {onDelete && (
            <button type="button" className="button button-small button-quiet" onClick={() => setDeleting(true)}>
              <DeleteIcon size={15} />
              {t(`${words}.delete`)}
            </button>
          )}
        </span>
      </div>
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
      {deleting && onDelete && (
        <Modal
          title={t("lists.delete_title", { name })}
          onClose={() => setDeleting(false)}
          footer={
            <button
              className="button button-accent"
              onClick={() => void attempt(onDelete).then(() => setDeleting(false))}
            >
              {t(`${words}.delete`)}
            </button>
          }
        >
          <p>{t("lists.delete_why")}</p>
        </Modal>
      )}
    </>
  );
}

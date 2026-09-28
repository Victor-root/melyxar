/*
 * Putting a title in the server's collections made by hand, or taking it out
 * of them, and making a new one with it in.
 *
 * Each line is ticked at once and put back as it was if the server refuses:
 * the tick is what the collection holds. Every screen showing a collection
 * reads it again once the server agrees.
 */

import { useEffect, useState } from "react";
import { api } from "../api";
import type { CollectionSummary } from "../api";
import { refusalOf, useAsked } from "../asking";
import { refusalKey } from "../i18n";
import { TickIcon } from "../icons";
import { useMarks } from "../marks";
import { useSettings } from "../settings";
import { Modal } from "./modal";

export function CollectingDialog({
  workId,
  title,
  onClose,
}: {
  workId: string;
  title: string;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const { rowsHaveMoved } = useMarks();
  const every = useAsked((signal) => api.collections(signal), []);
  const held = useAsked((signal) => api.collectionsHolding(workId, signal), [workId]);
  const [made, setMade] = useState<CollectionSummary[]>([]);
  const [inThem, setInThem] = useState<Set<string> | null>(null);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  useEffect(() => {
    if (held.answer && inThem === null) {
      setInThem(new Set(held.answer.collections));
    }
  }, [held.answer, inThem]);

  const byHand = [
    ...(every.answer ?? []).filter((collection) => collection.made_by_hand),
    ...made.filter((one) => !every.answer?.some((collection) => collection.id === one.id)),
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
      await api.putInCollection(id, [workId], putIn);
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
      const { id } = await api.createCollection(name, [workId]);
      setMade((before) => [
        ...before,
        { id, name, made_by_hand: true, count: 1, cover: null },
      ]);
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
    <Modal title={t("collect.title", { title })} onClose={onClose}>
      {failure && <p className="notice">{t(refusalKey(refusalOf(failure)))}</p>}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}

      {inThem && (
        <>
          {byHand.length === 0 ? (
            <p className="settings-why">{t("collect.none_yet")}</p>
          ) : (
            <ul className="collect-lines">
              {byHand.map((collection) => {
                const ticked = inThem.has(collection.id);
                return (
                  <li key={collection.id}>
                    <button
                      type="button"
                      className={`collect-line${ticked ? " collect-line-in" : ""}`}
                      aria-pressed={ticked}
                      onClick={() => toggle(collection.id)}
                    >
                      <span className="collect-mark" aria-hidden="true">
                        {ticked && <TickIcon size={14} />}
                      </span>
                      <span className="collect-name">{collection.name}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}

          <form
            className="collect-new"
            onSubmit={(event) => {
              event.preventDefault();
              void create();
            }}
          >
            <input
              type="text"
              className="field-line"
              aria-label={t("collect.new")}
              placeholder={t("collect.new")}
              maxLength={80}
              value={typed}
              onChange={(event) => setTyped(event.target.value)}
            />
            <button type="submit" className="button button-small button-accent" disabled={busy || typed.trim() === ""}>
              {t("collect.create")}
            </button>
          </form>
        </>
      )}
    </Modal>
  );
}

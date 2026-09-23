/*
 * Choosing several cards of a grid, and acting on all of them at once.
 *
 * A grid that can be chosen from is wrapped in this. Once one card is chosen,
 * pressing another chooses it too rather than opening it, a bar at the foot
 * of the window says how many are chosen and what can be done with them, and
 * Escape lets go of all of them. Offered only to an account allowed to
 * delete, since deleting is the one thing done with a choice so far.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "../account";
import { TickIcon } from "../icons";
import { useMarks } from "../marks";
import { howMany } from "../readable";
import { pressed } from "../selecting";
import { useSettings } from "../settings";
import { DeleteDialog } from "./deletion";

/** One card that can be chosen, as the question before deleting names it. */
export interface Choosable {
  id: string;
  title: string;
}

interface Selection {
  isChosen: (id: string) => boolean;
  /** Whether anything is chosen, which is when a press chooses rather than
      opens. */
  selecting: boolean;
  /** Chooses or lets go of one card, or chooses every card from the one
      pressed last with `range`. */
  press: (id: string, range: boolean) => void;
}

const SelectionContext = createContext<Selection | null>(null);

/** The choice the grid around this card offers, or nothing when it offers
 *  none. */
export function useSelection(): Selection | null {
  return useContext(SelectionContext);
}

export function Selecting({ items, children }: { items: Choosable[]; children: ReactNode }) {
  const { t } = useSettings();
  const { account } = useAccount();
  const marks = useMarks();
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  const [from, setFrom] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  /* What is still drawn: a card deleted from here is no longer one to
     choose, and a card the grid no longer holds after a change of filter is
     not chosen any more either. */
  const shown = useMemo(() => items.filter((item) => !marks.goneOf(item.id)), [items, marks]);
  const order = useMemo(() => shown.map((item) => item.id), [shown]);
  const picked = useMemo(() => shown.filter((item) => chosen.has(item.id)), [shown, chosen]);
  const selecting = picked.length > 0;

  const press = useCallback(
    (id: string, range: boolean) => {
      setChosen((before) => pressed(order, before, id, from, range));
      setFrom(id);
    },
    [order, from],
  );

  const letGo = useCallback(() => {
    setChosen(new Set());
    setFrom(null);
  }, []);

  useEffect(() => {
    if (!selecting) {
      return;
    }
    // On the window, so Escape closing a panel over the grid stops there.
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        letGo();
      }
    };
    window.addEventListener("keydown", away);
    return () => window.removeEventListener("keydown", away);
  }, [selecting, letGo]);

  const value = useMemo<Selection>(
    () => ({ isChosen: (id) => selecting && chosen.has(id), selecting, press }),
    [chosen, selecting, press],
  );

  if (!account?.may_delete) {
    return <>{children}</>;
  }
  return (
    <SelectionContext.Provider value={value}>
      {children}
      {selecting && (
        <div className="selection-bar" role="toolbar" aria-label={t("selection.label")}>
          <span className="selection-count">{howMany(picked.length, "selection.count", t)}</span>
          {picked.length < shown.length && (
            <button
              type="button"
              className="button button-small"
              onClick={() => setChosen(new Set(order))}
            >
              {t("selection.all")}
            </button>
          )}
          <button
            type="button"
            className="button button-small button-accent"
            onClick={() => setDeleting(true)}
          >
            {t("card.menu.delete")}
          </button>
          <button type="button" className="button button-small" onClick={letGo}>
            {t("selection.cancel")}
          </button>
        </div>
      )}
      {deleting && (
        <DeleteDialog
          works={picked}
          onClose={() => setDeleting(false)}
          onDeleted={() => {
            setDeleting(false);
            letGo();
          }}
        />
      )}
    </SelectionContext.Provider>
  );
}

/** The round mark at the corner of a card that chooses it. */
export function SelectMark({ id }: { id: string }) {
  const { t } = useSettings();
  const selection = useSelection();
  if (!selection) {
    return null;
  }
  const chosen = selection.isChosen(id);
  return (
    <button
      type="button"
      className={`select-mark${chosen ? " select-mark-on" : ""}`}
      aria-pressed={chosen}
      aria-label={t(chosen ? "selection.unchoose" : "selection.choose")}
      title={t(chosen ? "selection.unchoose" : "selection.choose")}
      onClick={(event) => {
        // Drawn over a link to the work, which it must not follow.
        event.preventDefault();
        event.stopPropagation();
        selection.press(id, event.shiftKey);
      }}
    >
      <TickIcon size={16} />
    </button>
  );
}

/** What pressing a card does while cards are being chosen: choose it rather
 *  than open it. Nothing otherwise, and the card opens as it always does. */
export function useChoosingPress(id: string): {
  chosen: boolean;
  selecting: boolean;
  onClick: (event: React.MouseEvent) => void;
} {
  const selection = useSelection();
  return {
    chosen: selection?.isChosen(id) ?? false,
    selecting: selection?.selecting ?? false,
    onClick: (event) => {
      if (selection?.selecting) {
        event.preventDefault();
        selection.press(id, event.shiftKey);
      }
    },
  };
}

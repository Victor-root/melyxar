/*
 * Choosing several cards of a grid, and acting on all of them at once.
 *
 * A grid that can be chosen from is wrapped in this. Once one card is chosen,
 * pressing another chooses it too rather than opening it, a bar at the foot
 * of the window says how many are chosen and what can be done with them, and
 * Escape lets go of all of them. Each action is offered only for what the
 * account may do, and applies to the chosen cards it makes sense for.
 */

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "../account";
import type { Card } from "../api";
import {
  ClockIcon,
  CollectionIcon,
  HeartIcon,
  PinIcon,
  PlaylistIcon,
  TickIcon,
} from "../icons";
import { useMarks } from "../marks";
import { howMany } from "../readable";
import { IN_THE_BANNER, marksThemAll, pressed } from "../selecting";
import { useSettings } from "../settings";
import { DeleteDialog } from "./deletion";
import { COLLECTIONS, PLAYLISTS, PutInListsDialog } from "./lists";

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

export function Selecting({ items, children }: { items: Card[]; children: ReactNode }) {
  const { t } = useSettings();
  const { account } = useAccount();
  const marks = useMarks();
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  const [from, setFrom] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [listing, setListing] = useState<"collect" | "playlist" | null>(null);

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

  if (!account) {
    return <>{children}</>;
  }

  const favourite = marksThemAll(picked, marks.favouriteOf);
  const later = marksThemAll(picked, marks.watchLaterOf);
  const markable = picked.filter((card) => card.watched_marks);
  const watched = marksThemAll(markable, (card) => marks.seenOf(card) === "watched");
  const collectable = account.may_manage_collections
    ? picked.filter((card) => card.kind !== "season" && card.kind !== "episode")
    : [];
  const listable = picked.filter(
    (card) => card.kind === "movie" || card.kind === "episode" || card.kind === "video",
  );
  const pinned = marksThemAll(picked, (card) => marks.pinnedOf(card) === true);
  const tooManyToPin = pinned && picked.length > IN_THE_BANNER;

  /** One action of the bar, drawn as its shape with its words under the
      pointer: seven words in a row did not fit a phone. */
  const act = (key: string, mark: ReactNode, onPress: () => void, why?: string) => (
    <button
      type="button"
      className="selection-act"
      aria-label={why ?? t(key)}
      title={why ?? t(key)}
      aria-disabled={why !== undefined}
      onClick={() => {
        if (why === undefined) {
          onPress();
        }
      }}
    >
      {mark}
    </button>
  );

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
          {act(
            favourite ? "card.menu.favourite" : "card.menu.unfavourite",
            <HeartIcon size={17} filled={!favourite} />,
            () => picked.forEach((card) => marks.setFavourite(card, favourite)),
          )}
          {act(
            later ? "card.menu.watch_later" : "card.menu.unwatch_later",
            <ClockIcon size={17} />,
            () => picked.forEach((card) => marks.setWatchLater(card, later)),
          )}
          {markable.length > 0 &&
            act(
              watched ? "card.menu.mark_watched" : "card.menu.mark_unwatched",
              <TickIcon size={17} />,
              () => markable.forEach((card) => marks.setWatched(card, watched)),
            )}
          {listable.length > 0 &&
            act("card.menu.playlist", <PlaylistIcon size={17} />, () => setListing("playlist"))}
          {collectable.length > 0 &&
            act("card.menu.collection", <CollectionIcon size={17} />, () => setListing("collect"))}
          {account.is_administrator &&
            act(
              pinned ? "card.menu.pin" : "card.menu.unpin",
              <PinIcon size={17} filled={!pinned} />,
              () => picked.forEach((card) => marks.setPinned(card, pinned)),
              tooManyToPin ? t("selection.too_many_to_pin", { count: IN_THE_BANNER }) : undefined,
            )}
          {account.may_delete && (
            <button
              type="button"
              className="button button-small button-accent"
              onClick={() => setDeleting(true)}
            >
              {t("card.menu.delete")}
            </button>
          )}
          <button type="button" className="button button-small" onClick={letGo}>
            {t("selection.cancel")}
          </button>
        </div>
      )}
      {listing && (
        <PutInListsDialog
          works={listing === "collect" ? collectable : listable}
          lists={listing === "collect" ? COLLECTIONS : PLAYLISTS}
          words={listing}
          onClose={() => setListing(null)}
        />
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
      <TickIcon size={14} />
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

/*
 * What a card offers beyond pressing it.
 *
 * Every action this interface is meant to have is listed, including the ones
 * the engine cannot do yet. Those are drawn greyed and say when rather than
 * being left out: a function nobody can see is a function nobody knows is
 * coming, and the shape of the menu stops moving under people's hands as each
 * one is wired.
 *
 * What is hidden rather than greyed is what this account has no right to. A
 * greyed entry says "later"; an entry somebody will never be allowed to press
 * says nothing worth saying, and saying it tells them what the server holds.
 *
 * Drawn over the page rather than inside the card. A card clips what it
 * holds, both to round its picture and because the browser is told it may
 * skip drawing the ones off screen, and a menu drawn inside one comes out cut
 * in half. So it is placed where the button is and drawn at the top of the
 * page, which is also what keeps it above the cards after it.
 */

import { Fragment, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { api } from "../api";
import type { Card } from "../api";
import { useAccount } from "../account";
import { refusalAbout } from "../asking";
import { useMarks } from "../marks";
import { useSettings } from "../settings";
import { isCatalogued, playsOnItsOwn } from "../works";
import { DeleteDialog } from "./deletion";
import { IdentifyDialog } from "./identify";
import { DetailsDialog } from "./details";
import { DownloadDialog } from "./download";
import { ForgetIdentityDialog } from "./forgetting";
import { PicturesDialog } from "./pictures";
import { playInTurn } from "../queue";
import { useCardsInOrder } from "./in-order";
import { COLLECTIONS, PLAYLISTS, PutInListsDialog } from "./lists";
import { OnlineSubtitlesDialog } from "./online-subtitles";
import { useSelection } from "./selection";
import { useToast } from "./toasts";
import {
  ClockIcon,
  CollectionIcon,
  DeleteIcon,
  DownloadIcon,
  EditIcon,
  ForgetIcon,
  HeartIcon,
  IdentifyIcon,
  ImageIcon,
  PinIcon,
  PlayAllIcon,
  PlayIcon,
  PlaylistIcon,
  RefreshIcon,
  SelectIcon,
  SubtitlesIcon,
  TickIcon,
} from "../icons";

/** How big the shape at the left of a line is. */
const SHAPE = 17;

/** One line of the menu. */
interface Entry {
  key: string;
  /** Its shape, which is what the eye finds before the words are read. */
  mark: React.ReactNode;
  /** Left out entirely when false: a right nobody has is not a grey line. */
  allowed?: boolean;
  /** Greyed, and saying when under the pointer, where there is no engine
      behind it yet. Written beside every such line it became a second column
      of the same three words repeated down the menu. */
  later?: boolean;
  act?: () => void;
}

/** How far from the edge of the window a menu is allowed to sit. */
const OFF_THE_EDGE = 8;

/** What a work's menu is opened from, and what it draws. */
export interface WorkMenu {
  /** Whether it is open, for the button that opens it to say so. */
  open: boolean;
  /** Opens it under a button, or shuts it when it was open. */
  toggle: (button: HTMLElement | null) => void;
  /** The menu and the panels its lines open, to be drawn beside the button. */
  drawn: ReactNode;
}

/**
 * A work's menu and the panels its lines open, for a card and for the page of
 * the work alike: the same lines, answering the same way.
 *
 * The panels are held here rather than by the menu, which is taken away the
 * moment one of its lines is pressed.
 */
export function useWorkMenu(
  card: Card,
  after: {
    /** Said once the work was named by hand. */
    identified: () => void;
    /** Said whenever the pictures it wears changed. */
    picturesChanged: () => void;
    /** Said once its details were written by hand. */
    detailsChanged: () => void;
    /** Said once it is gone, for a page that has nothing left to show. */
    deleted?: () => void;
  },
): WorkMenu {
  const [from, setFrom] = useState<{ rect: DOMRect; button: HTMLElement } | null>(null);
  const [identifying, setIdentifying] = useState(false);
  const [choosingPictures, setChoosingPictures] = useState(false);
  const [writingDetails, setWritingDetails] = useState(false);
  const [forgetting, setForgetting] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [collecting, setCollecting] = useState(false);
  const [listing, setListing] = useState(false);
  const [subtitling, setSubtitling] = useState(false);
  const { t } = useSettings();
  const toast = useToast();

  /* Asked straight away, with nothing to choose first; what came of it is
     said in the corner. */
  const refresh = async () => {
    try {
      const { outcome } = await api.refreshWork(card.id);
      toast(
        outcome === "described"
          ? { state: "ok", title: t("refresh.done", { title: card.title }) }
          : { state: "attention", title: t(`refresh.${outcome}`), detail: t(`refresh.${outcome}_why`) },
      );
      after.identified();
    } catch (error) {
      toast({ state: "trouble", title: t("refresh.failed"), detail: t(refusalAbout(error, "refresh")) });
    }
  };
  const [deleting, setDeleting] = useState(false);
  const shut = useCallback(() => setFrom(null), []);

  const toggle = useCallback((button: HTMLElement | null) => {
    setFrom((was) => (was || !button ? null : { rect: button.getBoundingClientRect(), button }));
  }, []);

  const drawn = (
    <>
      {from && (
        <CardMenu
          card={card}
          from={from.rect}
          openedBy={from.button}
          onIdentify={() => setIdentifying(true)}
          onEditImages={() => setChoosingPictures(true)}
          onEditDetails={() => setWritingDetails(true)}
          onForgetIdentity={() => setForgetting(true)}
          onRefresh={refresh}
          onDownload={() => setDownloading(true)}
          onCollect={() => setCollecting(true)}
          onPlaylist={() => setListing(true)}
          onEditSubtitles={() => setSubtitling(true)}
          onDelete={() => setDeleting(true)}
          onClose={shut}
        />
      )}
      {identifying && (
        <IdentifyDialog
          workId={card.id}
          title={card.title}
          onClose={() => setIdentifying(false)}
          onIdentified={after.identified}
        />
      )}
      {choosingPictures && (
        <PicturesDialog
          workId={card.id}
          onClose={() => setChoosingPictures(false)}
          onChanged={after.picturesChanged}
        />
      )}
      {writingDetails && (
        <DetailsDialog
          workId={card.id}
          series={card.kind === "series"}
          onClose={() => setWritingDetails(false)}
          onChanged={after.detailsChanged}
        />
      )}
      {downloading && (
        <DownloadDialog
          workId={card.id}
          title={card.title}
          onClose={() => setDownloading(false)}
        />
      )}
      {collecting && (
        <PutInListsDialog
          works={[card]}
          lists={COLLECTIONS}
          words="collect"
          onClose={() => setCollecting(false)}
        />
      )}
      {subtitling && card.source && (
        <OnlineSubtitlesDialog sourceId={card.source} title={card.title} onClose={() => setSubtitling(false)} />
      )}
      {listing && (
        <PutInListsDialog
          works={[card]}
          lists={PLAYLISTS}
          words="playlist"
          onClose={() => setListing(false)}
        />
      )}
      {forgetting && (
        <ForgetIdentityDialog
          workId={card.id}
          title={card.title}
          onClose={() => setForgetting(false)}
          onForgotten={after.identified}
        />
      )}
      {deleting && (
        <DeleteDialog
          works={[card]}
          onClose={() => setDeleting(false)}
          onDeleted={() => {
            setDeleting(false);
            after.deleted?.();
          }}
        />
      )}
    </>
  );

  return { open: from !== null, toggle, drawn };
}

export function CardMenu({
  card,
  from,
  openedBy,
  onIdentify,
  onEditImages,
  onEditDetails,
  onForgetIdentity,
  onRefresh,
  onDownload,
  onCollect,
  onPlaylist,
  onEditSubtitles,
  onDelete,
  onClose,
}: {
  card: Card;
  /** The button it was opened from, which is where it is drawn. */
  from: DOMRect;
  /** That button itself. A press on it is the one press anywhere outside
      this menu that must not close it, because the button is already going
      to: closing here as well shut it and let the button open it again, so
      it never seemed to close at all. */
  openedBy: HTMLElement | null;
  /** Opens the panel that says by hand what this work is. Asked of whoever
      drew this menu rather than opened from here: this menu is taken away the
      moment anything in it is pressed, and a panel drawn inside it would go
      with it. */
  onIdentify: () => void;
  /** Opens the panel that chooses the pictures this work wears, for the same
      reason as the one above. */
  onEditImages: () => void;
  /** Opens the window its details are written in by hand, for the same
      reason again. */
  onEditDetails: () => void;
  /** Asks before taking away what a provider said about it, for the same
      reason again. */
  onForgetIdentity: () => void;
  /** Asks the provider again about it. */
  onRefresh: () => void;
  /** Hands over its file to keep, or asks which one when it has several. */
  onDownload: () => void;
  /** Opens the window that puts it in the server's collections. */
  onCollect: () => void;
  /** Opens the window that puts it in this account's playlists. */
  onPlaylist: () => void;
  /** Opens the window of its subtitles, and those offered online. */
  onEditSubtitles: () => void;
  /** Opens the question put before this work is deleted, for the same
      reason again. */
  onDelete: () => void;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const { account } = useAccount();
  const marks = useMarks();
  const selection = useSelection();
  const holder = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);

  // Placed once it has been drawn, since where it fits depends on how tall it
  // came out: above the button when there is no room under it, and pulled
  // back from the edge rather than hanging off it.
  useLayoutEffect(() => {
    const menu = holder.current;
    if (!menu) {
      return;
    }
    const { width, height } = menu.getBoundingClientRect();
    const under = from.bottom + 6;
    const top =
      under + height > window.innerHeight - OFF_THE_EDGE
        ? Math.max(OFF_THE_EDGE, from.top - height - 6)
        : under;
    const left = Math.min(
      Math.max(OFF_THE_EDGE, from.right - width),
      window.innerWidth - width - OFF_THE_EDGE,
    );
    setAt({ top, left });
  }, [from]);

  useEffect(() => {
    const elsewhere = (event: MouseEvent) => {
      const where = event.target as Node;
      if (!holder.current?.contains(where) && !openedBy?.contains(where)) {
        onClose();
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        // Shut the menu and nothing else: the page under it answers to the
        // same key by going back.
        event.stopPropagation();
        onClose();
      }
    };
    /* Drawn at a place rather than next to the button, so a page that moves
       under it leaves it behind: it closes instead of floating. Its own
       scrolling is not the page moving, though, and watching every scroll in
       the document made a menu too tall for the window close the moment
       anybody reached for what was below the fold. */
    const moved = (event: Event) => {
      if (!holder.current?.contains(event.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    window.addEventListener("scroll", moved, true);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
      window.removeEventListener("scroll", moved, true);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose, openedBy]);

  const seen = marks.seenOf(card);
  const favourite = marks.favouriteOf(card);
  const later = marks.watchLaterOf(card);
  const pinned = marks.pinnedOf(card) === true;
  const playable = playsOnItsOwn(card);
  /* This card and every one after it in its row or grid that plays, in the
     order shown. */
  const inOrder = useCardsInOrder();
  const here = inOrder?.findIndex((one) => one.id === card.id) ?? -1;
  const fromHere =
    playable && inOrder && here >= 0
      ? inOrder.slice(here).filter(playsOnItsOwn).map((one) => one.id)
      : [];
  const catalogued = isCatalogued(card.identification);

  /* Kept in families with a rule between them, so a long menu is read by
     what each part is for: watching, keeping track, lists, the details,
     naming the work, and the one line that loses something. */
  const groups: Entry[][] = [
    [
      {
        key: "play",
        mark: <PlayIcon size={SHAPE} />,
        allowed: playable,
        act: () => navigate(`/work/${card.id}?play`),
      },
      {
        key: "play_from_here",
        mark: <PlayAllIcon size={SHAPE} />,
        allowed: fromHere.length > 1,
        act: () => {
          playInTurn(fromHere);
          navigate(`/work/${card.id}?play`);
        },
      },
      {
        key: "download",
        mark: <DownloadIcon size={SHAPE} />,
        allowed:
          account?.may_download === true && (card.kind === "movie" || card.kind === "episode"),
        act: onDownload,
      },
    ],
    [
      {
        key: favourite ? "unfavourite" : "favourite",
        mark: <HeartIcon size={SHAPE} filled={favourite} />,
        act: () => marks.setFavourite(card, !favourite),
      },
      {
        key: later ? "unwatch_later" : "watch_later",
        mark: <ClockIcon size={SHAPE} />,
        act: () => marks.setWatchLater(card, !later),
      },
      {
        key: seen === "watched" ? "mark_unwatched" : "mark_watched",
        mark: <TickIcon size={SHAPE} />,
        allowed: card.watched_marks,
        act: () => marks.setWatched(card, seen !== "watched"),
      },
      {
        key: "select",
        mark: <SelectIcon size={SHAPE} />,
        allowed: selection !== null,
        act: () => selection?.press(card.id, false),
      },
    ],
    [
      {
        key: "collection",
        mark: <CollectionIcon size={SHAPE} />,
        allowed: account?.may_manage_collections === true && card.kind !== "season" && card.kind !== "episode",
        act: onCollect,
      },
      {
        key: "playlist",
        mark: <PlaylistIcon size={SHAPE} />,
        allowed: card.kind === "movie" || card.kind === "episode" || card.kind === "video",
        act: onPlaylist,
      },
      {
        /* Says what it will do rather than always the same thing. Nothing is
           known about a card that arrives from the server, so it offers to put
           it there; once it has been put there from here, it offers to take it
           back off. */
        key: pinned ? "unpin" : "pin",
        mark: <PinIcon size={SHAPE} filled={pinned} />,
        allowed: account?.is_administrator === true,
        act: () => marks.setPinned(card, !pinned),
      },
    ],
    [
      {
        key: "edit_metadata",
        mark: <EditIcon size={SHAPE} />,
        allowed: account?.is_administrator === true,
        act: onEditDetails,
      },
      {
        key: "edit_images",
        mark: <ImageIcon size={SHAPE} />,
        allowed: account?.is_administrator === true && catalogued,
        act: onEditImages,
      },
      {
        key: "edit_subtitles",
        mark: <SubtitlesIcon size={SHAPE} />,
        allowed:
          account?.is_administrator === true &&
          card.source !== null &&
          (card.kind === "movie" || card.kind === "episode"),
        act: onEditSubtitles,
      },
    ],
    [
      {
        key: "identify",
        mark: <IdentifyIcon size={SHAPE} />,
        allowed: account?.is_administrator === true && catalogued,
        act: onIdentify,
      },
      {
        key: "forget_identity",
        mark: <ForgetIcon size={SHAPE} />,
        allowed:
          account?.is_administrator === true &&
          (card.kind === "movie" || card.kind === "series") &&
          (card.identification === "identified" || card.identification === "manual"),
        act: onForgetIdentity,
      },
      {
        key: "refresh",
        mark: <RefreshIcon size={SHAPE} />,
        allowed: account?.is_administrator === true && catalogued,
        act: onRefresh,
      },
    ],
    [
      {
        key: "delete",
        mark: <DeleteIcon size={SHAPE} />,
        allowed: account?.may_delete === true,
        act: onDelete,
      },
    ],
  ];

  return createPortal(
    <div
      className="header-menu-list card-menu"
      ref={holder}
      role="menu"
      style={{
        top: at?.top ?? from.bottom + 6,
        left: at?.left ?? from.left,
        // Drawn once it knows where it goes, so nobody sees it land.
        visibility: at ? "visible" : "hidden",
      }}
    >
      {groups
        .map((group) => group.filter((entry) => entry.allowed !== false))
        .filter((group) => group.length > 0)
        .map((group, at) => (
          <Fragment key={group[0].key}>
            {at > 0 && <div className="header-menu-divider" aria-hidden="true" />}
            {group.map((entry) => (
              <button
                key={entry.key}
                type="button"
                role="menuitem"
                className={`header-menu-line${entry.later ? " header-menu-later" : ""}`}
                /* Said rather than disabled outright: a disabled button takes no
                   pointer, so the very tooltip that explains why it is grey
                   never appears. */
                aria-disabled={entry.later}
                title={entry.later ? t("nav.later") : undefined}
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  if (entry.later) {
                    return;
                  }
                  entry.act?.();
                  onClose();
                }}
              >
                {entry.mark}
                {t(`card.menu.${entry.key}`)}
              </button>
            ))}
          </Fragment>
        ))}
    </div>,
    document.body,
  );
}

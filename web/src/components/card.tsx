/*
 * One card in a grid or a row.
 *
 * The colour of the poster is painted behind it before the picture arrives,
 * so a grid has colour from the first moment instead of a wall of grey holes.
 * A film nobody recognised keeps its place and wears a marker: a file set
 * aside is a file forgotten.
 *
 * What the hover offers is the whole ergonomics of this interface: play in
 * the middle, watched at the top right, liked and the rest at the bottom
 * right. None of it grows the card or moves its neighbours, because a grid
 * that reflows under the pointer is a grid nobody can aim at.
 *
 * And none of it is hover alone. A finger has no hover, so everything here is
 * reachable by a press on the card's own menu, and the menu opens on a tap
 * rather than on a pointer that never arrives. The mobile interface is a
 * later worksite; a card that could only be used with a mouse would be a
 * rewrite when it comes rather than an adjustment.
 */

import { createContext, memo, useContext, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type { Card as CardData } from "../api";
import { useMarks } from "../marks";
import { QuietLink, useGoTo } from "../navigating";
import { useSettings } from "../settings";
import { playsOnItsOwn } from "../works";
import { useShownPicture } from "./picture";
import { useWorkMenu } from "./cardmenu";
import { RunningPreview, useHoverPreview } from "./hover-preview";
import { SelectMark, useChoosingPress } from "./selection";
import { SeenMark } from "./seen";
import { HeartIcon, IdentifyIcon, ToolIcon, PlayIcon } from "../icons";
import { lengthOfAPlay } from "../watching";

/** How a card is laid out: standing like a poster, or lying like a still. */
export type CardShape = "standing" | "lying";

/**
 * How much room the picture of a card really has, for the browser to pick a
 * size with.
 *
 * Written out in plain lengths, and it has to be: this attribute is read
 * before the page has a stylesheet, so it knows nothing of the widths the
 * interface keeps as tokens. A `var()` in here is not a length the browser
 * can use, the whole thing is thrown away, and what is assumed instead is the
 * full width of the window. Every card then asked for the largest poster held,
 * eight hundred points across, to draw it at a hundred and sixty eight. That
 * is what left half a page as coloured rectangles until the pointer went
 * looking for them.
 *
 * The lengths here are the ones the stylesheet draws: a card is 168 points
 * wide, 186 past a wide screen, and a lying card is a little over one and a
 * half times that. The screen's own fineness is the browser's business and it
 * takes it into account by itself, which is how a fine screen still gets the
 * larger picture without anybody asking for it here.
 */
export const ROOM_FOR_A_PICTURE: Record<CardShape, string> = {
  standing: "(max-width: 700px) 40vw, (min-width: 1400px) 186px, 168px",
  lying: "(max-width: 700px) 70vw, (min-width: 1400px) 301px, 272px",
};

/**
 * The same for a standing card in a row, which keeps its width where a grid
 * shares the screen out: 132 points on a narrow screen. Asked for at the
 * width of a grid's card instead, every poster of a row on a phone came at
 * twice the size it is drawn at, four times the pixels to fetch and decode
 * for a picture that looks the same.
 */
export const ROOM_IN_A_ROW = "(max-width: 860px) 132px, (min-width: 1400px) 186px, 168px";

/** Whether the cards drawn here stand in a row of their own width, said by
 *  the row. A row showing one card at a time on a phone gives it the whole
 *  width, so it says nothing. */
export const InARow = createContext(false);

/**
 * The pictures a grid or a row holds, fetched ahead rather than as they near
 * the screen.
 *
 * A grid or a row starts once the page it stands on has been drawn, and every
 * picture off the screen is fetched then, in the order of the cards. Fetched
 * only as they neared the screen instead, the pictures arrived while the page
 * or the row was being scrolled, and each had to be put in place in the
 * middle of it: the stutter of a first scroll through a library that was gone
 * the second time.
 *
 * A few at a time, the next one asked for as one arrives. The browser fetches
 * no more than a handful at once anyway, and asked for all of them in one go
 * it queued thousands, then handled each answer while the page was opening:
 * on a phone, seconds of a frozen page for the two thousand posters of a
 * library of films. The cards drawn later are found as they arrive.
 *
 * Said to the pictures themselves rather than to the cards. It was a state
 * the cards read, so each row drew every one of its cards again when the
 * moment came, one row after another, as the page was opening: measured on a
 * home page slowed four times, four or five frames of a hundred milliseconds
 * each right after it was drawn. A picture's own setting is changed in place.
 */
export function useFetchingAhead(holder: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    /* The pictures still waiting, in the order of the cards, looked for again
       once a picture has come or gone. */
    let waiting: HTMLImageElement[] | null = null;
    let at = 0;
    /* The ones asked for and not arrived yet. A card taken away takes its
       picture with it, which then never says it arrived. */
    const asking = new Set<HTMLImageElement>();
    let started = false;
    const askMore = () => {
      waiting ??= Array.from(holder.current?.querySelectorAll<HTMLImageElement>("img[loading=lazy]") ?? []);
      while (asking.size < FETCHED_AHEAD_AT_ONCE && at < waiting.length) {
        const picture = waiting[at];
        at += 1;
        if (picture.loading !== "lazy" || !picture.isConnected) {
          continue;
        }
        picture.loading = "eager";
        // One already there says nothing more, and leaves its place free.
        if (picture.complete) {
          continue;
        }
        asking.add(picture);
        const arrived = () => {
          picture.removeEventListener("load", arrived);
          picture.removeEventListener("error", arrived);
          asking.delete(picture);
          askMore();
        };
        picture.addEventListener("load", arrived);
        picture.addEventListener("error", arrived);
      }
    };
    const holdsAPicture = (node: Node) =>
      node instanceof HTMLElement && (node.matches("img") || node.querySelector("img") !== null);
    const changed = new MutationObserver((records) => {
      if (records.some((record) => [...record.addedNodes, ...record.removedNodes].some(holdsAPicture))) {
        waiting = null;
        at = 0;
        for (const picture of asking) {
          if (!picture.isConnected) {
            asking.delete(picture);
          }
        }
        if (started) {
          askMore();
        }
      }
    });
    if (holder.current) {
      changed.observe(holder.current, { childList: true, subtree: true });
    }
    const start = () => {
      started = true;
      askMore();
    };
    // Safari has no idle moments to offer, so it is given a second instead.
    if (typeof window.requestIdleCallback === "function") {
      const asked = window.requestIdleCallback(start, { timeout: 2000 });
      return () => {
        window.cancelIdleCallback(asked);
        changed.disconnect();
      };
    }
    const asked = setTimeout(start, 1000);
    return () => {
      clearTimeout(asked);
      changed.disconnect();
    };
  }, [holder]);
}

/** How many pictures are fetched ahead at once: a little more than a browser
 *  fetches at the same time from one server. */
const FETCHED_AHEAD_AT_ONCE = 8;

export const Card = memo(function Card({
  card,
  shape = "standing",
  /** How far in, between nought and one, when a row knows and the card does
      not: what somebody left halfway carries it in the row's own answer. */
  watched,
  /** The name the card leads with, when it is not the work's own: an episode
      is shown under the name of its series, which is the only name anybody
      remembers. */
  lead,
  /** The faint line under it, in place of the year. */
  note,
  /** A few words at the right of the name, where a row of stills needs them:
      how much of it is left. */
  trailing,
  /** Whether the name is drawn under the picture. A line of a list says it
      beside the picture instead. */
  named = true,
  /** Whether the corner that marks it watched is drawn. A line of a list
      that has a watched button of its own beside the picture leaves it out,
      rather than offer the same button twice. */
  cornered = true,
  /** Whether this is the work the page is about, in a row of the ones around
      it: lit at rest, and what the row opens on. */
  here = false,
}: {
  card: CardData;
  shape?: CardShape;
  watched?: number;
  lead?: string;
  note?: string;
  trailing?: string;
  named?: boolean;
  cornered?: boolean;
  here?: boolean;
}) {
  const { t } = useSettings();
  const navigate = useGoTo();
  const marks = useMarks();
  const inARow = useContext(InARow);
  /* A lying card is nearly twice as wide as it is tall and a poster is two
     thirds as wide as it is tall: filling one with the other cuts a band out
     of the middle of the picture. So such a row is given something wide, and
     falls back to the poster only where the server had nothing wide to
     send. */
  const { picture: poster, itDidNotLoad } = useShownPicture(
    shape === "lying" && card.wide.length > 0 ? card.wide : card.poster,
  );
  /* The menu is drawn over the page rather than inside the card, which
     clips what it holds. The card says what it is from the answer the page
     was drawn from, so a renamed or repainted work has the page read
     again rather than mended here. */
  const kebab = useRef<HTMLButtonElement>(null);
  const menu = useWorkMenu(card, {
    identified: marks.rowsHaveMoved,
    picturesChanged: marks.rowsHaveMoved,
    detailsChanged: marks.rowsHaveMoved,
  });
  const choosing = useChoosingPress(card.id);
  /* The buttons that show only under the pointer are made the first time a
     card is reached, by the pointer or the keyboard, and kept from then on.
     A finger never reaches them: a touch screen has nothing to show them on.
     Made on every card, invisible, they were near half of what a grid of a
     few hundred films is made of, and the browser walks all of it for every
     card that scrolls into view: measured, a fifth of what drawing the page
     costs while it moves. Kept once made, the pointer going back and forth
     over a card makes nothing again, and they still fade out as they did. */
  const [reached, setReached] = useState(false);
  const reach = () => setReached(true);
  const shown = reached || menu.open;
  /* A video of one's own runs through what it holds under a resting pointer. */
  const preview = useHoverPreview(card.kind === "video" && card.source !== null);

  const unknown = card.identification === "unidentified" || card.identification === "pending";
  const seen = marks.seenOf(card);
  const favourite = marks.favouriteOf(card);
  /* A series plays the episode it carries on with: its page works out which
     and hands over to it. */
  const playable = playsOnItsOwn(card);
  /* Gone once marked watched, which lets go of where it was left: said at
     once, not after the next reading of the page. */
  const resume = marks.resumeOf(card);
  const length = lengthOfAPlay(card);
  const howFar = watched ?? (resume !== null && length ? resume / length : undefined);

  if (marks.goneOf(card.id)) {
    return null;
  }

  const stop = (doing: () => void) => (event: React.MouseEvent) => {
    // The card is one big link; everything drawn on top of it has to say so.
    event.preventDefault();
    event.stopPropagation();
    doing();
  };

  return (
    <article
      className={`card card-${shape}${here ? " card-here" : ""}${choosing.selecting ? " selecting" : ""}${choosing.chosen ? " card-chosen" : ""}`}
      data-card={card.id}
      style={{ ["--card-color" as string]: card.color ?? "var(--surface-raised)" }}
      onPointerEnter={(event) => {
        if (event.pointerType !== "touch") {
          reach();
          preview.begin();
        }
      }}
      onPointerLeave={preview.end}
      onFocus={reach}
    >
      <div className="card-picture">
        {poster ? (
          <img
            src={poster.src}
            srcSet={poster.srcSet}
            sizes={inARow && shape === "standing" ? ROOM_IN_A_ROW : ROOM_FOR_A_PICTURE[shape]}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={itDidNotLoad}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {card.title.slice(0, 1)}
          </span>
        )}
        {preview.on && card.source !== null && <RunningPreview source={card.source} />}

        {/* The whole card leads to the work. Stretched over the picture
            rather than wrapped around everything, so the buttons drawn on top
            are buttons and not parts of a link. */}
        <QuietLink
          className="card-open"
          to={`/work/${card.id}`}
          title={card.title}
          draggable={false}
          aria-current={here ? "page" : undefined}
          onClick={choosing.onClick}
        >
          <span className="visually-hidden">{card.title}</span>
        </QuietLink>

        {/* The reason wins over the state: knowing a film is not identified is
            what the grid already showed, knowing why is what sends somebody to
            rename a file rather than to press the button again. */}
        {unknown && (
          <span
            className={`card-flag${card.identification === "unidentified" ? " card-flag-attention" : ""}`}
          >
            {card.identification === "unidentified" && <IdentifyIcon size={12} />}
            {card.identification_note
              ? t(`note.short.${card.identification_note}`)
              : t(card.identification === "pending" ? "work.pending" : "work.unidentified")}
          </span>
        )}

        {/* Where somebody is with it: how many episodes are left, or a tick
            once there are none and for a film that was watched. One badge,
            the same one the player draws, and under the pointer the button
            that marks and unmarks. */}
        {cornered && card.watched_marks && (
          <SeenMark
            watched={seen === "watched"}
            episodes={card.episodes}
            unwatched={marks.unwatchedOf(card)}
            onPress={(watched) => marks.setWatched(card, watched)}
            offered={shown}
          />
        )}

        {shown && (
          <div className="card-hover">
            {playable && (
              <button
                type="button"
                className="card-play"
                aria-label={t("work.play")}
                title={t("work.play")}
                onClick={stop(() => navigate(`/work/${card.id}?play`))}
              >
                <PlayIcon size={32} />
              </button>
            )}

            <div className="card-corner">
              <button
                type="button"
                className={`card-mark${favourite ? " card-mark-on" : ""}`}
                aria-pressed={favourite}
                aria-label={t(favourite ? "card.unfavourite" : "card.favourite")}
                title={t(favourite ? "card.unfavourite" : "card.favourite")}
                onClick={stop(() => marks.setFavourite(card, !favourite))}
              >
                <HeartIcon size={17} filled={favourite} />
              </button>
              <button
                ref={kebab}
                type="button"
                className={`card-mark${menu.open ? " card-mark-on" : ""}`}
                aria-label={t("card.more")}
                title={t("card.more")}
                aria-expanded={menu.open}
                onClick={stop(() => menu.toggle(kebab.current))}
              >
                <ToolIcon size={17} />
              </button>
            </div>
          </div>
        )}

        {/* Only once a choice is being made: it starts from the card's menu. */}
        {choosing.selecting && <SelectMark id={card.id} />}

        {/* How far in this film already is, drawn on the picture itself: it is
            the one thing that tells two cards of a row apart at a glance. */}
        {howFar !== undefined && howFar > 0 && (
          <span className="card-progress" aria-hidden="true">
            <span
              className="card-progress-done"
              style={{ width: `${Math.min(howFar, 1) * 100}%` }}
            />
          </span>
        )}

      </div>

      {/* The name, and at its right what a row of half watched films is read
          for: how much of each one is left. Underneath, which episode it is,
          or the year for anything that is not one. */}
      {named && (
        <>
          <span className="card-line">
            <span className="card-title">{lead ?? card.title}</span>
            {trailing && <span className="card-trailing">{trailing}</span>}
          </span>
          <span className="card-year">{note ?? card.year ?? ""}</span>
        </>
      )}

      {menu.drawn}
    </article>
  );
});

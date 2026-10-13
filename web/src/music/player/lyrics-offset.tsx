/*
 * Moving the lyrics of the song playing sooner or later by hand, for an
 * administrator: a button of the player opening the same panel the player
 * of films opens for its subtitles, over the button on a wide screen and up
 * from the foot of a phone.
 *
 * The lines on screen move at once, through the shared marks; the server is
 * told once the hand rests, and keeps it for the song and every account.
 */

import { useEffect, useRef, useState } from "react";
import { useAccount } from "../../account";
import { refusalOf } from "../../asking";
import { BottomSheet } from "../../components/bottom-sheet";
import { refusalKey } from "../../i18n";
import { PHONE, useMediaQuery } from "../../media-query";
import { useToast } from "../../notifications/toasts";
import { Line, Menu, OffsetSlider, offsetSaid } from "../../player/menu";
import { ICON } from "../../player/sound";
import { useSettings } from "../../settings";
import { music as server } from "../api";
import { useMusicMarks } from "../marks";
import { LyricsShiftIcon } from "./icons";
import { FURTHEST_OFFSET_MS, OFFSET_STEP_MS, nudgedOffset } from "./lyrics";

/** How long the hand rests before the server is told. */
const SENT_AFTER_MS = 600;

export function LyricsShiftButton({ song }: { song: string }) {
  const { t } = useSettings();
  const { account } = useAccount();
  const [open, setOpen] = useState(false);
  if (!account?.is_administrator) {
    return null;
  }
  return (
    <span className="music-lyrics-shift">
      <button
        type="button"
        className={`player-button${open ? " player-button-lit" : ""}`}
        aria-expanded={open}
        aria-label={t("music.lyrics_offset")}
        title={t("music.lyrics_offset")}
        onClick={() => setOpen(!open)}
      >
        <LyricsShiftIcon size={ICON} />
      </button>
      {open && <ShiftPanel key={song} song={song} onClose={() => setOpen(false)} />}
    </span>
  );
}

function ShiftPanel({ song, onClose }: { song: string; onClose: () => void }) {
  const { t } = useSettings();
  const toast = useToast();
  const onAPhone = useMediaQuery(PHONE);
  const { lyricsAt, lyricsShift, shiftLyrics } = useMusicMarks();
  // What the server holds, and whether the song has stamped lines at all.
  const [saved, setSaved] = useState<{ offsetMs: number; stamped: boolean } | null>(null);
  const sending = useRef<{ timer: number; send: () => void } | null>(null);
  const box = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const stop = new AbortController();
    server.lyrics(song, stop.signal).then(
      (found) => setSaved({ offsetMs: found?.offset_ms ?? 0, stamped: (found?.lines.length ?? 0) > 0 }),
      () => {
        if (!stop.signal.aborted) {
          setSaved({ offsetMs: 0, stamped: false });
        }
      },
    );
    return () => stop.abort();
  }, [song, lyricsAt]);

  // What is still waiting is sent when the panel shuts.
  useEffect(
    () => () => {
      const waiting = sending.current;
      if (waiting) {
        window.clearTimeout(waiting.timer);
        waiting.send();
      }
    },
    [],
  );

  // Shut by a press anywhere else, as the panels of the player of films are.
  useEffect(() => {
    if (onAPhone) {
      return;
    }
    const away = (event: PointerEvent) => {
      if (!box.current?.parentElement?.contains(event.target as Node)) {
        onClose();
      }
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", key);
    };
  }, [onAPhone, onClose]);

  const offset = lyricsShift?.song === song ? lyricsShift.offsetMs : (saved?.offsetMs ?? 0);
  const moveTo = (next: number) => {
    shiftLyrics(song, next);
    if (sending.current) {
      window.clearTimeout(sending.current.timer);
    }
    const send = () => {
      sending.current = null;
      server.setLyricsOffset(song, next).then(
        () => setSaved((was) => was && { ...was, offsetMs: next }),
        (error: unknown) => {
          shiftLyrics(song, saved?.offsetMs ?? 0);
          toast({ state: "trouble", title: t(refusalKey(refusalOf(error))) });
        },
      );
    };
    sending.current = { timer: window.setTimeout(send, SENT_AFTER_MS), send };
  };

  const lines = !saved ? null : !saved.stamped ? (
    <p className="player-menu-why">{t("music.lyrics_offset_none")}</p>
  ) : (
    <>
      <OffsetSlider
        label={t("music.lyrics_offset")}
        seconds={offset / 1000}
        furthest={FURTHEST_OFFSET_MS / 1000}
        step={OFFSET_STEP_MS / 1000}
        onCommit={(seconds) => moveTo(Math.round(seconds * 1000))}
      />
      <div className="player-menu-nudge">
        <button
          type="button"
          className="button button-small"
          aria-label={t("music.lyrics_offset_sooner")}
          onClick={() => moveTo(nudgedOffset(offset, -OFFSET_STEP_MS))}
        >
          {`−${offsetSaid(OFFSET_STEP_MS / 1000).slice(1)}`}
        </button>
        <button
          type="button"
          className="button button-small"
          aria-label={t("music.lyrics_offset_later")}
          onClick={() => moveTo(nudgedOffset(offset, OFFSET_STEP_MS))}
        >
          {offsetSaid(OFFSET_STEP_MS / 1000)}
        </button>
      </div>
      <p className="player-menu-why">{t("music.lyrics_offset_why")}</p>
      <Line label={t("music.lyrics_offset_reset")} onPick={() => moveTo(0)} />
    </>
  );

  if (onAPhone) {
    return (
      <BottomSheet title={t("music.lyrics_offset")} onClose={onClose}>
        {lines}
      </BottomSheet>
    );
  }
  return (
    <span className="music-lyrics-shift-menu" ref={box}>
      <Menu title={t("music.lyrics_offset")}>{lines}</Menu>
    </span>
  );
}

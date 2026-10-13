/*
 * The words of the song playing, on the page of what is playing.
 *
 * Stamped words follow the song: the line being sung is lit and brought to
 * the middle, and a line pressed takes the song there. Plain words are shown
 * as they are. The lines are drawn once per song; only which one is lit
 * changes as the song plays.
 *
 * An administrator moves every line of a song sooner or later by hand, as
 * the subtitles of a film are: shown moved at once, kept by the server for
 * every account once the presses rest.
 */

import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useAccount } from "../../account";
import { refusalOf } from "../../asking";
import { refusalKey } from "../../i18n";
import { ResetIcon } from "../../icons";
import { useToast } from "../../notifications/toasts";
import { shiftInSeconds } from "../../readable";
import { useSettings } from "../../settings";
import { music as server } from "../api";
import type { LyricLine, SongLyrics } from "../api";
import { useMusicMarks } from "../marks";
import { OFFSET_STEP_MS, litLineAt, nudgedOffset, shiftedLines } from "./lyrics";
import { LOOK_EVERY_MS, LyricsReport, lookAt, sayLyricsRead } from "./lyrics-watch";
import { currentMusicClock, useExactMusicPosition, useMusicControls } from "./player";

export function LyricsPanel({ song }: { song: string }) {
  const { t } = useSettings();
  const { lyricsAt } = useMusicMarks();
  const [lyrics, setLyrics] = useState<SongLyrics | null | undefined>(undefined);

  useEffect(() => {
    const stop = new AbortController();
    setLyrics(undefined);
    server
      .lyrics(song, stop.signal)
      .then((found) => {
        setLyrics(found);
        if (found) {
          sayLyricsRead(song, found);
        }
      })
      .catch(() => {
        if (!stop.signal.aborted) {
          setLyrics(null);
        }
      });
    return () => stop.abort();
  }, [song, lyricsAt]);

  if (lyrics === undefined) {
    return <p className="music-lyrics-note">{t("library.loading")}</p>;
  }
  if (lyrics === null || (lyrics.instrumental && lyrics.plain === "")) {
    return (
      <p className="music-lyrics-note">
        {t(lyrics?.instrumental ? "music.lyrics_instrumental" : "music.lyrics_none")}
      </p>
    );
  }
  if (lyrics.lines.length === 0) {
    return <div className="music-lyrics music-lyrics-plain">{lyrics.plain}</div>;
  }
  return <Moved key={song} song={song} lyrics={lyrics} />;
}

/** How long the presses rest before the offset is sent. */
const SENT_AFTER_MS = 600;

/** The lines as they are shown: as the server sends them, moved by what was
 *  pressed since, and the bar that moves them for an administrator. */
function Moved({ song, lyrics }: { song: string; lyrics: SongLyrics }) {
  const { t, language } = useSettings();
  const { account } = useAccount();
  const toast = useToast();
  const [offset, setOffset] = useState(lyrics.offset_ms);
  const saved = useRef(lyrics.offset_ms);
  const sending = useRef<{ timer: number; send: () => void } | null>(null);
  const lines = useMemo(() => shiftedLines(lyrics.lines, offset - lyrics.offset_ms), [lyrics, offset]);

  // Read again after the lines were lined up anew elsewhere, which puts the
  // offset back to nothing, unless a press here is still on its way.
  useEffect(() => {
    if (!sending.current) {
      saved.current = lyrics.offset_ms;
      setOffset(lyrics.offset_ms);
    }
  }, [lyrics]);

  // What is still waiting goes when the song or the panel does.
  useEffect(
    () => () => {
      const waiting = sending.current;
      if (waiting) {
        window.clearTimeout(waiting.timer);
        waiting.send();
      }
    },
    [song],
  );

  const moveTo = (next: number) => {
    setOffset(next);
    if (sending.current) {
      window.clearTimeout(sending.current.timer);
    }
    const send = () => {
      sending.current = null;
      server.setLyricsOffset(song, next).then(
        () => {
          saved.current = next;
        },
        (error: unknown) => {
          setOffset(saved.current);
          toast({ state: "trouble", title: t(refusalKey(refusalOf(error))) });
        },
      );
    };
    sending.current = { timer: window.setTimeout(send, SENT_AFTER_MS), send };
  };

  return (
    <>
      <Following song={song} lines={lines} />
      {account?.is_administrator && (
        <div className="music-lyrics-offset" role="group" aria-label={t("music.lyrics_offset")}>
          <span className="music-lyrics-offset-name">{t("music.lyrics_offset")}</span>
          <button
            type="button"
            className="music-lyrics-offset-step"
            aria-label={t("music.lyrics_offset_sooner")}
            title={t("music.lyrics_offset_sooner")}
            onClick={() => moveTo(nudgedOffset(offset, -OFFSET_STEP_MS))}
          >
            −
          </button>
          <span className={`music-lyrics-offset-amount${offset === 0 ? "" : " music-lyrics-offset-moved"}`}>
            {shiftInSeconds(offset, language)} s
          </span>
          <button
            type="button"
            className="music-lyrics-offset-step"
            aria-label={t("music.lyrics_offset_later")}
            title={t("music.lyrics_offset_later")}
            onClick={() => moveTo(nudgedOffset(offset, OFFSET_STEP_MS))}
          >
            +
          </button>
          <button
            type="button"
            className="music-lyrics-offset-step"
            aria-label={t("music.lyrics_offset_reset")}
            title={t("music.lyrics_offset_reset")}
            disabled={offset === 0}
            onClick={() => moveTo(0)}
          >
            <ResetIcon size={15} />
          </button>
        </div>
      )}
    </>
  );
}

function Following({ song, lines }: { song: string; lines: LyricLine[] }) {
  const position = useExactMusicPosition();
  const { seek } = useMusicControls();
  const active = litLineAt(lines, position);
  return <Lines song={song} lines={lines} active={active} seek={seek} />;
}

const Lines = memo(function Lines({
  song,
  lines,
  active,
  seek,
}: {
  song: string;
  lines: LyricLine[];
  active: number;
  seek: (seconds: number) => void;
}) {
  const box = useRef<HTMLOListElement>(null);
  const report = useMemo(() => new LyricsReport(song, lines), [song, lines]);

  /* What is lit, and where the song was then, for the journal: a line that
     stops following the song is only ever seen from here. */
  useEffect(() => {
    if (active >= 0) {
      report.lineLit(active, currentMusicClock().position);
    }
  }, [active, report]);

  /* On a beat of its own, apart from what is drawn: the line lit against the
     line the clock says, and whether the clock of a song that plays moves. */
  useEffect(() => {
    let before: { mismatches: number; stills: number; position: number | null } = {
      mismatches: 0,
      stills: 0,
      position: null,
    };
    const look = window.setInterval(() => {
      const clock = currentMusicClock();
      const shown = Array.from(box.current?.children ?? []).findIndex((line) =>
        line.classList.contains("music-lyrics-now"),
      );
      const expected = litLineAt(lines, clock.position);
      const seen = lookAt({ expected, shown, position: clock.position, playing: clock.playing }, before);
      before = seen;
      if (seen.lineNotLit) {
        report.lineNotLit(clock.position, expected, shown);
      }
      if (seen.clockStill) {
        report.clockStoodStill(clock.position, LOOK_EVERY_MS * 2);
      }
    }, LOOK_EVERY_MS);
    return () => window.clearInterval(look);
  }, [lines, report]);

  /* Only the words move, never the page around them: on a phone the page
     scrolls too, and following a song must not drag the controls away. */
  useEffect(() => {
    const list = box.current;
    const lit = list?.children[active] as HTMLElement | undefined;
    if (list && lit) {
      list.scrollTo({ top: lit.offsetTop - (list.clientHeight - lit.offsetHeight) / 2, behavior: "smooth" });
    }
  }, [active]);

  return (
    <ol className="music-lyrics" ref={box}>
      {lines.map((line, index) => (
        <li
          key={`${line.at_ms}-${index}`}
          className={`music-lyrics-line${index === active ? " music-lyrics-now" : ""}${index < active ? " music-lyrics-sung" : ""}`}
        >
          <button type="button" onClick={() => seek(line.at_ms / 1000)}>
            {line.text || "♪"}
          </button>
        </li>
      ))}
    </ol>
  );
});

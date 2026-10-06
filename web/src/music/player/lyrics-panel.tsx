/*
 * The words of the song playing, on the page of what is playing.
 *
 * Stamped words follow the song: the line being sung is lit and brought to
 * the middle, and a line pressed takes the song there. Plain words are shown
 * as they are. The lines are drawn once per song; only which one is lit
 * changes as the song plays.
 */

import { memo, useEffect, useRef, useState } from "react";
import { useSettings } from "../../settings";
import { music as server } from "../api";
import type { LyricLine, SongLyrics } from "../api";
import { lineAt } from "./lyrics";
import { useMusicControls, useMusicTime } from "./player";

export function LyricsPanel({ song }: { song: string }) {
  const { t } = useSettings();
  const [lyrics, setLyrics] = useState<SongLyrics | null | undefined>(undefined);

  useEffect(() => {
    const stop = new AbortController();
    setLyrics(undefined);
    server
      .lyrics(song, stop.signal)
      .then(setLyrics)
      .catch(() => {
        if (!stop.signal.aborted) {
          setLyrics(null);
        }
      });
    return () => stop.abort();
  }, [song]);

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
  return <Following lines={lyrics.lines} />;
}

function Following({ lines }: { lines: LyricLine[] }) {
  const { position } = useMusicTime();
  const { seek } = useMusicControls();
  /* A quarter of a second ahead: the time is read four times a second, and
     a line lit a moment late reads as one lit wrong. */
  const active = lineAt(lines, position * 1000 + 250);
  return <Lines lines={lines} active={active} seek={seek} />;
}

const Lines = memo(function Lines({
  lines,
  active,
  seek,
}: {
  lines: LyricLine[];
  active: number;
  seek: (seconds: number) => void;
}) {
  const box = useRef<HTMLOListElement>(null);

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

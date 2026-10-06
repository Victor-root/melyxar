/*
 * What is done with a library as a whole, in the piece of the browse bar the
 * sort is in: putting files into it, for an account that may, then playing
 * all of it and shuffling it. The songs are asked for when pressed: most
 * visits to a library never play all of it.
 */

import { useState } from "react";
import type { Library } from "../api";
import { UploadButton } from "../components/upload-button";
import { PlayIcon } from "../icons";
import { useSettings } from "../settings";
import { music } from "./api";
import { ShuffleIcon } from "./player/icons";
import { useMusicControls } from "./player/player";
import { shuffleOffset } from "./queueing";

export function LibraryTools({ library }: { library: Library }) {
  const { t } = useSettings();
  const player = useMusicControls();
  const [starting, setStarting] = useState(false);

  const start = async (shuffle: boolean) => {
    setStarting(true);
    try {
      let offset = 0;
      if (shuffle) {
        const held = await music.songs(library.id, "title", false, 0, 1);
        offset = shuffleOffset(held.total, Math.random());
      }
      const queue = await music.queue(library.id, offset);
      if (queue.items.length > 0) {
        player.play(queue.items, shuffle ? Math.floor(Math.random() * queue.items.length) : 0, shuffle);
      }
    } catch {
      // Nothing started: the buttons are there to be pressed again.
    } finally {
      setStarting(false);
    }
  };

  return (
    <span className="browse-field music-library-tools" role="group" aria-label={t("music.play_tools")}>
      <UploadButton library={library} bare className="music-tab music-play-tool" />
      <button
        type="button"
        className="music-tab music-play-tool"
        disabled={starting}
        onClick={() => void start(false)}
        aria-label={t("music.play_all")}
        title={t("music.play_all")}
      >
        <PlayIcon size={18} />
      </button>
      <button
        type="button"
        className="music-tab music-play-tool"
        disabled={starting}
        onClick={() => void start(true)}
        aria-label={t("music.shuffle")}
        title={t("music.shuffle")}
      >
        <ShuffleIcon size={18} />
      </button>
    </span>
  );
}

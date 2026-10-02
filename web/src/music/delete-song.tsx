/*
 * Deleting a song, offered on its line, or in its menu where the line has no
 * room, to an account allowed to delete: the question put for a film, asked of this one song, and the song
 * gone from every list once it is answered.
 */

import { useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "../account";
import { DeleteDialog } from "../components/deletion";
import { DeleteIcon } from "../icons";
import { useSettings } from "../settings";
import type { Song } from "./api";
import { useMusicMarks } from "./marks";
import type { MenuLine } from "./song-menu";

export function useSongDeletion(songs: Song[]): { lines: MenuLine[]; dialog: ReactNode } {
  const { t } = useSettings();
  const { account } = useAccount();
  const marks = useMusicMarks();
  const [asking, setAsking] = useState(false);
  const song = songs.length === 1 ? songs[0] : null;
  if (!song || account?.may_delete !== true) {
    return { lines: [], dialog: null };
  }
  return {
    lines: [{ key: "delete", said: t("card.menu.delete"), mark: <DeleteIcon size={17} />, act: () => setAsking(true) }],
    dialog: asking ? (
      <DeleteDialog
        works={[{ id: song.id, title: song.title }]}
        onClose={() => setAsking(false)}
        onDeleted={() => {
          setAsking(false);
          marks.setGone([song.id]);
        }}
      />
    ) : null,
  };
}

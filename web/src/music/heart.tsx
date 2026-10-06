/*
 * The heart of a song, an album or an artist: lit when this account likes
 * it, and pressed to like it or to stop.
 */

import { HeartIcon } from "../icons";
import { useSettings } from "../settings";
import { useLiking } from "./marks";

export function Heart({ id, size = 18, className = "" }: { id: string; size?: number; className?: string }) {
  const { t } = useSettings();
  const { liked, setLiked } = useLiking(id);
  const label = t(liked ? "card.unfavourite" : "card.favourite");
  return (
    <button
      type="button"
      className={`music-heart${liked ? " music-heart-on" : ""}${className ? ` ${className}` : ""}`}
      aria-pressed={liked}
      aria-label={label}
      title={label}
      onClick={(event) => {
        event.stopPropagation();
        setLiked(!liked);
      }}
    >
      <HeartIcon size={size} filled={liked} />
    </button>
  );
}

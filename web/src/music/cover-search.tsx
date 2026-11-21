/*
 * The covers MusicBrainz and Deezer hold for an album, searched from the tag
 * manager and chosen by eye, in a window over it. Searched at once with the
 * album and artist the tag manager holds, and again with whatever is typed
 * here: the album, the artist, or both. The pictures come through the server, never from the services.
 *
 * A cover chosen is handed back as a picture, the way one taken from the
 * device is: the tag manager shows it and writes it with the rest.
 */

import { useCallback, useEffect, useState } from "react";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { refusalKey } from "../i18n";
import { useSettings } from "../settings";
import { music } from "./api";
import type { CoverSource, OfferedCover, OfferedCovers } from "./api";

/** How each service is named on screen: names of their own, never translated. */
const SERVICES: Record<CoverSource, string> = {
  musicbrainz: "MusicBrainz",
  deezer: "Deezer",
};

export function CoverSearch({
  album,
  title: firstTitle,
  artist: firstArtist,
  onChosen,
  onClose,
}: {
  album: string;
  title: string;
  artist: string;
  onChosen: (picture: Blob) => void;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const [title, setTitle] = useState(firstTitle);
  const [artist, setArtist] = useState(firstArtist);
  const [found, setFound] = useState<OfferedCovers | null>(null);
  const [searching, setSearching] = useState(false);
  const [choosing, setChoosing] = useState<string | null>(null);
  const [refused, setRefused] = useState<string | null>(null);
  /** The covers whose picture would not load: a service names an album and
      holds no picture of it more often than it says so. */
  const [missing, setMissing] = useState<Set<string>>(() => new Set());

  const search = useCallback(
    (asked: { title: string; artist: string }, signal?: AbortSignal) => {
      if (!typedAny(asked)) {
        return;
      }
      setSearching(true);
      setRefused(null);
      music
        .offeredCovers(album, asked.title, asked.artist, signal)
        .then((answer) => {
          setFound(answer);
          setMissing(new Set());
        })
        .catch((error) => {
          if (!signal?.aborted) {
            setRefused(refusalOf(error));
          }
        })
        .finally(() => {
          if (!signal?.aborted) {
            setSearching(false);
          }
        });
    },
    [album],
  );

  useEffect(() => {
    const stop = new AbortController();
    search({ title: firstTitle, artist: firstArtist }, stop.signal);
    return () => stop.abort();
  }, [search, firstTitle, firstArtist]);

  const choose = (cover: OfferedCover) => {
    setChoosing(cover.key);
    setRefused(null);
    music
      .offeredCover(album, cover)
      .then((picture) => {
        onChosen(picture);
        onClose();
      })
      .catch((error) => {
        setRefused(refusalOf(error));
        setChoosing(null);
      });
  };

  const shown = found?.covers.filter((cover) => !missing.has(cover.key)) ?? [];

  return (
    <Modal title={t("music.tag.cover_search_title")} onClose={onClose} className="cover-search">
      <form
        className="cover-search-asked"
        onSubmit={(event) => {
          event.preventDefault();
          search({ title, artist });
        }}
      >
        <label className="tag-dialog-field">
          <span className="tag-dialog-label">{t("music.tag.album")}</span>
          <input className="field-line" value={title} onChange={(event) => setTitle(event.target.value)} />
        </label>
        <label className="tag-dialog-field">
          <span className="tag-dialog-label">{t("music.tag.album_artists")}</span>
          <input className="field-line" value={artist} onChange={(event) => setArtist(event.target.value)} />
        </label>
        <button type="submit" className="button button-accent" disabled={searching || !typedAny({ title, artist })}>
          {t("music.tag.cover_search_go")}
        </button>
      </form>

      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
      {found && found.unanswered.length > 0 && (
        <p className="finder-quiet">
          {t("music.tag.cover_search_unanswered", {
            services: found.unanswered.map((source) => SERVICES[source]).join(", "),
          })}
        </p>
      )}
      {found && !searching && shown.length === 0 && <p className="finder-quiet">{t("music.tag.cover_search_none")}</p>}

      {shown.length > 0 && (
        <div className="finder-offers">
          <ul className="pictures-offered pictures-offered-cover">
            {shown.map((cover) => (
              <li key={`${cover.source}-${cover.key}`}>
                <button
                  type="button"
                  className="pictures-choice"
                  disabled={choosing !== null}
                  onClick={() => choose(cover)}
                >
                  <span className="pictures-frame">
                    <img
                      src={music.offeredCoverAddress(album, cover, false)}
                      alt=""
                      loading="lazy"
                      onError={() => setMissing((was) => new Set(was).add(cover.key))}
                    />
                  </span>
                  <span className="cover-search-title">{cover.title}</span>
                  <span className="cover-search-by">{[cover.artist, cover.year].filter(Boolean).join(" · ")}</span>
                  <span className="cover-search-source">{SERVICES[cover.source]}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Modal>
  );
}

/** Whether anything was typed to search with: an album, an artist or both. */
function typedAny({ title, artist }: { title: string; artist: string }): boolean {
  return title.trim() !== "" || artist.trim() !== "";
}

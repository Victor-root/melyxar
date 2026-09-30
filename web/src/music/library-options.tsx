/*
 * What a library of music does beyond what every library does, on the
 * administration's screen of that library: whether it looks up lyrics
 * online, whether the covers its albums lack are looked up online, and
 * whether the tag manager may write into its files. Saved as soon as it is
 * switched, and switched back if the server refuses. The same choices are
 * offered when the library is declared.
 */

import { useEffect, useState } from "react";
import { Setting, Toggle } from "../components/panel";
import { useSettings } from "../settings";
import { music } from "./api";
import type { MusicLibraryOptions } from "./api";

/** The options of a library saved as soon as they are switched. */
export function MusicLibraryOptionsFields({ library }: { library: string }) {
  const [options, setOptions] = useState<MusicLibraryOptions | null>(null);

  useEffect(() => {
    const stop = new AbortController();
    music
      .libraryOptions(library, stop.signal)
      .then(setOptions)
      .catch(() => {});
    return () => stop.abort();
  }, [library]);

  if (!options) {
    return null;
  }
  const change = (next: MusicLibraryOptions) => {
    const was = options;
    setOptions(next);
    music.setLibraryOptions(library, next).catch(() => setOptions(was));
  };
  return <MusicOptionsFields options={options} onChange={change} />;
}

/** The options themselves, for a library being declared as well as for one
    that exists. */
export function MusicOptionsFields({
  options,
  onChange: change,
}: {
  options: MusicLibraryOptions;
  onChange: (next: MusicLibraryOptions) => void;
}) {
  const { t } = useSettings();
  return (
    <>
      <h3 className="settings-heading">{t("music.lyrics")}</h3>
      <Setting label={t("music.lyrics_online")} why={t("music.lyrics_online_why")}>
        <Toggle
          label={t("music.lyrics_online")}
          checked={options.lyrics_online}
          onChange={(lyrics_online) => change({ ...options, lyrics_online })}
        />
      </Setting>
      <h3 className="settings-heading">{t("music.covers")}</h3>
      <Setting label={t("music.covers_online")} why={t("music.covers_online_why")}>
        <Toggle
          label={t("music.covers_online")}
          checked={options.covers_online}
          onChange={(covers_online) => change({ ...options, covers_online })}
        />
      </Setting>
      <Setting label={t("music.artist_photos_online")} why={t("music.artist_photos_online_why")}>
        <Toggle
          label={t("music.artist_photos_online")}
          checked={options.artist_photos_online}
          onChange={(artist_photos_online) => change({ ...options, artist_photos_online })}
        />
      </Setting>
      <h3 className="settings-heading">{t("music.tag_manager")}</h3>
      <Setting label={t("music.tag_writing")} why={t("music.tag_writing_why")}>
        <Toggle
          label={t("music.tag_writing")}
          checked={options.tag_writing}
          onChange={(tag_writing) => change({ ...options, tag_writing })}
        />
      </Setting>
    </>
  );
}

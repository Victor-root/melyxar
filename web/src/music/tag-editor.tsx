/*
 * The tag manager, for one album: what its songs' files carry, edited here
 * and written into the files, the files renamed by a pattern when asked, a
 * copy of each kept first when asked, and a new cover put beside them.
 *
 * What is about to change is shown before anything is written, unless the
 * account chose otherwise in its settings. The library is read again once
 * the files are written, and every screen showing the album follows.
 */

import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useAccount } from "../account";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { Setting, Toggle } from "../components/panel";
import { useToast } from "../components/toasts";
import { refusalKey } from "../i18n";
import { useLeave } from "../leaving";
import { useRunning } from "../running";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { music } from "./api";
import type { EditedTags, PlannedTags, SongTags, TagsAsked } from "./api";
import { useMusic } from "./player/player";
import { albumFieldsOf, namesField, namesIn, numberOf, textOf, withAlbum } from "./tag-form";
import type { AlbumFields } from "./tag-form";

/** A song's own fields, as they are typed. */
interface SongFields {
  song: string;
  fileName: string;
  track: string;
  disc: string;
  title: string;
  artists: string;
  kept: EditedTags;
}

/** The pattern offered first, the one most collections are named by. */
const FIRST_PATTERN = "{track} - {title}";

/** The largest side a cover is sent at: larger shows nothing more and weighs
 *  on every screen that loads it. */
const LARGEST_COVER = 1600;

/** Why the server would not, said as what is missing: the right of the
 *  account, or the option of the library, which are put right in two
 *  different places. */
function Refusal({ refused }: { refused: string }) {
  const { t } = useSettings();
  const { account } = useAccount();
  if (refused !== "forbidden") {
    return <p className="notice">{t(refusalKey(refused))}</p>;
  }
  return (
    <p className="notice">
      {t(account?.may_edit_tags ? "music.tags_library_off" : "music.tags_no_right")}
      {account?.is_administrator && (
        <>
          {" "}
          <Link to={account.may_edit_tags ? "/admin/libraries" : "/admin/users"}>
            {t(account.may_edit_tags ? "music.tags_open_libraries" : "music.tags_open_users")}
          </Link>
        </>
      )}
    </p>
  );
}

export function TagEditorPage() {
  const { t } = useSettings();
  const { id = "" } = useParams();
  const leave = useLeave(`/music/album/${id}`);
  const toast = useToast();
  const { preferences } = useMusic();
  const { watch } = useRunning();
  const [songs, setSongs] = useState<SongFields[] | null>(null);
  const [album, setAlbum] = useState<AlbumFields>(albumFieldsOf(undefined));
  const [renaming, setRenaming] = useState(false);
  const [pattern, setPattern] = useState(FIRST_PATTERN);
  const [keepACopy, setKeepACopy] = useState(true);
  const [refused, setRefused] = useState<string | null>(null);
  const [planned, setPlanned] = useState<PlannedTags[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [cover, setCover] = useState<{ jpeg: Blob; shown: string } | null>(null);

  useEffect(() => {
    const stop = new AbortController();
    music
      .albumTags(id, stop.signal)
      .then((found: SongTags[]) => {
        setSongs(found.map(fieldsOf));
        setAlbum(albumFieldsOf(found[0]?.tags));
      })
      .catch((error) => {
        if (!stop.signal.aborted) {
          setRefused(refusalOf(error));
        }
      });
    return () => stop.abort();
  }, [id]);

  useEffect(() => () => {
    if (cover) {
      URL.revokeObjectURL(cover.shown);
    }
  }, [cover]);

  if (refused && !songs) {
    return (
      <main className="page">
        <Refusal refused={refused} />
      </main>
    );
  }
  if (!songs) {
    return <main className="page" aria-busy="true" />;
  }

  const asked = (): TagsAsked => ({
    songs: songs.map((one) => ({
      song: one.song,
      tags: withAlbum(
        {
          ...one.kept,
          title: textOf(one.title),
          artists: namesIn(one.artists),
          track: numberOf(one.track),
          disc: numberOf(one.disc),
        },
        album,
      ),
    })),
    pattern: renaming ? pattern : null,
    keep_a_copy: keepACopy,
  });
  const edit = (song: string, changes: Partial<SongFields>) =>
    setSongs((was) => was?.map((one) => (one.song === song ? { ...one, ...changes } : one)) ?? was);

  const write = async () => {
    setBusy(true);
    setRefused(null);
    try {
      if (cover) {
        await music.setCover(id, cover.jpeg, keepACopy);
      }
      const done = await music.writeTags(asked());
      if (done.failed.length > 0) {
        toast({
          state: "attention",
          title: t("music.tags_partly_written", {
            written: howMany(done.written, "music.songs_count", t),
            failed: howMany(done.failed.length, "music.songs_count", t),
          }),
          detail: done.failed.map((one) => one.reason).join(" · "),
        });
      } else {
        toast({ state: "ok", title: t("music.tags_written", { what: howMany(done.written, "music.songs_count", t) }) });
      }
      // The server reads the library again once it has written: watched at
      // once, so the album and every screen showing it follow without a reload.
      watch();
      leave();
    } catch (error) {
      setRefused(refusalOf(error));
      setPlanned(null);
    } finally {
      setBusy(false);
    }
  };
  const save = async () => {
    if (!preferences.tag_preview) {
      await write();
      return;
    }
    setBusy(true);
    setRefused(null);
    try {
      setPlanned(await music.previewTags(asked()));
    } catch (error) {
      setRefused(refusalOf(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="page music-page tag-editor">
      <div className="section-head">
        <h1>{t("music.edit_tags")}</h1>
      </div>
      {refused && <Refusal refused={refused} />}

      <section className="tag-editor-album">
        <h2>{t("music.album")}</h2>
        <div className="tag-editor-fields">
          <Field label={t("music.tag.album")} value={album.album} onChange={(value) => setAlbum({ ...album, album: value })} />
          <Field
            label={t("music.tag.album_artists")}
            value={album.albumArtists}
            onChange={(value) => setAlbum({ ...album, albumArtists: value })}
            hint={t("music.tag.names_hint")}
          />
          <Field label={t("music.tag.year")} value={album.year} onChange={(value) => setAlbum({ ...album, year: value })} narrow />
          <Field
            label={t("music.tag.genres")}
            value={album.genres}
            onChange={(value) => setAlbum({ ...album, genres: value })}
            hint={t("music.tag.names_hint")}
          />
        </div>
        <Setting label={t("music.compilation")} why={t("music.tag.compilation_why")}>
          <Toggle
            label={t("music.compilation")}
            checked={album.compilation}
            onChange={(compilation) => setAlbum({ ...album, compilation })}
          />
        </Setting>
        <Setting label={t("music.tag.cover")} why={t("music.tag.cover_why")}>
          <span className="tag-editor-cover">
            {cover && <img src={cover.shown} alt="" />}
            <label className="button button-small">
              {t("music.tag.choose_cover")}
              <input
                type="file"
                accept="image/*"
                hidden
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file) {
                    void asJpeg(file).then((jpeg) => setCover({ jpeg, shown: URL.createObjectURL(jpeg) }));
                  }
                }}
              />
            </label>
          </span>
        </Setting>
      </section>

      <section className="tag-editor-songs">
        <h2>{t("music.tab.songs")}</h2>
        <div className="tag-editor-table" role="table">
          <div className="tag-editor-row tag-editor-head" role="row">
            <span role="columnheader">{t("music.tag.disc")}</span>
            <span role="columnheader">{t("music.tag.track")}</span>
            <span role="columnheader">{t("music.tag.title")}</span>
            <span role="columnheader">{t("music.tag.artists")}</span>
          </div>
          {songs.map((one) => (
            <div className="tag-editor-row" role="row" key={one.song}>
              <input
                className="field-line"
                aria-label={t("music.tag.disc")}
                inputMode="numeric"
                value={one.disc}
                onChange={(event) => edit(one.song, { disc: event.target.value })}
              />
              <input
                className="field-line"
                aria-label={t("music.tag.track")}
                inputMode="numeric"
                value={one.track}
                onChange={(event) => edit(one.song, { track: event.target.value })}
              />
              <span className="tag-editor-title">
                <input
                  className="field-line"
                  aria-label={t("music.tag.title")}
                  value={one.title}
                  onChange={(event) => edit(one.song, { title: event.target.value })}
                />
                <span className="tag-editor-file">{one.fileName}</span>
              </span>
              <input
                className="field-line"
                aria-label={t("music.tag.artists")}
                value={one.artists}
                onChange={(event) => edit(one.song, { artists: event.target.value })}
              />
            </div>
          ))}
        </div>
      </section>

      <section className="tag-editor-writing">
        <Setting label={t("music.tag.rename")} why={t("music.tag.rename_why")}>
          <Toggle label={t("music.tag.rename")} checked={renaming} onChange={setRenaming} />
        </Setting>
        {renaming && (
          <Field
            label={t("music.tag.pattern")}
            value={pattern}
            onChange={setPattern}
            hint={t("music.tag.pattern_hint")}
          />
        )}
        <Setting label={t("music.tag.keep_a_copy")} why={t("music.tag.keep_a_copy_why")}>
          <Toggle label={t("music.tag.keep_a_copy")} checked={keepACopy} onChange={setKeepACopy} />
        </Setting>
        <div className="tag-editor-actions">
          <button type="button" className="button button-quiet" onClick={leave}>
            {t("lists.cancel")}
          </button>
          <button type="button" className="button button-accent" disabled={busy} onClick={() => void save()}>
            {t("music.tag.save")}
          </button>
        </div>
      </section>

      {planned && (
        <Modal
          title={t("music.tag.preview")}
          onClose={() => setPlanned(null)}
          className="tag-preview"
          footer={
            <button type="button" className="button button-accent" disabled={busy} onClick={() => void write()}>
              {t("music.tag.write")}
            </button>
          }
        >
          <Preview planned={planned} coverChanges={cover !== null} />
        </Modal>
      )}
    </main>
  );
}

function Preview({ planned, coverChanges }: { planned: PlannedTags[]; coverChanges: boolean }) {
  const { t } = useSettings();
  const changing = planned.filter((one) => one.changed.length > 0 || one.new_file_name !== null);
  if (changing.length === 0 && !coverChanges) {
    return <p>{t("music.tag.nothing_changes")}</p>;
  }
  return (
    <ul className="tag-preview-list">
      {coverChanges && <li className="tag-preview-song">{t("music.tag.cover_changes")}</li>}
      {changing.map((one) => (
        <li key={one.song} className="tag-preview-song">
          <strong>{one.after.title ?? one.file_name}</strong>
          {one.new_file_name && (
            <span className="tag-preview-line">
              <span className="tag-preview-before">{one.file_name}</span>
              <span aria-hidden="true">→</span>
              <span className="tag-preview-after">{one.new_file_name}</span>
            </span>
          )}
          {one.changed.map((field) => (
            <span key={field} className="tag-preview-line">
              <span className="tag-preview-field">{t(`music.tag.${field}`)}</span>
              <span className="tag-preview-before">{shown(one.before[field]) || t("music.tag.empty")}</span>
              <span aria-hidden="true">→</span>
              <span className="tag-preview-after">{shown(one.after[field]) || t("music.tag.empty")}</span>
            </span>
          ))}
        </li>
      ))}
    </ul>
  );
}

function Field({
  label,
  value,
  onChange,
  hint,
  narrow = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  narrow?: boolean;
}) {
  return (
    <label className={`tag-editor-field${narrow ? " tag-editor-field-narrow" : ""}`}>
      <span className="tag-editor-label">{label}</span>
      <input className="field-line" value={value} onChange={(event) => onChange(event.target.value)} />
      {hint && <span className="tag-editor-hint">{hint}</span>}
    </label>
  );
}

function fieldsOf(found: SongTags): SongFields {
  return {
    song: found.song,
    fileName: found.file_name,
    track: found.tags.track === null ? "" : String(found.tags.track),
    disc: found.tags.disc === null ? "" : String(found.tags.disc),
    title: found.tags.title ?? "",
    artists: namesField(found.tags.artists),
    kept: found.tags,
  };
}

/** A value of the tags as the preview shows it. */
function shown(value: EditedTags[keyof EditedTags]): string {
  if (Array.isArray(value)) {
    return value.join("; ");
  }
  if (typeof value === "boolean") {
    return value ? "✓" : "";
  }
  return value === null ? "" : String(value);
}

/** Any picture as a JPEG no larger than the largest a cover is kept at, which
 *  is what a cover beside the songs is. */
async function asJpeg(file: File): Promise<Blob> {
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, LARGEST_COVER / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(bitmap.width * scale);
  canvas.height = Math.round(bitmap.height * scale);
  canvas.getContext("2d")?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  return new Promise((done, failed) =>
    canvas.toBlob((blob) => (blob ? done(blob) : failed(new Error("no picture"))), "image/jpeg", 0.92),
  );
}

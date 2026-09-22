/*
 * Declaring a library, and putting one right.
 *
 * The screen that replaces opening the configuration file on the server. Its
 * whole job is to gather four things: a name, what kind of thing the library
 * holds, the language its films are described in, and the folders it looks in.
 *
 * Everything the server refuses comes back as a word rather than a sentence,
 * and is shown where it was typed: a form that says "the server would not"
 * sends somebody back to try the same thing again.
 */

import { useState } from "react";
import type { Library, WouldGo } from "../api";
import { FolderPicker } from "./folders";
import { languageName, METADATA_LANGUAGES } from "../languages";
import { KINDS } from "../libraries";
import {
  useDeclaring,
  useLibraryEditing,
  useRemoving,
} from "../screens/declaring";
import { useSettings } from "../settings";

export function LibraryEditor({
  libraries,
  onChanged,
}: {
  libraries: Library[];
  /** Called whenever the server has kept something, so the page reads it back
      rather than showing what it hoped for. */
  onChanged: () => void;
}) {
  const { t, language } = useSettings();
  const { refused, outcome, report, settle, addFolderTo, rename, renameFolder, refuse } =
    useLibraryEditing(onChanged);
  const [adding, setAdding] = useState(false);
  /* Which library is being given another folder, when one is. */
  const [addingTo, setAddingTo] = useState<string | null>(null);
  /* What the question of taking something away is being put about, when it is.
     A folder when one is named, the whole library otherwise. */
  const [removing, setRemoving] = useState<{ library: string; root?: string } | null>(null);

  const said =
    outcome === null
      ? null
      : outcome.kind === "asked_about_again"
        ? t("settings.asked_about_again", { count: outcome.count })
        : t("settings.removal_done", { works: outcome.works, files: outcome.files });

  return (
    <>
      {refused && <p className="notice">{t(refused)}</p>}
      {said && <p className="notice">{said}</p>}

      {libraries.map((library) => (
        <div className="library-block" key={library.id}>
          <div className="library-options">
            <Editable
              value={library.name}
              className="library-options-name"
              label={t("settings.library_name")}
              onSettled={(name) => rename(library, name)}
            />
            <label className="choice">
              <span className="choice-label">{t("settings.metadata_language")}</span>
              <select
                value={library.metadata_language}
                onChange={(event) => settle(library, { metadata_language: event.target.value })}
              >
                {METADATA_LANGUAGES.map((code) => (
                  <option key={code} value={code}>
                    {languageName(code, language)}
                  </option>
                ))}
              </select>
            </label>
            {/* Buttons that stay pressed rather than tick boxes, which is how
                every other switch in this interface is drawn. */}
            <button
              className={`button button-small${library.key_frames_during_scan ? " button-on" : ""}`}
              aria-pressed={library.key_frames_during_scan}
              onClick={() =>
                settle(library, { key_frames_during_scan: !library.key_frames_during_scan })
              }
            >
              {t("settings.key_frames_during_scan")}
            </button>
            <button
              className={`button button-small${library.thumbnails_during_scan ? " button-on" : ""}`}
              aria-pressed={library.thumbnails_during_scan}
              onClick={() =>
                settle(library, { thumbnails_during_scan: !library.thumbnails_during_scan })
              }
            >
              {t("settings.thumbnails_during_scan")}
            </button>
          </div>

          {/* The folders it looks in. A log line shows only the label, but
              this is the one screen an administrator manages roots from, and
              telling two of them apart by more than a label they gave one
              themselves is the point of being here. */}
          <div className="library-roots">
            {library.roots.map((root) => (
              <span className="library-root" key={root.id}>
                <Editable
                  value={root.label}
                  label={t("settings.folder_label")}
                  onSettled={(label) => renameFolder(library, root.id, label)}
                />
                <span className="library-root-path">{root.path}</span>
                {root.access !== "read_only" && root.access !== "read_write" && (
                  <span className="library-root-trouble">{t(`root.${root.explanation_code}`)}</span>
                )}
                <button
                  className="button button-small"
                  onClick={() =>
                    setRemoving(
                      removing?.root === root.id
                        ? null
                        : { library: library.id, root: root.id },
                    )
                  }
                >
                  {t("settings.remove_folder")}
                </button>
              </span>
            ))}
            <button
              className="button button-small"
              onClick={() => setAddingTo(addingTo === library.id ? null : library.id)}
            >
              {t("settings.add_folder")}
            </button>
            <button
              className="button button-small"
              onClick={() =>
                setRemoving(
                  removing?.library === library.id && !removing.root
                    ? null
                    : { library: library.id },
                )
              }
            >
              {t("settings.remove_library")}
            </button>
          </div>

          {addingTo === library.id && (
            <FolderPicker
              onPick={(path) => addFolderTo(library.id, path)}
              onClose={() => setAddingTo(null)}
            />
          )}

          {removing?.library === library.id && (
            <Removal
              library={library}
              root={removing.root}
              onDone={(went) => {
                setRemoving(null);
                report({ kind: "removal_done", works: went.works, files: went.files });
                onChanged();
              }}
              onRefused={(key) => {
                setRemoving(null);
                refuse(key);
                onChanged();
              }}
              onCancel={() => setRemoving(null)}
            />
          )}
        </div>
      ))}

      {adding ? (
        <NewLibrary
          onDone={() => {
            setAdding(false);
            onChanged();
          }}
          onCancel={() => setAdding(false)}
          onRefused={refuse}
        />
      ) : (
        <button className="button" onClick={() => setAdding(true)}>
          {t("settings.add_library")}
        </button>
      )}
    </>
  );
}

/**
 * The question put before a library, or one of its folders, is taken away.
 *
 * Two steps on purpose, and the count is fetched the moment the question
 * appears rather than read off the listing: somebody is about to lose what the
 * server learned about a few hundred films, and the number they say yes to has
 * to be the number that goes.
 *
 * The one thing this screen cannot be unclear about is that nothing leaves the
 * disk. It is written out in full, every time, under the count.
 */
function Removal({
  library,
  root,
  onDone,
  onRefused,
  onCancel,
}: {
  library: Library;
  /** The folder being taken away, or nothing when it is the whole library. */
  root?: string;
  onDone: (went: WouldGo) => void;
  onRefused: (key: string) => void;
  onCancel: () => void;
}) {
  const { t } = useSettings();
  const { going, counting, busy, goAhead } = useRemoving(library, root, onDone, onRefused);

  const label = library.roots.find((one) => one.id === root)?.label ?? "";

  return (
    <div className="removal">
      {counting ? (
        <p className="notice">{t(counting)}</p>
      ) : going === null ? (
        <p className="notice notice-faint">{t("settings.removal_counting")}</p>
      ) : (
        <p className="notice">
          {root
            ? t("settings.removal_folder_asks", {
                works: going.works,
                files: going.files,
                name: library.name,
                label,
              })
            : t("settings.removal_library_asks", {
                works: going.works,
                files: going.files,
                name: library.name,
              })}
        </p>
      )}
      <p className="settings-why">{t("settings.removal_keeps_the_files")}</p>
      <div className="controls">
        <button
          className="button button-accent"
          disabled={busy || going === null}
          onClick={goAhead}
        >
          {t("settings.removal_go_ahead")}
        </button>
        <button className="button" onClick={onCancel}>
          {t("settings.cancel")}
        </button>
      </div>
    </div>
  );
}

/**
 * A library being filled in.
 *
 * Nothing is sent until the whole thing is there: a library with no folder is
 * a library that cannot be scanned, and the server says so, but asking twice
 * for what one form can carry is worse than waiting for it.
 */
function NewLibrary({
  onDone,
  onCancel,
  onRefused,
}: {
  onDone: () => void;
  onCancel: () => void;
  onRefused: (key: string) => void;
}) {
  const { t, language } = useSettings();
  const { name, setName, kind, setKind, metadata, setMetadata, roots, addRoot, dropRoot, busy, create } =
    useDeclaring(language, onDone, onRefused);
  const [picking, setPicking] = useState(false);

  return (
    <div className="library-block">
      <div className="controls">
        <label className="choice">
          <span className="choice-label">{t("settings.library_name")}</span>
          <input
            type="text"
            value={name}
            autoFocus
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="choice">
          <span className="choice-label">{t("settings.library_kind")}</span>
          <select value={kind} onChange={(event) => setKind(event.target.value)}>
            {KINDS.map((one) => (
              <option key={one} value={one}>
                {t(`library.kind.${one}`)}
              </option>
            ))}
          </select>
        </label>
        <label className="choice">
          <span className="choice-label">{t("settings.metadata_language")}</span>
          <select value={metadata} onChange={(event) => setMetadata(event.target.value)}>
            {METADATA_LANGUAGES.map((code) => (
              <option key={code} value={code}>
                {languageName(code, language)}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="library-roots">
        {roots.map((path) => (
          <span className="library-root" key={path}>
            <span className="library-root-path">{path}</span>
            <button
              className="button button-small"
              onClick={() => dropRoot(path)}
            >
              {t("settings.forget_folder")}
            </button>
          </span>
        ))}
        <button className="button button-small" onClick={() => setPicking(!picking)}>
          {t("settings.add_folder")}
        </button>
      </div>

      {picking && (
        <FolderPicker
          onPick={(path) => {
            setPicking(false);
            addRoot(path);
          }}
          onClose={() => setPicking(false)}
        />
      )}

      <p className="settings-why">{t("settings.new_library_why")}</p>

      <div className="controls">
        <button
          className="button button-accent"
          disabled={busy || !name.trim() || roots.length === 0}
          onClick={create}
        >
          {t("settings.create_library")}
        </button>
        <button className="button" onClick={onCancel}>
          {t("settings.cancel")}
        </button>
      </div>
    </div>
  );
}

/**
 * A word that can be typed over, and is sent once somebody is done with it.
 *
 * Sent when the field is left rather than at every letter: a name is only ever
 * half typed while it is being typed, and a server told about each half would
 * refuse most of them.
 */
function Editable({
  value,
  label,
  className,
  onSettled,
}: {
  value: string;
  label: string;
  className?: string;
  onSettled: (value: string) => void;
}) {
  const [typed, setTyped] = useState(value);
  const [typing, setTyping] = useState(false);

  // What the server holds wins whenever nobody is typing, so a refusal puts
  // the old name back on the screen rather than leaving a name nobody kept.
  if (!typing && typed !== value) {
    setTyped(value);
  }

  return (
    <input
      type="text"
      aria-label={label}
      className={`editable ${className ?? ""}`}
      value={typed}
      onFocus={() => setTyping(true)}
      onChange={(event) => setTyped(event.target.value)}
      onBlur={() => {
        setTyping(false);
        if (typed.trim() && typed !== value) {
          onSettled(typed.trim());
        }
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.currentTarget.blur();
        }
      }}
    />
  );
}

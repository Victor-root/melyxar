/*
 * Declaring a library, and putting one right.
 *
 * The screen that replaces opening the configuration file on the server: a
 * library is a name, a kind, the language its works are described in and the
 * folders it looks in. Each library is a panel of its own, with its folders
 * and the state of each disk said beside it.
 *
 * Everything the server refuses comes back as a word rather than a sentence,
 * and is shown where it was typed.
 */

import { useState } from "react";
import { api } from "../../api";
import type { Library, LibraryKind, Root, SetAsideFile } from "../../api";
import { useAsked, useTold } from "../../asking";
import { FolderPicker } from "../../components/folders";
import { Modal } from "../../components/modal";
import { PageHead, Panel, Picker, Setting, StatePill, Toggle } from "../../components/panel";
import type { State } from "../../components/panel";
import { refusalKey } from "../../i18n";
import { DeleteIcon, FolderIcon, KindIcon, RefreshIcon } from "../../icons";
import type { IconProps } from "../../icons";
import { languageName, METADATA_LANGUAGES } from "../../languages";
import { KINDS, useLibraries } from "../../libraries";
import { howMany } from "../../readable";
import { useStartScan } from "../../running";
import { useDeclaring, useLibraryEditing, useRemoving } from "../../screens/declaring";
import type { LibraryEditing } from "../../screens/declaring";
import { useSettings } from "../../settings";

export function AdminLibraries() {
  const { t } = useSettings();
  const { all: libraries, refresh } = useLibraries();
  const editing = useLibraryEditing(refresh);
  const [adding, setAdding] = useState(false);

  const said =
    editing.outcome === null
      ? null
      : editing.outcome.kind === "asked_about_again"
        ? t("settings.asked_about_again", { count: editing.outcome.count })
        : t("settings.removal_done", {
            works: editing.outcome.works,
            files: editing.outcome.files,
          });

  return (
    <>
      <PageHead
        lead={t("admin.libraries_page_lead")}
        actions={
          !adding && (
            <button className="button button-accent" onClick={() => setAdding(true)}>
              <FolderIcon size={18} />
              {t("settings.add_library")}
            </button>
          )
        }
      />

      {editing.refused && (
        <p className="panel-notice panel-notice-trouble">{t(editing.refused)}</p>
      )}
      {said && <p className="panel-notice">{said}</p>}

      {adding && (
        <NewLibrary
          onDone={() => {
            setAdding(false);
            refresh();
          }}
          onCancel={() => setAdding(false)}
          onRefused={editing.refuse}
        />
      )}

      {libraries.map((library) => (
        <LibraryPanel key={library.id} library={library} editing={editing} onChanged={refresh} />
      ))}
    </>
  );
}

/** The mark of a kind of library, in the shape a panel takes an icon in. */
function markOf(kind: LibraryKind) {
  return function KindMark(props: IconProps) {
    return <KindIcon kind={kind} {...props} />;
  };
}

/** What a disk lets the server do, as a state. */
const ACCESS_STATE: Record<Root["access"], State> = {
  read_write: "ok",
  read_only: "ok",
  missing: "trouble",
  unreadable: "trouble",
};

function LibraryPanel({
  library,
  editing,
  onChanged,
}: {
  library: Library;
  editing: LibraryEditing;
  onChanged: () => void;
}) {
  const { t, language } = useSettings();
  const scan = useStartScan([library]);
  const [addingFolder, setAddingFolder] = useState(false);
  /* What the question of taking something away is being put about, when it
     is: a folder when one is named, the whole library otherwise. */
  const [removing, setRemoving] = useState<{ root?: string } | null>(null);
  const [takingBack, setTakingBack] = useState(false);

  return (
    <Panel
      icon={markOf(library.kind)}
      title={library.name}
      lead={`${t(`kind.${library.kind}`)} · ${howMany(library.works, "admin.works", t)}`}
      className="library-panel"
      action={
        <>
          <button className="button button-small" onClick={scan.start} disabled={scan.starting}>
            <RefreshIcon size={15} />
            {t("home.scan")}
          </button>
          <button
            className="button button-small button-danger"
            onClick={() => setRemoving(removing && !removing.root ? null : {})}
          >
            <DeleteIcon size={15} />
            {t("settings.remove_library")}
          </button>
        </>
      }
    >
      {scan.refused && <p className="panel-notice panel-notice-trouble">{t(refusalKey(scan.refused))}</p>}

      {removing && (
        <Removal
          library={library}
          root={removing.root}
          onDone={(went) => {
            setRemoving(null);
            editing.report({ kind: "removal_done", works: went.works, files: went.files });
            onChanged();
          }}
          onRefused={(key) => {
            setRemoving(null);
            editing.refuse(key);
            onChanged();
          }}
          onCancel={() => setRemoving(null)}
        />
      )}

      <div className="settings-lines">
        <Setting label={t("settings.library_name")}>
          <Editable
            value={library.name}
            label={t("settings.library_name")}
            onSettled={(name) => editing.rename(library, name)}
          />
        </Setting>
        <Setting label={t("settings.key_frames_during_scan")} why={t("admin.key_frames_why")}>
          <Toggle
            label={t("settings.key_frames_during_scan")}
            checked={library.key_frames_during_scan}
            onChange={(key_frames_during_scan) => editing.settle(library, { key_frames_during_scan })}
          />
        </Setting>
        <Setting label={t("settings.thumbnails_during_scan")} why={t("admin.thumbnails_scan_why")}>
          <Toggle
            label={t("settings.thumbnails_during_scan")}
            checked={library.thumbnails_during_scan}
            onChange={(thumbnails_during_scan) => editing.settle(library, { thumbnails_during_scan })}
          />
        </Setting>
      </div>

      {/* The folders it looks in. A log line shows only the label, but this is
          the one screen roots are managed from, and telling two of them apart
          by more than a label somebody gave one is the point of being here. */}
      <div className="library-folders">
        <div className="library-folders-head">
          <span>{t("admin.folders")}</span>
          <button className="button button-small" onClick={() => setAddingFolder(true)}>
            <FolderIcon size={15} />
            {t("settings.add_folder")}
          </button>
        </div>
        <div className="lines">
          {library.roots.map((root) => (
            <div className="line" key={root.id}>
              <span className="line-mark" aria-hidden="true">
                <FolderIcon size={18} />
              </span>
              <span className="line-words">
                <Editable
                  value={root.label}
                  label={t("settings.folder_label")}
                  className="field-quiet"
                  onSettled={(label) => editing.renameFolder(library, root.id, label)}
                />
                <span className="line-note line-path">{root.path}</span>
                {ACCESS_STATE[root.access] === "trouble" && (
                  <span className="line-note line-trouble">{t(`root.${root.explanation_code}`)}</span>
                )}
              </span>
              <span className="line-end">
                <StatePill state={ACCESS_STATE[root.access]}>
                  {t(`admin.access_state.${root.access}`)}
                </StatePill>
                <button
                  className="button button-small button-quiet"
                  title={t("settings.remove_folder")}
                  aria-label={t("settings.remove_folder")}
                  onClick={() =>
                    setRemoving(removing?.root === root.id ? null : { root: root.id })
                  }
                >
                  <DeleteIcon size={15} />
                </button>
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* What was taken out of it and left on the disk is walked past by every
          scan; this is the way to change one's mind. */}
      {library.set_aside > 0 && (
        <div className="line">
          <span className="line-words">
            <span className="line-name">{howMany(library.set_aside, "library.set_aside", t)}</span>
          </span>
          <span className="line-end">
            <button className="button button-small" onClick={() => setTakingBack(true)}>
              {t("library.take_back")}
            </button>
          </span>
        </div>
      )}

      {addingFolder && (
        <FolderPicker
          onPick={(path) => {
            setAddingFolder(false);
            void editing.addFolderTo(library.id, path);
          }}
          onClose={() => setAddingFolder(false)}
        />
      )}

      {takingBack && (
        <SetAsideDialog
          library={library}
          onClose={() => setTakingBack(false)}
          onTakenBack={() => {
            setTakingBack(false);
            onChanged();
          }}
        />
      )}

      {/* Read here as well as on the page of what the works are described
          in, because it is also what a library was made with. */}
      <span className="panel-say">
        {t("admin.described_in", { language: languageName(library.metadata_language, language) })}
      </span>
    </Panel>
  );
}

/**
 * The files taken out of a library and left on the disk, one per line, to
 * take back the ones chosen or all of them. The library is scanned straight
 * after, so what is taken back is in the grid a moment later.
 */
function SetAsideDialog({
  library,
  onClose,
  onTakenBack,
}: {
  library: Library;
  onClose: () => void;
  onTakenBack: () => void;
}) {
  const { t } = useSettings();
  const listed = useAsked((signal) => api.setAsideFiles(library.id, signal), [library.id]);
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  const told = useTold(async (files: SetAsideFile[] | null) => {
    await api.takeBackSetAside(library.id, files);
    onTakenBack();
  });

  const files = listed.answer ?? [];
  const keyOf = (file: SetAsideFile) => `${file.root}/${file.relative_path}`;
  const picked = files.filter((file) => chosen.has(keyOf(file)));
  const refused = told.failure ?? listed.failure;

  return (
    <Modal
      title={t("set_aside.title", { name: library.name })}
      onClose={onClose}
      footer={
        <>
          <button
            className="button"
            disabled={files.length === 0 || told.busy}
            onClick={() => told.tell(null)}
          >
            {t("set_aside.take_back_all")}
          </button>
          <button
            className="button button-accent"
            disabled={picked.length === 0 || told.busy}
            onClick={() => told.tell(picked)}
          >
            {howMany(picked.length, "set_aside.take_back_chosen", t)}
          </button>
        </>
      }
    >
      <p className="panel-say">{t("set_aside.why")}</p>
      <ul className="set-aside-files">
        {files.map((file) => (
          <li key={keyOf(file)}>
            <label className="delete-choice">
              <input
                type="checkbox"
                checked={chosen.has(keyOf(file))}
                onChange={() =>
                  setChosen((before) => {
                    const after = new Set(before);
                    if (!after.delete(keyOf(file))) {
                      after.add(keyOf(file));
                    }
                    return after;
                  })
                }
              />
              <span>{file.path}</span>
            </label>
          </li>
        ))}
      </ul>
      {refused && <p className="panel-notice panel-notice-trouble">{t(refusalKey(refused.code))}</p>}
    </Modal>
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
 * The one thing this cannot be unclear about is that nothing leaves the disk.
 * It is written out in full, every time, under the count.
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
  onDone: (went: { works: number; files: number }) => void;
  onRefused: (key: string) => void;
  onCancel: () => void;
}) {
  const { t } = useSettings();
  const { going, counting, busy, goAhead } = useRemoving(library, root, onDone, onRefused);
  const label = library.roots.find((one) => one.id === root)?.label ?? "";

  return (
    <div className="removal-box">
      <p className="removal-ask">
        {counting
          ? t(counting)
          : going === null
            ? t("settings.removal_counting")
            : root
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
      <p className="panel-say">{t("settings.removal_keeps_the_files")}</p>
      <div className="removal-actions">
        <button className="button button-small" onClick={onCancel}>
          {t("settings.cancel")}
        </button>
        <button
          className="button button-small button-danger-full"
          disabled={busy || going === null}
          onClick={goAhead}
        >
          {t("settings.removal_go_ahead")}
        </button>
      </div>
    </div>
  );
}

/**
 * A library being filled in.
 *
 * Nothing is sent until the whole thing is there: a library with no folder is
 * a library that cannot be scanned, and asking twice for what one form can
 * carry is worse than waiting for it.
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
    <Panel
      icon={FolderIcon}
      title={t("settings.add_library")}
      lead={t("settings.new_library_why")}
      className="library-new"
    >
      <div className="settings-lines">
        <Setting label={t("settings.library_name")}>
          <input
            type="text"
            className="field-line"
            value={name}
            autoFocus
            aria-label={t("settings.library_name")}
            onChange={(event) => setName(event.target.value)}
          />
        </Setting>
        <Setting label={t("settings.library_kind")}>
          <Picker
            label={t("settings.library_kind")}
            value={kind}
            options={KINDS.map((one) => [one, t(`library.kind.${one}`)] as const)}
            onPick={setKind}
          />
        </Setting>
        <Setting label={t("settings.metadata_language")}>
          <Picker
            label={t("settings.metadata_language")}
            value={metadata}
            options={METADATA_LANGUAGES.map((code) => [code, languageName(code, language)] as const)}
            onPick={setMetadata}
          />
        </Setting>
      </div>

      <div className="library-folders">
        <div className="library-folders-head">
          <span>{t("admin.folders")}</span>
          <button className="button button-small" onClick={() => setPicking(true)}>
            <FolderIcon size={15} />
            {t("settings.add_folder")}
          </button>
        </div>
        <div className="lines">
          {roots.length === 0 && <p className="empty-line">{t("admin.no_folder_yet")}</p>}
          {roots.map((path) => (
            <div className="line" key={path}>
              <span className="line-mark" aria-hidden="true">
                <FolderIcon size={18} />
              </span>
              <span className="line-words">
                <span className="line-name line-path">{path}</span>
              </span>
              <span className="line-end">
                <button className="button button-small button-quiet" onClick={() => dropRoot(path)}>
                  {t("settings.forget_folder")}
                </button>
              </span>
            </div>
          ))}
        </div>
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

      <div className="panel-foot">
        <button className="button" onClick={onCancel}>
          {t("settings.cancel")}
        </button>
        <button
          className="button button-accent"
          disabled={busy || !name.trim() || roots.length === 0}
          onClick={create}
        >
          {t("settings.create_library")}
        </button>
      </div>
    </Panel>
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
      className={`field-line${className ? ` ${className}` : ""}`}
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

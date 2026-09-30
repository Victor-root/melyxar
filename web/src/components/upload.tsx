/*
 * The window for putting files into a library from the browser: into a folder
 * of the library, or into the folder of an album. The files are sent one
 * after the other, each with how far it has got, and the library is read
 * again once they are in.
 *
 * What may be sent, and where, is the server's to say: a refusal comes back
 * for each file and is worded here, so a film thrown into a library of music
 * says so on its own line and the others go on.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { Library } from "../api";
import { ApiError } from "../api";
import { UploadIcon } from "../icons";
import { useRunning } from "../running";
import { useSettings } from "../settings";
import { sendFile } from "../uploading";
import type { Destination } from "../uploading";
import { Modal } from "./modal";
import { Picker, Setting } from "./panel";

interface Item {
  id: number;
  file: File;
  state: "waiting" | "sending" | "done" | "failed";
  share: number;
  /** The word of the refusal, when there was one. */
  refused?: string;
}

/** How a file that did not go is worded. */
const REFUSALS = ["forbidden", "path_not_allowed", "conflict", "no_room_left", "invalid_input", "unreachable"];

export function UploadDialog({
  library,
  album,
  onClose,
}: {
  library: Library;
  /** The album whose folder the files go into, in place of a folder chosen. */
  album?: { id: string; title: string };
  onClose: () => void;
}) {
  const { t } = useSettings();
  const { watch } = useRunning();
  const roots = library.roots.filter((root) => root.access === "read_write");
  const [root, setRoot] = useState(roots[0]?.id ?? "");
  const [folder, setFolder] = useState("");
  const [items, setItems] = useState<Item[]>([]);
  const [over, setOver] = useState(false);
  const counted = useRef(0);
  const stop = useRef(new AbortController());
  const sending = useRef(false);
  const anySent = useRef(false);
  const where = useRef<Destination | null>(null);
  where.current = album ? { album: album.id } : { root, folder };

  const change = useCallback((id: number, changes: Partial<Item>) => {
    setItems((held) => held.map((item) => (item.id === id ? { ...item, ...changes } : item)));
  }, []);

  /* One file at a time: a disk asked for several films at once is a disk
     asked for none of them quickly. */
  const run = useCallback(
    async (queue: Item[]) => {
      if (sending.current) {
        return;
      }
      sending.current = true;
      for (const item of queue) {
        const destination = where.current;
        if (!destination || stop.current.signal.aborted) {
          break;
        }
        change(item.id, { state: "sending", share: 0 });
        try {
          await sendFile(library.id, destination, item.file, (share) => change(item.id, { share }), stop.current.signal);
          anySent.current = true;
          change(item.id, { state: "done", share: 1 });
        } catch (error) {
          if (error instanceof DOMException) {
            break;
          }
          change(item.id, { state: "failed", refused: error instanceof ApiError ? error.code : "generic" });
        }
      }
      sending.current = false;
    },
    [library.id, change],
  );

  const add = (files: FileList | File[]) => {
    const fresh: Item[] = Array.from(files).map((file) => ({
      id: (counted.current += 1),
      file,
      state: "waiting",
      share: 0,
    }));
    if (fresh.length === 0) {
      return;
    }
    setItems((held) => [...held, ...fresh]);
    void run(fresh);
  };

  /* Read again once the files are in, and not at each one: a scan started
     under the next file would be over before it. */
  const finish = () => {
    stop.current.abort();
    if (anySent.current) {
      void api.scan(library.id).then(watch).catch(() => {});
    }
    onClose();
  };
  useEffect(() => () => stop.current.abort(), []);

  const wording = (item: Item) => {
    switch (item.state) {
      case "waiting":
        return t("upload.waiting");
      case "sending":
        return `${Math.round(item.share * 100)} %`;
      case "done":
        return t("upload.done_one");
      default:
        return t(
          item.refused && REFUSALS.includes(item.refused)
            ? `upload.refused.${item.refused}`
            : "upload.refused.generic",
        );
    }
  };

  return (
    <Modal
      title={album ? t("upload.title_album", { title: album.title }) : t("upload.title")}
      onClose={finish}
      className="upload"
      footer={
        <button type="button" className="button button-accent" onClick={finish}>
          {t("upload.close")}
        </button>
      }
    >
      {roots.length === 0 && !album ? (
        <p className="notice">{t("upload.no_root")}</p>
      ) : (
        <>
          {!album && (
            <div className="settings-lines">
              {roots.length > 1 && (
                <Setting label={t("upload.root")}>
                  <Picker
                    label={t("upload.root")}
                    value={root}
                    options={roots.map((one) => [one.id, one.label] as const)}
                    onPick={setRoot}
                  />
                </Setting>
              )}
              <Setting label={t("upload.folder")} why={t("upload.folder_why")}>
                <input
                  type="text"
                  className="field-line"
                  value={folder}
                  aria-label={t("upload.folder")}
                  onChange={(event) => setFolder(event.target.value)}
                />
              </Setting>
            </div>
          )}
          <label
            className={`upload-drop${over ? " upload-drop-over" : ""}`}
            onDragOver={(event) => {
              event.preventDefault();
              setOver(true);
            }}
            onDragLeave={() => setOver(false)}
            onDrop={(event) => {
              event.preventDefault();
              setOver(false);
              add(event.dataTransfer.files);
            }}
          >
            <UploadIcon size={28} />
            <span className="upload-drop-words">{t("upload.choose")}</span>
            <span className="upload-drop-note">{t("upload.drop")}</span>
            <input
              type="file"
              multiple
              className="visually-hidden"
              onChange={(event) => {
                if (event.target.files) {
                  add(event.target.files);
                }
                event.target.value = "";
              }}
            />
          </label>
          {items.length > 0 && (
            <ul className="upload-list">
              {items.map((item) => (
                <li key={item.id} className={`upload-item upload-item-${item.state}`}>
                  <span className="upload-item-name">{item.file.name}</span>
                  <span className="upload-item-state">{wording(item)}</span>
                  <span className="upload-item-bar" aria-hidden="true">
                    <span style={{ width: `${item.share * 100}%` }} />
                  </span>
                </li>
              ))}
            </ul>
          )}
          <p className="upload-note">{t("upload.note")}</p>
        </>
      )}
    </Modal>
  );
}

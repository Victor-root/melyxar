/*
 * What the server has been saying, sorted by tag.
 *
 * This screen exists for one exchange, the one that happens twenty times a
 * day: "it did not work" / "what did the server say". Answering that used to
 * mean knowing the name of a service, the syntax of a journal filter and the
 * shape of a date, on a machine the person may not be sitting at.
 *
 * So: the tags the server really wrote, ticked with one click, a box to narrow
 * by a word, and a button that copies exactly what is on screen. One person
 * says "send me playback and subtitles", the other ticks two boxes and pastes.
 *
 * Clearing before a test is offered next to the rest, because what is copied
 * afterwards is then about that test alone.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { Journal, JournalLine } from "../api";
import { putOnTheClipboard } from "../clipboard";
import { useSettings } from "../settings";

/** How often the screen refreshes while it is open. */
const EVERY_MS = 2_000;

export function JournalPage() {
  const { t } = useSettings();
  const [journal, setJournal] = useState<Journal>({ tags: [], lines: [] });
  const [ticked, setTicked] = useState<string[]>([]);
  const [holding, setHolding] = useState("");
  const [failed, setFailed] = useState(false);
  const [copied, setCopied] = useState(false);
  const [shown, setShown] = useState("");
  const selectable = useRef<HTMLTextAreaElement>(null);

  const look = useCallback(
    async (signal?: AbortSignal) => {
      try {
        setJournal(await api.journal({ tags: ticked, holding, most: 500 }, signal));
        setFailed(false);
      } catch (error) {
        if (!(error instanceof DOMException)) {
          setFailed(true);
        }
      }
    },
    [ticked, holding],
  );

  /* Read again on its own beat, so a test running right now fills the screen
     as it goes rather than after a reload. */
  useEffect(() => {
    const controller = new AbortController();
    look(controller.signal);
    const beat = window.setInterval(() => look(), EVERY_MS);
    return () => {
      controller.abort();
      window.clearInterval(beat);
    };
  }, [look]);

  const toggle = (tag: string) =>
    setTicked((was) => (was.includes(tag) ? was.filter((one) => one !== tag) : [...was, tag]));

  /* The server renders the text, so that what is pasted is what the command
     line would print rather than a second rendering nobody checked. */
  const copy = async () => {
    setShown("");
    let text: string;
    try {
      text = await api.journalText({ tags: ticked, holding });
    } catch {
      setFailed(true);
      return;
    }
    // Every way a browser offers, the old one included, which is the one that
    // works on a plain address. Showing the text to be copied by hand is the
    // last resort and not the ordinary answer: being handed a box to select is
    // not what anybody means by "copy".
    if (await putOnTheClipboard(text)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2_000);
      return;
    }
    setShown(text);
    window.setTimeout(() => selectable.current?.select(), 0);
  };

  return (
    <main className="page">
      <div className="section-head">
        <h1>{t("journal.title")}</h1>
        <span className="control-group">
          <button className="button" onClick={copy}>
            {copied ? t("journal.copied") : t("journal.copy")}
          </button>
          <button
            className="button button-small"
            onClick={() => api.forgetJournal().then(() => look())}
          >
            {t("journal.forget")}
          </button>
        </span>
      </div>

      <p className="notice notice-faint">{t("journal.why")}</p>

      <div className="controls">
        {journal.tags.map((tag) => (
          <button
            key={tag.name}
            className={`button button-small${ticked.includes(tag.name) ? " button-on" : ""}`}
            onClick={() => toggle(tag.name)}
          >
            {tag.name} · {tag.lines}
          </button>
        ))}
        {ticked.length > 0 && (
          <button className="button button-small" onClick={() => setTicked([])}>
            {t("journal.every_tag")}
          </button>
        )}
      </div>

      <input
        className="field"
        type="search"
        value={holding}
        placeholder={t("journal.holding")}
        onChange={(event) => setHolding(event.target.value)}
      />

      {failed && <p className="notice">{t("error.unreachable")}</p>}

      {shown && (
        <>
          <p className="notice">{t("report.select")}</p>
          <textarea className="report-text" ref={selectable} readOnly value={shown} />
        </>
      )}

      {journal.lines.length === 0 ? (
        <p className="notice notice-faint">{t("journal.nothing")}</p>
      ) : (
        <div className="journal">
          {journal.lines.map((line, index) => (
            <Line key={`${line.at}-${index}`} line={line} />
          ))}
        </div>
      )}
    </main>
  );
}

function Line({ line }: { line: JournalLine }) {
  return (
    <div className={`journal-line journal-${line.level}`}>
      <span className="journal-at">{line.at.slice(11, 19)}</span>
      <span className="journal-tag">{line.tag}</span>
      <span className="journal-said">{line.message}</span>
    </div>
  );
}

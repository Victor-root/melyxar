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

import { useEffect, useRef, useState } from "react";
import type { JournalLine } from "../api";
import { timeOfDay } from "../readable";
import { useJournalScreen } from "../screens/journal";
import { useSettings } from "../settings";

export function JournalPage() {
  const { t } = useSettings();
  const {
    journal,
    failed,
    ticked,
    toggle,
    everyTag,
    holding,
    setHolding,
    copy,
    copied,
    shown,
    forget,
    forgetConvertedSubtitles,
  } = useJournalScreen();
  /* What the last throwing away came to, shown on the button itself so the
     answer is where the question was asked. */
  const [thrownAway, setThrownAway] = useState("");
  const selectable = useRef<HTMLTextAreaElement>(null);

  /* Handed a box to select from, it is selected: nobody means "find the text
     and highlight it yourself" by copy. */
  useEffect(() => {
    if (shown) {
      selectable.current?.select();
    }
  }, [shown]);

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
            onClick={forget}
          >
            {t("journal.forget")}
          </button>
          {/* For trying the slow path again. A subtitle already converted is
              served in a millisecond and proves nothing about the minute it
              took to get there. */}
          <button
            className="button button-small"
            onClick={() =>
              void forgetConvertedSubtitles().then((forgotten) => {
                if (forgotten !== null) {
                  setThrownAway(t("journal.subtitles_gone", { count: forgotten }));
                }
              })
            }
          >
            {thrownAway || t("journal.forget_subtitles")}
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
          <button className="button button-small" onClick={everyTag}>
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
      <span className="journal-at">{timeOfDay(line.at)}</span>
      <span className="journal-tag">{line.tag}</span>
      <span className="journal-said">{line.message}</span>
    </div>
  );
}

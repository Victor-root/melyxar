/*
 * What the server has been saying, narrowed to the parts of it asked about,
 * and copied exactly as it is shown.
 *
 * The question asked twenty times a day is "what did the server say", and the
 * answer used to be a terminal. Every line carries the word of the part of
 * the server that wrote it, and ticking those words is how a page of noise
 * becomes the three lines that matter.
 */

import { useEffect, useRef, useState } from "react";
import type { JournalLine } from "../../api";
import { PageHead, Panel } from "../../components/panel";
import { JournalIcon, SearchIcon } from "../../icons";
import { timeOfDay } from "../../readable";
import { useJournalScreen } from "../../screens/journal";
import { useSettings } from "../../settings";

export function AdminJournal() {
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
    <>
      <PageHead lead={t("journal.why")} />

      {/* On the panel they act on rather than beside the name of the page,
          where three of them left the name no room. */}
      <Panel
        icon={JournalIcon}
        title={t("admin.journal_lines")}
        className="journal-panel"
        action={
          <>
            <button className="button button-accent" onClick={copy}>
              {copied ? t("journal.copied") : t("journal.copy")}
            </button>
            <button className="button" onClick={forget}>
              {t("journal.forget")}
            </button>
            {/* For trying the slow path again. A subtitle already converted
                is served in a millisecond and proves nothing about the minute
                it took to get there. */}
            <button
              className="button"
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
          </>
        }
      >
        <div className="journal-filters">
          <label className="journal-search">
            <SearchIcon size={18} />
            <input
              type="search"
              value={holding}
              placeholder={t("journal.holding")}
              aria-label={t("journal.holding")}
              onChange={(event) => setHolding(event.target.value)}
            />
          </label>
          <div className="chips">
            <button
              className={`chip${ticked.length === 0 ? " chip-on" : ""}`}
              aria-pressed={ticked.length === 0}
              onClick={everyTag}
            >
              {t("journal.every_tag")}
            </button>
            {journal.tags.map((tag) => (
              <button
                key={tag.name}
                className={`chip${ticked.includes(tag.name) ? " chip-on" : ""}`}
                aria-pressed={ticked.includes(tag.name)}
                onClick={() => toggle(tag.name)}
              >
                {tag.name}
                <span className="chip-count">{tag.lines}</span>
              </button>
            ))}
          </div>
        </div>

        {failed && <p className="panel-notice panel-notice-trouble">{t("error.unreachable")}</p>}

        {shown && (
          <>
            <p className="panel-say">{t("report.select")}</p>
            <textarea className="report-text" ref={selectable} readOnly value={shown} />
          </>
        )}

        {journal.lines.length === 0 ? (
          <p className="empty-line">{t("journal.nothing")}</p>
        ) : (
          <div className="log">
            {journal.lines.map((line, index) => (
              <Line key={`${line.at}-${index}`} line={line} />
            ))}
          </div>
        )}
      </Panel>
    </>
  );
}

function Line({ line }: { line: JournalLine }) {
  return (
    <div className={`log-line log-${line.level}`}>
      <span className="log-at">{timeOfDay(line.at)}</span>
      <span className="log-level">{line.level}</span>
      <span className="log-tag">{line.tag}</span>
      <span className="log-said">{line.message}</span>
    </div>
  );
}

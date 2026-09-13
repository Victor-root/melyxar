/*
 * Saying by hand what a film is, when no rule could work it out.
 *
 * There will always be some: a copy named after the wrong film, a title the
 * provider spells another way, a name that is only somebody's marker. The
 * rules get better and never finish the job, so the last word belongs to
 * whoever is looking at the film, and it has to be sayable from here rather
 * than from a terminal.
 *
 * A choice made here is remembered as a choice, and no later run undoes it.
 */

import { useState } from "react";
import { api } from "../api";
import type { Candidate } from "../api";
import { refusalKey } from "../i18n";
import { ApiError } from "../api";
import { useSettings } from "../settings";

export function IdentifyByHand({
  workId,
  title,
  onIdentified,
}: {
  workId: string;
  /** What the film is called now, which is the first thing worth searching. */
  title: string;
  onIdentified: () => void;
}) {
  const { t } = useSettings();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState(title);
  const [candidates, setCandidates] = useState<Candidate[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  const look = async () => {
    setBusy(true);
    setRefused(null);
    try {
      setCandidates(await api.candidates(workId, query));
    } catch (error) {
      setRefused(error instanceof ApiError ? error.code : "generic");
    } finally {
      setBusy(false);
    }
  };

  const pick = async (candidate: Candidate) => {
    setBusy(true);
    setRefused(null);
    try {
      await api.identifyByHand(workId, candidate.external_id);
      onIdentified();
    } catch (error) {
      setRefused(error instanceof ApiError ? error.code : "generic");
      setBusy(false);
    }
  };

  if (!open) {
    return (
      <button className="button" onClick={() => setOpen(true)}>
        {t("byhand.open")}
      </button>
    );
  }

  return (
    <section className="byhand">
      <form
        className="byhand-ask"
        onSubmit={(event) => {
          event.preventDefault();
          look();
        }}
      >
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          aria-label={t("byhand.field")}
          /* The title is already there and usually wrong in one word, so the
             cursor lands in it ready to be corrected. */
          autoFocus
        />
        <button className="button button-accent" type="submit" disabled={busy}>
          {t("byhand.look")}
        </button>
      </form>

      {refused && <p className="notice">{t(refusalKey(refused))}</p>}

      {candidates !== null &&
        (candidates.length === 0 ? (
          <p className="notice">{t("byhand.nothing")}</p>
        ) : (
          <ul className="byhand-list">
            {candidates.map((candidate) => (
              <li key={candidate.external_id}>
                <button className="byhand-choice" onClick={() => pick(candidate)} disabled={busy}>
                  {candidate.poster ? (
                    <img src={candidate.poster} alt="" loading="lazy" width={2} height={3} />
                  ) : (
                    <span className="byhand-blank" aria-hidden="true" />
                  )}
                  <span className="byhand-words">
                    <span className="byhand-title">
                      {candidate.title}
                      {candidate.year !== null && <span className="byhand-year"> {candidate.year}</span>}
                    </span>
                    {/* The title it was shot under, when it differs: that is
                        often how the file was named in the first place. */}
                    {candidate.original_title && candidate.original_title !== candidate.title && (
                      <span className="byhand-original">{candidate.original_title}</span>
                    )}
                    {candidate.overview && (
                      <span className="byhand-overview">{candidate.overview}</span>
                    )}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        ))}
    </section>
  );
}

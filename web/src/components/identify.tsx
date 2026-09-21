/*
 * Saying by hand what a work is, when no rule could work it out.
 *
 * There will always be some: a copy named after the wrong film, a title the
 * provider spells another way, a name that is only somebody's marker. The
 * rules get better and never finish the job, so the last word belongs to
 * whoever is looking at the work, and it has to be sayable from wherever they
 * are looking at it rather than only from its own page.
 *
 * Three steps, because the job has three: say what to look for, see what came
 * back, agree to the one that is right. Each is a screenful of its own, and
 * the arrow at the top left walks back through them without throwing away
 * what was typed.
 *
 * A choice made here is remembered as a choice, and no later run undoes it.
 */

import { useState } from "react";
import { api } from "../api";
import type { Candidate, SearchCriteria } from "../api";
import { refusalOf } from "../asking";
import { refusalKey } from "../i18n";
import { useSettings } from "../settings";
import { Modal } from "./modal";

/** Which screenful of the panel is being looked at. */
type Step = "asking" | "results" | "agreeing";

export function IdentifyDialog({
  workId,
  /** What the work is called now, which is the first thing worth searching
      and what the field opens on. */
  title,
  /** Where the file is, when the screen that opened this knows: it is what
      somebody reads to work out what the film actually is, and it is often
      the only clue there is. */
  path,
  onClose,
  /** Said once the work has been named, so the screen underneath reads it
      again rather than keeping what it had. */
  onIdentified,
}: {
  workId: string;
  title: string;
  path?: string;
  onClose: () => void;
  onIdentified: () => void;
}) {
  const { t } = useSettings();
  const [step, setStep] = useState<Step>("asking");
  const [asked, setAsked] = useState<SearchCriteria>({ name: title });
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [chosen, setChosen] = useState<Candidate | null>(null);
  const [replacePictures, setReplacePictures] = useState(true);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  const field = (which: keyof SearchCriteria) => ({
    value: asked[which] ?? "",
    onChange: (event: React.ChangeEvent<HTMLInputElement>) =>
      setAsked((before) => ({ ...before, [which]: event.target.value })),
  });

  const look = async () => {
    setBusy(true);
    setRefused(null);
    try {
      setCandidates(await api.candidates(workId, asked));
      setStep("results");
    } catch (error) {
      setRefused(refusalOf(error));
    } finally {
      setBusy(false);
    }
  };

  const agree = async () => {
    if (!chosen) {
      return;
    }
    setBusy(true);
    setRefused(null);
    try {
      await api.identifyByHand(workId, chosen.external_id, replacePictures);
      onIdentified();
      onClose();
    } catch (error) {
      setRefused(refusalOf(error));
      setBusy(false);
    }
  };

  const back =
    step === "results"
      ? () => setStep("asking")
      : step === "agreeing"
        ? () => setStep("results")
        : undefined;

  return (
    <Modal
      title={t("identify.title")}
      onBack={back}
      onClose={onClose}
      footer={
        step === "asking" ? (
          <button className="button button-accent button-large" onClick={look} disabled={busy}>
            {t("identify.look")}
          </button>
        ) : step === "agreeing" ? (
          <button className="button button-accent button-large" onClick={agree} disabled={busy}>
            {t("identify.agree")}
          </button>
        ) : undefined
      }
    >
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}

      {step === "asking" && (
        <form
          className="identify-form"
          onSubmit={(event) => {
            event.preventDefault();
            look();
          }}
        >
          <p className="identify-said">{t("identify.how")}</p>

          {path && (
            <p className="identify-path">
              <span className="identify-label">{t("identify.path")}</span>
              <span className="identify-file">{path}</span>
            </p>
          )}

          <label className="identify-field">
            <span className="identify-label">{t("identify.name")}</span>
            <input type="text" autoFocus {...field("name")} />
          </label>

          <label className="identify-field">
            <span className="identify-label">{t("identify.year")}</span>
            <input type="text" inputMode="numeric" maxLength={4} {...field("year")} />
          </label>

          <label className="identify-field">
            <span className="identify-label">{t("identify.imdb")}</span>
            <input type="text" {...field("imdbId")} />
          </label>

          <label className="identify-field">
            <span className="identify-label">{t("identify.provider")}</span>
            <input type="text" {...field("providerId")} />
          </label>

          {/* The form is sent by the button at the foot; this one is here so
              that the key that sends a form still sends it. */}
          <button type="submit" className="visually-hidden" tabIndex={-1}>
            {t("identify.look")}
          </button>
        </form>
      )}

      {step === "results" &&
        (candidates.length === 0 ? (
          <p className="notice">{t("identify.nothing")}</p>
        ) : (
          <>
            <h3 className="identify-heading">{t("identify.results")}</h3>
            <ul className="identify-found">
              {candidates.map((candidate) => (
                <li key={candidate.external_id}>
                  <button
                    className="identify-choice"
                    onClick={() => {
                      setChosen(candidate);
                      setStep("agreeing");
                    }}
                  >
                    <span className="identify-poster">
                      {candidate.poster ? (
                        <img src={candidate.poster} alt="" loading="lazy" sizes="140px" />
                      ) : (
                        <span className="identify-blank" aria-hidden="true" />
                      )}
                    </span>
                    <span className="identify-name">{candidate.title}</span>
                    {/* The title it was shot under, when it differs: that is
                        often how the file was named in the first place. */}
                    {candidate.original_title && candidate.original_title !== candidate.title && (
                      <span className="identify-original">{candidate.original_title}</span>
                    )}
                    {candidate.year !== null && (
                      <span className="identify-year">{candidate.year}</span>
                    )}
                  </button>
                </li>
              ))}
            </ul>
          </>
        ))}

      {step === "agreeing" && chosen && (
        <div className="identify-agreeing">
          <div className="identify-chosen">
            <span className="identify-poster">
              {chosen.poster ? (
                <img src={chosen.poster} alt="" sizes="160px" />
              ) : (
                <span className="identify-blank" aria-hidden="true" />
              )}
            </span>
            <span className="identify-said">
              <span className="identify-name">{chosen.title}</span>
              {chosen.year !== null && <span className="identify-year">{chosen.year}</span>}
              {chosen.overview && <span className="identify-overview">{chosen.overview}</span>}
            </span>
          </div>

          {/* Set by default: correcting a work that was wholly the wrong work
              is the ordinary case, and a poster of the wrong film is the most
              visible part of it. Unset, everything else is corrected and the
              pictures stay, which is what somebody who chose them by hand
              wants. */}
          <label className="identify-also">
            <input
              type="checkbox"
              checked={replacePictures}
              onChange={(event) => setReplacePictures(event.target.checked)}
            />
            <span>{t("identify.replace_pictures")}</span>
          </label>
        </div>
      )}
    </Modal>
  );
}

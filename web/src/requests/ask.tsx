/*
 * Asking for one title: what it is, for a series the whole of it or the
 * seasons wanted (those already here cannot be chosen), and a line to say
 * more, such as which cut or which language.
 */

import { useState } from "react";
import { refusalAbout, useAsked } from "../asking";
import { Modal } from "../components/modal";
import { Toggle } from "../components/panel";
import { numberOfOne } from "../readable";
import { useSettings } from "../settings";
import { useToast } from "../notifications/toasts";
import { requestsApi } from "./api";
import type { Found } from "./api";
import { TitleCard } from "./card";
import { useRequests } from "./store";

/** The longest note the server keeps. */
const LONGEST_NOTE = 500;

export function AskDialog({ found, onClose }: { found: Found; onClose: () => void }) {
  const { t, language } = useSettings();
  const toast = useToast();
  const { ask } = useRequests();
  const series = found.catalogue === "series";
  const seasons = useAsked(
    (signal) => (series ? requestsApi.seasons(found.tmdb_id, language, signal) : Promise.resolve([])),
    [found.tmdb_id, language],
  );
  const heldAny = (found.held?.seasons.length ?? 0) > 0;
  /* A series held in part is asked for by its seasons: the whole of it is
     here already. */
  const [whole, setWhole] = useState(!heldAny);
  const [chosen, setChosen] = useState<number[]>([]);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  const ready = !busy && (!series || whole || chosen.length > 0);

  const send = async () => {
    setBusy(true);
    setRefused(null);
    try {
      await ask({
        catalogue: found.catalogue,
        tmdb_id: found.tmdb_id,
        seasons: series && !whole ? [...chosen].sort((a, b) => a - b) : [],
        note: note.trim(),
        language,
      });
      toast({ state: "ok", title: t("requests.sent"), detail: found.title });
      onClose();
    } catch (error) {
      setRefused(refusalAbout(error, "requests"));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={t("requests.ask_title")}
      onClose={onClose}
      className="request-dialog"
      footer={
        <button type="button" className="button button-accent button-large" disabled={!ready} onClick={send}>
          {t("requests.send")}
        </button>
      }
    >
      <TitleCard
        catalogue={found.catalogue}
        title={found.title}
        year={found.year}
        poster={found.poster}
        overview={found.overview}
      />
      {refused && <p className="notice">{t(refused)}</p>}
      {series && (
        <fieldset className="request-seasons">
          <legend className="request-label">{t("requests.seasons")}</legend>
          <label className="request-season request-season-whole">
            <span>{t("requests.whole_series")}</span>
            <Toggle label={t("requests.whole_series")} checked={whole} disabled={heldAny} onChange={setWhole} />
          </label>
          {!whole &&
            (seasons.answer ?? []).map((season) => (
              <label key={season.number} className="request-season">
                <input
                  type="checkbox"
                  checked={season.held || chosen.includes(season.number)}
                  disabled={season.held}
                  onChange={(event) =>
                    setChosen((was) =>
                      event.target.checked
                        ? [...was, season.number]
                        : was.filter((number) => number !== season.number),
                    )
                  }
                />
                <span>{numberOfOne("season", season.number, t)}</span>
                <span className="request-season-note">
                  {season.held ? t("requests.season_here") : t("requests.episodes", { count: season.episodes })}
                </span>
              </label>
            ))}
          {!whole && seasons.failure && <p className="notice">{t("requests.seasons_unread")}</p>}
        </fieldset>
      )}
      <label className="request-note">
        <span className="request-label">{t("requests.note")}</span>
        <textarea
          className="field-line"
          rows={2}
          maxLength={LONGEST_NOTE}
          placeholder={t("requests.note_example")}
          value={note}
          onChange={(event) => setNote(event.target.value)}
        />
      </label>
    </Modal>
  );
}

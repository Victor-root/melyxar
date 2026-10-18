/*
 * The subtitles of a copy, for an administrator: the tracks it has, where
 * each comes from, and those OpenSubtitles offers in a chosen language, one
 * of which is downloaded as one more track.
 *
 * Laid out as the lyrics window is (finder.css): the copy on top with its
 * tracks, the search under it, and the offers, which alone scroll.
 *
 * Whatever changes here is read again by the screens showing the work, so
 * the player offers a downloaded subtitle the next time it is opened.
 */

import { useState } from "react";
import { api } from "../api";
import type { SubtitleOffer } from "../api";
import { refusalOf, useAsked } from "../asking";
import { CloseIcon, DownloadIcon, SearchIcon, SubtitlesIcon } from "../icons";
import { languageName, METADATA_LANGUAGES } from "../languages";
import { useMarks } from "../marks";
import { howManyVoted } from "../readable";
import { useSettings } from "../settings";
import { Modal } from "./modal";
import { Picker, Toggle } from "./panel";

export function OnlineSubtitlesDialog({
  sourceId,
  title,
  onClose,
}: {
  sourceId: string;
  title: string;
  onClose: () => void;
}) {
  const { t, language } = useSettings();
  const { rowsHaveMoved } = useMarks();
  const [again, setAgain] = useState(0);
  const tracks = useAsked((signal) => api.subtitleTracks(sourceId, signal), [sourceId, again]);
  const settings = useAsked((signal) => api.openSubtitles(signal), []);
  const [wanted, setWanted] = useState<string>(METADATA_LANGUAGES.includes(language) ? language : "en");
  /* Only forced subtitles: the lines in another language than the film's. */
  const [forcedOnly, setForcedOnly] = useState(false);
  const [offers, setOffers] = useState<SubtitleOffer[] | null>(null);
  const [busy, setBusy] = useState<number | "search" | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const [refused, setRefused] = useState<string | null>(null);
  const [fetched, setFetched] = useState<Set<number>>(new Set());
  /* The last subtitle downloaded ends after the film does, so it was timed
     on another version. */
  const [long, setLong] = useState(false);

  const attempt = async (doing: () => Promise<void>) => {
    setRefused(null);
    try {
      await doing();
    } catch (error) {
      setRefused(`online_subtitles.refused.${refusalOf(error)}`);
    }
    setBusy(null);
  };

  const search = () => {
    setBusy("search");
    setSaid(null);
    void attempt(async () => {
      setOffers(await api.subtitleOffers(sourceId, [wanted], forcedOnly));
    });
  };

  const download = (offer: SubtitleOffer) => {
    setBusy(offer.file_id);
    setSaid(null);
    setLong(false);
    void attempt(async () => {
      const { remaining, ends_after_the_film } = await api.downloadSubtitle(sourceId, offer);
      setFetched((before) => new Set([...before, offer.file_id]));
      setSaid(
        remaining === null
          ? t("online_subtitles.downloaded")
          : t("online_subtitles.downloaded_left", { count: remaining }),
      );
      setLong(ends_after_the_film);
      setAgain((count) => count + 1);
      rowsHaveMoved();
    });
  };

  const take = (track: string) =>
    void attempt(async () => {
      await api.removeSubtitle(sourceId, track);
      setAgain((count) => count + 1);
      rowsHaveMoved();
    });

  const languages = METADATA_LANGUAGES.map((code) => [code, languageName(code, language)] as const);
  const noKey = settings.answer && !settings.answer.has_key;
  const held = tracks.answer ?? [];

  return (
    <Modal title={t("online_subtitles.title", { title })} onClose={onClose} className="finder">
      <section className="finder-head" aria-label={t("online_subtitles.tracks")}>
        <span className="finder-mark" aria-hidden="true">
          <SubtitlesIcon size={26} />
        </span>
        <div className="finder-head-words">
          <h3 className="finder-head-title">{title}</h3>
          <span className="finder-head-by">{t("online_subtitles.tracks")}</span>
          {tracks.answer && (
            <div className="finder-head-state">
              {held.length === 0 && (
                <span className="state-pill state-attention">{t("online_subtitles.no_tracks")}</span>
              )}
              {held.map((track) => (
                <span
                  key={track.id}
                  className={`state-pill subtitles-held${track.origin === "downloaded" ? " state-news" : " finder-pill-plain"}`}
                >
                  {track.language ? languageName(track.language, language) : t("online_subtitles.unknown_language")}
                  {" · "}
                  {t(`online_subtitles.origin.${track.origin}`)}
                  {track.origin === "downloaded" && (
                    <button
                      type="button"
                      className="subtitles-off"
                      aria-label={t("online_subtitles.remove")}
                      title={track.title ? `${t("online_subtitles.remove")} : ${track.title}` : t("online_subtitles.remove")}
                      onClick={() => take(track.id)}
                    >
                      <CloseIcon size={13} />
                    </button>
                  )}
                </span>
              ))}
            </div>
          )}
        </div>
        <div className="finder-head-side">
          <span className="finder-figure">
            <span className="finder-figure-name">{t("online_subtitles.held")}</span>
            <span className="finder-figure-value">{tracks.answer ? held.length : "–"}</span>
          </span>
        </div>
      </section>

      {noKey ? (
        <p className="settings-why">{t("online_subtitles.no_key")}</p>
      ) : (
        <div className="finder-search subtitles-search" role="search" aria-label={t("online_subtitles.search")}>
          <Picker
            label={t("online_subtitles.language")}
            value={wanted}
            options={languages}
            onPick={(picked) => {
              setWanted(picked);
              setOffers(null);
            }}
          />
          <label className="subtitles-forced">
            <Toggle
              checked={forcedOnly}
              label={t("online_subtitles.forced_only")}
              onChange={(only) => {
                setForcedOnly(only);
                setOffers(null);
              }}
            />
            <span>{t("online_subtitles.forced_only")}</span>
          </label>
          <button type="button" className="button button-small button-accent" disabled={busy !== null} onClick={search}>
            <SearchIcon size={15} />
            {t("online_subtitles.look")}
          </button>
        </div>
      )}
      {refused && <p className="notice">{t(refused)}</p>}
      {said && <p className="panel-notice panel-notice-ok">{said}</p>}
      {long && <p className="panel-notice panel-notice-trouble">{t("online_subtitles.too_long")}</p>}

      <div className="finder-offers" aria-busy={busy === "search"}>
        {busy === "search" && <p className="finder-quiet">{t("online_subtitles.searching")}</p>}
        {busy !== "search" && offers?.length === 0 && <p className="finder-quiet">{t("online_subtitles.nothing")}</p>}
        {offers?.map((offer) => (
          <article key={offer.file_id} className={`finder-offer${fetched.has(offer.file_id) ? " finder-offer-taken" : ""}`}>
            <div className="finder-offer-lead">
              <span className="finder-offer-lead-main">{languageName(offer.language, language)}</span>
              <span className="finder-offer-lead-note subtitles-downloads">
                {t("online_subtitles.downloads", { count: howManyVoted(offer.downloads, language) })}
              </span>
            </div>
            <div className="finder-offer-words">
              <span className="finder-offer-title" title={offer.release}>
                {offer.release}
              </span>
              <span className="finder-offer-marks">
                {offer.matches_the_file && <span className="state-pill state-ok">{t("online_subtitles.fits")}</span>}
                {offer.other_speed && (
                  <span className="state-pill state-attention" title={t("online_subtitles.other_speed_why")}>
                    {t("online_subtitles.other_speed")}
                  </span>
                )}
                {offer.forced && <span className="state-pill state-news">{t("online_subtitles.forced")}</span>}
                {offer.hearing_impaired && (
                  <span className="state-pill finder-pill-plain">{t("online_subtitles.hearing_impaired")}</span>
                )}
                {offer.machine_translated && (
                  <span className="state-pill finder-pill-plain">{t("online_subtitles.machine")}</span>
                )}
                {offer.trusted && <span className="state-pill finder-pill-plain">{t("online_subtitles.trusted")}</span>}
              </span>
            </div>
            {fetched.has(offer.file_id) ? (
              <span className="state-pill state-ok">{t("online_subtitles.done")}</span>
            ) : (
              <button type="button" className="button button-small" disabled={busy !== null} onClick={() => download(offer)}>
                <DownloadIcon size={15} />
                {t("online_subtitles.download")}
              </button>
            )}
          </article>
        ))}
      </div>
    </Modal>
  );
}

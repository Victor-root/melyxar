/*
 * The subtitles of a copy, for an administrator: the tracks it has, where
 * each comes from, and those OpenSubtitles offers in a chosen language, one
 * of which is downloaded as one more track.
 *
 * Whatever changes here is read again by the screens showing the work, so
 * the player offers a downloaded subtitle the next time it is opened.
 */

import { useState } from "react";
import { api } from "../api";
import type { SubtitleOffer } from "../api";
import { refusalOf, useAsked } from "../asking";
import { CloseIcon, DownloadIcon } from "../icons";
import { languageName, METADATA_LANGUAGES } from "../languages";
import { useMarks } from "../marks";
import { howManyVoted } from "../readable";
import { useSettings } from "../settings";
import { Modal } from "./modal";
import { Picker } from "./panel";

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
      setOffers(await api.subtitleOffers(sourceId, [wanted]));
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

  return (
    <Modal title={t("online_subtitles.title", { title })} onClose={onClose}>
      <h3 className="subtitles-heading">{t("online_subtitles.tracks")}</h3>
      {tracks.answer?.length === 0 && <p className="settings-why">{t("online_subtitles.no_tracks")}</p>}
      {tracks.answer && tracks.answer.length > 0 && (
        <ul className="subtitles-lines">
          {tracks.answer.map((track) => (
            <li key={track.id} className="subtitles-line">
              <span className="subtitles-language">
                {track.language ? languageName(track.language, language) : t("online_subtitles.unknown_language")}
              </span>
              <span className="subtitles-release">{track.title ?? ""}</span>
              <span className="subtitles-origin">{t(`online_subtitles.origin.${track.origin}`)}</span>
              {track.origin === "downloaded" && (
                <button
                  type="button"
                  className="subtitles-off"
                  aria-label={t("online_subtitles.remove")}
                  title={t("online_subtitles.remove")}
                  onClick={() => take(track.id)}
                >
                  <CloseIcon size={15} />
                </button>
              )}
            </li>
          ))}
        </ul>
      )}

      <h3 className="subtitles-heading">{t("online_subtitles.search")}</h3>
      {noKey ? (
        <p className="settings-why">{t("online_subtitles.no_key")}</p>
      ) : (
        <div className="subtitles-search">
          <Picker
            label={t("online_subtitles.language")}
            value={wanted}
            options={languages}
            onPick={(picked) => {
              setWanted(picked);
              setOffers(null);
            }}
          />
          <button type="button" className="button button-small button-accent" disabled={busy !== null} onClick={search}>
            {t("online_subtitles.look")}
          </button>
        </div>
      )}
      {refused && <p className="notice">{t(refused)}</p>}
      {said && <p className="panel-notice panel-notice-ok">{said}</p>}
      {long && <p className="panel-notice panel-notice-trouble">{t("online_subtitles.too_long")}</p>}
      {offers?.length === 0 && <p className="settings-why">{t("online_subtitles.nothing")}</p>}
      {offers && offers.length > 0 && (
        <ul className="subtitles-lines subtitles-offers">
          {offers.map((offer) => (
            <li key={offer.file_id} className="subtitles-line">
              <span className="subtitles-release">{offer.release}</span>
              <span className="subtitles-marks">
                {offer.matches_the_file && (
                  <span className="work-badge subtitles-fits">{t("online_subtitles.fits")}</span>
                )}
                {offer.other_speed && (
                  <span className="work-badge work-badge-warning" title={t("online_subtitles.other_speed_why")}>
                    {t("online_subtitles.other_speed")}
                  </span>
                )}
                <span>{t("online_subtitles.downloads", { count: howManyVoted(offer.downloads, language) })}</span>
                {offer.hearing_impaired && <span className="work-badge">{t("online_subtitles.hearing_impaired")}</span>}
                {offer.machine_translated && <span className="work-badge">{t("online_subtitles.machine")}</span>}
                {offer.trusted && <span className="work-badge">{t("online_subtitles.trusted")}</span>}
              </span>
              <button
                type="button"
                className="button button-small"
                disabled={busy !== null || fetched.has(offer.file_id)}
                onClick={() => download(offer)}
              >
                <DownloadIcon size={15} />
                {t(fetched.has(offer.file_id) ? "online_subtitles.done" : "online_subtitles.download")}
              </button>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}

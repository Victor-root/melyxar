/*
 * The pictures a work wears, and choosing them by hand.
 *
 * What a provider puts forward is right most of the time and wrong often
 * enough to matter: a poster in the wrong language, a banner too small for a
 * screen, a title image nobody drew. Whoever is looking at the work can see
 * which of the thirty the provider holds is the right one, and this is where
 * they say so.
 *
 * Two screenfuls. The first is what the work wears now, one tile per kind,
 * with what each picture really measures: too small is the one fault that
 * cannot be seen on a tile. The second is everything the provider offers of
 * one kind, shown from the provider's own addresses, so looking through
 * thirty of them costs this server nothing at all.
 *
 * A choice made here is remembered as a choice: no later run replaces it, and
 * a picture taken off does not come back.
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { HeldPicture, OfferedPicture, PictureKind } from "../api";
import { refusalOf } from "../asking";
import { refusalKey } from "../i18n";
import { useSettings } from "../settings";
import { DeleteIcon, SearchIcon } from "../icons";
import { Modal } from "./modal";

/** The kinds a work wears, in the order the panel shows them: the one every
 *  card shows first, then the one behind the banner, then the one a lying
 *  card shows, then the drawn title. */
const KINDS: PictureKind[] = ["poster", "backdrop", "thumb", "logo"];

export function PicturesDialog({
  workId,
  onClose,
  /** Said whenever anything changed, so the screens showing this work read it
      again rather than keeping the picture that is no longer there. */
  onChanged,
}: {
  workId: string;
  onClose: () => void;
  onChanged: () => void;
}) {
  const { t } = useSettings();
  const [held, setHeld] = useState<HeldPicture[] | null>(null);
  /** Which kind is being chosen, when one is. Nothing on the first screen. */
  const [choosing, setChoosing] = useState<PictureKind | null>(null);
  const [offered, setOffered] = useState<OfferedPicture[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  const readHeld = useCallback(async () => {
    try {
      setHeld(await api.heldPictures(workId));
    } catch (error) {
      setRefused(refusalOf(error));
    }
  }, [workId]);

  useEffect(() => {
    void readHeld();
  }, [readHeld]);

  const look = async (kind: PictureKind) => {
    setChoosing(kind);
    setOffered(null);
    setRefused(null);
    setBusy(true);
    try {
      setOffered(await api.offeredPictures(workId));
    } catch (error) {
      setRefused(refusalOf(error));
    } finally {
      setBusy(false);
    }
  };

  const choose = async (kind: PictureKind, path: string) => {
    setBusy(true);
    setRefused(null);
    try {
      await api.choosePicture(workId, kind, path);
      await readHeld();
      onChanged();
      setChoosing(null);
    } catch (error) {
      setRefused(refusalOf(error));
    } finally {
      setBusy(false);
    }
  };

  const forget = async (kind: PictureKind) => {
    setBusy(true);
    setRefused(null);
    try {
      await api.forgetPicture(workId, kind);
      await readHeld();
      onChanged();
    } catch (error) {
      setRefused(refusalOf(error));
    } finally {
      setBusy(false);
    }
  };

  const ofKind = (kind: PictureKind) => held?.find((picture) => picture.kind === kind);

  return (
    <Modal
      title={t("pictures.title")}
      onBack={choosing ? () => setChoosing(null) : undefined}
      onClose={onClose}
    >
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}

      {!choosing && (
        <ul className="pictures-held">
          {KINDS.map((kind) => {
            const picture = ofKind(kind);
            return (
              <li className={`pictures-tile pictures-tile-${kind}`} key={kind}>
                <span className="pictures-frame">
                  {picture ? (
                    <img src={picture.url} alt="" />
                  ) : (
                    <span className="pictures-none">{t("pictures.none")}</span>
                  )}
                </span>
                <span className="pictures-what">
                  <span className="pictures-kind">{t(`pictures.kind.${kind}`)}</span>
                  <span className="pictures-size">
                    {picture && picture.width && picture.height
                      ? `${picture.width} × ${picture.height}`
                      : t("pictures.nothing_here")}
                  </span>
                </span>
                <span className="pictures-doing">
                  <button
                    className="pictures-act"
                    onClick={() => look(kind)}
                    disabled={busy}
                    aria-label={t("pictures.look")}
                    title={t("pictures.look")}
                  >
                    <SearchIcon size={18} />
                  </button>
                  {picture && (
                    <button
                      className="pictures-act"
                      onClick={() => forget(kind)}
                      disabled={busy}
                      aria-label={t("pictures.forget")}
                      title={t("pictures.forget")}
                    >
                      <DeleteIcon size={18} />
                    </button>
                  )}
                </span>
              </li>
            );
          })}
        </ul>
      )}

      {choosing && (
        <>
          <h3 className="pictures-heading">{t(`pictures.kind.${choosing}`)}</h3>
          {offered === null ? (
            <p className="notice">{t(busy ? "pictures.looking" : "pictures.nothing")}</p>
          ) : (
            <Offered
              kind={choosing}
              offered={offered}
              busy={busy}
              onChoose={(path) => choose(choosing, path)}
            />
          )}
        </>
      )}
    </Modal>
  );
}

/** Everything the provider holds of the one kind being chosen. */
function Offered({
  kind,
  offered,
  busy,
  onChoose,
}: {
  kind: PictureKind;
  offered: OfferedPicture[];
  busy: boolean;
  onChoose: (path: string) => void;
}) {
  const { t } = useSettings();
  /* Of this kind, and largest first: the one fault a tile cannot show is a
     picture too small for where it is drawn, so the ones that are big enough
     come first rather than being hunted for. */
  const shown = offered
    .filter((picture) => picture.kind === kind)
    .sort((left, right) => (right.width ?? 0) - (left.width ?? 0));

  if (shown.length === 0) {
    return <p className="notice">{t("pictures.nothing")}</p>;
  }
  return (
    <ul className={`pictures-offered pictures-offered-${kind}`}>
      {shown.map((picture) => (
        <li key={picture.path}>
          <button
            className="pictures-choice"
            onClick={() => onChoose(picture.path)}
            disabled={busy}
          >
            <span className="pictures-frame">
              <img src={picture.url} alt="" loading="lazy" />
            </span>
            <span className="pictures-size">
              {picture.width && picture.height
                ? `${picture.width} × ${picture.height}`
                : ""}
              {picture.language && <span className="pictures-tongue">{picture.language}</span>}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

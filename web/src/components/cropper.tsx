/*
 * Framing a photo before it becomes somebody's picture.
 *
 * The photo is pulled around by hand and brought closer with the slider or
 * the wheel, inside the square it will be shown in. What is sent is exactly
 * that square, cut here: the browser already sees the photo the right way up,
 * so what is sent is what was seen, and a photo of ten megabytes leaves as a
 * few dozen kilobytes.
 */

import { useEffect, useRef, useState } from "react";
import type { Framing } from "../cropping";
import { centred, MOST_ZOOM, moved, squareShown, zoomedTo } from "../cropping";
import { useSettings } from "../settings";
import { Modal } from "./modal";

/** How many points across the frame is drawn, which fits a phone held
 *  upright with the margins of the panel around it. */
const FRAME = 260;

/** How many pixels across the square sent is: twice the largest it is shown
 *  at, which the server brings down to its own size. */
const SENT = 512;

/** How much one notch of the wheel zooms. */
const NOTCH = 0.1;

export function Cropper({
  image,
  onClose,
  onFramed,
}: {
  image: File;
  onClose: () => void;
  /** Handed the square chosen, ready to be sent. */
  onFramed: (square: Blob) => void;
}) {
  const { t } = useSettings();
  const [url, setUrl] = useState<string | null>(null);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  const [framing, setFraming] = useState<Framing | null>(null);
  const [unreadable, setUnreadable] = useState(false);
  const photo = useRef<HTMLImageElement>(null);
  const pulledFrom = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    const made = URL.createObjectURL(image);
    setUrl(made);
    return () => URL.revokeObjectURL(made);
  }, [image]);

  const arrived = () => {
    const shown = photo.current;
    if (!shown || shown.naturalWidth === 0) {
      setUnreadable(true);
      return;
    }
    setSize({ width: shown.naturalWidth, height: shown.naturalHeight });
    setFraming(centred(FRAME, shown.naturalWidth, shown.naturalHeight));
  };

  const zoomTo = (zoom: number) => {
    if (framing && size) {
      setFraming(zoomedTo(framing, zoom, FRAME, size.width, size.height));
    }
  };

  const keep = () => {
    const shown = photo.current;
    if (!shown || !framing || !size) {
      return;
    }
    const square = squareShown(framing, FRAME, size.width, size.height);
    const side = Math.min(SENT, Math.round(square.side));
    const canvas = document.createElement("canvas");
    canvas.width = side;
    canvas.height = side;
    canvas
      .getContext("2d")
      ?.drawImage(shown, square.x, square.y, square.side, square.side, 0, 0, side, side);
    canvas.toBlob(
      (blob) => (blob ? onFramed(blob) : setUnreadable(true)),
      "image/jpeg",
      0.92,
    );
  };

  const scale = framing && size ? (FRAME / Math.min(size.width, size.height)) * framing.zoom : 0;

  return (
    <Modal
      title={t("crop.title")}
      onClose={onClose}
      footer={
        <>
          <button className="button" onClick={onClose}>
            {t("crop.cancel")}
          </button>
          <button className="button button-accent" disabled={!framing} onClick={keep}>
            {t("crop.keep")}
          </button>
        </>
      }
    >
      {unreadable ? (
        <p className="notice">{t("refused.avatar.could_not_be_read")}</p>
      ) : (
        <div className="cropper">
          <p className="settings-why">{t("crop.why")}</p>
          <div
            className="cropper-frame"
            style={{ width: FRAME, height: FRAME }}
            onPointerDown={(event) => {
              event.currentTarget.setPointerCapture(event.pointerId);
              pulledFrom.current = { x: event.clientX, y: event.clientY };
            }}
            onPointerMove={(event) => {
              const from = pulledFrom.current;
              if (!from || !framing || !size) {
                return;
              }
              pulledFrom.current = { x: event.clientX, y: event.clientY };
              setFraming(
                moved(
                  framing,
                  event.clientX - from.x,
                  event.clientY - from.y,
                  FRAME,
                  size.width,
                  size.height,
                ),
              );
            }}
            onPointerUp={() => (pulledFrom.current = null)}
            onPointerCancel={() => (pulledFrom.current = null)}
            onWheel={(event) => framing && zoomTo(framing.zoom - Math.sign(event.deltaY) * NOTCH)}
          >
            {url && (
              <img
                ref={photo}
                src={url}
                alt=""
                draggable={false}
                onLoad={arrived}
                onError={() => setUnreadable(true)}
                style={
                  framing && size
                    ? {
                        left: framing.x,
                        top: framing.y,
                        width: size.width * scale,
                        height: size.height * scale,
                      }
                    : { visibility: "hidden" }
                }
              />
            )}
          </div>
          <label className="cropper-zoom">
            <span>{t("crop.zoom")}</span>
            <input
              type="range"
              min={1}
              max={MOST_ZOOM}
              step={0.01}
              value={framing?.zoom ?? 1}
              disabled={!framing}
              onChange={(event) => zoomTo(Number(event.target.value))}
            />
          </label>
        </div>
      )}
    </Modal>
  );
}

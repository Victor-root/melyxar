/*
 * A camera watched live in a browser, over the whole window: the picture the
 * server relays, and along its top the camera's name, that it is live, and
 * what can be done with it. The cross, the escape key and the back key of a
 * phone close it, and closing it lets go of the camera on the server.
 */

import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../api";
import type { CameraShown } from "../api";
import { useBackClosesIt } from "../components/back-closes";
import { useHoldsTheScreen } from "../components/hold-the-screen";
import { CloseIcon, SecurityCameraIcon } from "../icons";
import { FullscreenIcon, VolumeIcon } from "../player/icons";
import { playRelayed } from "../player/relayed-camera";
import type { Trouble } from "../player/relayed-camera";
import { useSettings } from "../settings";

type State = "opening" | "playing" | Trouble;

export function CameraLive({
  camera,
  onClose,
}: {
  camera: CameraShown;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const frame = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState<State>("opening");
  const [muted, setMuted] = useState(false);
  const [whole, setWhole] = useState(false);
  /** Counted up to open the camera again from the start. */
  const [tries, setTries] = useState(0);
  useHoldsTheScreen(frame, onClose);
  const leave = useBackClosesIt(onClose);

  useEffect(() => {
    const shown = video.current;
    if (!shown) {
      return;
    }
    setState("opening");
    return playRelayed(shown, api.relayedCamera(camera.id), setState);
  }, [camera.id, tries]);

  useEffect(() => {
    const told = () => setWhole(document.fullscreenElement === frame.current);
    document.addEventListener("fullscreenchange", told);
    return () => document.removeEventListener("fullscreenchange", told);
  }, []);

  const toggleWhole = () => {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void frame.current?.requestFullscreen();
    }
  };

  const toggleSound = () => {
    if (video.current) {
      video.current.muted = !video.current.muted;
    }
  };

  return createPortal(
    <div
      ref={frame}
      className="camera-live"
      role="dialog"
      aria-modal="true"
      aria-label={t("cameras.watch", { name: camera.name })}
      tabIndex={-1}
    >
      <video
        ref={video}
        className="camera-live-picture"
        playsInline
        onPlaying={() => setState("playing")}
        onVolumeChange={(event) => setMuted(event.currentTarget.muted)}
      />
      <header className="camera-live-head">
        <span className="camera-live-mark" aria-hidden="true">
          <SecurityCameraIcon size={22} />
        </span>
        <span className="camera-live-name">{camera.name}</span>
        <span className="camera-live-badge">{t("cameras.live")}</span>
        <span className="camera-live-gap" />
        <button
          type="button"
          className="camera-live-button"
          aria-label={t(muted ? "player.unmute" : "player.mute")}
          title={t(muted ? "player.unmute" : "player.mute")}
          onClick={toggleSound}
        >
          <VolumeIcon level={muted ? "off" : "high"} size={22} />
        </button>
        <button
          type="button"
          className="camera-live-button"
          aria-label={t(
            whole ? "player.leave_fullscreen" : "player.fullscreen",
          )}
          title={t(whole ? "player.leave_fullscreen" : "player.fullscreen")}
          onClick={toggleWhole}
        >
          <FullscreenIcon leaving={whole} size={22} />
        </button>
        <button
          type="button"
          className="camera-live-button"
          aria-label={t("modal.close")}
          title={t("modal.close")}
          onClick={() => leave(onClose)}
        >
          <CloseIcon size={22} />
        </button>
      </header>
      {state !== "playing" && (
        <div className="camera-live-say" role="status">
          <p>
            {t(
              state === "opening"
                ? "cameras.opening"
                : state === "unplayable"
                  ? "cameras.unplayable"
                  : "cameras.lost",
            )}
          </p>
          {state === "lost" && (
            <button
              type="button"
              className="button"
              onClick={() => setTries((count) => count + 1)}
            >
              {t("error.retry")}
            </button>
          )}
        </div>
      )}
    </div>,
    document.body,
  );
}

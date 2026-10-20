/*
 * A library of cameras: a tile for each, its mark and its name, with nothing
 * taken from its picture. A tile opens the camera live: inside the Android
 * application in its own screen, the application opening the camera itself;
 * in a browser over the page, relayed by the server.
 */

import { useState } from "react";
import { api } from "../api";
import type { CameraShown, Library } from "../api";
import { useAsked } from "../asking";
import { SecurityCameraIcon } from "../icons";
import { appCamera } from "../player/app-camera";
import { CameraLive } from "./camera-live";
import { howMany } from "../readable";
import { useSettings } from "../settings";

export function CamerasPage({ library }: { library: Library }) {
  const { t } = useSettings();
  const cameras = useAsked(
    (signal) => api.cameras(library.id, signal),
    [library.id, library.version],
  );
  const [refused, setRefused] = useState(false);
  const [watching, setWatching] = useState<CameraShown | null>(null);

  /* The address holds a password: handed straight to the application and
     kept nowhere on the page. A browser is never given it. */
  const watch = (camera: CameraShown) => {
    if (!appCamera) {
      setWatching(camera);
      return;
    }
    const opener = appCamera;
    setRefused(false);
    api
      .liveCamera(camera.id)
      .then(({ address }) => opener.watch(address, camera.name))
      .catch(() => setRefused(true));
  };

  return (
    <main className="page">
      <div className="section-head">
        <h1>{library.name}</h1>
        <span className="count">
          {howMany(library.works, "admin.cameras_count", t)}
        </span>
      </div>
      {refused && <p className="notice">{t("cameras.cannot_open")}</p>}
      {cameras.answer?.length === 0 && (
        <p className="notice">{t("cameras.none")}</p>
      )}
      <div className="camera-grid">
        {cameras.answer?.map((camera) => (
          <button
            type="button"
            key={camera.id}
            className="camera-tile"
            aria-label={t("cameras.watch", { name: camera.name })}
            onClick={() => watch(camera)}
          >
            <CameraFace name={camera.name} />
          </button>
        ))}
      </div>
      {watching && (
        <CameraLive camera={watching} onClose={() => setWatching(null)} />
      )}
    </main>
  );
}

function CameraFace({ name }: { name: string }) {
  return (
    <>
      <span className="camera-mark" aria-hidden="true">
        <SecurityCameraIcon size={40} />
      </span>
      <span className="camera-name">{name}</span>
    </>
  );
}

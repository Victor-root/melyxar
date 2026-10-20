/*
 * The cameras of a library of cameras, in its panel of the administration:
 * each one declared with its name, its RTSP address and its account. The
 * password is written here and never shown again; the server only says
 * whether one is kept.
 */

import { useState } from "react";
import { api } from "../../api";
import type { CameraShown, Library } from "../../api";
import { refusalAbout, useAsked, useTold } from "../../asking";
import { Setting, Toggle } from "../../components/panel";
import { DeleteIcon, EditIcon, SecurityCameraIcon } from "../../icons";
import { useSettings } from "../../settings";

export function CamerasAdmin({
  library,
  onChanged,
}: {
  library: Library;
  onChanged: () => void;
}) {
  const { t } = useSettings();
  const cameras = useAsked(
    (signal) => api.cameras(library.id, signal),
    [library.id, library.version],
  );
  /* The camera being written: a new one, one of the list by its id, or none. */
  const [writing, setWriting] = useState<"new" | string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const removing = useTold(async (camera: CameraShown) => {
    try {
      await api.removeCamera(camera.id);
      setRefusal(null);
    } catch (error) {
      setRefusal(refusalAbout(error, "camera"));
    }
    cameras.again();
    onChanged();
  });

  const done = () => {
    setWriting(null);
    setRefusal(null);
    cameras.again();
    onChanged();
  };

  return (
    <div className="library-folders">
      <div className="library-folders-head">
        <span>{t("admin.cameras")}</span>
        <button
          className="button button-small"
          onClick={() => setWriting("new")}
        >
          <SecurityCameraIcon size={15} />
          {t("admin.camera_add")}
        </button>
      </div>
      {refusal && (
        <p className="panel-notice panel-notice-trouble">{t(refusal)}</p>
      )}
      {writing === "new" && (
        <CameraForm
          library={library.id}
          onDone={done}
          onCancel={() => setWriting(null)}
        />
      )}
      <div className="lines">
        {cameras.answer?.length === 0 && writing !== "new" && (
          <p className="empty-line">{t("admin.cameras_none")}</p>
        )}
        {cameras.answer?.map((camera) =>
          writing === camera.id ? (
            <CameraForm
              key={camera.id}
              library={library.id}
              camera={camera}
              onDone={done}
              onCancel={() => setWriting(null)}
            />
          ) : (
            <div className="line" key={camera.id}>
              <span className="line-mark" aria-hidden="true">
                <SecurityCameraIcon size={18} />
              </span>
              <span className="line-words">
                <span className="line-name">{camera.name}</span>
                <span className="line-note line-path">{camera.address}</span>
                <span className="line-note">
                  {[
                    camera.username &&
                      t("admin.camera_user_is", { name: camera.username }),
                    t(
                      camera.has_password
                        ? "admin.camera_password_kept"
                        : "admin.camera_no_password",
                    ),
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </span>
              <span className="line-end">
                <button
                  className="button button-small button-quiet"
                  title={t("admin.camera_change")}
                  aria-label={t("admin.camera_change")}
                  onClick={() => setWriting(camera.id)}
                >
                  <EditIcon size={15} />
                </button>
                <button
                  className="button button-small button-quiet"
                  title={t("admin.camera_remove")}
                  aria-label={t("admin.camera_remove")}
                  disabled={removing.busy}
                  onClick={() => void removing.tell(camera)}
                >
                  <DeleteIcon size={15} />
                </button>
              </span>
            </div>
          ),
        )}
      </div>
    </div>
  );
}

/** Declares a camera, or changes `camera` when it is given. */
function CameraForm({
  library,
  camera,
  onDone,
  onCancel,
}: {
  library: string;
  camera?: CameraShown;
  onDone: () => void;
  onCancel: () => void;
}) {
  const { t } = useSettings();
  const [name, setName] = useState(camera?.name ?? "");
  const [address, setAddress] = useState(camera?.address ?? "rtsp://");
  const [username, setUsername] = useState(camera?.username ?? "");
  const [password, setPassword] = useState("");
  const [forgetting, setForgetting] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const saving = useTold(async () => {
    const declared = {
      name,
      address,
      username,
      // Left empty, the kept password stays as it is.
      password: password || undefined,
    };
    try {
      if (camera) {
        await api.changeCamera(camera.id, {
          ...declared,
          forget_password: forgetting,
        });
      } else {
        await api.declareCamera(library, declared);
      }
      onDone();
    } catch (error) {
      setRefusal(refusalAbout(error, "camera"));
    }
  });

  return (
    <div className="camera-form settings-lines">
      <Setting label={t("admin.camera_name")}>
        <input
          type="text"
          className="field-line"
          value={name}
          autoFocus
          aria-label={t("admin.camera_name")}
          onChange={(event) => setName(event.target.value)}
        />
      </Setting>
      <Setting
        label={t("admin.camera_address")}
        why={t("admin.camera_address_why")}
      >
        <input
          type="text"
          className="field-line"
          value={address}
          spellCheck={false}
          autoComplete="off"
          aria-label={t("admin.camera_address")}
          onChange={(event) => setAddress(event.target.value)}
        />
      </Setting>
      <Setting label={t("admin.camera_username")}>
        <input
          type="text"
          className="field-line"
          value={username}
          autoComplete="off"
          aria-label={t("admin.camera_username")}
          onChange={(event) => setUsername(event.target.value)}
        />
      </Setting>
      <Setting
        label={t("admin.camera_password")}
        why={t(
          camera?.has_password
            ? "admin.camera_password_unchanged"
            : "admin.camera_password_why",
        )}
      >
        <input
          type="password"
          className="field-line"
          value={password}
          autoComplete="new-password"
          disabled={forgetting}
          aria-label={t("admin.camera_password")}
          onChange={(event) => setPassword(event.target.value)}
        />
      </Setting>
      {camera?.has_password && (
        <Setting label={t("admin.camera_forget_password")}>
          <Toggle
            checked={forgetting}
            onChange={setForgetting}
            label={t("admin.camera_forget_password")}
          />
        </Setting>
      )}
      {refusal && (
        <p className="panel-notice panel-notice-trouble">{t(refusal)}</p>
      )}
      <div className="panel-foot">
        <button className="button" onClick={onCancel}>
          {t("settings.cancel")}
        </button>
        <button
          className="button button-accent"
          disabled={saving.busy || !name.trim()}
          onClick={() => void saving.tell()}
        >
          {t(camera ? "admin.camera_save" : "admin.camera_add")}
        </button>
      </div>
    </div>
  );
}

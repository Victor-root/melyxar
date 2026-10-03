/*
 * "Calibrer cet appareil": the one control a viewer who does not know what a
 * codec is ever has to touch.
 *
 * How the measurement is made lives in `run.ts`; this only shows where it
 * stands and starts it. A device is calibrated or it is not: a run that
 * stopped before the end left nothing behind, and says so.
 */

import { useEffect, useRef, useState } from "react";
import { Panel, Setting } from "../components/panel";
import { DeviceIcon } from "../icons";
import { useSettings } from "../settings";
import { forgetMeasuredCapabilities } from "../player/profile";
import { resetCalibration, runCalibration, storedCalibration } from "./run";
import type { CalibrationProgress } from "./run";

type Status = "checking" | "idle" | "running" | "interrupted";

export function DeviceOptimization() {
  const { t } = useSettings();
  const [status, setStatus] = useState<Status>("checking");
  const [calibrated, setCalibrated] = useState(false);
  const [progress, setProgress] = useState<CalibrationProgress | null>(null);
  /* Where the clips really play, on the page: a browser asked to put a film
     somewhere nobody can see skips the work of showing it, and then reports
     having dropped none of it. */
  const testing = useRef<HTMLVideoElement>(null);

  const check = () =>
    storedCalibration()
      .then((kept) => setCalibrated(kept !== null))
      .catch(() => setCalibrated(false));

  useEffect(() => {
    check().finally(() => setStatus("idle"));
  }, []);

  /* Started from here rather than from the button, so that the element the
     clips play in is on the page before anything is asked to play in it. */
  useEffect(() => {
    if (status !== "running") {
      return;
    }
    const video = testing.current;
    if (!video) {
      return;
    }
    video.scrollIntoView({ block: "center", behavior: "smooth" });

    let gone = false;
    runCalibration(video, setProgress)
      .then(() => {
        if (gone) return;
        /* The very next question about this device answers from what was
           just measured. */
        forgetMeasuredCapabilities();
        setProgress(null);
        check().finally(() => setStatus("idle"));
      })
      .catch(() => {
        if (gone) return;
        setProgress(null);
        setStatus("interrupted");
      });
    return () => {
      gone = true;
    };
  }, [status]);

  const reset = async () => {
    await resetCalibration();
    forgetMeasuredCapabilities();
    check();
  };

  const label = () => {
    if (status === "checking") return "";
    if (status === "running") {
      if (progress?.phase === "preparing") {
        return t("settings.device_preparing", { done: progress.done, total: progress.total });
      }
      if (progress?.phase === "measuring") {
        return t("settings.device_running", {
          codec: progress.codec.toUpperCase(),
          height: progress.height,
        });
      }
      return t("settings.device_starting");
    }
    if (status === "interrupted") return t("settings.device_interrupted");
    return t(calibrated ? "settings.device_optimized" : "settings.device_not_optimized");
  };

  return (
    <Panel icon={DeviceIcon} title={t("settings.device")} lead={t("settings.device_why")}>
      <Setting label={label()}>
        {calibrated && status === "idle" && (
          <button className="button button-small button-quiet" onClick={reset}>
            {t("settings.device_reset")}
          </button>
        )}
        <button
          className="button button-small button-accent"
          onClick={() => {
            setProgress(null);
            setStatus("running");
          }}
          disabled={status === "running" || status === "checking"}
        >
          {t("settings.device_optimize")}
        </button>
      </Setting>

      {status === "running" && (
        <video ref={testing} className="device-test" muted playsInline aria-hidden="true" />
      )}
    </Panel>
  );
}

/*
 * "Optimiser cet appareil": the one control a viewer who does not know what a
 * codec is ever has to touch.
 *
 * Everything about how the measurement is made lives in `calibration.ts`;
 * this only shows where it stands and starts it. No jargon in the ordinary
 * state: a device is either optimized or it is not, and the codec names only
 * appear while a test is actually running, which is the one moment naming
 * them means something to somebody watching a progress bar.
 */

import { useEffect, useState } from "react";
import { useSettings } from "../settings";
import { isCurrent, runCalibration, storedCalibration } from "./calibration";
import type { CalibrationProgress } from "./calibration";
import { forgetMeasuredCapabilities } from "./profile";

type Status = "checking" | "idle" | "running" | "done";

export function DeviceOptimization() {
  const { t } = useSettings();
  const [status, setStatus] = useState<Status>("checking");
  const [optimized, setOptimized] = useState(false);
  const [progress, setProgress] = useState<CalibrationProgress | null>(null);

  const check = () => {
    storedCalibration()
      .then((entries) => setOptimized(isCurrent(entries)))
      .catch(() => setOptimized(false))
      .finally(() => setStatus((current) => (current === "running" ? current : "idle")));
  };

  useEffect(check, []);

  const start = async () => {
    setStatus("running");
    setProgress(null);
    try {
      await runCalibration(setProgress);
    } finally {
      // The very next question about this device must answer from what was
      // just measured, not from whatever was cached before the button was
      // pressed.
      forgetMeasuredCapabilities();
      setProgress(null);
      check();
    }
  };

  return (
    <section className="settings-block">
      <h2>{t("settings.device")}</h2>
      <p className="settings-why">{t("settings.device_why")}</p>

      <p className="device-status">
        {status === "running" && progress
          ? t("settings.device_running", {
              codec: progress.codec.toUpperCase(),
              height: progress.height,
            })
          : status !== "checking" &&
            t(optimized ? "settings.device_optimized" : "settings.device_not_optimized")}
      </p>

      <button
        className="button button-small"
        onClick={start}
        disabled={status === "running" || status === "checking"}
      >
        {t("settings.device_optimize")}
      </button>
    </section>
  );
}

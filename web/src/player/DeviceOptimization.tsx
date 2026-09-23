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

import { useEffect, useRef, useState } from "react";
import { Panel, Setting } from "../components/panel";
import { DeviceIcon } from "../icons";
import { useSettings } from "../settings";
import {
  resetCalibration,
  runCalibration,
  storedCalibration,
  worthTrusting,
} from "./calibration";
import type { CalibrationProgress } from "./calibration";
import { forgetMeasuredCapabilities } from "./profile";

type Status = "checking" | "idle" | "running";

export function DeviceOptimization() {
  const { t } = useSettings();
  const [status, setStatus] = useState<Status>("checking");
  const [optimized, setOptimized] = useState(false);
  /* Whether anything at all is on record for this device, current or not:
     a run interrupted partway through still leaves rows behind, and those
     have to be clearable too, not only a finished calibration. */
  const [hasStored, setHasStored] = useState(false);
  const [progress, setProgress] = useState<CalibrationProgress | null>(null);
  /* Where the reference film really plays, and not a detail: a browser asked
     to put a film somewhere nobody can see skips the work of showing it, and
     then reports having dropped none of it. Every machine passed that way. */
  const testing = useRef<HTMLVideoElement>(null);

  const check = () =>
    storedCalibration()
      .then((entries) => {
        setOptimized(worthTrusting(entries));
        setHasStored(entries.length > 0);
      })
      .catch(() => {
        setOptimized(false);
        setHasStored(false);
      });

  useEffect(() => {
    check().finally(() => setStatus("idle"));
  }, []);

  /* Started from here rather than from the button, so that the element the
     film plays in is on the page before anything is asked to play in it. */
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
    runCalibration(video, setProgress).finally(() => {
      if (gone) {
        return;
      }
      // The very next question about this device must answer from what was
      // just measured, not from whatever was cached before the button was
      // pressed.
      forgetMeasuredCapabilities();
      setProgress(null);
      check().finally(() => setStatus("idle"));
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

  return (
    <Panel icon={DeviceIcon} title={t("settings.device")} lead={t("settings.device_why")}>
      <Setting
        label={
          status === "running" && progress
            ? t("settings.device_running", {
                codec: progress.codec.toUpperCase(),
                height: progress.height,
              })
            : status === "checking"
              ? ""
              : t(optimized ? "settings.device_optimized" : "settings.device_not_optimized")
        }
      >
        {hasStored && status !== "running" && status !== "checking" && (
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

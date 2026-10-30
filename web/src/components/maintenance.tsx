/*
 * What a maintenance looks like: the screen that stands in for the whole
 * interface while it is in force for whoever is not an administrator, and
 * the band that counts down to it on every screen, a film included, and
 * reminds an administrator that it is still on.
 */

import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../api";
import type { Branding } from "../api";
import { asClock } from "../clock";
import { WarningIcon } from "../icons";
import { maintenanceHeard, useMaintenance } from "../maintenance";
import type { Standing } from "../maintenance";
import { writtenMoment } from "../readable";
import { useSettings } from "../settings";
import { DoorBackground } from "./door-background";
import { useFillingTheScreen } from "./filling-the-screen";
import { ServerMark } from "./server-mark";

/** What a server is called before its own name has arrived, as the server
 *  itself calls one nobody named. */
export const UNNAMED = "Melyxar";

/** When it is expected to end, as this browser reads an hour. */
function useBackAt(standing: Standing): string | null {
  const { language } = useSettings();
  return standing.until === null
    ? null
    : writtenMoment(new Date(standing.until), language, { hour: "2-digit", minute: "2-digit" });
}

/**
 * The screen of a maintenance in force. Somebody signed in is offered to sign
 * out, so an administrator can come in on this browser; nobody signed in is
 * offered the door, kept for administrators until the maintenance ends.
 */
export function MaintenanceScreen({
  branding,
  signedIn,
  onDoor,
  onLeave,
}: {
  /** What the door wears, when it is known already. */
  branding: Branding | null;
  signedIn: boolean;
  onDoor: () => void;
  onLeave: () => void;
}) {
  const { t } = useSettings();
  const standing = useMaintenance();
  const backAt = useBackAt(standing);
  const [wearing, setWearing] = useState<Branding | null>(branding);

  useEffect(() => {
    if (wearing) {
      return;
    }
    const controller = new AbortController();
    api
      .branding(controller.signal)
      .then(setWearing)
      .catch(() => {
        // The screen stands without the server's own picture behind it.
      });
    return () => controller.abort();
  }, [wearing]);

  return (
    <main className="door maintenance" aria-live="polite">
      {wearing && <DoorBackground branding={wearing} />}
      <div className="door-stage" aria-hidden="true" />
      <div className="maintenance-hall">
        <ServerMark branding={wearing} size={120} className="door-greeting-mark" />
        <h1 className="maintenance-title">{t("maintenance.title")}</h1>
        {/* What the administrator wrote, or that this server is closed. */}
        <p className="maintenance-message">
          {standing.message ?? t("maintenance.closed", { name: wearing?.server_name ?? UNNAMED })}
        </p>
        <p className="maintenance-when">
          {backAt ? t("maintenance.back_at", { time: backAt }) : t("maintenance.back_soon")}
        </p>
        <p className="maintenance-note">{t("maintenance.comes_back")}</p>
        <button
          type="button"
          className="button button-small button-quiet"
          onClick={signedIn ? onLeave : onDoor}
        >
          {t(signedIn ? "nav.sign_out" : "maintenance.administrator")}
        </button>
      </div>
    </main>
  );
}

/**
 * The band over every screen: the time left before an announced maintenance
 * takes hold, to everybody, and to an administrator, that one is in force.
 * Drawn inside whatever fills the screen, a film among them.
 */
export function MaintenanceBand({ administrator }: { administrator: boolean }) {
  const standing = useMaintenance();
  const filling = useFillingTheScreen();
  if (standing.state === "announced" && standing.startsAt !== null) {
    return createPortal(<Countdown standing={standing} startsAt={standing.startsAt} />, filling ?? document.body);
  }
  if (standing.state === "on" && administrator) {
    return createPortal(<InForce standing={standing} />, filling ?? document.body);
  }
  return null;
}

/** The time left before it takes hold, said again every second. */
function Countdown({ standing, startsAt }: { standing: Standing; startsAt: number }) {
  const { t } = useSettings();
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const beat = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(beat);
  }, []);
  const left = Math.max(0, Math.ceil((startsAt - now) / 1000));
  return (
    <div className="maintenance-band" role="status">
      <WarningIcon size={18} />
      <span className="maintenance-band-words">
        <strong>{t("maintenance.announced", { time: asClock(left) })}</strong>
        <span>{standing.message ?? t("maintenance.announced_detail")}</span>
      </span>
    </div>
  );
}

/** A maintenance in force, said to the administrator it lets through. */
function InForce({ standing }: { standing: Standing }) {
  const { t } = useSettings();
  const backAt = useBackAt(standing);
  const [ending, setEnding] = useState(false);
  const [failed, setFailed] = useState(false);
  /* Switched off from here, wherever the administrator is: every page,
     this one included, hears it at once. */
  const end = () => {
    setEnding(true);
    setFailed(false);
    api
      .setMaintenance({ on: false, delay_minutes: 0, until: null, message: null })
      .then(maintenanceHeard)
      .catch(() => {
        setFailed(true);
        setEnding(false);
      });
  };
  return (
    <div className="maintenance-band" role="status">
      <WarningIcon size={18} />
      <span className="maintenance-band-words">
        <strong>{t(failed ? "admin.maintenance_failed" : "maintenance.in_force")}</strong>
        {backAt && <span>{t("maintenance.back_at", { time: backAt })}</span>}
      </span>
      <button type="button" className="button button-small" disabled={ending} onClick={end}>
        {t("maintenance.end")}
      </button>
    </div>
  );
}

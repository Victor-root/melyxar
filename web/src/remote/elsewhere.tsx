/*
 * What plays on another device of the account, shown at the end of the bar
 * while the remote is shut, as the television shows the song it plays: a
 * round with its picture, which opens the remote on that device, and the
 * piece of glass filled with the accent as far as it has played.
 *
 * The fill runs on by itself in the browser between two words of the line,
 * drawn on its own layer: nothing of the bar is drawn again as it moves.
 */

import type { SteerableDevice } from "../api";
import { DeviceKindIcon } from "../components/device-lines";
import { deviceKind, deviceName } from "../devices";
import { useSettings } from "../settings";
import { positionNow, useSteerableDevices } from "./devices";
import { openTheRemote, useRemoteOpen } from "./remote";

/** The first device of the account playing something, if any. */
export function playingElsewhere(devices: SteerableDevice[] | null): SteerableDevice | null {
  return devices?.find((device) => device.playing !== null) ?? null;
}

export function PlayingElsewhere() {
  const { t } = useSettings();
  const { devices, heardAt } = useSteerableDevices();
  const remoteOpen = useRemoteOpen();
  const device = playingElsewhere(devices);
  const playing = device?.playing ?? null;
  if (!device || !playing || remoteOpen) {
    return null;
  }
  const length = playing.duration_seconds;
  const at = positionNow(playing, heardAt, Date.now());
  const share = length ? Math.min(1, at / length) : 0;
  const said = t(playing.paused ? "remote.paused_on" : "remote.playing_on", { title: playing.title });
  return (
    <>
      {length !== null && (
        <span
          key={heardAt}
          className="header-elsewhere-fill"
          aria-hidden="true"
          data-running={playing.paused ? undefined : ""}
          style={{
            ["--from" as string]: `${share}`,
            ["--left" as string]: `${Math.max(0, length - at)}s`,
          }}
        />
      )}
      <button
        type="button"
        className="header-icon header-elsewhere"
        title={`${said} · ${deviceName(device.name, t, device.browser)}`}
        aria-label={said}
        onClick={() => openTheRemote(device.device)}
      >
        {playing.picture ? (
          <img src={playing.picture} alt="" />
        ) : (
          <DeviceKindIcon kind={deviceKind(device.name, device.browser)} size={18} />
        )}
      </button>
    </>
  );
}

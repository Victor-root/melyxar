/*
 * Devices signed in, one line each, and the way to sign one out.
 *
 * The same lines for the administration, which sees every account's, and for
 * one's own profile. The device the list is looked at from says so and is not
 * signed out from here: that is what signing out is for.
 */

import { useState } from "react";
import type { SignedInDevice } from "../api";
import { refusalOf, useTold } from "../asking";
import { aboutDevice, deviceName } from "../devices";
import { refusalKey } from "../i18n";
import { DeviceIcon } from "../icons";
import { useSettings } from "../settings";
import { Face } from "./face";
import { useToast } from "./toasts";

export function DeviceLines({
  devices,
  withAccount,
  signOut,
  onSignedOut,
}: {
  devices: SignedInDevice[];
  /** Whether the lines are of more than one account, and say whose. */
  withAccount: boolean;
  signOut: (device: string) => Promise<unknown>;
  onSignedOut: () => void;
}) {
  const { t, language } = useSettings();
  const toast = useToast();
  /* The device whose sign out is being asked, one at a time. */
  const [asking, setAsking] = useState<string | null>(null);
  const now = Date.now();

  const told = useTold(async (device: SignedInDevice) => {
    const name = deviceName(device.name, t, device.browser);
    try {
      await signOut(device.id);
      toast({
        state: "ok",
        title: t("device.signed_out"),
        detail: withAccount ? t("device.signed_out_of", { device: name, user: device.user_name }) : name,
      });
    } catch (error) {
      // Signed out meanwhile, from here or elsewhere, is what was wanted.
      if (refusalOf(error) !== "not_found") {
        toast({ state: "trouble", title: t("device.sign_out_failed"), detail: t(refusalKey(refusalOf(error))) });
      }
    }
    setAsking(null);
    onSignedOut();
  });

  if (devices.length === 0) {
    return <p className="empty-line">{t("device.none")}</p>;
  }

  return (
    <div className="lines">
      {devices.map((device) => (
        <div className="line" key={device.id}>
          {withAccount ? (
            <span className="line-mark line-mark-face">
              <Face name={device.user_name} avatar={device.user_avatar} className="account-face" />
            </span>
          ) : (
            <span className="line-mark" aria-hidden="true">
              <DeviceIcon size={18} />
            </span>
          )}
          <span className="line-words">
            <span className="line-name" title={device.name}>
              {deviceName(device.name, t, device.browser)}
              {device.is_this_one && <span className="device-here">{t("device.this_one")}</span>}
            </span>
            <span className="line-note">{aboutDevice(device, withAccount, now, language, t)}</span>
          </span>
          {!device.is_this_one && (
            <span className="line-end">
              {asking === device.id ? (
                <>
                  <button className="button button-small" onClick={() => setAsking(null)}>
                    {t("settings.cancel")}
                  </button>
                  <button
                    className="button button-small button-danger-full"
                    disabled={told.busy}
                    onClick={() => void told.tell(device)}
                  >
                    {t("device.sign_out")}
                  </button>
                </>
              ) : (
                <button className="button button-small" onClick={() => setAsking(device.id)}>
                  {t("device.sign_out")}
                </button>
              )}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

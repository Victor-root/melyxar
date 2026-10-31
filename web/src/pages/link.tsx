/*
 * Letting in a device waiting at the door: its code typed here, or brought
 * by the QR code it shows, then the device it names said before anything
 * is done, so a code mistyped never lets in somebody else's. An
 * administrator may open it on another account than their own.
 */

import { useEffect, useState } from "react";
import { useLocation, useSearchParams } from "react-router-dom";
import { api, ApiError } from "../api";
import type { ManagedAccount } from "../api";
import { useAccount } from "../account";
import { PageBackdrop } from "../components/backdrop";
import { Picker } from "../components/panel";
import { CameraIcon, DeviceIcon, TickIcon } from "../icons";
import { spacedCode } from "./door-link";
import { appScanner, codeRead, scanWithTheApp } from "../app-scanner";
import { useSettings } from "../settings";

/** How many digits a code holds, as the server draws it. */
const DIGITS = 6;

/** What a refusal says to whoever typed the code. */
function refusalOf(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.code === "not_found") return "link.refused.not_found";
    if (error.code === "too_many_attempts") return "link.refused.too_many";
    if (error.code === "forbidden") return "link.refused.forbidden";
  }
  return "error.unreachable";
}

export function LinkPage() {
  const { t } = useSettings();
  const { account } = useAccount();
  const [search] = useSearchParams();
  const [typed, setTyped] = useState(() => (search.get("code") ?? "").replace(/\D/g, "").slice(0, DIGITS));
  const [device, setDevice] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const administrator = account?.is_administrator === true;
  const [accounts, setAccounts] = useState<ManagedAccount[]>([]);
  const [on, setOn] = useState<string>(account?.id ?? "");

  // The device a whole code names, asked once the six digits are in.
  useEffect(() => {
    setDevice(null);
    if (typed.length !== DIGITS) {
      return;
    }
    setRefusal(null);
    api
      .deviceWaiting(typed)
      .then((named) => setDevice(named.device))
      .catch((error) => setRefusal(refusalOf(error)));
  }, [typed]);

  useEffect(() => {
    if (!administrator) {
      return;
    }
    const controller = new AbortController();
    api
      .accounts(controller.signal)
      .then(setAccounts)
      .catch(() => {
        // The administrator's own account stays the one offered.
      });
    return () => controller.abort();
  }, [administrator]);

  const scan = () =>
    void scanWithTheApp().then((read) => {
      if (read === null) return;
      const code = codeRead(read);
      setDone(null);
      if (code) {
        setTyped(code);
      } else {
        setRefusal("link.refused.not_a_code");
      }
    });

  // Come by the shortcut of the application's icon: the camera at once.
  const location = useLocation();
  useEffect(() => {
    if (appScanner && new URLSearchParams(location.search).has("scan")) {
      scan();
    }
    // Once per arrival, the shortcut taken again included.
  }, [location.key]);

  const letIn = () => {
    setSending(true);
    api
      .letIn(typed, on && on !== account?.id ? on : null)
      .then(() => {
        setDone(device);
        setTyped("");
      })
      .catch((error) => setRefusal(refusalOf(error)))
      .finally(() => setSending(false));
  };

  return (
    <>
      <PageBackdrop />
      <main className="page link-page">
        <div className="section-head">
          <h1>{t("link.title")}</h1>
        </div>
        <section className="link-card">
          <p className="link-lead">{t("link.lead")}</p>
          {done && (
            <p className="panel-notice panel-notice-ok">
              <TickIcon size={16} />
              {t("link.done", { device: done })}
            </p>
          )}
          <input
            className="field-line link-code"
            inputMode="numeric"
            autoComplete="one-time-code"
            aria-label={t("link.code")}
            placeholder="000 000"
            value={spacedCode(typed)}
            onChange={(event) => {
              setDone(null);
              setTyped(event.target.value.replace(/\D/g, "").slice(0, DIGITS));
            }}
          />
          {/* The phone's own camera, in the application: no reader of QR codes
              of its own to find, and no browser opened beside it. */}
          {appScanner && (
            <button
              type="button"
              className="button"
              onClick={scan}
            >
              <CameraIcon size={17} />
              {t("link.scan")}
            </button>
          )}
          {refusal && (
            <p className="panel-notice panel-notice-trouble" role="alert">
              {t(refusal)}
            </p>
          )}
          {device && (
            <div className="link-device">
              <span className="link-device-name">
                <DeviceIcon size={18} />
                {device}
              </span>
              {administrator && accounts.length > 1 && (
                <span className="link-on">
                  <span className="line-note">{t("link.on")}</span>
                  <Picker
                    label={t("link.on")}
                    value={on}
                    options={accounts.map((one) => [one.id, one.name] as const)}
                    onPick={setOn}
                  />
                </span>
              )}
              <button type="button" className="button button-accent" disabled={sending} onClick={letIn}>
                {t("link.go")}
              </button>
            </div>
          )}
        </section>
      </main>
    </>
  );
}

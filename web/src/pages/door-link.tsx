/*
 * Signing in at the door from a telephone: this browser shows a code, and
 * an account signed in elsewhere lets it in. The code is typed there, or
 * read from the QR code shown on asking. Nothing is typed here.
 *
 * Asked again on its own once a code is over, and the server asked every
 * couple of seconds whether somebody said yes: the answer that does is the
 * account this browser now holds a session of.
 */

import { useEffect, useState } from "react";
import { api, ApiError } from "../api";
import type { Account, LinkAsked } from "../api";
import { asClock } from "../clock";
import { QrSquares } from "../components/qr-squares";
import { deviceIdentity } from "../deviceIdentity";
import { deviceName, foundBrowser } from "../devices";
import { useSettings } from "../settings";

/** How often the server is asked whether somebody let this browser in. */
const ASKED_EVERY_MS = 2_000;

/** A code as it is read aloud and typed: two groups of three. */
export function spacedCode(code: string): string {
  return code.length === 6 ? `${code.slice(0, 3)} ${code.slice(3)}` : code;
}

export function LinkAtTheDoor({ cameIn, onBack }: { cameIn: (who: Account) => void; onBack: () => void }) {
  const { t } = useSettings();
  const [asked, setAsked] = useState<LinkAsked | null>(null);
  const [left, setLeft] = useState(0);
  const [failed, setFailed] = useState(false);
  const [qrShown, setQrShown] = useState(false);
  /* Moved on to ask for a code afresh. */
  const [round, setRound] = useState(0);

  useEffect(() => {
    let over = false;
    void (async () => {
      const name = deviceName(navigator.userAgent, t, await foundBrowser().catch(() => null));
      try {
        const answer = await api.askToBeLetIn(window.location.origin, name, deviceIdentity());
        if (!over) {
          setAsked(answer);
          setLeft(answer.lasts_seconds);
          setFailed(false);
        }
      } catch {
        if (!over) setFailed(true);
      }
    })();
    return () => {
      over = true;
    };
  }, [round, t]);

  useEffect(() => {
    if (!asked) {
      return;
    }
    const asking = window.setInterval(() => {
      api
        .letInYet(asked.secret)
        .then((answer) => {
          if ("seconds_left" in answer) {
            setLeft(answer.seconds_left);
          } else {
            cameIn(answer);
          }
        })
        .catch((error) => {
          // Over, or forgotten by a server that restarted: another code.
          if (error instanceof ApiError && error.status === 404) {
            setAsked(null);
            setRound((was) => was + 1);
          }
        });
    }, ASKED_EVERY_MS);
    const ticking = window.setInterval(() => setLeft((was) => Math.max(0, was - 1)), 1_000);
    return () => {
      window.clearInterval(asking);
      window.clearInterval(ticking);
    };
  }, [asked, cameIn]);

  return (
    <div className="door-link">
      <p className="door-link-how">{t("door.link.how")}</p>
      {failed ? (
        <p className="door-refusal" role="alert">
          {t("error.unreachable")}
        </p>
      ) : (
        <>
          <span className="door-link-code" aria-live="polite">
            {asked ? spacedCode(asked.code) : "··· ···"}
          </span>
          {asked && <span className="door-link-left">{t("door.link.left", { time: asClock(left) })}</span>}
          {asked && qrShown && (
            <div className="door-link-qr">
              <QrSquares squares={asked.squares} label={t("door.link.qr")} />
              <span className="door-link-left">{t("door.link.qr_how")}</span>
            </div>
          )}
        </>
      )}
      <div className="door-link-actions">
        {asked && (
          <button type="button" className="door-by-hand" onClick={() => setQrShown((was) => !was)}>
            {t(qrShown ? "door.link.qr_hide" : "door.link.qr_show")}
          </button>
        )}
        {failed && (
          <button type="button" className="door-by-hand" onClick={() => setRound((was) => was + 1)}>
            {t("door.link.again")}
          </button>
        )}
        <button type="button" className="door-by-hand" onClick={onBack}>
          {t("door.back")}
        </button>
      </div>
    </div>
  );
}

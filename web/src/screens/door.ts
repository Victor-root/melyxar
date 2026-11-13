/*
 * What the door is driven by, and nothing about how it looks.
 *
 * Two screens that are one: a server nobody has set up asks for a first
 * account, and a server that has been set up asks for a password. They send
 * different things to different addresses and everything around them is the
 * same, which is why they are one screen with one thing said differently.
 *
 * The wording of a refusal is chosen here rather than in the drawing, because
 * a refusal at a door is the one thing on this screen that has to be exactly
 * right: somebody who has mistyped their password and somebody whose account
 * is being held back have to be told different things.
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../api";
import type { Account, Branding, NameAtTheDoor } from "../api";
import { figuresOf, heldBackFor } from "../asking";
import { deviceIdentity } from "../deviceIdentity";
import { refusalKey } from "../i18n";
import { DeviceSaidNo, passkeysHere, signInWithAPasskey } from "../passkeys";

/** A refusal, worded and with whatever its wording needs. */
export interface Refusal {
  key: string;
  values: Record<string, string | number>;
}

export interface DoorScreen {
  /** A server nobody has set up asks for a first account. */
  brandNew: boolean;
  /** The names this server offers, which may be none of them.
   *
   *  Empty for three different reasons that the screen has no business
   *  telling apart: a server told not to offer them, everybody having asked
   *  to be left off, and a server with no accounts at all. */
  names: NameAtTheDoor[];
  /** What this server calls itself. */
  serverName: string;
  /** Sends what was typed. Never throws: what came of it is below.
   *
   *  Remembering says whether the browser is to hold on to the session once
   *  it is closed, which is the one thing about it somebody chooses. */
  knock: (name: string, password: string, remember: boolean) => Promise<void>;
  /** Whether a passkey can open this door from this page. */
  byPasskey: boolean;
  /** Signs in with whichever passkey the device offers. Never throws. */
  knockWithAKey: (remember: boolean) => Promise<void>;
  /** Whether somebody may make their own account here. */
  signUpsOpen: boolean;
  /** Makes an account at the door, and signs it in when it does not have to
   *  wait for an administrator. Never throws, like knocking. */
  signUp: (name: string, password: string, remember: boolean) => Promise<void>;
  /** An account was just made here and waits for an administrator. */
  awaitingApproval: boolean;
  /** Whether the names have been asked for and answered, so the card is only
   *  drawn once it can show the view it opens on. True at once on a new server,
   *  which asks for nothing. */
  asked: boolean;
  /** Whether the server is being asked right now. */
  asking: boolean;
  /** Why it said no, or nothing. */
  refused: Refusal | null;
  /** Put the refusal aside, and an account said to wait, which is what typing
   *  again or turning the screen does. */
  forget: () => void;
}

export function useDoorScreen(
  branding: Branding,
  cameIn: (who: Account) => void,
  language: string,
): DoorScreen {
  const [asking, setAsking] = useState(false);
  const [refused, setRefused] = useState<Refusal | null>(null);
  const [names, setNames] = useState<NameAtTheDoor[]>([]);
  const [answered, setAnswered] = useState(false);
  const [awaitingApproval, setAwaitingApproval] = useState(false);
  const brandNew = !branding.setup_complete;

  // Asked for once, and never on a server that has no accounts yet: the only
  // answer there is an empty list, and the screen it draws asks for a first
  // account rather than offering one.
  useEffect(() => {
    if (brandNew) {
      return;
    }
    const controller = new AbortController();
    api
      .namesAtTheDoor(controller.signal)
      .then((offered) => setNames(offered.names))
      .catch(() => {
        // A server that will not say is a server that offers nothing, which
        // is the same screen as a server with nothing to offer. The field is
        // there either way.
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setAnswered(true);
        }
      });
    return () => controller.abort();
  }, [brandNew]);

  const knock = useCallback(
    async (name: string, password: string, remember: boolean) => {
      setAsking(true);
      setRefused(null);
      try {
        const who = brandNew
          ? await api.setUp(name, password, remember, deviceIdentity(), language)
          : await api.signIn(name, password, remember, deviceIdentity());
        cameIn(who);
      } catch (error) {
        setRefused(whatTheServerSaid(error));
        setAsking(false);
      }
    },
    [brandNew, cameIn, language],
  );

  const knockWithAKey = useCallback(
    async (remember: boolean) => {
      setAsking(true);
      setRefused(null);
      try {
        cameIn(await signInWithAPasskey(remember));
      } catch (error) {
        setRefused(whatTheDeviceSaid(error));
        setAsking(false);
      }
    },
    [cameIn],
  );

  const signUp = useCallback(
    async (name: string, password: string, remember: boolean) => {
      setAsking(true);
      setRefused(null);
      try {
        const made = await api.signUp(name, password);
        if (made.awaiting_approval) {
          setAwaitingApproval(true);
          setAsking(false);
          return;
        }
        cameIn(await api.signIn(name, password, remember, deviceIdentity()));
      } catch (error) {
        setRefused(whatTheServerSaid(error));
        setAsking(false);
      }
    },
    [cameIn],
  );

  return {
    brandNew,
    names,
    signUpsOpen: !brandNew && branding.sign_ups_open,
    signUp,
    awaitingApproval,
    asked: brandNew || answered,
    serverName: branding.server_name,
    knock,
    byPasskey: !brandNew && passkeysHere(),
    knockWithAKey,
    asking,
    refused,
    forget: useCallback(() => {
      setRefused(null);
      setAwaitingApproval(false);
    }, []),
  };
}

/**
 * The refusal, as this screen says it.
 *
 * Three of them are worth telling apart, and the rest are the server's usual
 * answers. A pair that is not a pair is the ordinary mistake. Being held back
 * has to say how long, or somebody types their own password again and again
 * and believes they have forgotten it. And a form refused for something typed
 * names the thing to put right.
 */
function whatTheServerSaid(error: unknown, about = "account"): Refusal {
  if (!(error instanceof ApiError)) {
    return { key: "door.refused.generic", values: {} };
  }

  const seconds = heldBackFor(error);
  if (seconds !== null) {
    return { key: "door.refused.held_back", values: { seconds } };
  }
  if (error.code === "unauthenticated") {
    return { key: about === "passkey" ? "door.refused.wrong_passkey" : "door.refused.wrong", values: {} };
  }
  if (error.reason) {
    const subject = error.reason === "awaiting_approval" ? "account" : about;
    return { key: `refused.${subject}.${error.reason}`, values: figuresOf(error) };
  }
  return { key: refusalKey(error.code), values: {} };
}

/**
 * The refusal of a passkey brought to the door, or nothing when somebody put
 * the device's question away: that is changing one's mind, not a fault.
 */
function whatTheDeviceSaid(error: unknown): Refusal | null {
  if (error instanceof DeviceSaidNo) {
    return error.why === "cancelled" ? null : { key: "door.refused.passkey_failed", values: {} };
  }
  return whatTheServerSaid(error, "passkey");
}

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

import { useCallback, useState } from "react";
import { api, ApiError } from "../api";
import type { Account, Branding } from "../api";
import { refusalKey } from "../i18n";

/** A refusal, worded and with whatever its wording needs. */
export interface Refusal {
  key: string;
  values: Record<string, string | number>;
}

export interface DoorScreen {
  /** A server nobody has set up asks for a first account. */
  brandNew: boolean;
  /** What this server calls itself. */
  serverName: string;
  /** Sends what was typed. Never throws: what came of it is below. */
  knock: (name: string, password: string) => Promise<void>;
  /** Whether the server is being asked right now. */
  asking: boolean;
  /** Why it said no, or nothing. */
  refused: Refusal | null;
  /** Put the refusal aside, which is what typing again does. */
  forget: () => void;
}

export function useDoorScreen(branding: Branding, cameIn: (who: Account) => void): DoorScreen {
  const [asking, setAsking] = useState(false);
  const [refused, setRefused] = useState<Refusal | null>(null);
  const brandNew = !branding.setup_complete;

  const knock = useCallback(
    async (name: string, password: string) => {
      setAsking(true);
      setRefused(null);
      try {
        const who = brandNew
          ? await api.setUp(name, password)
          : await api.signIn(name, password);
        cameIn(who);
      } catch (error) {
        setRefused(whatTheServerSaid(error));
        setAsking(false);
      }
    },
    [brandNew, cameIn],
  );

  return {
    brandNew,
    serverName: branding.server_name,
    knock,
    asking,
    refused,
    forget: useCallback(() => setRefused(null), []),
  };
}

/**
 * The refusal, as this screen says it.
 *
 * Three of them are worth telling apart, and the rest are the server's usual
 * answers. A pair that is not a pair is the ordinary mistake. An account being
 * held back has to say how long, or somebody types their own password again
 * and again and believes they have forgotten it. And a form refused for
 * something typed names the thing to put right.
 */
function whatTheServerSaid(error: unknown): Refusal {
  if (!(error instanceof ApiError)) {
    return { key: "door.refused.generic", values: {} };
  }

  if (error.code === "too_many_attempts") {
    return {
      key: "door.refused.held_back",
      values: { seconds: Number(error.details?.seconds ?? 60) },
    };
  }
  if (error.code === "unauthenticated") {
    return { key: "door.refused.wrong", values: {} };
  }
  if (error.reason) {
    return {
      key: `refused.account.${error.reason}`,
      values: { shortest: Number(error.details?.shortest ?? 0) },
    };
  }
  return { key: refusalKey(error.code), values: {} };
}

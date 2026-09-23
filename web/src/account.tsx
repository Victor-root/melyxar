/*
 * Who is signed in, held once for the whole interface.
 *
 * Everything this interface draws belongs to somebody: what was left halfway,
 * what is marked as liked, what the theme is. So the account comes before the
 * first screen rather than beside it, and a screen never has to wonder whether
 * there is one.
 *
 * A session can end between two clicks: signed out on another machine, swept
 * by a server that restarted, or simply run out. The door tells this on its
 * own from wherever it happened, so the interface goes back to the door
 * instead of carrying on drawing a library it can no longer read.
 */

import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { api, whenTheDoorCloses } from "./api";
import type { Account, Branding } from "./api";
import { foundBrowser } from "./devices";

export interface Who {
  /** The account, or nothing when nobody is signed in. */
  account: Account | null;
  /** Whether the server has not answered yet, so nothing is drawn too early:
      a door that flashes up before the answer arrives is worse than a moment
      of nothing. */
  stillAsking: boolean;
  /** What the door draws itself with. Absent until it is needed. */
  branding: Branding | null;
  /** Somebody came through the door. */
  cameIn: (account: Account) => void;
  /** Somebody asked to leave. Never fails: what matters is that this browser
      stops holding a session, and the server sweeps whatever it kept. */
  leave: () => Promise<void>;
}

const WhoContext = createContext<Who | null>(null);

export function useWhoIsThere(): Who {
  const [account, setAccount] = useState<Account | null>(null);
  const [stillAsking, setStillAsking] = useState(true);
  const [branding, setBranding] = useState<Branding | null>(null);

  // Asked once, at the start. A refusal here is the ordinary answer for
  // somebody who has not signed in yet, not a fault worth showing.
  useEffect(() => {
    const controller = new AbortController();
    api
      .me(controller.signal)
      .then((who) => {
        setAccount(who);
        setStillAsking(false);
      })
      .catch(() => setStillAsking(false));
    return () => controller.abort();
  }, []);

  // Which browser this really is, said once somebody is signed in on it: the
  // line a browser sends about itself cannot tell every one apart.
  const signedIn = account?.id ?? null;
  useEffect(() => {
    if (signedIn === null) {
      return;
    }
    foundBrowser()
      .then((browser) => api.nameTheBrowser(browser))
      .catch(() => {
        // The name of a browser is shown and never decided from: missing,
        // the line the browser sends stands in for it.
      });
  }, [signedIn]);

  // What the door needs, asked for only once there is a door to draw.
  useEffect(() => {
    if (account !== null || stillAsking || branding !== null) {
      return;
    }
    const controller = new AbortController();
    api
      .branding(controller.signal)
      .then(setBranding)
      .catch(() => {
        // A server that will not even say its own name is a server that is not
        // answering. The door says so itself rather than waiting for ever.
        setBranding({
          server_name: "",
          logo_path: null,
          login_background_path: null,
          login_background_style: "abstract",
          setup_complete: true,
        });
      });
    return () => controller.abort();
  }, [account, stillAsking, branding]);

  // From wherever it happens: one refusal anywhere sends the whole interface
  // back to the door, and the door is drawn again from a fresh answer.
  useEffect(() => {
    whenTheDoorCloses(() => {
      setAccount(null);
      setBranding(null);
    });
  }, []);

  const cameIn = useCallback((who: Account) => {
    setAccount(who);
    setStillAsking(false);
  }, []);

  const leave = useCallback(async () => {
    try {
      await api.signOut();
    } catch {
      // The cookie is the server's to take back, and it does so with the
      // answer. One that never arrives leaves a session the server sweeps on
      // its own, and there is nothing here to say about it.
    }
    setAccount(null);
    setBranding(null);
  }, []);

  return { account, stillAsking, branding, cameIn, leave };
}

export function WhoProvider({ who, children }: { who: Who; children: React.ReactNode }) {
  return <WhoContext.Provider value={who}>{children}</WhoContext.Provider>;
}

/** The account every screen is drawn for. */
export function useAccount(): Who {
  const who = useContext(WhoContext);
  if (!who) {
    throw new Error("the account was asked for outside its provider");
  }
  return who;
}

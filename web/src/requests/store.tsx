/*
 * Whether this account may ask for titles, and the requests it made, held
 * once for the whole interface.
 *
 * The menu that leads to the requests, the page that looks a title up and
 * the page of this account's requests all show the same thing, so it is
 * held here and read again every time the live line says the requests
 * moved. Asking writes the request here once the server has it; taking one
 * back takes it out at once, and puts it back if the server refuses.
 */

import { createContext, useCallback, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useRequestsNews } from "../live";
import { requestsApi } from "./api";
import type { Asking, RequestAccess, TitleRequest } from "./api";

interface Requests {
  /** Nothing until read. */
  access: RequestAccess | null;
  /** This account's requests, newest first; nothing until read. */
  mine: TitleRequest[] | null;
  ask: (asking: Asking) => Promise<TitleRequest>;
  /** Throws what the server refused, once the request is back in place. */
  withdraw: (id: string) => Promise<void>;
}

const RequestsContext = createContext<Requests>({
  access: null,
  mine: null,
  ask: () => Promise.reject(new Error("no requests here")),
  withdraw: () => Promise.reject(new Error("no requests here")),
});

export function useRequests(): Requests {
  return useContext(RequestsContext);
}

export function RequestsProvider({ children }: { children: ReactNode }) {
  const [access, setAccess] = useState<RequestAccess | null>(null);
  const [mine, setMine] = useState<TitleRequest[] | null>(null);
  const [asked, setAsked] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    requestsApi
      .access(controller.signal)
      .then(async (read) => {
        setAccess(read);
        setMine(read.may_ask ? await requestsApi.mine(controller.signal) : null);
      })
      .catch(() => {
        /* Kept as it was: the next word of the line asks again. */
      });
    return () => controller.abort();
  }, [asked]);

  useRequestsNews(() => setAsked((count) => count + 1));

  const ask = useCallback(async (asking: Asking) => {
    const made = await requestsApi.ask(asking);
    setMine((before) => [made, ...(before ?? []).filter((one) => one.id !== made.id)]);
    return made;
  }, []);

  const withdraw = useCallback(
    async (id: string) => {
      const before = mine;
      setMine((held) => held?.filter((one) => one.id !== id) ?? null);
      try {
        await requestsApi.withdraw(id);
      } catch (error) {
        setMine(before);
        throw error;
      }
    },
    [mine],
  );

  return (
    <RequestsContext.Provider value={{ access, mine, ask, withdraw }}>{children}</RequestsContext.Provider>
  );
}

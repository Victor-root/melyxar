/*
 * The Android application's choice of server, when this page runs inside it:
 * the page asks, and the application opens its screen of servers over the
 * page, the servers known and those found at home. Absent in a browser,
 * where another server is another address, and in an application older than
 * this page.
 */

interface AppServers {
  choose(): void;
}

declare global {
  interface Window {
    MelyxarServers?: AppServers;
  }
}

/** The application's choice of server, or nothing where there is none. */
export const appServers: AppServers | null =
  typeof window !== "undefined" ? (window.MelyxarServers ?? null) : null;

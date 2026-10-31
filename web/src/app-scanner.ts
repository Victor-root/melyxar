/*
 * The Android application's camera reading a QR code, when this page runs
 * inside it. The page asks, the application opens its camera over the page,
 * and what the code held comes back once, or nothing when it was closed.
 * Absent in a browser, and in an application older than this page.
 */

interface AppScanner {
  scan(): void;
}

declare global {
  interface Window {
    MelyxarScanner?: AppScanner;
    /** Told by the application what the camera read. */
    melyxarScanned?: (read: string | null) => void;
  }
}

/** The application's camera, or nothing where there is none. */
export const appScanner: AppScanner | null =
  typeof window !== "undefined" ? (window.MelyxarScanner ?? null) : null;

/** Reads a QR code with the application's camera: what it held, or nothing. */
export function scanWithTheApp(): Promise<string | null> {
  return new Promise((done) => {
    window.melyxarScanned = (read) => {
      window.melyxarScanned = undefined;
      done(read);
    };
    appScanner?.scan();
  });
}

/** The code a QR code shown at a door holds: the one its address carries,
 *  or six digits read as they are. Nothing for anything else. */
export function codeRead(read: string): string | null {
  const digits = /^\d{6}$/;
  if (digits.test(read.trim())) {
    return read.trim();
  }
  try {
    const code = new URL(read).searchParams.get("code");
    return code && digits.test(code) ? code : null;
  } catch {
    return null;
  }
}

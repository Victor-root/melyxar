/*
 * The Android application's own screen for a camera watched live, when this
 * page runs inside it.
 *
 * The page hands it the camera's address, account written in, and the
 * application opens the camera itself, on a screen of its own over the page
 * that the back key closes. Absent in a browser, and in an application older
 * than this page.
 */

export interface AppCamera {
  watch(address: string, name: string): void;
}

declare global {
  interface Window {
    MelyxarCamera?: AppCamera;
  }
}

/** The application's camera screen, or nothing where there is none. */
export const appCamera: AppCamera | null =
  typeof window !== "undefined" ? (window.MelyxarCamera ?? null) : null;

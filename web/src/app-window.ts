/*
 * The window of Melyxar's Android application, when this page runs inside it.
 *
 * The application draws the page under the phone's bars, its camera and its
 * keyboard, and says itself how much room each takes, rather than leaving it
 * to the web view, which only learnt to say so in its recent versions. The
 * bars' icons it draws dark or light itself, from what it sees under them.
 *
 * Absent in a browser, where nothing here is used.
 */

import type { KeyboardSlide } from "./keyboard-slide";

/** The room taken at each edge, in the page's pixels. */
export interface Edges {
  top: number;
  bottom: number;
  left: number;
  right: number;
  /** Whether the keyboard is up, the page having given it its room. */
  keyboard?: boolean;
}

interface AppWindow {
  /** The JSON of the {@link Edges} as they are now. */
  edges(): string;
  /** Where the foot of the field just reached stands, for the application
   *  to raise the page with the keyboard that is coming. An application
   *  older than this page does not have it. */
  fieldAt?(foot: number): void;
}

declare global {
  interface Window {
    MelyxarWindow?: AppWindow;
    /** Called by the application whenever the edges change. */
    melyxarEdgesChanged?: () => void;
    /** Called by the application to scroll by what it raised the page by. */
    melyxarRaise?: (pixels: number) => void;
    /** Called by the application as the keyboard starts moving. */
    melyxarKeyboardSlide?: (slide: KeyboardSlide) => void;
  }
}

/** The application's window, or nothing in a browser. */
export const appWindow: AppWindow | null =
  typeof window !== "undefined" ? (window.MelyxarWindow ?? null) : null;

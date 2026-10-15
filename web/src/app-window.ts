/*
 * The window of Melyxar's Android application, when this page runs inside it.
 *
 * The application draws the page under the phone's bars, its camera and its
 * keyboard, and says itself how much room each takes, rather than leaving it
 * to the web view, which only learnt to say so in its recent versions. In
 * return the page tells it the colour of its window, from which the bars'
 * icons are drawn dark or light.
 *
 * Absent in a browser, where nothing here is used.
 */

/** The room taken at each edge, in the page's pixels. */
export interface Edges {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

interface AppWindow {
  /** The JSON of the {@link Edges} as they are now. */
  edges(): string;
  /** The colour the page gives its window, as `#rrggbb`. */
  colour(colour: string): void;
}

declare global {
  interface Window {
    MelyxarWindow?: AppWindow;
    /** Called by the application whenever the edges change. */
    melyxarEdgesChanged?: () => void;
  }
}

/** The application's window, or nothing in a browser. */
export const appWindow: AppWindow | null =
  typeof window !== "undefined" ? (window.MelyxarWindow ?? null) : null;

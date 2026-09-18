/*
 * Handing a block of text the server rendered to whoever asked for it.
 *
 * Two screens do this: the report of the whole installation, and the journal
 * as it is on screen. Both ask the server for the text rather than rendering
 * it a second time here, so that what is pasted is what the command line would
 * print and not a second version nobody checked.
 *
 * A browser only grants the clipboard on a secure page, and a server reached
 * at an address on the local network is not one. So there is a way down, and
 * it always works: the text is handed back to be shown and selected, which
 * leaves one key press to do rather than a terminal to open. Being handed a
 * box to select is not what anybody means by copy, which is why it is the last
 * resort and never the first answer.
 */

import { putOnTheClipboard } from "./clipboard";

/** What became of handing the text over. */
export type Handed =
  | { how: "clipboard" }
  | { how: "by hand"; text: string }
  | { how: "not at all" };

/** Asks the server for the text, and hands it over the best way it can. */
export async function handOver(askFor: () => Promise<string>): Promise<Handed> {
  let text: string;
  try {
    text = await askFor();
  } catch {
    return { how: "not at all" };
  }
  // Every way a browser offers, the old one included, which is the one that
  // works on a plain address.
  return (await putOnTheClipboard(text)) ? { how: "clipboard" } : { how: "by hand", text };
}

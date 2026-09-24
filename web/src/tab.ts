/*
 * What the browser's tab is called: what is playing while something is, and
 * the server's own name the rest of the time, as its administrator named it.
 */

/** Until the server has said its name. */
const UNNAMED = document.title;

let server: string | null = null;
let playing: string | null = null;

function name() {
  document.title = playing ?? (server || UNNAMED);
}

/** The server's name, once it is known. */
export function nameTheTab(serverName: string | null): void {
  server = serverName;
  name();
}

/** What is playing, or nothing once it stops. */
export function showPlaying(what: string | null): void {
  playing = what;
  name();
}

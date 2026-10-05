/*
 * What the browser's tab is called: the film playing while there is one,
 * else that music is playing, and the server's own name the rest of the
 * time, as its administrator named it, followed by the page being read.
 */

/** What the page was called before anything named it, until the server has
    said its name. Read at the first naming, which nothing has come before. */
let unnamed: string | null = null;

let server: string | null = null;
let playing: string | null = null;
let listening: string | null = null;
let page: string | null = null;

/** Between the server's name and the page. */
const SEPARATOR = " · ";

function name() {
  unnamed ??= document.title;
  const home = server || unnamed;
  document.title = playing ?? listening ?? (page ? `${home}${SEPARATOR}${page}` : home);
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

/** Music heard from this tab, or nothing once it is not. */
export function showListening(what: string | null): void {
  listening = what;
  name();
}

/** The page being read, or nothing once it is left. */
export function showPage(what: string | null): void {
  page = what;
  name();
}

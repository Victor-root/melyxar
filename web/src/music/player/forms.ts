/*
 * The forms of sound this browser plays, asked of the browser itself, and
 * sent with every song so the server knows what can go out as it is.
 */

/** What the browser is asked, form by form. */
const ASKED: [string, string][] = [
  ["mp3", "audio/mpeg"],
  ["aac", 'audio/mp4; codecs="mp4a.40.2"'],
  ["flac", "audio/flac"],
  ["opus", 'audio/ogg; codecs="opus"'],
  ["vorbis", 'audio/ogg; codecs="vorbis"'],
  ["alac", 'audio/mp4; codecs="alac"'],
  ["wav", 'audio/wav; codecs="1"'],
];

let known: string | null = null;

/** The forms this browser plays, as the server reads them: `mp3,aac,flac`. */
export function formsPlayedHere(): string {
  if (known === null) {
    const probe = document.createElement("audio");
    known = ASKED.filter(([, type]) => probe.canPlayType(type) !== "")
      .map(([form]) => form)
      .join(",");
  }
  return known;
}

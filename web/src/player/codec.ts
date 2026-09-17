/*
 * Which codec a transcode is asked to come out in.
 *
 * For now the only reason to ask is chasing a stutter: forcing one exact
 * codec and watching whether it goes away, rather than leaving the choice to
 * the usual negotiation and never being sure which codec was actually
 * produced. "auto" is the choice that asks for nothing, which is what a
 * viewer who never opens this panel gets.
 *
 * The choice travels to the server as a field of its own beside the profile,
 * since it is not something measured about the client: it is what somebody
 * asked for.
 */

/** One codec a transcode may be asked for, or the absence of a choice. */
export interface Codec {
  /** What it is called in an address and in storage, never on screen.
   *  "auto" means nothing is asked for. */
  key: string;
}

/** The choices, best first, "auto" ahead of all of them. */
export const CODECS: Codec[] = [
  { key: "auto" },
  { key: "av1" },
  { key: "hevc" },
  { key: "h264" },
];

/** The choice a player starts on: nothing asked for. */
export const AUTOMATIC = CODECS[0];

/** Names that are the same in every language, unlike "auto" itself. */
const NAMED: Record<string, string> = {
  h264: "H.264",
  hevc: "HEVC",
  av1: "AV1",
};

/** What a choice is called on screen. */
export function codecName(codec: Codec, auto: string): string {
  return NAMED[codec.key] ?? auto;
}

/** The choice by that name, or nothing asked for when the name means nothing. */
export function codecCalled(key: string | null): Codec {
  return CODECS.find((codec) => codec.key === key) ?? AUTOMATIC;
}

/** What this choice is sent to the server as. Null for "auto", which asks
 *  for nothing rather than for a codec named "auto". */
export function requestedCodec(codec: Codec): string | null {
  return codec === AUTOMATIC ? null : codec.key;
}

/** Where the choice is kept, so a viewer sets it once rather than per film. */
const REMEMBERED = "melyxar.video_codec";

export function storedCodec(): Codec {
  try {
    return codecCalled(window.localStorage.getItem(REMEMBERED));
  } catch {
    // A browser refusing storage still transcodes perfectly well with
    // nothing asked for, and nothing is said about it.
    return AUTOMATIC;
  }
}

export function rememberCodec(codec: Codec) {
  try {
    window.localStorage.setItem(REMEMBERED, codec.key);
  } catch {
    // The choice still holds for this sitting, which is what is being watched.
  }
}

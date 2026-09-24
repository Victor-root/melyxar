/*
 * What a playback is, in words: what the file holds, what is being done to
 * it, and how hard the machine works at it.
 *
 * Said the same way to somebody watching the film, in the panel of its
 * details, and to an administrator following it from elsewhere: the same
 * facts worded twice drift apart, and then one screen says the other is
 * wrong.
 */

import type { FilmHolds, PictureRebuild, PlaybackTrack, Producing } from "../api";
import { languageName } from "../languages";

type Wording = (key: string, values?: Record<string, string | number>) => string;

/** Below this the machine is producing the film more slowly than it plays. */
export const KEEPING_UP = 1;

/** A rate in bits per second, as somebody reads one. */
export function asRate(bits: number | null): string | null {
  if (bits === null || bits <= 0) {
    return null;
  }
  return bits >= 1_000_000
    ? `${(bits / 1_000_000).toFixed(bits >= 10_000_000 ? 0 : 1)} Mb/s`
    : `${Math.round(bits / 1_000)} kb/s`;
}

/** How the tool's work reads, when it is doing any.
 *
 * Pictures a second only when the tool is making pictures: a film whose
 * picture is carried over untouched is repackaged rather than drawn, and the
 * tool answers nothing at all when asked how many a second it is drawing. The
 * speed stands on its own there, and it is the number that matters anyway.
 */
export function asWork(working: Producing, t: Wording): string {
  const speed = working.speed >= 10 ? Math.round(working.speed) : working.speed.toFixed(2);
  return working.pictures_a_second > 0
    ? t("facts.working_at", { pictures: working.pictures_a_second.toFixed(1), speed })
    : t("facts.working_speed", { speed });
}

/** A size in bytes, as somebody reads one. */
export function asSize(bytes: number): string {
  const giga = bytes / 1_000_000_000;
  return giga >= 1 ? `${giga.toFixed(1)} GB` : `${Math.round(bytes / 1_000_000)} MB`;
}

/** Every reason behind the answer, in one line. */
export function reasonsSaid(reasons: { code: string }[], t: Wording): string | null {
  return reasons.length > 0 ? reasons.map((reason) => t(`reason.${reason.code}`)).join(" · ") : null;
}

/** The picture the file holds. */
export function pictureHeld(picture: NonNullable<FilmHolds["picture"]>, t: Wording): string {
  return [
    picture.codec.toUpperCase(),
    picture.profile,
    `${picture.width}x${picture.height}`,
    picture.bit_depth ? `${picture.bit_depth} bit` : null,
    picture.hdr ? t(`facts.hdr.${picture.hdr}`) : null,
    picture.frame_rate ? `${picture.frame_rate.toFixed(3)} fps` : null,
    asRate(picture.bitrate),
  ]
    .filter(Boolean)
    .join(" · ");
}

/** What becomes of the picture on its way. */
export function pictureDone(rebuild: PictureRebuild | null, t: Wording): string {
  return rebuild
    ? [
        t(`player.rebuilt_by.${rebuild.by}`),
        rebuild.codec.toUpperCase(),
        rebuild.height !== null ? `${rebuild.height}p` : null,
        asRate(rebuild.bitrate),
      ]
        .filter(Boolean)
        .join(" · ")
    : t("facts.carried_over");
}

/** The soundtrack being played, as the file holds it. */
export function soundHeld(sound: NonNullable<FilmHolds["sound"]>, t: Wording): string {
  return [
    sound.codec.toUpperCase(),
    sound.channel_layout ?? t("facts.channels", { count: sound.channels }),
    sound.sample_rate ? `${(sound.sample_rate / 1000).toFixed(1)} kHz` : null,
    asRate(sound.bitrate),
  ]
    .filter(Boolean)
    .join(" · ");
}

/** What becomes of the sound on its way, from the way the film is played. */
export function soundDone(method: string, t: Wording): string {
  return method === "full_transcode" || method === "transcode_audio"
    ? t("facts.rebuilt_sound")
    : t("facts.carried_over");
}

/**
 * What to call a track in a list, in the player and on the page that
 * chooses one before it opens.
 *
 * The language first, since that is what a viewer is looking for, then what
 * the file itself calls it when it says something, and the number of channels
 * when there is more than a pair.
 */
export function trackName(track: PlaybackTrack, t: Wording, speaking: string): string {
  const parts = [
    track.language ? languageName(track.language, speaking) : t("player.unknown_language"),
  ];
  if (track.title) {
    parts.push(track.title);
  }
  if (track.channels && track.channels > 2) {
    parts.push(`${track.channels}`);
  }
  if (track.burns_in) {
    parts.push(t("player.burns_in_short"));
  }
  return parts.join(" · ");
}

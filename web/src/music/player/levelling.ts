/*
 * How much a song is raised or lowered for every song to sound as loud as
 * any other, from how loud it was measured to be.
 */

import type { Song, VolumeMode } from "../api";

/** The level every song is brought to, as the services people listen on
 *  bring theirs. */
export const TARGET_LUFS = -14;

/** Room left under full scale once a song is raised, so its loudest moment
 *  never clips. */
const HEADROOM_DB = 1;

/** Quieter than this is silence, or a measure gone wrong: left alone. */
const TOO_QUIET_LUFS = -60;

/** The gain, as a factor on the sound, for this song in this mode. A song
 *  never measured is left as it is, and a song is raised only as far as its
 *  loudest moment allows. */
export function levelOf(song: Song, mode: VolumeMode): number {
  if (mode === "off") {
    return 1;
  }
  const loudness = mode === "album" ? (song.album_lufs ?? song.lufs) : song.lufs;
  if (loudness === null || loudness < TOO_QUIET_LUFS) {
    return 1;
  }
  let gainDb = TARGET_LUFS - loudness;
  if (gainDb > 0) {
    gainDb = song.peak_dbfs === null ? 0 : Math.min(gainDb, Math.max(0, -HEADROOM_DB - song.peak_dbfs));
  }
  return 10 ** (gainDb / 20);
}

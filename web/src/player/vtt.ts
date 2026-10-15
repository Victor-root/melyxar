/*
 * Reading a WebVTT file into its cues.
 *
 * A browser does this itself for a track element hung on its own video
 * element. The application's picture is not one, so its words are read here:
 * the server only ever hands WebVTT to a player, written by its own converter,
 * so what is read is the times and the text of each cue, and the settings
 * after the times are left aside, as the player draws its words itself.
 */

export interface Cue {
  start: number;
  end: number;
  text: string;
}

/** A WebVTT time, `hh:mm:ss.ttt` or `mm:ss.ttt`, in seconds. */
export function secondsOf(time: string): number | null {
  const match = /^(?:(\d+):)?(\d{2}):(\d{2})[.,](\d{3})$/.exec(time.trim());
  if (!match) {
    return null;
  }
  const [, hours, minutes, seconds, thousandths] = match;
  return Number(hours ?? 0) * 3600 + Number(minutes) * 60 + Number(seconds) + Number(thousandths) / 1000;
}

/** Every cue of a WebVTT file, in the order the file gives them. */
export function cuesOf(file: string): Cue[] {
  const cues: Cue[] = [];
  const blocks = file.replace(/^﻿/, "").replace(/\r\n?/g, "\n").trim().split(/\n{2,}/);
  for (const block of blocks) {
    const lines = block.split("\n");
    const timing = lines.findIndex((line) => line.includes("-->"));
    if (timing < 0) {
      continue;
    }
    const [from, rest] = lines[timing].split("-->");
    const start = secondsOf(from);
    const end = secondsOf(rest.trim().split(/\s+/)[0] ?? "");
    if (start === null || end === null) {
      continue;
    }
    cues.push({ start, end, text: lines.slice(timing + 1).join("\n") });
  }
  return cues;
}

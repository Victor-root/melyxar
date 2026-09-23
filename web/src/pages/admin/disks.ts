/*
 * How a disk is named and judged on the summary.
 */

import type { State } from "../../components/panel";

/** Past this share a disk asks for a look, and the server says so too. */
const NEARLY_FULL = 0.9;

/** Past this share a disk is worth keeping an eye on. */
const FILLING_UP = 0.8;

/** How full a disk is, from nought to one. */
export function usedShare(disk: { total_bytes: number; available_bytes: number }): number {
  return disk.total_bytes > 0 ? (disk.total_bytes - disk.available_bytes) / disk.total_bytes : 0;
}

/** The state a disk this full is in, or nothing while it has room. */
export function fullness(used: number): State | null {
  return used >= NEARLY_FULL ? "trouble" : used >= FILLING_UP ? "attention" : null;
}

/**
 * The name of a disk: the last folder of where it is mounted, which is the
 * name its owner gave it. Nothing for the disk the system itself lives on,
 * which has no name of its own.
 */
export function diskName(mount: string): string | null {
  return mount.split("/").filter(Boolean).pop() ?? null;
}

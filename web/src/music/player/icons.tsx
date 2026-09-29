/*
 * The pictures on the buttons of the player of music, drawn on the grid and
 * with the strokes of every other icon of the interface.
 */

import { Icon } from "../../icons";
import type { IconProps } from "../../icons";

export function PauseIcon(props: IconProps) {
  return (
    <Icon {...props} strokeWidth={2.6}>
      <path d="M8.6 6.4v11.2M15.4 6.4v11.2" />
    </Icon>
  );
}

/** A square: stops for good, which a pause does not. */
export function StopIcon(props: IconProps) {
  return (
    <Icon {...props} strokeWidth={2.2}>
      <rect x="6.6" y="6.6" width="10.8" height="10.8" rx="1.6" fill="currentColor" />
    </Icon>
  );
}

export function NextIcon(props: IconProps) {
  return (
    <Icon {...props} strokeWidth={2.2}>
      <path d="M6.2 6.6 14.4 12l-8.2 5.4Z" fill="currentColor" />
      <path d="M17.8 6.4v11.2" />
    </Icon>
  );
}

export function PreviousIcon(props: IconProps) {
  return (
    <Icon {...props} strokeWidth={2.2}>
      <path d="M17.8 6.6 9.6 12l8.2 5.4Z" fill="currentColor" />
      <path d="M6.2 6.4v11.2" />
    </Icon>
  );
}

/** Two crossed ways: in no particular order. */
export function ShuffleIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.6 7.2h3.2c2.2 0 3.4 1.1 4.6 3.4l1.2 2.8c1.2 2.3 2.4 3.4 4.6 3.4h3.2" />
      <path d="M3.6 16.8h3.2c1.5 0 2.5-.5 3.4-1.5M13.8 8.7c.9-1 1.9-1.5 3.4-1.5h3.2" />
      <path d="m18 4.9 2.4 2.3L18 9.5M18 14.5l2.4 2.3-2.4 2.3" />
    </Icon>
  );
}

/** A loop, with a 1 inside it when one song is repeated. */
export function RepeatIcon({ one, ...props }: IconProps & { one: boolean }) {
  return (
    <Icon {...props}>
      <path d="M4.4 11.2V9.6a2.8 2.8 0 0 1 2.8-2.8h12.2M17 4.4l2.4 2.4L17 9.2" />
      <path d="M19.6 12.8v1.6a2.8 2.8 0 0 1-2.8 2.8H4.6M7 19.6l-2.4-2.4L7 14.8" />
      {one && <path d="M11.2 10.6 12.4 9.8v4.6" strokeWidth={1.6} />}
    </Icon>
  );
}

/** A speaker, with its waves, or crossed out when the sound is off. */
export function VolumeIcon({ off, ...props }: IconProps & { off: boolean }) {
  return (
    <Icon {...props}>
      <path d="M3.4 9.6h2.8l4.4-3.8v12.4l-4.4-3.8H3.4Z" />
      {off ? (
        <path d="m15 9.4 5 5.2M20 9.4l-5 5.2" />
      ) : (
        <path d="M14.6 9.2a4 4 0 0 1 0 5.6M17.2 6.8a7.4 7.4 0 0 1 0 10.4" />
      )}
    </Icon>
  );
}

/** Lines of a list, the first one playing: what comes next. */
export function QueueIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 6.4h11M4 11.2h11M4 16h7" />
      <path d="M15.6 14.2v5.2l4.2-2.6Z" fill="currentColor" />
    </Icon>
  );
}

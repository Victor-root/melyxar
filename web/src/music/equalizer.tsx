/*
 * Bars that rise and fall in the accent, the mark of the song that is
 * playing: they move while it plays and settle low when it is paused.
 */

export function Equalizer({ playing }: { playing: boolean }) {
  return (
    <span className={`equalizer${playing ? " equalizer-playing" : ""}`} aria-hidden="true">
      <span />
      <span />
      <span />
      <span />
    </span>
  );
}

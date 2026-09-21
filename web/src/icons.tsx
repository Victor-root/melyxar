/*
 * The interface's icons, drawn rather than typed.
 *
 * The player made this argument first and it holds for the whole interface:
 * an icon that is a character out of a font is drawn by whichever font the
 * machine happens to have, so the same header is a different weight on
 * Windows and on a phone, the shapes sit off the baseline next to text, and
 * none of them can take a colour of its own. Drawn here they are one weight
 * everywhere, they line up because they are all the same square, and they
 * take the colour of whatever they sit in, which is what lets the accent
 * reach them without a single colour being written down twice.
 *
 * No icon library is added for this. One would bring a few hundred shapes to
 * use a dozen, in somebody else's drawing style, and the two sets would never
 * quite match.
 *
 * All of them are one square of twenty four, stroked rather than filled
 * except where a shape reads better solid, so they sit together as one set.
 * The base is shared with the player's own icons, which is why it lives here
 * rather than there.
 */

export interface IconProps {
  /** Drawn at the size of the text around it unless told otherwise. */
  size?: number;
  /** For the player, whose stylesheet has a word about its own. */
  className?: string;
}

export function Icon({
  size = 22,
  className,
  strokeWidth = 1.8,
  children,
}: IconProps & { strokeWidth?: number; children: React.ReactNode }) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  );
}

/* -------------------------------------------------------------------------
 * The ones the whole interface uses, player included
 * ---------------------------------------------------------------------- */

/** Solid: the one control a hand goes to without looking. */
export function PlayIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M8 5.2v13.6a.6.6 0 0 0 .92.5l10.6-6.8a.6.6 0 0 0 0-1l-10.6-6.8A.6.6 0 0 0 8 5.2Z"
        fill="currentColor"
        stroke="none"
      />
    </Icon>
  );
}

/** Outlined when nothing has been said, solid once somebody has. */
export function HeartIcon({ filled, ...props }: IconProps & { filled: boolean }) {
  return (
    <Icon {...props}>
      <path
        d="M12 20.3 4.6 13a4.7 4.7 0 0 1 6.6-6.7l.8.8.8-.8A4.7 4.7 0 0 1 19.4 13Z"
        fill={filled ? "currentColor" : "none"}
      />
    </Icon>
  );
}

/** A tick, for what is chosen and for what has been watched. */
export function TickIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5 12.6l4.6 4.6L19 7.4" />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * The bar at the top
 * ---------------------------------------------------------------------- */

export function HomeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.4 10.6 12 3.6l8.6 7v8.2a1.6 1.6 0 0 1-1.6 1.6H5a1.6 1.6 0 0 1-1.6-1.6Z" />
      <path d="M9.4 20.4v-6.2h5.2v6.2" />
    </Icon>
  );
}

/** A reel of film, for the library of films. */
export function FilmIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2.8" y="4.4" width="18.4" height="15.2" rx="2.2" />
      <path d="M7.4 4.4v15.2M16.6 4.4v15.2M2.8 12h18.4M2.8 8.2h4.6M2.8 15.8h4.6M16.6 8.2h4.6M16.6 15.8h4.6" />
    </Icon>
  );
}

/** A screen on its stand, for series. */
export function SeriesIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2.8" y="6.6" width="18.4" height="12" rx="2.2" />
      <path d="M8.4 3.2 12 6.6l3.6-3.4" />
      <path d="M8.6 21.4h6.8" />
    </Icon>
  );
}

/** A star, for animation: the one category a screen alone cannot tell from
 *  the one above it. */
export function AnimeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3.2l2.7 5.6 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1L3.2 9.7l6.1-.9Z" />
    </Icon>
  );
}

/** An aerial, for broadcast programmes. */
export function ShowsIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="9.4" r="2.2" />
      <path d="M7.8 5.2a6 6 0 0 0 0 8.4M16.2 5.2a6 6 0 0 1 0 8.4" />
      <path d="M4.8 2.6a10 10 0 0 0 0 13.6M19.2 2.6a10 10 0 0 1 0 13.6" />
      <path d="M12 11.6 9.6 21.4M12 11.6l2.4 9.8" />
    </Icon>
  );
}

export function MusicIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M9.4 18.2V5.6l10-2v12.2" />
      <circle cx="6.8" cy="18.2" r="2.6" />
      <circle cx="16.8" cy="15.8" r="2.6" />
    </Icon>
  );
}

export function SearchIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="10.6" cy="10.6" r="6.8" />
      <path d="M15.6 15.6 20.6 20.6" />
    </Icon>
  );
}

/** Sending what is playing to another screen. Not to be confused with the
 *  player's own icon of that name, which is about who is in a film. */
export function ScreenCastIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.4 8V6.6A2.2 2.2 0 0 1 5.6 4.4h12.8a2.2 2.2 0 0 1 2.2 2.2v10.8a2.2 2.2 0 0 1-2.2 2.2H14" />
      <path d="M3.4 12.6a7 7 0 0 1 7 7" />
      <path d="M3.4 16.8a2.8 2.8 0 0 1 2.8 2.8" />
      <path d="M3.5 19.6v.1" strokeWidth={2.4} />
    </Icon>
  );
}

export function BellIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M18 9.4a6 6 0 1 0-12 0c0 5-2 6.4-2 6.4h16s-2-1.4-2-6.4Z" />
      <path d="M13.7 19.4a2 2 0 0 1-3.4 0" />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * Moving about
 * ---------------------------------------------------------------------- */

export function ChevronDownIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5.6 9 12 15.4 18.4 9" />
    </Icon>
  );
}

export function ChevronLeftIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M15 5.6 8.6 12 15 18.4" />
    </Icon>
  );
}

export function ChevronRightIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M9 5.6 15.4 12 9 18.4" />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * On a card
 * ---------------------------------------------------------------------- */

/** The three dots every interface hides the rest of its actions behind. */
export function MoreIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="5.4" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="12" cy="18.6" r="1.7" fill="currentColor" stroke="none" />
    </Icon>
  );
}

/** A circle with a tick in it, for what has been watched, and an empty one
 *  for what has not: the mark is pressed, so it has to look pressable. */
export function WatchedIcon({ watched, ...props }: IconProps & { watched: boolean }) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="9" fill={watched ? "currentColor" : "none"} />
      <path
        d="M7.8 12.2 10.7 15 16.2 9.4"
        stroke={watched ? "var(--accent-contrast, #fff)" : "currentColor"}
      />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * What a row is
 * ---------------------------------------------------------------------- */

/** A clock, for what was left halfway. */
export function ClockIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="8.8" />
      <path d="M12 6.6V12l3.6 2.2" />
    </Icon>
  );
}

/** An arrow into a tray, for what arrived last. */
export function ArrivedIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3.4v10.4" />
      <path d="M7.8 9.8 12 14l4.2-4.2" />
      <path d="M4 16.4v2.4a1.8 1.8 0 0 0 1.8 1.8h12.4a1.8 1.8 0 0 0 1.8-1.8v-2.4" />
    </Icon>
  );
}

/** A flame, for what a server is offering. */
export function SparkIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3.2s5.4 4 5.4 9.2a5.4 5.4 0 0 1-10.8 0c0-1.8.8-3.4 1.8-4.6.2 1.4 1 2.4 2 2.4 1.4 0 1.8-2.2 1.6-7Z" />
    </Icon>
  );
}

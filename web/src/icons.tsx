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

import type { LibraryKind } from "./api";

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

/**
 * Solid: the one control a hand goes to without looking.
 *
 * Filled and stroked with the same colour, which is what rounds its three
 * corners without drawing them by hand. Its bounding box leans a hair to the
 * right of the square's middle on purpose: a triangle centred by measurement
 * reads as sitting too far left, which is the whole reason play buttons have
 * been nudged by eye since the first tape deck.
 */
export function PlayIcon(props: IconProps) {
  return (
    <Icon {...props} strokeWidth={2.6}>
      <path d="M7.9 6.1 17.3 12 7.9 17.9Z" fill="currentColor" />
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

/**
 * The icon of a kind of library, whichever kind it is.
 *
 * One place rather than a match written out at each of the three screens that
 * needs one: the day a kind is added, the bar, the band and the rows all get
 * its shape at once.
 */
export function KindIcon({ kind, ...props }: IconProps & { kind: LibraryKind }) {
  switch (kind) {
    case "series":
      return <SeriesIcon {...props} />;
    case "anime":
      return <AnimeIcon {...props} />;
    case "shows":
      return <ShowsIcon {...props} />;
    case "music":
      return <MusicIcon {...props} />;
    default:
      return <FilmIcon {...props} />;
  }
}

/** A letter i in a circle: what opening the page of a work leads to. */
export function InfoIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="8.8" />
      <path d="M12 11v5.4" />
      <path d="M12 7.8h.01" />
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

/* -------------------------------------------------------------------------
 * What belongs to the account, in the bar
 * ---------------------------------------------------------------------- */

/** A cog, for the settings. */
export function GearIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="3.2" />
      <path d="M19.2 14.6a1.5 1.5 0 0 0 .3 1.66l.06.06a1.8 1.8 0 1 1-2.56 2.56l-.06-.06a1.5 1.5 0 0 0-1.66-.3 1.5 1.5 0 0 0-.9 1.38v.17a1.8 1.8 0 1 1-3.6 0v-.09a1.5 1.5 0 0 0-.98-1.37 1.5 1.5 0 0 0-1.66.3l-.06.06a1.8 1.8 0 1 1-2.56-2.56l.06-.06a1.5 1.5 0 0 0 .3-1.66 1.5 1.5 0 0 0-1.38-.9H4.2a1.8 1.8 0 1 1 0-3.6h.09a1.5 1.5 0 0 0 1.37-.98 1.5 1.5 0 0 0-.3-1.66l-.06-.06A1.8 1.8 0 1 1 7.86 4.9l.06.06a1.5 1.5 0 0 0 1.66.3h.07a1.5 1.5 0 0 0 .9-1.38V3.7a1.8 1.8 0 1 1 3.6 0v.09a1.5 1.5 0 0 0 .9 1.38 1.5 1.5 0 0 0 1.66-.3l.06-.06a1.8 1.8 0 1 1 2.56 2.56l-.06.06a1.5 1.5 0 0 0-.3 1.66v.07a1.5 1.5 0 0 0 1.38.9h.17a1.8 1.8 0 1 1 0 3.6h-.09a1.5 1.5 0 0 0-1.38.9Z" />
    </Icon>
  );
}

/** A dial, for what the server is doing. */
export function ActivityIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.4 12a8.6 8.6 0 0 1 17.2 0" />
      <path d="M12 12 16 8.6" />
      <path d="M3.4 12h2M18.6 12h2M12 3.4v2" />
    </Icon>
  );
}

/** Lines on a page, for the journal. */
export function JournalIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5.4 4.6h13.2v14.8H5.4z" />
      <path d="M8.4 8.6h7.2M8.4 12h7.2M8.4 15.4h4.4" />
    </Icon>
  );
}

/** A door with an arrow out of it, for leaving. */
export function LeaveIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M14.4 4.6H6.6a1.8 1.8 0 0 0-1.8 1.8v11.2a1.8 1.8 0 0 0 1.8 1.8h7.8" />
      <path d="M15.6 8.4 19.2 12l-3.6 3.6" />
      <path d="M19.2 12H9.6" />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * What a card's menu offers
 * ---------------------------------------------------------------------- */

/** A triangle with a bar after it, for playing everything from here. */
export function PlayAllIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M6 5.6 14 12l-8 6.4Z" fill="currentColor" />
      <path d="M18.4 5.6v12.8" />
    </Icon>
  );
}

/** Two squares one behind the other, for a collection. */
export function CollectionIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8.6 4.6h10.8v10.8" />
      <rect x="4.6" y="8.6" width="10.8" height="10.8" rx="1.8" />
    </Icon>
  );
}

/** A short list with a plus, for a playlist. */
export function PlaylistIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.4 7h11M4.4 12h11M4.4 17h6.6" />
      <path d="M16.6 14.4v5.2M14 17h5.2" />
    </Icon>
  );
}

/** A box with a tick, for choosing several at once. */
export function SelectIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="4.4" y="4.4" width="15.2" height="15.2" rx="2.4" />
      <path d="M8.4 12.2 11 14.8l4.8-5" />
    </Icon>
  );
}

/** An arrow into a tray, for taking a copy away. */
export function DownloadIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3.6v11" />
      <path d="M7.8 10.4 12 14.6l4.2-4.2" />
      <path d="M4.4 17.4v1.2a1.8 1.8 0 0 0 1.8 1.8h11.6a1.8 1.8 0 0 0 1.8-1.8v-1.2" />
    </Icon>
  );
}

/** A pencil, for editing what is written. */
export function EditIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M16.2 4.6 19.4 7.8 8.6 18.6l-4.2 1 1-4.2Z" />
      <path d="M14.2 6.6 17.4 9.8" />
    </Icon>
  );
}

/** A framed picture, for editing the pictures. */
export function ImageIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.6" y="5.4" width="16.8" height="13.2" rx="2" />
      <circle cx="8.6" cy="10" r="1.6" />
      <path d="M4.4 16.4 9.4 12l3.4 3 2.6-2.2 4.2 3.6" />
    </Icon>
  );
}

/** A screen with two lines at its foot, for the subtitles. */
export function SubtitlesIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.6" y="5.4" width="16.8" height="13.2" rx="2" />
      <path d="M7 14.6h4.4M13.6 14.6h3.4" />
      <path d="M7 11h2.4M11.6 11h5.4" />
    </Icon>
  );
}

/** A tag, for naming a work. */
export function IdentifyIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M11 3.8H19a1.2 1.2 0 0 1 1.2 1.2v8l-9.4 9.4a1.2 1.2 0 0 1-1.7 0l-6.7-6.7a1.2 1.2 0 0 1 0-1.7Z" />
      <circle cx="15.6" cy="8.4" r="1.4" fill="currentColor" stroke="none" />
    </Icon>
  );
}

/** The same tag, crossed out: forgetting what was found. */
export function ForgetIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M10.6 4.2h8.2a1.2 1.2 0 0 1 1.2 1.2v8" />
      <path d="M4.4 13.4 12 21a1.2 1.2 0 0 0 1.7 0l4.6-4.6" />
      <path d="M4.4 19.6 19.6 4.4" />
    </Icon>
  );
}

/** Two arrows chasing each other, for asking the catalogue again. */
export function RefreshIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M20 11.4a8 8 0 0 0-13.7-4.6L3.6 9.4" />
      <path d="M3.6 4.8v4.6h4.6" />
      <path d="M4 12.6a8 8 0 0 0 13.7 4.6l2.7-2.6" />
      <path d="M20.4 19.2v-4.6h-4.6" />
    </Icon>
  );
}

/** A pin, for what an administrator puts in front of everybody. */
export function PinIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M14.4 3.6 20.4 9.6l-2.4 1.2-1.2 3.6-5.4-5.4 3.6-1.2Z" />
      <path d="M11.4 9 4.6 19.4l10.4-6.8" />
    </Icon>
  );
}

/** A bin, for taking a work away for good. */
export function DeleteIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.6 6.6h14.8" />
      <path d="M9.4 6.6V4.8h5.2v1.8" />
      <path d="M6.4 6.6l.9 12a1.8 1.8 0 0 0 1.8 1.6h5.8a1.8 1.8 0 0 0 1.8-1.6l.9-12" />
      <path d="M10.4 10.4v6M13.6 10.4v6" />
    </Icon>
  );
}

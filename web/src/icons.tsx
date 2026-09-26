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

/** Two theatre masks, both smiling, for anime. */
export function AnimeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M13.192 9h6.616a2 2 0 0 1 1.992 2.183l-.567 6.182a4 4 0 0 1 -3.983 3.635h-1.5a4 4 0 0 1 -3.983 -3.635l-.567 -6.182a2 2 0 0 1 1.992 -2.183" />
      <path d="M15 13h.01" />
      <path d="M18 13h.01" />
      <path d="M15 16.5c1 .667 2 .667 3 0" />
      <path d="M8.632 15.982a4.037 4.037 0 0 1 -.382 .018h-1.5a4 4 0 0 1 -3.983 -3.635l-.567 -6.182a2 2 0 0 1 1.992 -2.183h6.616a2 2 0 0 1 2 2" />
      <path d="M6 8h.01" />
      <path d="M9 8h.01" />
      <path d="M6 12c.884 .251 1.648 .131 2.291 -.36" />
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

/** A photo of a hill under the sun, for what people filmed and photographed
 *  themselves. */
export function HomeMediaIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3" y="3" width="18" height="18" rx="3" />
      <path d="M15 8h.01" />
      <path d="M3 16l5-5c.93-.9 2.07-.9 3 0l5 5" />
      <path d="M14 14l1-1c.93-.9 2.07-.9 3 0l3 3" />
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
    case "home_media":
      return <HomeMediaIcon {...props} />;
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

/** Six dots, for what a hand takes hold of to move a line. Filled rather
 *  than stroked: dots drawn as rings read as holes at this size. */
export function GripIcon(props: IconProps) {
  return (
    <Icon {...props} strokeWidth={0}>
      {[9, 15].map((x) =>
        [6, 12, 18].map((y) => <circle key={`${x}:${y}`} cx={x} cy={y} r="1.6" fill="currentColor" />),
      )}
    </Icon>
  );
}

export function ChevronUpIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5.6 15 12 8.6 18.4 15" />
    </Icon>
  );
}

export function ChevronDownIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5.6 9 12 15.4 18.4 9" />
    </Icon>
  );
}

/* Both are drawn about the middle of the square to the decimal: a chevron
   that sits off centre in a round button is a chevron nobody can unsee, and
   the one in the banner did exactly that. */
export function ChevronLeftIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M15.2 5.6 8.8 12 15.2 18.4" />
    </Icon>
  );
}

export function ChevronRightIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8.8 5.6 15.2 12 8.8 18.4" />
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

/** A pair of binoculars, for what a series is being watched for. */
export function BinocularsIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 16a3 3 0 1 0 6 0a3 3 0 1 0 -6 0" />
      <path d="M14 16a3 3 0 1 0 6 0a3 3 0 1 0 -6 0" />
      <path d="M16.346 9.17l-.729 -1.261c-.16 -.248 -1.056 -.203 -1.117 .091l-.177 1.38" />
      <path d="M19.761 14.813l-2.84 -5.133c-.189 -.31 -.592 -.68 -1.421 -.68c-.828 0 -1.5 .448 -1.5 1v6" />
      <path d="M7.654 9.17l.729 -1.261c.16 -.249 1.056 -.203 1.117 .091l.177 1.38" />
      <path d="M4.239 14.813l2.84 -5.133c.189 -.31 .592 -.68 1.421 -.68c.828 0 1.5 .448 1.5 1v6" />
      <path d="M10 12h4v2h-4l0 -2" />
    </Icon>
  );
}

/** A video camera with a plus, for what was just added to it. */
export function CameraIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M15 10l4.553 -2.276a1 1 0 0 1 1.447 .894v6.764a1 1 0 0 1 -1.447 .894l-4.553 -2.276v-4" />
      <path d="M3 8a2 2 0 0 1 2 -2h8a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-8a2 2 0 0 1 -2 -2l0 -8" />
      <path d="M7 12l4 0" />
      <path d="M9 10l0 4" />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * What belongs to the account, in the bar
 * ---------------------------------------------------------------------- */

/* The wheel's own middle sat away from the hole cut in it, off by most of a
   pixel, which is what read as a hole leaning to one side. Moved as a whole
   rather than redrawn: every tooth keeps the shape it already had, and only
   the wheel's centre was ever wrong. */
export function GearIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="3.1" />
      <path d="M18.66 14.63a1.5 1.5 0 0 0 .3 1.66l.06.05a1.8 1.8 0 1 1-2.55 2.55l-.05-.06a1.5 1.5 0 0 0-1.66-.3 1.5 1.5 0 0 0-.9 1.37v.16a1.8 1.8 0 1 1-3.6 0v-.09a1.5 1.5 0 0 0-.99-1.37 1.5 1.5 0 0 0-1.65.3l-.06.06a1.8 1.8 0 1 1-2.55-2.55l.06-.05a1.5 1.5 0 0 0 .3-1.66 1.5 1.5 0 0 0-1.38-.9h-.16a1.8 1.8 0 1 1 0-3.6h.09a1.5 1.5 0 0 0 1.37-.99 1.5 1.5 0 0 0-.3-1.65l-.06-.06A1.8 1.8 0 1 1 7.56 5.11l.05.06a1.5 1.5 0 0 0 1.66.3h.07a1.5 1.5 0 0 0 .9-1.38v-.16a1.8 1.8 0 1 1 3.6 0v.09a1.5 1.5 0 0 0 .9 1.37 1.5 1.5 0 0 0 1.66-.3l.05-.06a1.8 1.8 0 1 1 2.55 2.55l-.06.05a1.5 1.5 0 0 0-.3 1.66v.07a1.5 1.5 0 0 0 1.38.9h.16a1.8 1.8 0 1 1 0 3.6h-.09a1.5 1.5 0 0 0-1.37.9Z" />
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
 * The page of one work
 * ---------------------------------------------------------------------- */

/** Solid, for how well a work is rated. */
export function StarIcon(props: IconProps) {
  return (
    <Icon {...props} strokeWidth={1.4}>
      <path
        d="m12 3.6 2.5 5.3 5.8.7-4.3 4 1.1 5.7L12 16.5l-5.1 2.8L8 13.6l-4.3-4 5.8-.7Z"
        fill="currentColor"
      />
    </Icon>
  );
}

/** A clapperboard, for the trailer: the film before the film. */
export function TrailerIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3" y="10" width="18" height="10" rx="1.8" />
      <path d="M3 10 19.8 5.5 19 2.6 2.2 7.1Z" />
      <path d="M7.4 5.7l2.3 3M12.2 4.4l2.3 3" />
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

/** One arrow turning back on itself, for putting something back the way it
 *  came. */
export function ResetIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.4 12a7.6 7.6 0 1 0 2.2-5.4L4.4 8.8" />
      <path d="M4.4 4.2v4.6H9" />
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

/** A pin, for what an administrator puts in front of everybody. Filled once
 *  the work is there, the way the heart is once a work is a favourite. */
export function PinIcon({ filled = false, ...props }: IconProps & { filled?: boolean }) {
  return (
    <Icon {...props}>
      <path
        d="M14.4 3.6 20.4 9.6l-2.4 1.2-1.2 3.6-5.4-5.4 3.6-1.2Z"
        fill={filled ? "currentColor" : "none"}
      />
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

/** A cross, for shutting a panel. */
export function CloseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M6.2 6.2 17.8 17.8" />
      <path d="M17.8 6.2 6.2 17.8" />
    </Icon>
  );
}

/** An arrow back, for a step of a panel that is left rather than shut. */
export function BackIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M19.4 12H5.2" />
      <path d="m11 5.6-5.8 6.4 5.8 6.4" />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * The door
 * ---------------------------------------------------------------------- */

/** Whose account this is: the mark beside the name somebody types. */
export function AccountIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="8.2" r="3.6" />
      <path d="M5.4 19.4a6.6 6.6 0 0 1 13.2 0" />
    </Icon>
  );
}

/** A padlock, closed: the mark beside the password. */
export function LockIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="4.8" y="10.4" width="14.4" height="9.4" rx="2.2" />
      <path d="M8.4 10.4V7.8a3.6 3.6 0 0 1 7.2 0v2.6" />
      <path d="M12 14.2v2" />
    </Icon>
  );
}

/** Shown: what pressing it does, which is let the password be read. */
export function EyeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.6 12S6.4 5.8 12 5.8 21.4 12 21.4 12 17.6 18.2 12 18.2 2.6 12 2.6 12Z" />
      <circle cx="12" cy="12" r="3" />
    </Icon>
  );
}

/** The same eye, struck through: what pressing it does once it is shown. */
export function EyeOffIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M9.9 6.1A8.5 8.5 0 0 1 12 5.8c5.6 0 9.4 6.2 9.4 6.2a17 17 0 0 1-2.7 3.4" />
      <path d="M6.3 8A17.5 17.5 0 0 0 2.6 12s3.8 6.2 9.4 6.2a8.7 8.7 0 0 0 3.5-.7" />
      <path d="M10 10a2.8 2.8 0 0 0 4 4" />
      <path d="M4.4 4.4 19.6 19.6" />
    </Icon>
  );
}

/** A way in: an arrow that goes through the door rather than out of it. */
export function EnterIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M13.2 4.6h4.2a1.8 1.8 0 0 1 1.8 1.8v11.2a1.8 1.8 0 0 1-1.8 1.8h-4.2" />
      <path d="m9 8.4 3.6 3.6L9 15.6" />
      <path d="M12.6 12H4.8" />
    </Icon>
  );
}

/** Somebody who is not here yet. */
export function AccountAddIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="9.8" cy="8.2" r="3.6" />
      <path d="M3.6 19.4a6.4 6.4 0 0 1 12.4-2.2" />
      <path d="M18.4 14.6v5.2M21 17.2h-5.2" />
    </Icon>
  );
}

/* -------------------------------------------------------------------------
 * The administration
 * ---------------------------------------------------------------------- */

/** Three bars of rising height, for the summary. */
export function SummaryIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5.4 19.2v-6M12 19.2V5.2M18.6 19.2v-9.6" />
    </Icon>
  );
}

/** A folder, for the libraries. */
export function FolderIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.6 7.2a1.8 1.8 0 0 1 1.8-1.8h4l2 2.2h7.2a1.8 1.8 0 0 1 1.8 1.8v7.8a1.8 1.8 0 0 1-1.8 1.8H5.4a1.8 1.8 0 0 1-1.8-1.8Z" />
    </Icon>
  );
}

/** A label with its hole, for what is known about the works. */
export function TagIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.8 12.6V5.4a1.6 1.6 0 0 1 1.6-1.6h7.2l7.6 7.6a1.6 1.6 0 0 1 0 2.3l-6.9 6.9a1.6 1.6 0 0 1-2.3 0Z" />
      <circle cx="8.4" cy="8.4" r="1.4" />
    </Icon>
  );
}

/** A play mark in a ring, for what is being watched. */
export function PlaybackIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="8.6" />
      <path d="m10.2 8.8 5 3.2-5 3.2Z" />
    </Icon>
  );
}

/** A speaker, and a double arrow for the channels its sound is spread
 *  across. */
export function SoundIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.4 9.6h2.8l4.4-3.8v12.4l-4.4-3.8H3.4Z" />
      <path d="M14.2 12h6.8M16.4 9.8 14.2 12l2.2 2.2M18.8 9.8 21 12l-2.2 2.2" />
    </Icon>
  );
}

/** A character beside a letter, for what is said in which language. */
export function LanguagesIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 5h7M9 3v2c0 4.4-2.2 8-5 8M5 9c0 2.1 3 3.9 6.7 4" />
      <path d="M12 20l4-9 4 9M19.1 18h-6.2" />
    </Icon>
  );
}

/** The mark of Melyxar, in the accent somebody chose.
 *
 *  Drawn from its relief alone, a grey picture of the logo's light and shade:
 *  laid over the accent, cut to the logo's shape, it keeps every fold of the
 *  ribbon in whatever colour the accent is. The larger copy is taken once the
 *  small one would be blown up. */
export function MelyxarMark({ size = 22, className }: IconProps) {
  const relief = size > 64 ? "/melyxar-shade-512.png" : "/melyxar-shade-64.png";
  return (
    <span
      className={`melyxar-mark${className ? ` ${className}` : ""}`}
      style={{ width: size, height: size, "--relief": `url(${relief})` } as React.CSSProperties}
      aria-hidden="true"
    >
      <img src={relief} alt="" />
    </span>
  );
}

/** The mark of FFmpeg, the tool every film goes through, in the colour of
 *  whatever it sits in rather than its own green. */
export function FfmpegIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        transform="translate(2.2 2.35) scale(0.3165)"
        fill="currentColor"
        stroke="none"
        d="m23.808 3.106-12.804 14.933v6.319l19.018-21.778 30.485-2.58-44.687 47.006 5.894.34 32.454-33.446v31.748l-3.63 3.411 9.221.545v8.799l-28.786-2.438 14.624-14.228v-7.06l-20.876 20.76-24.721-2.093 39.798-43.302-6.603.39-28.74 31.885v-27.091l2.704-3.255-6.648.393v-7.286z"
      />
    </Icon>
  );
}

/** Two people, for the accounts. */
export function PeopleIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="9.2" cy="8.4" r="3.2" />
      <path d="M3.4 19.2a5.8 5.8 0 0 1 11.6 0" />
      <path d="M15.4 5.4a3.2 3.2 0 0 1 0 6M17.6 14a5.8 5.8 0 0 1 3 5.2" />
    </Icon>
  );
}

/** A screen on its foot, for the devices. */
export function DeviceIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.4" y="4.6" width="17.2" height="11.6" rx="1.8" />
      <path d="M9 19.6h6M12 16.2v3.4" />
    </Icon>
  );
}

/** Ticked lines, for the work the server does on its own. */
export function TasksIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="m4 6.6 1.6 1.6 2.8-2.8M4 13.4l1.6 1.6 2.8-2.8" />
      <path d="M11.6 7h8.4M11.6 13.8h8.4M11.6 19h8.4" />
    </Icon>
  );
}

/** A shield, for who may come in. */
export function ShieldIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3.4 19 6v5.6c0 4.2-2.9 7.6-7 9-4.1-1.4-7-4.8-7-9V6Z" />
      <path d="m9.2 12 2 2 3.8-3.8" />
    </Icon>
  );
}

/** A trace across a card, for what the server says about itself. */
export function DiagnosticsIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.4" y="4.4" width="17.2" height="15.2" rx="2" />
      <path d="M6.6 12.4h2.6l1.8-3.6 2.4 6.4 1.6-2.8h2.4" />
    </Icon>
  );
}

/** Two units of a rack, one above the other, each with its lights, for the
 *  server itself. */
export function ServerIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.6" y="4" width="16.8" height="6.6" rx="1.6" />
      <rect x="3.6" y="13.4" width="16.8" height="6.6" rx="1.6" />
      <path d="M7.2 7.3h.01M7.2 16.7h.01M10.4 7.3h.01M10.4 16.7h.01M14 7.3h3.2M14 16.7h3.2" />
    </Icon>
  );
}

/** A clock face, for how long and how long ago. */
export function ClockIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="8.6" />
      <path d="M12 7.4V12l3 1.8" />
    </Icon>
  );
}

/** Stacked discs, for the database. */
export function DatabaseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <ellipse cx="12" cy="6" rx="7" ry="2.6" />
      <path d="M5 6v12c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6V6" />
      <path d="M5 12c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6" />
    </Icon>
  );
}

/** A chip with its pins, for the processor. */
export function ChipIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="6.4" y="6.4" width="11.2" height="11.2" rx="1.6" />
      <rect x="9.6" y="9.6" width="4.8" height="4.8" rx="0.6" />
      <path d="M9.6 3.6v2.8M14.4 3.6v2.8M9.6 17.6v2.8M14.4 17.6v2.8M3.6 9.6h2.8M3.6 14.4h2.8M17.6 9.6h2.8M17.6 14.4h2.8" />
    </Icon>
  );
}

/** A strip of memory with its contacts. */
export function MemoryIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.4" y="7" width="17.2" height="8.6" rx="1.4" />
      <path d="M7.4 10v2.6M11 10v2.6M14.6 10v2.6M6.4 15.6v2.2M10 15.6v2.2M14 15.6v2.2M17.6 15.6v2.2" />
    </Icon>
  );
}

/** A drive, for the disks. */
export function DiskIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 13.4 6.6 5.8A1.8 1.8 0 0 1 8.3 4.6h7.4a1.8 1.8 0 0 1 1.7 1.2l2.6 7.6" />
      <rect x="4" y="13.4" width="16" height="6" rx="1.8" />
      <path d="M16.2 16.4h.01" strokeWidth={2.6} />
    </Icon>
  );
}

/** Up and down, for what passes over the network. */
export function NetworkIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8 19.4V4.8L4.6 8.2M16 4.6v14.6l3.4-3.4" />
    </Icon>
  );
}

/** A graphics card with its fan. */
export function GraphicsCardIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.4 5.4v14" />
      <rect x="3.4" y="7" width="17.2" height="9.6" rx="1.4" />
      <circle cx="10.4" cy="11.8" r="2.6" />
      <path d="M15.6 10.2h2.2M15.6 13.4h2.2M6.4 16.6v2.2M9.4 16.6v2.2" />
    </Icon>
  );
}

/** A triangle with a mark in it, for what needs a look. */
export function WarningIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M10.4 4.6a1.8 1.8 0 0 1 3.2 0l7 12.4a1.8 1.8 0 0 1-1.6 2.7H5a1.8 1.8 0 0 1-1.6-2.7Z" />
      <path d="M12 9.6v3.8M12 16.6h.01" />
    </Icon>
  );
}

/** A clock with an arrow running back round it, for what happened lately. */
export function HistoryIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.2 12a7.8 7.8 0 1 0 2.3-5.5L4.2 8.8" />
      <path d="M4.2 4.6v4.2h4.2M12 8v4l2.8 1.6" />
    </Icon>
  );
}

/** An arrow leaving to the right, for "go and see". */
export function ArrowRightIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.8 12h14.4M13.8 6.6l5.4 5.4-5.4 5.4" />
    </Icon>
  );
}

/** A face with its shoulders in a frame, for somebody's profile. */
export function ProfileIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="3.6" y="3.6" width="16.8" height="16.8" rx="3" />
      <circle cx="12" cy="10" r="3" />
      <path d="M7 18.2a5.4 5.4 0 0 1 10 0" />
    </Icon>
  );
}

/** A brush over a drop, for how the interface looks. */
export function PaletteIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 3.6a8.4 8.4 0 0 0 0 16.8c1 0 1.6-.7 1.6-1.5 0-.4-.2-.8-.4-1.1-.3-.3-.4-.6-.4-1 0-.8.7-1.5 1.5-1.5h1.9a4.2 4.2 0 0 0 4.2-4.2c0-4.1-3.8-7.5-8.4-7.5Z" />
      <circle cx="7.6" cy="11.4" r="1" />
      <circle cx="10" cy="7.6" r="1" />
      <circle cx="14.4" cy="7.6" r="1" />
    </Icon>
  );
}

/** A house, for the front page. */
export function HomeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 10.6 12 4l8 6.6V19a1.4 1.4 0 0 1-1.4 1.4H5.4A1.4 1.4 0 0 1 4 19Z" />
      <path d="M9.6 20.4v-5.6h4.8v5.6" />
    </Icon>
  );
}

/** Sliders, for the settings of the server as a whole. */
export function SlidersIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.4 7h9.2M17.8 7h1.8M4.4 17h1.8M10.4 17h9.2" />
      <circle cx="15.7" cy="7" r="2.1" />
      <circle cx="8.3" cy="17" r="2.1" />
    </Icon>
  );
}

/** A tick in a disc, for what is fine. */
export function OkIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="8.6" />
      <path d="m8.4 12.2 2.4 2.4 4.8-4.8" />
    </Icon>
  );
}

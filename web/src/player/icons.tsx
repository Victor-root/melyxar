/*
 * The player's icons, drawn rather than typed.
 *
 * They used to be characters out of a font: a play triangle, a pair of bars, a
 * speaker. That works and looks like it works. Every one of them is drawn by
 * whichever font the machine happens to have, so the same player is a
 * different weight on Windows and on a phone, they sit off the baseline next
 * to text, and none of them can be given a colour of its own.
 *
 * Drawn here, they are one weight everywhere, they line up because they are
 * all the same square, and they take the colour of whatever they sit in, which
 * is what lets the accent reach them without a single colour being written
 * down twice.
 *
 * All of them are one square of twenty four, stroked rather than filled except
 * where a shape reads better solid, so they sit together as one set.
 */

interface IconProps {
  /** Drawn at the size of the text around it unless told otherwise. */
  size?: number;
}

function Icon({ size = 22, children }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      className="player-icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  );
}

export function PlayIcon(props: IconProps) {
  // Solid: the one control a hand goes to without looking, and the only one
  // worth making heavier than the rest.
  return (
    <Icon {...props}>
      <path d="M8 5.2v13.6a.6.6 0 0 0 .92.5l10.6-6.8a.6.6 0 0 0 0-1l-10.6-6.8A.6.6 0 0 0 8 5.2Z" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function PauseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="6.5" y="5" width="4" height="14" rx="1.2" fill="currentColor" stroke="none" />
      <rect x="13.5" y="5" width="4" height="14" rx="1.2" fill="currentColor" stroke="none" />
    </Icon>
  );
}

/** A step back, with the number of seconds written inside the arc. */
export function StepBackIcon({ seconds, ...props }: IconProps & { seconds: number }) {
  return (
    <Icon {...props}>
      <path d="M12 5a7 7 0 1 1-6.8 8.6" />
      <path d="M5 3.4V7.4h4" />
      <text
        x="12"
        y="15.4"
        textAnchor="middle"
        fontSize="7.4"
        fontWeight="700"
        fill="currentColor"
        stroke="none"
      >
        {seconds}
      </text>
    </Icon>
  );
}

/** A step on, the same arc the other way round. */
export function StepOnIcon({ seconds, ...props }: IconProps & { seconds: number }) {
  return (
    <Icon {...props}>
      <path d="M12 5a7 7 0 1 0 6.8 8.6" />
      <path d="M19 3.4V7.4h-4" />
      <text
        x="12"
        y="15.4"
        textAnchor="middle"
        fontSize="7.4"
        fontWeight="700"
        fill="currentColor"
        stroke="none"
      >
        {seconds}
      </text>
    </Icon>
  );
}

export function PreviousChapterIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M18 6.4v11.2a.6.6 0 0 1-.93.5L9 13.1v4.5a.6.6 0 0 1-.9.52l-.1-.06V5.94a.6.6 0 0 1 .9-.52l.1.06v4.5l8.07-5.08a.6.6 0 0 1 .93.5Z" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function NextChapterIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M6 6.4v11.2a.6.6 0 0 0 .93.5L15 13.1v4.5a.6.6 0 0 0 .9.52l.1-.06V5.94a.6.6 0 0 0-.9-.52l-.1.06v4.5L6.93 4.9a.6.6 0 0 0-.93.5Z" fill="currentColor" stroke="none" />
    </Icon>
  );
}

export function BackIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M19 12H5" />
      <path d="M11 6l-6 6 6 6" />
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

/** The rectangle every player uses for words on a picture. */
export function SubtitlesIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2.8" y="5" width="18.4" height="14" rx="2.4" />
      <path d="M6.6 14.6h4.2M13.4 14.6h4M6.6 11h2.6M11.8 11h5.6" />
    </Icon>
  );
}

export function AudioIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 9.6v4.8M8 6.6v10.8M12 4v16M16 7.4v9.2M20 10.4v3.2" />
    </Icon>
  );
}

/** How loud, in four states: a slider at nought is not the same as a sound
 *  switched off, and one wave is not the same as three. */
export function VolumeIcon({
  level,
  ...props
}: IconProps & { level: "off" | "low" | "middling" | "high" }) {
  return (
    <Icon {...props}>
      <path
        d="M3.4 9.4h3L11.4 5.2v13.6L6.4 14.6h-3Z"
        fill="currentColor"
        stroke="currentColor"
        strokeWidth={1.4}
        strokeLinejoin="round"
      />
      {level === "off" ? (
        <path d="M15.4 9.6l4.8 4.8M20.2 9.6l-4.8 4.8" />
      ) : (
        <>
          <path d="M14.4 9.8a3.1 3.1 0 0 1 0 4.4" />
          {level !== "low" && <path d="M17 7.6a6.3 6.3 0 0 1 0 8.8" />}
          {level === "high" && <path d="M19.6 5.4a9.4 9.4 0 0 1 0 13.2" />}
        </>
      )}
    </Icon>
  );
}

/* Worked out from its own geometry rather than copied from somewhere, which
   is how the last one ended up with a hole that leaned to one side: eight
   teeth around one middle, and a circle on that same middle. */
export function SettingsIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M9.81 2.86L14.19 2.86L14.79 5.26L16.91 3.99L20.01 7.09L18.74 9.21L21.14 9.81L21.14 14.19L18.74 14.79L20.01 16.91L16.91 20.01L14.79 18.74L14.19 21.14L9.81 21.14L9.21 18.74L7.09 20.01L3.99 16.91L5.26 14.79L2.86 14.19L2.86 9.81L5.26 9.21L3.99 7.09L7.09 3.99L9.21 5.26Z" />
      <circle cx="12" cy="12" r="3.4" />
    </Icon>
  );
}

/** The picture kept in a corner of the screen. */
export function CornerIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M21 11.4V6.6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v10.8a2 2 0 0 0 2 2h6" />
      <rect x="12.6" y="12.6" width="8.4" height="6.8" rx="1.6" fill="currentColor" stroke="currentColor" />
    </Icon>
  );
}

export function FullscreenIcon({ leaving, ...props }: IconProps & { leaving: boolean }) {
  return (
    <Icon {...props}>
      {leaving ? (
        <path d="M9.4 4.6v4.8H4.6M14.6 4.6v4.8h4.8M9.4 19.4v-4.8H4.6M14.6 19.4v-4.8h4.8" />
      ) : (
        <path d="M4.6 9.4V4.6h4.8M19.4 9.4V4.6h-4.8M4.6 14.6v4.8h4.8M19.4 14.6v4.8h-4.8" />
      )}
    </Icon>
  );
}

/** A tick, for what is chosen in a menu. */
export function ChosenIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5 12.6l4.6 4.6L19 7.4" />
    </Icon>
  );
}

/** The arrow that says a menu entry opens another one. */
export function IntoIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M9.6 5.4l6.6 6.6-6.6 6.6" />
    </Icon>
  );
}

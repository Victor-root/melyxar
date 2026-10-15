/*
 * What the player asks of the thing the film plays in.
 *
 * In a browser, that is the browser's own video element. In the Android
 * application, it is an element of this interface that hands the film to the
 * application's own player, which decodes far more than a browser does, and
 * draws nothing but transparency over its picture (see native-picture.ts).
 * Everything the player reads, sets and listens to is named here, so the two
 * answer the same questions and the player never knows which one it has.
 *
 * What only a browser can do is optional, and the player already asks before
 * using it: counting the pictures it decoded, saying when each reaches the
 * screen, putting itself in a corner of the screen.
 */

/** The words of one subtitle track, as the player reads and shifts them. */
export interface WordsTrack extends EventTarget {
  readonly kind: TextTrackKind;
  mode: TextTrackMode;
  readonly cues: ArrayLike<TextTrackCue> | null;
  readonly activeCues: ArrayLike<TextTrackCue> | null;
}

/** Every subtitle track of the picture. */
export interface WordsTracks extends EventTarget, ArrayLike<WordsTrack> {}

/** What carries the words of a film onto its picture: a track element in a
 *  browser. */
export interface WordsHolder extends EventTarget {
  readonly track: WordsTrack;
}

export interface Picture extends HTMLElement {
  currentTime: number;
  readonly duration: number;
  readonly paused: boolean;
  readonly ended: boolean;
  readonly seeking: boolean;
  readonly buffered: TimeRanges;
  volume: number;
  muted: boolean;
  playbackRate: number;
  loop: boolean;
  autoplay: boolean;
  src: string;
  readonly videoWidth: number;
  readonly videoHeight: number;
  readonly error: MediaError | null;
  readonly readyState: number;
  readonly networkState: number;
  readonly textTracks: WordsTracks;
  play(): Promise<void>;
  pause(): void;
  canPlayType(type: string): string;
  getVideoPlaybackQuality?(): VideoPlaybackQuality;
  requestVideoFrameCallback?(callback: VideoFrameRequestCallback): number;
  cancelVideoFrameCallback?(handle: number): void;
  requestPictureInPicture?(): Promise<unknown>;
}

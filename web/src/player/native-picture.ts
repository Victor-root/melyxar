/*
 * The picture of the Android application, as the player sees it.
 *
 * An element of this page that answers every question the player asks of a
 * video element (see picture.ts), and passes every request on to the
 * application's own player, which draws the film beneath this see-through
 * page. Nothing is drawn here: the element only holds the place of the
 * picture, so a click on it is a click on the film.
 *
 * Its words come from a child element, `melyxar-words`, standing for the
 * track element a browser would read: the file is fetched and read here, and
 * the words on screen are worked out from where the film is each time the
 * application says where that is.
 */

import { appPlayer, hear } from "./app-player";
import type { AppPlayer, Told } from "./app-player";
import type { Shape, Turn } from "./panels";
import type { Picture, WordsHolder, WordsTrack, WordsTracks } from "./picture";
import { cuesOf } from "./vtt";

const PICTURE_TAG = "melyxar-picture";
const WORDS_TAG = "melyxar-words";

/** What a picture knows before the application has said anything. */
const UNTOLD: Omit<Told, "id"> = {
  position: 0,
  duration: null,
  buffered: 0,
  playing: false,
  advancing: false,
  ended: false,
  seeking: false,
  width: 0,
  height: 0,
  volume: 1,
  muted: false,
  shown: 0,
  dropped: 0,
  error: null,
  events: [],
};

/** One track of words, read from its file and shown by the film's clock. */
class NativeWordsTrack extends EventTarget implements WordsTrack {
  readonly kind: TextTrackKind = "subtitles";
  mode: TextTrackMode = "hidden";
  cues: VTTCue[] = [];
  activeCues: VTTCue[] = [];

  /** The cues on screen at this second, said when they change. */
  showAt(seconds: number) {
    const now = this.cues.filter((cue) => cue.startTime <= seconds && seconds < cue.endTime);
    if (now.length !== this.activeCues.length || now.some((cue, index) => cue !== this.activeCues[index])) {
      this.activeCues = now;
      this.dispatchEvent(new Event("cuechange"));
    }
  }
}

/** The tracks of words of one picture, read as a browser's list of them. */
class NativeWordsTracks extends EventTarget implements WordsTracks {
  [index: number]: WordsTrack;
  length = 0;
  private held: NativeWordsTrack[] = [];

  add(track: NativeWordsTrack) {
    this.hold([...this.held, track]);
  }

  remove(track: NativeWordsTrack) {
    this.hold(this.held.filter((other) => other !== track));
  }

  showAt(seconds: number) {
    for (const track of this.held) {
      track.showAt(seconds);
    }
  }

  private hold(tracks: NativeWordsTrack[]) {
    for (let index = 0; index < this.length; index += 1) {
      delete this[index];
    }
    tracks.forEach((track, index) => {
      this[index] = track;
    });
    this.held = tracks;
    this.length = tracks.length;
  }
}

/** The track element's stand in: fetches its file and reads it into cues. */
class NativeWords extends HTMLElement implements WordsHolder {
  readonly track = new NativeWordsTrack();

  set src(address: string) {
    fetch(address, { credentials: "same-origin" })
      .then((answer) => (answer.ok ? answer.text() : Promise.reject(new Error(String(answer.status)))))
      .then((file) => {
        this.track.cues = cuesOf(file).map((cue) => new VTTCue(cue.start, cue.end, cue.text));
        this.dispatchEvent(new Event("load"));
      })
      .catch(() => this.dispatchEvent(new Event("error")));
  }

  connectedCallback() {
    const picture = this.closest(PICTURE_TAG);
    if (picture instanceof NativePicture) {
      picture.textTracks.add(this.track);
    }
  }

  disconnectedCallback() {
    const picture = this.closest(PICTURE_TAG);
    if (picture instanceof NativePicture) {
      picture.textTracks.remove(this.track);
    }
  }
}

let pictures = 0;

export class NativePicture extends HTMLElement implements Picture {
  /** Told apart from the picture before it, whose last words may still be
   *  on their way when this one opens. */
  private readonly serial = (pictures += 1);
  private told: Omit<Told, "id"> = UNTOLD;
  /** When the application last said where the film is, to carry the clock
   *  on between two of its reports. */
  private toldAt = performance.now();
  private address = "";
  private opened = false;
  private stopHearing: (() => void) | null = null;
  private rate = 1;
  private looping = false;
  private fitting: Shape = "auto";
  private turning: Turn = 0;
  private lightness = 1;
  private contrasting = 1;
  autoplay = true;
  readonly textTracks = new NativeWordsTracks();

  get src() {
    return this.address;
  }

  /** Opened once whoever set it is done setting the rest: the player sets the
   *  sound, the loop and whether to start on the same picture in the same
   *  moment, after the address. */
  set src(address: string) {
    this.address = address;
    queueMicrotask(() => this.open());
  }

  private open() {
    if (!appPlayer || !this.isConnected || this.address === "") {
      return;
    }
    this.stopHearing?.();
    this.stopHearing = hear(this.serial, (told) => this.heard(told));
    this.opened = true;
    appPlayer.open(
      this.serial,
      new URL(this.address, window.location.href).href,
      JSON.stringify({
        start: this.told.position,
        autoplay: this.autoplay,
        volume: this.told.volume,
        muted: this.told.muted,
        rate: this.rate,
        loop: this.looping,
        fit: this.fitting,
        turn: this.turning,
        brightness: this.lightness,
        contrast: this.contrasting,
      }),
    );
  }

  /** Passed on once the application has the film; until then, what is set
   *  here goes along with the opening instead. */
  private ask(request: (player: AppPlayer, id: number) => void) {
    if (appPlayer && this.opened) {
      request(appPlayer, this.serial);
    }
  }

  disconnectedCallback() {
    this.stopHearing?.();
    this.stopHearing = null;
    this.ask((player, id) => player.close(id));
    this.opened = false;
  }

  private heard(told: Told) {
    const before = this.told;
    this.told = told;
    this.toldAt = performance.now();
    this.textTracks.showAt(told.position);
    if (told.duration !== before.duration) {
      this.dispatchEvent(new Event("durationchange"));
    }
    if (told.buffered !== before.buffered) {
      this.dispatchEvent(new Event("progress"));
    }
    if (told.width !== before.width || told.height !== before.height) {
      this.dispatchEvent(new Event("resize"));
    }
    if (told.volume !== before.volume || told.muted !== before.muted) {
      this.dispatchEvent(new Event("volumechange"));
    }
    for (const name of told.events) {
      this.dispatchEvent(new Event(name));
    }
    this.dispatchEvent(new Event("timeupdate"));
  }

  get currentTime() {
    const { position, advancing, seeking, duration } = this.told;
    if (!advancing || seeking) {
      return position;
    }
    const carried = position + ((performance.now() - this.toldAt) / 1000) * this.rate;
    return duration === null ? carried : Math.min(duration, carried);
  }

  set currentTime(seconds: number) {
    this.told = { ...this.told, position: seconds, seeking: this.opened, ended: false };
    this.toldAt = performance.now();
    if (this.opened) {
      this.dispatchEvent(new Event("seeking"));
      this.ask((player, id) => player.seek(id, seconds));
    }
  }

  get duration() {
    return this.told.duration ?? Number.NaN;
  }

  get paused() {
    return !this.told.playing;
  }

  get ended() {
    return this.told.ended;
  }

  get seeking() {
    return this.told.seeking;
  }

  get buffered(): TimeRanges {
    const held = this.told.buffered;
    const length = held > 0 ? 1 : 0;
    return {
      length,
      start: () => 0,
      end: () => held,
    };
  }

  get volume() {
    return this.told.volume;
  }

  set volume(volume: number) {
    this.told = { ...this.told, volume };
    this.ask((player, id) => player.setSound(id, volume, this.told.muted));
    this.dispatchEvent(new Event("volumechange"));
  }

  get muted() {
    return this.told.muted;
  }

  set muted(muted: boolean) {
    this.told = { ...this.told, muted };
    this.ask((player, id) => player.setSound(id, this.told.volume, muted));
    this.dispatchEvent(new Event("volumechange"));
  }

  get playbackRate() {
    return this.rate;
  }

  set playbackRate(rate: number) {
    this.rate = rate;
    this.ask((player, id) => player.setRate(id, rate));
    this.dispatchEvent(new Event("ratechange"));
  }

  get loop() {
    return this.looping;
  }

  set loop(loop: boolean) {
    this.looping = loop;
    this.ask((player, id) => player.setLoop(id, loop));
  }

  /** How the picture is fitted into the screen and turned: drawn by the
   *  application, so told to it rather than written in a style. */
  get fit() {
    return this.fitting;
  }

  set fit(fit: Shape) {
    this.fitting = fit;
    this.ask((player, id) => player.setShape(id, fit, this.turning));
  }

  get turn() {
    return this.turning;
  }

  set turn(turn: Turn) {
    this.turning = turn;
    this.ask((player, id) => player.setShape(id, this.fitting, turn));
  }

  /** How bright and contrasted the picture is drawn, by the application
   *  too, as a share of what comes. */
  get brightness() {
    return this.lightness;
  }

  set brightness(brightness: number) {
    this.lightness = brightness;
    this.ask((player, id) => player.setLight?.(id, brightness, this.contrasting));
  }

  get contrast() {
    return this.contrasting;
  }

  set contrast(contrast: number) {
    this.contrasting = contrast;
    this.ask((player, id) => player.setLight?.(id, this.lightness, contrast));
  }

  get videoWidth() {
    return this.told.width;
  }

  get videoHeight() {
    return this.told.height;
  }

  get error(): MediaError | null {
    const message = this.told.error;
    return message === null
      ? null
      : {
          code: MediaError.MEDIA_ERR_DECODE,
          message,
          MEDIA_ERR_ABORTED: MediaError.MEDIA_ERR_ABORTED,
          MEDIA_ERR_NETWORK: MediaError.MEDIA_ERR_NETWORK,
          MEDIA_ERR_DECODE: MediaError.MEDIA_ERR_DECODE,
          MEDIA_ERR_SRC_NOT_SUPPORTED: MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED,
        };
  }

  get readyState() {
    return this.told.duration === null ? HTMLMediaElement.HAVE_NOTHING : HTMLMediaElement.HAVE_ENOUGH_DATA;
  }

  get networkState() {
    return this.opened ? HTMLMediaElement.NETWORK_LOADING : HTMLMediaElement.NETWORK_EMPTY;
  }

  play() {
    this.autoplay = true;
    this.ask((player, id) => player.play(id));
    return Promise.resolve();
  }

  pause() {
    this.autoplay = false;
    this.ask((player, id) => player.pause(id));
  }

  /** The application reads every container and playlist the server sends;
   *  what it decodes inside them is in its profile. */
  canPlayType() {
    return "probably";
  }

  getVideoPlaybackQuality(): VideoPlaybackQuality {
    return {
      creationTime: performance.now(),
      totalVideoFrames: this.told.shown,
      droppedVideoFrames: this.told.dropped,
      corruptedVideoFrames: 0,
    };
  }

  requestPictureInPicture() {
    appPlayer?.pictureInPicture();
    return Promise.resolve();
  }
}

if (appPlayer) {
  customElements.define(PICTURE_TAG, NativePicture);
  customElements.define(WORDS_TAG, NativeWords);
}

type Tag<T> = React.HTMLAttributes<HTMLElement> & {
  ref?: React.Ref<T>;
  key?: React.Key;
  src?: string;
};

declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
      "melyxar-picture": Tag<NativePicture> & {
        fit?: Shape;
        turn?: Turn;
        brightness?: number;
        contrast?: number;
      };
      "melyxar-words": Tag<WordsHolder>;
    }
  }
}

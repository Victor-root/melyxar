/*
 * A song heard on its own, apart from the player: a cut listened to before
 * it is made, or lines of lyrics placed on the words they are sung on.
 * Nothing is counted as listened to, and the queue is left alone.
 *
 * A song the browser plays as it lies is moved within; one converted on the
 * way only ever begins where it was asked for, so moving it is asking for it
 * again from there.
 */

import { soundOf } from "./player/player";

export class HeardSong {
  private audio: HTMLAudioElement | null = null;
  /** Where the sound asked for begins in the song, and whether it can be
      moved within: unknown until the browser has read what it holds. */
  private began = 0;
  private movable: boolean | null = null;
  /** Where it stands while no sound is in hand. */
  private at = 0;
  /** Whether it is meant to play, which the sound does once it is read. */
  private wanted = false;
  private rate = 1;
  private volume = 1;

  constructor(
    private readonly song: string,
    /** Told when it starts or stops playing. */
    private readonly told: () => void = () => {},
  ) {}

  get playing(): boolean {
    const audio = this.audio;
    if (!audio || this.movable === null) {
      return this.wanted;
    }
    return !audio.paused && !audio.ended;
  }

  /** Where it has got to, in seconds from the start of the song. */
  position(): number {
    const audio = this.audio;
    if (!audio || this.movable === null) {
      return this.at;
    }
    return (this.movable ? 0 : this.began) + audio.currentTime;
  }

  /** Plays from `from`, or on from where it stands. */
  play(from?: number): void {
    this.wanted = true;
    if (from !== undefined) {
      this.seek(from);
    }
    if (!this.audio) {
      this.open(this.at);
    } else if (this.movable !== null) {
      void this.audio.play().catch(() => this.pause());
    }
    this.told();
  }

  pause(): void {
    this.wanted = false;
    this.audio?.pause();
    this.told();
  }

  seek(to: number): void {
    const at = Math.max(0, to);
    const audio = this.audio;
    if (!audio) {
      this.at = at;
    } else if (this.movable) {
      audio.currentTime = at;
    } else {
      // Not read yet, or converted on the way: asked for again from there.
      this.wanted = this.playing;
      this.open(at);
    }
  }

  /** How fast it plays, the voice kept at its own pitch. */
  setRate(rate: number): void {
    this.rate = rate;
    if (this.audio) {
      this.audio.playbackRate = rate;
    }
  }

  /** How loud it plays, from nought to one. */
  setVolume(volume: number): void {
    this.volume = volume;
    if (this.audio) {
      this.audio.volume = volume;
    }
  }

  /** Lets go of the sound, keeping where it stood. */
  stop(): void {
    this.at = this.position();
    this.wanted = false;
    this.letGo();
    this.told();
  }

  private open(start: number): void {
    this.letGo();
    const audio = new Audio(soundOf(this.song, start));
    this.audio = audio;
    this.at = start;
    this.began = start;
    audio.preservesPitch = true;
    audio.volume = this.volume;
    audio.addEventListener("loadedmetadata", () => {
      if (this.audio !== audio) {
        return;
      }
      this.movable = Number.isFinite(audio.duration);
      if (this.movable) {
        audio.currentTime = start;
      }
      audio.playbackRate = this.rate;
      if (this.wanted) {
        void audio.play().catch(() => this.pause());
      }
    });
    for (const change of ["play", "pause", "ended"]) {
      audio.addEventListener(change, () => {
        if (this.audio === audio) {
          this.wanted = !audio.paused && !audio.ended;
          this.told();
        }
      });
    }
  }

  private letGo(): void {
    const audio = this.audio;
    this.audio = null;
    this.movable = null;
    if (audio) {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    }
  }
}

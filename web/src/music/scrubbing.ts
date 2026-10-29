/*
 * A song heard at the pace of a hand dragging the playhead along it, as an
 * editing desk does: the song plays on without a break, its speed following
 * the hand, faster as it moves faster, slowing down to a stop like a tape
 * once it stands still, and backwards when it goes back.
 *
 * Where the sound is chases where the hand is: a gap between the two is made
 * up within a twelfth of a second, which is what sets the speed, and the
 * speed itself changes smoothly, so that a hand moving in small jumps is not
 * heard as a warble. The speed is worked out on a beat of its own, apart
 * from the hand, so the song slows down even when no move comes.
 *
 * The song is read once, whole, into memory: as one channel and at a lower
 * rate than it was recorded, which is plenty to tell a syllable by, and
 * backwards too the first time the hand goes back.
 */

import { soundOf } from "./player/player";

/** The fastest the song is heard while dragged, either way. */
const FASTEST = 4;

/** How soon the sound makes up the gap to the hand, in seconds. */
const CHASE_SECONDS = 0.08;

/** How soon the speed reaches the one the gap asks for, in seconds. */
const EASE_SECONDS = 0.03;

/** Under this speed the song is no longer heard: a hand at rest. */
const QUIET_BELOW = 0.04;

/** How often the speed is worked out, in milliseconds. */
const BEAT_MS = 10;

/** How long the sound takes to come in and go out, in seconds. */
const FADE_SECONDS = 0.01;

/** The rate the song is read at: a voice is told well below it. */
const READ_AT_RATE = 22_050;

/** The speed the song is heard at, `seconds` after it was heard at `rate`,
 *  for the hand to be `gap` seconds ahead of the sound: negative is
 *  backwards. */
export function speedAfter(rate: number, gap: number, seconds: number): number {
  const wanted = Math.min(Math.max(gap / CHASE_SECONDS, -FASTEST), FASTEST);
  return rate + (wanted - rate) * Math.min(1, seconds / EASE_SECONDS);
}

/** Whether the song is heard at this speed. */
export function heardAt(rate: number): boolean {
  return Math.abs(rate) >= QUIET_BELOW;
}

export class Scrubber {
  private context: AudioContext | null = null;
  private forwards: AudioBuffer | null = null;
  private backwards: AudioBuffer | null = null;
  private reading: Promise<void> | null = null;
  private stop = new AbortController();
  private loudness: GainNode | null = null;
  private source: AudioBufferSourceNode | null = null;
  private sourceGoesBack = false;
  /** Where the hand is, where the sound is, both in seconds, and how fast
      the sound goes. */
  private hand = 0;
  private heard = 0;
  private rate = 0;
  private beat = 0;
  private beatAt = 0;

  constructor(private readonly song: string) {}

  /** Reads the song into memory, once, ahead of the first drag. */
  read(): void {
    this.reading ??= (async () => {
      const answer = await fetch(soundOf(this.song, 0), { signal: this.stop.signal });
      const bytes = await answer.arrayBuffer();
      const context = new AudioContext({ sampleRate: READ_AT_RATE });
      this.context = context;
      const read = await context.decodeAudioData(bytes);
      this.forwards = oneChannel(context, read);
      this.loudness = context.createGain();
      this.loudness.gain.value = 0;
      this.loudness.connect(context.destination);
    })().catch(() => {
      // Not heard while dragged, and dragged all the same.
    });
  }

  /** The hand took the playhead at `at` seconds. */
  begin(at: number): void {
    const context = this.context;
    this.hand = at;
    this.heard = at;
    this.rate = 0;
    if (!context) {
      return;
    }
    void context.resume();
    window.clearInterval(this.beat);
    this.beatAt = context.currentTime;
    this.beat = window.setInterval(() => this.follow(), BEAT_MS);
  }

  /** The hand moved the playhead to `at` seconds. */
  to(at: number): void {
    this.hand = at;
  }

  /** The hand let go: the sound goes out. */
  end(): void {
    window.clearInterval(this.beat);
    this.rate = 0;
    this.fadeTo(0);
    this.letGoOfSource();
  }

  /** Lets go of the song for good. */
  close(): void {
    this.end();
    this.stop.abort();
    void this.context?.close();
    this.context = null;
    this.forwards = null;
    this.backwards = null;
  }

  /** On each beat: the sound moves on at its speed, and the speed is set
      anew from how far the hand is. */
  private follow(): void {
    const context = this.context;
    const forwards = this.forwards;
    if (!context || !forwards) {
      return;
    }
    const now = context.currentTime;
    const seconds = now - this.beatAt;
    this.beatAt = now;
    this.heard = Math.min(Math.max(this.heard + this.rate * seconds, 0), forwards.duration);
    this.rate = speedAfter(this.rate, this.hand - this.heard, seconds);
    if (!heardAt(this.rate)) {
      // Taken up again from where the sound stands once the hand moves.
      this.fadeTo(0);
      this.letGoOfSource();
      return;
    }
    const goesBack = this.rate < 0;
    if (!this.source || this.sourceGoesBack !== goesBack) {
      this.startSource(context, forwards, goesBack);
    }
    if (this.source) {
      this.source.playbackRate.value = Math.abs(this.rate);
    }
    this.fadeTo(1);
  }

  private startSource(context: AudioContext, forwards: AudioBuffer, goesBack: boolean): void {
    this.letGoOfSource();
    if (goesBack) {
      this.backwards ??= backwardsOf(context, forwards);
    }
    const source = context.createBufferSource();
    source.buffer = goesBack ? this.backwards : forwards;
    source.playbackRate.value = Math.abs(this.rate);
    source.connect(this.loudness ?? context.destination);
    const offset = goesBack ? forwards.duration - this.heard : this.heard;
    this.loudness?.gain.setValueAtTime(0, context.currentTime);
    source.start(context.currentTime, Math.min(Math.max(offset, 0), forwards.duration));
    this.source = source;
    this.sourceGoesBack = goesBack;
  }

  private letGoOfSource(): void {
    const source = this.source;
    this.source = null;
    if (source && this.context) {
      source.stop(this.context.currentTime + FADE_SECONDS);
    }
  }

  private fadeTo(level: number): void {
    const context = this.context;
    if (context && this.loudness) {
      this.loudness.gain.setTargetAtTime(level, context.currentTime, FADE_SECONDS / 3);
    }
  }
}

/** The song as one channel, its channels heard together. */
function oneChannel(context: AudioContext, read: AudioBuffer): AudioBuffer {
  const one = context.createBuffer(1, read.length, read.sampleRate);
  const into = one.getChannelData(0);
  for (let channel = 0; channel < read.numberOfChannels; channel += 1) {
    const from = read.getChannelData(channel);
    for (let at = 0; at < from.length; at += 1) {
      into[at] += from[at] / read.numberOfChannels;
    }
  }
  return one;
}

function backwardsOf(context: AudioContext, forwards: AudioBuffer): AudioBuffer {
  const backwards = context.createBuffer(1, forwards.length, forwards.sampleRate);
  const from = forwards.getChannelData(0);
  const into = backwards.getChannelData(0);
  for (let at = 0; at < from.length; at += 1) {
    into[at] = from[from.length - 1 - at];
  }
  return backwards;
}

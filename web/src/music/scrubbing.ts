/*
 * The song heard while the hand drags the playhead along it in the lyrics
 * editor, as an editing desk plays it: at the speed the hand moves, slowed
 * down, sped up and run backwards like a tape, and silent while the hand
 * holds still.
 *
 * The sound is made on the thread the browser makes sound on, one block of
 * samples at a time (scrub-worklet.ts). This hands it the song, read once
 * into memory as one channel and at a rate plenty to tell a syllable by,
 * and the path of the hand: every place it was seen at, the ones the
 * browser gathers between two frames included, each with the moment it was
 * there on the clock of the sound.
 */

import scrubWorklet from "./scrub-worklet?worker&url";
import type { HandPoint } from "./scrub-follow";
import type { ScrubWord } from "./scrub-worklet";
import { soundOf } from "./player/player";

/** The rate the song is read at: a voice is told well below it. */
const READ_AT_RATE = 22_050;

/** How much later than the time it takes to be heard the sound follows the
 *  hand, and how late it is at the least and at the most, in seconds: the
 *  hand is seen up to a frame late, and the sound is only ever smooth
 *  between two places the hand was seen at. */
const LATE_BEYOND = 0.04;
const LEAST_LATE = 0.06;
const MOST_LATE = 0.4;

/** What the sound says it plays, said this long ago or more, in
 *  milliseconds, was said before it was paused. */
const STALE_MS = 200;

/** Where the hand was along the song, in seconds, and when, in milliseconds
 *  as the page's events are stamped. */
export interface HandSeen {
  seconds: number;
  stamp: number;
}

/** How far ahead of the page's clock the clock of the sound is, in seconds,
 *  `now` milliseconds into the page's: from what the sound says it plays,
 *  and when, unless it said so before it was paused; then from where the
 *  sound is made, `making`, less the time it `takes` to be heard. */
export function clocksApart(heard: AudioTimestamp, now: number, making: number, takes: number): number {
  const { contextTime, performanceTime } = heard;
  if (contextTime !== undefined && performanceTime !== undefined && performanceTime > 0 && now - performanceTime < STALE_MS) {
    return contextTime - performanceTime / 1000;
  }
  return making - takes - now / 1000;
}

export class Scrubber {
  private context: AudioContext | null = null;
  private sound: AudioWorkletNode | null = null;
  private volume: GainNode | null = null;
  private level = 1;
  private reading: Promise<void> | null = null;
  private readonly stop = new AbortController();
  /** How far ahead of the page's clock the clock of the sound is, read when
      the playhead is taken and kept while it is held, in seconds. */
  private apart = 0;

  constructor(private readonly song: string) {}

  /** Reads the song into memory, once, ahead of the first drag. */
  read(): void {
    this.reading ??= (async () => {
      const answer = await fetch(soundOf(this.song, 0), { signal: this.stop.signal });
      const read = await new OfflineAudioContext(1, 1, READ_AT_RATE).decodeAudioData(await answer.arrayBuffer());
      if (this.stop.signal.aborted) {
        return;
      }
      const context = new AudioContext({ latencyHint: "interactive" });
      this.context = context;
      await context.audioWorklet.addModule(scrubWorklet);
      if (this.stop.signal.aborted) {
        return;
      }
      const sound = new AudioWorkletNode(context, "melyxar-scrub", { numberOfInputs: 0, outputChannelCount: [1] });
      const volume = context.createGain();
      volume.gain.value = this.level;
      sound.connect(volume).connect(context.destination);
      this.sound = sound;
      this.volume = volume;
      const samples = oneChannel(read);
      this.tell({ kind: "sound", samples, rate: read.sampleRate }, [samples.buffer]);
    })().catch(() => {
      // Not heard while dragged, and dragged all the same.
    });
  }

  /** How loud the song is heard, from nought to one. */
  setVolume(level: number): void {
    this.level = level;
    if (this.volume) {
      this.volume.gain.value = level;
    }
  }

  /** The hand took the playhead. */
  begin(hand: HandSeen): void {
    const context = this.context;
    if (!context) {
      return;
    }
    void context.resume();
    const takes = context.baseLatency + (context.outputLatency || 0);
    this.apart = clocksApart(context.getOutputTimestamp(), performance.now(), context.currentTime, takes);
    const late = Math.min(Math.max(takes + LATE_BEYOND, LEAST_LATE), MOST_LATE);
    this.tell({ kind: "take", point: this.pointOf(hand), late });
  }

  /** Every place the hand was seen at since, the latest last. */
  to(hands: readonly HandSeen[]): void {
    this.tell({ kind: "move", points: hands.map((hand) => this.pointOf(hand)) });
  }

  /** The hand let go: the sound goes out. */
  end(): void {
    this.tell({ kind: "let go" });
  }

  /** Lets go of the song for good. */
  close(): void {
    this.stop.abort();
    void this.context?.close();
    this.context = null;
    this.sound = null;
    this.volume = null;
  }

  private pointOf(hand: HandSeen): HandPoint {
    return { at: hand.seconds, time: hand.stamp / 1000 + this.apart };
  }

  private tell(word: ScrubWord, handedOver: Transferable[] = []): void {
    this.sound?.port.postMessage(word, handedOver);
  }
}

/** The song as one channel, its channels heard together. */
function oneChannel(read: AudioBuffer): Float32Array {
  const one = new Float32Array(read.length);
  for (let channel = 0; channel < read.numberOfChannels; channel += 1) {
    const from = read.getChannelData(channel);
    for (let at = 0; at < from.length; at += 1) {
      one[at] += from[at] / read.numberOfChannels;
    }
  }
  return one;
}

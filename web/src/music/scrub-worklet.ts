/*
 * The sound of the playhead dragged in the lyrics editor, made on the thread
 * the browser makes sound on: the page hands over the song once, then where
 * the hand goes (scrubbing.ts), and each block of samples is worked out by
 * the follower of the hand (scrub-follow.ts), never late for the speakers
 * however busy the page is.
 */

import { ScrubFollower } from "./scrub-follow";
import type { HandPoint } from "./scrub-follow";

/* What the browser gives the thread that makes sound, which the types of a
   page do not know. */
declare const sampleRate: number;
declare const currentTime: number;
declare class AudioWorkletProcessor {
  readonly port: MessagePort;
}
declare function registerProcessor(name: string, processor: new () => AudioWorkletProcessor): void;

/** What the page tells the thread that makes the sound. */
export type ScrubWord =
  | { kind: "sound"; samples: Float32Array; rate: number }
  | { kind: "take"; point: HandPoint; late: number }
  | { kind: "move"; points: HandPoint[] }
  | { kind: "let go" };

class ScrubProcessor extends AudioWorkletProcessor {
  private follower: ScrubFollower | null = null;

  constructor() {
    super();
    this.port.onmessage = ({ data }: MessageEvent<ScrubWord>) => {
      if (data.kind === "sound") {
        this.follower = new ScrubFollower(data.samples, data.rate, sampleRate);
      } else if (data.kind === "take") {
        this.follower?.take(data.point, data.late);
      } else if (data.kind === "move") {
        this.follower?.move(data.points);
      } else {
        this.follower?.letGo();
      }
    };
  }

  process(_inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
    this.follower?.render(outputs[0][0], currentTime);
    return true;
  }
}

registerProcessor("melyxar-scrub", ScrubProcessor);

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { lookWhileSeen } from "./polling";

let hidden = false;
let listeners: Array<() => void> = [];

beforeEach(() => {
  vi.useFakeTimers();
  hidden = false;
  listeners = [];
  vi.stubGlobal("window", { setInterval, clearInterval });
  vi.stubGlobal("document", {
    get hidden() {
      return hidden;
    },
    addEventListener: (_: string, listener: () => void) => listeners.push(listener),
    removeEventListener: (_: string, listener: () => void) => {
      listeners = listeners.filter((one) => one !== listener);
    },
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("looking while the page is seen", () => {
  it("looks on every beat while the page is shown", () => {
    const look = vi.fn();
    lookWhileSeen(look, 1000);

    vi.advanceTimersByTime(3000);

    expect(look).toHaveBeenCalledTimes(3);
  });

  it("says nothing while the page is hidden", () => {
    const look = vi.fn();
    lookWhileSeen(look, 1000);
    hidden = true;

    vi.advanceTimersByTime(5000);

    expect(look).not.toHaveBeenCalled();
  });

  it("looks once when the page comes back", () => {
    const look = vi.fn();
    lookWhileSeen(look, 1000);
    hidden = true;
    vi.advanceTimersByTime(5000);
    hidden = false;

    for (const listener of listeners) {
      listener();
    }

    expect(look).toHaveBeenCalledTimes(1);
  });

  it("does not look when the page goes away", () => {
    const look = vi.fn();
    lookWhileSeen(look, 1000);
    hidden = true;

    for (const listener of listeners) {
      listener();
    }

    expect(look).not.toHaveBeenCalled();
  });

  it("stops for good once told to", () => {
    const look = vi.fn();
    const stop = lookWhileSeen(look, 1000);
    stop();

    vi.advanceTimersByTime(5000);

    expect(look).not.toHaveBeenCalled();
    expect(listeners).toHaveLength(0);
  });
});

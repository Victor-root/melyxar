import { describe, expect, it, vi } from "vitest";
import { createMarkStore } from "./marks-store";

describe("the marks store", () => {
  it("keeps what was said about a work and tells whoever listens", () => {
    const store = createMarkStore();
    const heard = vi.fn();
    store.subscribe(heard);

    store.say({ one: { favourite: true } });

    expect(store.of("one")).toEqual({ favourite: true });
    expect(heard).toHaveBeenCalledTimes(1);
  });

  it("adds to what was said before rather than replacing it", () => {
    const store = createMarkStore();
    store.say({ one: { favourite: true } });
    store.say({ one: { gone: true } });

    expect(store.of("one")).toEqual({ favourite: true, gone: true });
  });

  it("lets a mark be taken back by saying nothing in its place", () => {
    const store = createMarkStore();
    store.say({ one: { pinned: true } });
    store.say({ one: { pinned: undefined } });

    expect(store.of("one")?.pinned).toBeUndefined();
  });

  it("leaves the entry of a work nobody touched exactly as it was", () => {
    const store = createMarkStore();
    store.say({ one: { favourite: true }, two: { gone: true } });
    const two = store.of("two");

    store.say({ one: { favourite: false } });

    expect(store.of("two")).toBe(two);
  });

  it("says several works at once and tells once", () => {
    const store = createMarkStore();
    const heard = vi.fn();
    store.subscribe(heard);

    store.say({ one: { gone: true }, two: { gone: true } });

    expect(store.of("one")?.gone).toBe(true);
    expect(store.of("two")?.gone).toBe(true);
    expect(heard).toHaveBeenCalledTimes(1);
  });

  it("stops telling whoever stopped listening", () => {
    const store = createMarkStore();
    const heard = vi.fn();
    const stop = store.subscribe(heard);
    stop();

    store.say({ one: { gone: true } });

    expect(heard).not.toHaveBeenCalled();
  });

  it("hands out a new record at every change, which is what tells React", () => {
    const store = createMarkStore();
    const before = store.all();
    store.say({ one: { gone: true } });

    expect(store.all()).not.toBe(before);
  });
});

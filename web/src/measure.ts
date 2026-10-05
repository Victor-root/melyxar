/*
 * A recorder of the page's smoothness, asleep until somebody arms it.
 *
 * Typed into the browser's console on the machine where a page stutters,
 * which is the only place a stutter can be seen: `melyxar.measure()` arms it
 * for the next load of the page, `melyxar.report()` writes down what it saw
 * and puts it away again. Armed, it notes every frame drawn, the long frames
 * of the main thread with what kept them busy, every picture and question
 * the page sent, where the page stood as it was scrolled, what the person
 * did and what the page set moving next to every late frame, the events the
 * page was slow to answer, and what the machine is. Nothing leaves the
 * browser.
 *
 * Armed through the storage of the tab rather than the page, so that it is
 * still armed after the reload that makes a load cold.
 */

import { putOnTheClipboard } from "./clipboard";
import { describe, noteDoings } from "./measure-doings";
import type { Doing, Drawn, Pass, Scrolled, Slow, Span, Timed } from "./measure-report";
import {
  doingsAround,
  eventParts,
  frameStats,
  framesInView,
  framesLost,
  framesWhile,
  heldFor,
  heldUpByTheMainThread,
  latestBefore,
  passesOf,
  percentile,
  slowMoments,
} from "./measure-report";

const ARMED = "melyxar.measure";

/** A frame counts as drawn while scrolling when the page moved this recently. */
const WHILE_SCROLLING_MS = 150;

/** How far around a stalled frame what happened is looked for. */
const AROUND_MS = 120;

/** How many of the worst frames are written out one by one. */
const WORST_FRAMES = 40;

/** How many things the person did are named beside a stalled frame. */
const DOINGS_NAMED = 5;

/** The browser reports an event only when it took this long to answer, which
 *  is the shortest it allows. */
const SLOW_EVENT_MS = 16;

/** How many of the slowest events are written out one by one. */
const SLOW_EVENTS = 15;

/** A long frame of the main thread, as the browser describes it. */
interface LongFrame {
  startTime: number;
  duration: number;
  blockingDuration: number;
  renderStart: number;
  styleAndLayoutStart: number;
  scripts: {
    invoker: string;
    sourceURL: string;
    sourceFunctionName: string;
    sourceCharPosition: number;
    duration: number;
    forcedStyleAndLayoutDuration: number;
  }[];
}

interface Loaded {
  at: number;
  src: string;
  natural: number;
}

interface Recording {
  frames: Drawn[];
  scrolls: Scrolled[];
  /** When a row was scrolled sideways. */
  sideways: number[];
  loaded: Loaded[];
  longFrames: LongFrame[];
  /** What the person did and the page set moving, as it happened. */
  doings: Doing[];
  /** The page that was shown from each moment on. */
  pages: Doing[];
  /** When the page was out of view, where no frame is drawn. */
  away: Span[];
  slow: Slow[];
  largestPaint: number;
  shifts: number;
}

let recording: Recording | null = null;

declare global {
  interface Window {
    melyxar: {
      measure: () => void;
      report: () => string;
      last: string;
    };
  }
}

/** Offers the recorder to the console, and starts it when it was armed. */
export function setUpMeasuring() {
  window.melyxar = { measure, report, last: "" };
  let armed = false;
  try {
    armed = sessionStorage.getItem(ARMED) !== null;
  } catch {
    // A tab that keeps nothing cannot have been armed.
  }
  if (armed) {
    begin();
    console.info(
      "melyxar: measuring since this page loaded. Use the page as usual (scroll, hover, click, open pages), then run melyxar.report().",
    );
  }
}

function measure() {
  try {
    sessionStorage.setItem(ARMED, "yes");
  } catch {
    console.warn("melyxar: this tab keeps nothing, so the recorder cannot be armed.");
    return;
  }
  console.info(
    "melyxar: armed. Reload now: Ctrl+Shift+R for a cold load, F5 for a load from the cache.",
  );
}

function begin() {
  const now: Recording = {
    frames: [],
    scrolls: [],
    sideways: [],
    loaded: [],
    longFrames: [],
    doings: [],
    pages: [{ at: 0, what: location.pathname }],
    away: [],
    slow: [],
    largestPaint: 0,
    shifts: 0,
  };
  recording = now;
  const note = noteDoings(now.doings);
  performance.setResourceTimingBufferSize(5000);

  /* How long the main thread holds each frame, however short. The browser
     only describes frames of more than fifty milliseconds, and a frame lost
     while scrolling is one of thirty three: without this, every one of them
     read as the main thread being free. A message posted from the recorder's
     turn in a frame is only answered once that frame has been painted. */
  const painted = new MessageChannel();
  const waiting: { drawn: Drawn; turn: number }[] = [];
  painted.port1.onmessage = () => {
    const one = waiting.shift();
    if (one) {
      one.drawn.painting = performance.now() - one.turn;
    }
  };

  let last = performance.now();
  let page = location.pathname;
  const tick = (at: number) => {
    if (recording !== now) {
      return;
    }
    if (location.pathname !== page) {
      page = location.pathname;
      now.pages.push({ at, what: page });
      note(at, `page ${page}`);
    }
    const turn = performance.now();
    const drawn: Drawn = { at, gap: at - last, before: Math.max(0, turn - at) };
    now.frames.push(drawn);
    waiting.push({ drawn, turn });
    painted.port2.postMessage(null);
    last = at;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);

  document.addEventListener(
    "scroll",
    (event) => {
      const box = event.target;
      if (!(box instanceof HTMLElement)) {
        return;
      }
      if (box.classList.contains("shell-scroll")) {
        now.scrolls.push({ at: performance.now(), top: box.scrollTop });
      } else {
        now.sideways.push(performance.now());
      }
    },
    { capture: true, passive: true },
  );
  document.addEventListener("visibilitychange", () => {
    const at = performance.now();
    if (document.hidden) {
      now.away.push({ from: at, to: Infinity });
    } else if (now.away.length > 0) {
      now.away[now.away.length - 1].to = at;
    }
  });
  document.addEventListener(
    "load",
    (event) => {
      if (event.target instanceof HTMLImageElement) {
        now.loaded.push({
          at: performance.now(),
          src: event.target.currentSrc,
          natural: event.target.naturalWidth,
        });
      }
    },
    { capture: true },
  );

  const watch = (
    type: string,
    heard: (entries: PerformanceEntryList) => void,
    options: { durationThreshold?: number } = {},
  ) => {
    if (PerformanceObserver.supportedEntryTypes.includes(type)) {
      new PerformanceObserver((list) => heard(list.getEntries())).observe({
        type,
        buffered: true,
        ...options,
      });
    }
  };
  watch(
    "event",
    (entries) => {
      for (const entry of entries as unknown as (Timed & { name: string; target: Node | null })[]) {
        now.slow.push({
          startTime: entry.startTime,
          duration: entry.duration,
          processingStart: entry.processingStart,
          processingEnd: entry.processingEnd,
          name: entry.name,
          what: describe(entry.target),
        });
      }
    },
    { durationThreshold: SLOW_EVENT_MS },
  );
  watch("long-animation-frame", (entries) =>
    now.longFrames.push(...(entries as unknown as LongFrame[])),
  );
  watch("largest-contentful-paint", (entries) => {
    now.largestPaint = entries[entries.length - 1]?.startTime ?? now.largestPaint;
  });
  watch("layout-shift", (entries) => {
    for (const entry of entries as unknown as { value: number; hadRecentInput: boolean }[]) {
      if (!entry.hadRecentInput) {
        now.shifts += entry.value;
      }
    }
  });
}

function report(): string {
  const now = recording;
  if (!now) {
    console.warn("melyxar: nothing recorded. Run melyxar.measure() and reload first.");
    return "";
  }
  recording = null;
  try {
    sessionStorage.removeItem(ARMED);
  } catch {
    // Already nothing kept.
  }

  const text = written(now);
  window.melyxar.last = text;
  console.log(text);
  void putOnTheClipboard(text).then((copied) =>
    console.info(
      copied
        ? "melyxar: the report is in the clipboard."
        : "melyxar: to copy the report, run copy(melyxar.last).",
    ),
  );
  return text;
}

const ms = (value: number) => `${Math.round(value)} ms`;
const seconds = (value: number) => `${(value / 1000).toFixed(2)} s`;

/** The end of an address, which is what tells two pictures apart. */
function shortName(url: string): string {
  const path = new URL(url, location.href).pathname;
  return path.split("/").slice(-2).join("/");
}

function graphicsCard(): string {
  try {
    const gl = document.createElement("canvas").getContext("webgl");
    const info = gl?.getExtension("WEBGL_debug_renderer_info");
    return gl && info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : "unknown";
  } catch {
    return "unknown";
  }
}

function serverTime(entry: PerformanceResourceTiming): number | null {
  return entry.serverTiming.find((timing) => timing.name === "total")?.duration ?? null;
}

function written(now: Recording): string {
  const lines: string[] = [];
  const say = (line = "") => lines.push(line);

  const gaps = now.frames.map((frame) => frame.gap).filter((gap) => gap > 0);
  const frame = percentile(gaps, 0.5);

  say("=== Melyxar smoothness report ===");
  say(`page ${location.pathname}, recorded for ${seconds(performance.now())}`);
  if (now.pages.length > 1) {
    say(`pages ${now.pages.map((one) => `${seconds(one.at)} ${one.what}`).join(", ")}`);
  }
  say();
  say("--- machine ---");
  const nav = navigator as Navigator & {
    deviceMemory?: number;
    userAgentData?: { brands: { brand: string; version: string }[]; platform: string };
  };
  say(`agent ${navigator.userAgent}`);
  if (nav.userAgentData) {
    say(
      `brands ${nav.userAgentData.brands.map((brand) => `${brand.brand} ${brand.version}`).join(", ")} on ${nav.userAgentData.platform}`,
    );
  }
  say(`graphics ${graphicsCard()}`);
  say(
    `threads ${navigator.hardwareConcurrency}, memory ${nav.deviceMemory ?? "?"} GB, pixel ratio ${devicePixelRatio}`,
  );
  say(
    `window ${innerWidth}x${innerHeight}, screen ${screen.width}x${screen.height}, one frame ${frame.toFixed(2)} ms (${(1000 / (frame || 1)).toFixed(0)} Hz)`,
  );
  const heap = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory;
  if (heap) {
    say(`script memory ${(heap.usedJSHeapSize / 1048576).toFixed(0)} MB`);
  }
  say(
    `page: ${document.querySelectorAll("*").length} elements, ${document.querySelectorAll(".card").length} cards, ${document.images.length} pictures, ${document.querySelectorAll(".row-track").length} rows`,
  );

  say();
  say("--- loading ---");
  const [navigation] = performance.getEntriesByType("navigation") as PerformanceNavigationTiming[];
  if (navigation) {
    say(
      `${navigation.type}, first byte ${ms(navigation.responseStart)}, ready ${ms(navigation.domContentLoadedEventEnd)}, loaded ${ms(navigation.loadEventEnd)}, protocol ${navigation.nextHopProtocol}`,
    );
  }
  const firstPaint = performance.getEntriesByName("first-contentful-paint")[0];
  say(
    `first paint ${firstPaint ? ms(firstPaint.startTime) : "?"}, largest paint ${ms(now.largestPaint)}, layout shift ${now.shifts.toFixed(3)}`,
  );

  const resources = performance.getEntriesByType("resource") as PerformanceResourceTiming[];
  const pictures = resources.filter((entry) => entry.name.includes("/api/v1/images/"));
  const questions = resources.filter(
    (entry) => entry.name.includes("/api/v1/") && !entry.name.includes("/api/v1/images/"),
  );
  const cached = pictures.filter((entry) => entry.transferSize === 0 && entry.decodedBodySize > 0);
  const took = pictures.map((entry) => entry.duration);
  const queued = pictures.map((entry) => Math.max(0, entry.requestStart - entry.startTime));
  const served = pictures.map(serverTime).filter((value): value is number => value !== null);
  say(
    `pictures ${pictures.length}, from the cache ${cached.length}, ${(pictures.reduce((sum, entry) => sum + entry.transferSize, 0) / 1048576).toFixed(1)} MB over the network`,
  );
  say(
    `  each took p50 ${ms(percentile(took, 0.5))}, p95 ${ms(percentile(took, 0.95))}, worst ${ms(Math.max(0, ...took))}`,
  );
  say(
    `  waited to be sent p50 ${ms(percentile(queued, 0.5))}, p95 ${ms(percentile(queued, 0.95))}; server p50 ${ms(percentile(served, 0.5))}, p95 ${ms(percentile(served, 0.95))}`,
  );
  say(`  protocols ${[...new Set(pictures.map((entry) => entry.nextHopProtocol))].join(", ")}`);
  const fetched = pictures.filter((entry) => entry.transferSize > 0);
  for (const entry of [...fetched].sort((a, b) => b.duration - a.duration).slice(0, 8)) {
    say(
      `  slow ${shortName(entry.name)} at ${seconds(entry.startTime)} took ${ms(entry.duration)} (waited ${ms(entry.requestStart - entry.startTime)}, server ${ms(serverTime(entry) ?? 0)}, ${(entry.transferSize / 1024).toFixed(0)} kB)`,
    );
  }
  for (const entry of questions) {
    say(
      `question ${new URL(entry.name).pathname} at ${seconds(entry.startTime)} took ${ms(entry.duration)}, server ${ms(serverTime(entry) ?? 0)}`,
    );
  }

  say();
  say("--- pictures as drawn ---");
  let larger = 0;
  let smaller = 0;
  let pixels = 0;
  for (const image of Array.from(document.images)) {
    if (!image.complete || image.naturalWidth === 0) {
      continue;
    }
    const drawn = image.getBoundingClientRect().width * devicePixelRatio;
    pixels += image.naturalWidth * image.naturalHeight;
    if (drawn > 0 && image.naturalWidth > drawn * 1.6) {
      larger += 1;
    } else if (drawn > 0 && image.naturalWidth < drawn * 0.9) {
      smaller += 1;
    }
  }
  say(
    `decoded ${(pixels / 1e6).toFixed(1)} megapixels (${((pixels * 4) / 1048576).toFixed(0)} MB once decoded), much larger than drawn ${larger}, smaller than drawn ${smaller}`,
  );

  say();
  say("--- scrolling ---");
  const shown = framesInView(now.frames, now.away);
  const passes = passesOf(now.scrolls);
  const whileScrolling = shown.filter((drawn) => {
    const scrolled = latestBefore(now.scrolls, drawn.at);
    return scrolled !== null && drawn.at - scrolled.at <= WHILE_SCROLLING_MS;
  });
  const passOf = (at: number): string => {
    const index = passes.findIndex((pass) => at >= pass.from && at <= pass.to + WHILE_SCROLLING_MS);
    return index < 0 ? "-" : `${index + 1}.${passes[index].direction}`;
  };
  const pageOf = (at: number) => latestBefore(now.pages, at)?.what ?? location.pathname;

  const mainThread = (frames: Drawn[]): string => {
    const held = frames.map(heldFor);
    const first = frames.map((drawn) => drawn.before);
    const second = frames.map((drawn) => drawn.painting ?? 0);
    return `main thread per frame p50 ${percentile(held, 0.5).toFixed(1)}, p95 ${percentile(held, 0.95).toFixed(1)}, worst ${ms(held.length > 0 ? Math.max(...held) : 0)} (scroll and hand p50 ${percentile(first, 0.5).toFixed(1)}; work, style, layout, paint p50 ${percentile(second, 0.5).toFixed(1)}, p95 ${percentile(second, 0.95).toFixed(1)})`;
  };
  const framesLine = (frames: Drawn[]): string => {
    const stats = frameStats(frames.map((drawn) => drawn.gap), frame);
    return `${stats.frames} frames, ${stats.lost} lost, ${stats.stalls} stalls, p50 ${stats.p50.toFixed(1)}, p95 ${stats.p95.toFixed(1)}, worst ${ms(stats.worst)} | ${mainThread(frames)}`;
  };

  const box = document.querySelector(".shell-scroll");
  say(
    `page ${box ? box.scrollHeight - box.clientHeight : "?"} points of scrolling, sideways scrolls of rows ${now.sideways.length}`,
  );
  passes.forEach((pass: Pass, index) => {
    const inside = whileScrolling.filter(
      (drawn) => drawn.at >= pass.from && drawn.at <= pass.to + WHILE_SCROLLING_MS,
    );
    say(
      `pass ${index + 1} ${pass.direction} ${Math.round(pass.startTop)} -> ${Math.round(pass.endTop)} from ${seconds(pass.from)} for ${seconds(pass.to - pass.from)}: ${framesLine(inside)}`,
    );
  });

  /* Rows scrolled sideways, which the passes above do not see: they only
     follow the page up and down. */
  const scrollingFrames = new Set(whileScrolling);
  const sideways = framesWhile(shown, now.sideways, WHILE_SCROLLING_MS).filter(
    (drawn) => !scrollingFrames.has(drawn),
  );
  const sidewaysFrames = new Set(sideways);
  if (sideways.length > 0) {
    const arrived = now.loaded.filter((loaded) =>
      sideways.some((drawn) => loaded.at >= drawn.at - drawn.gap && loaded.at <= drawn.at),
    ).length;
    say(`rows sideways: ${framesLine(sideways)} | pictures arrived meanwhile ${arrived}`);
  }

  const overlapping = (from: number, to: number) =>
    now.longFrames.filter((long) => long.startTime < to && long.startTime + long.duration > from);
  /** What held a late frame: the main thread, or something else, and what the
   *  person and the page were doing at the time. */
  const whatHeld = (drawn: Drawn): string => {
    const from = drawn.at - drawn.gap;
    const busy = overlapping(from, drawn.at);
    const near = (at: number) => at >= from - AROUND_MS && at <= drawn.at;
    const arrived = now.loaded.filter((loaded) => near(loaded.at)).length;
    const ended = resources.filter((entry) => near(entry.responseEnd)).length;
    const previous = now.frames[now.frames.indexOf(drawn) - 1];
    const before = previous
      ? `frame before held ${ms(heldFor(previous))} (scroll ${ms(previous.before)}, work+style+layout+paint ${ms(previous.painting ?? 0)})`
      : "no frame before";
    const main =
      busy.length === 0
        ? heldUpByTheMainThread(previous, frame)
          ? `main thread, ${before}`
          : `main thread light, ${before}: compositor, pictures or graphics card`
        : busy
            .map((long) => {
              const script = long.scripts.reduce((sum, one) => sum + one.duration, 0);
              const layout = long.startTime + long.duration - (long.styleAndLayoutStart || long.startTime + long.duration);
              const render = long.startTime + long.duration - (long.renderStart || long.startTime + long.duration);
              return `main ${ms(long.duration)} (script ${ms(script)}, render ${ms(render)}, style+layout ${ms(layout)})`;
            })
            .join(" + ");
    const doing = doingsAround(now.doings, from, drawn.at, AROUND_MS, DOINGS_NAMED);
    return `${main} | doing ${doing.length > 0 ? doing.join("; ") : "nothing noted"} | pictures arrived ${arrived}, answers ended ${ended}`;
  };
  const worstOf = (frames: Drawn[]): Drawn[] =>
    frames
      .filter((drawn) => drawn.gap > frame * 1.5)
      .sort((a, b) => b.gap - a.gap)
      .slice(0, WORST_FRAMES)
      .sort((a, b) => a.at - b.at);

  say();
  say("--- worst frames while scrolling ---");
  for (const drawn of worstOf([...whileScrolling, ...sideways])) {
    const where = sidewaysFrames.has(drawn) ? "rows sideways" : `pass ${passOf(drawn.at)}`;
    say(
      `${seconds(drawn.at)} ${where} top ${latestBefore(now.scrolls, drawn.at)?.top ?? "?"}: ${ms(drawn.gap)} (${framesLost(drawn.gap, frame)} lost) | ${whatHeld(drawn)}`,
    );
  }

  /* Everything the passes above leave out: the pointer crossing cards, the
     hero turning, a page opening, a menu unfolding. */
  say();
  say("--- everything else: frames drawn while nothing was scrolling ---");
  const others = shown.filter((drawn) => !scrollingFrames.has(drawn) && !sidewaysFrames.has(drawn));
  say(framesLine(others));
  for (const drawn of worstOf(others)) {
    say(
      `${seconds(drawn.at)} page ${pageOf(drawn.at)}: ${ms(drawn.gap)} (${framesLost(drawn.gap, frame)} lost) | ${whatHeld(drawn)}`,
    );
  }

  say();
  say("--- slow answers to the hand ---");
  const moments = slowMoments(now.slow);
  say(
    `${moments.length} movements of the hand took ${SLOW_EVENT_MS} ms or more to be drawn (the browser rounds to 8 ms)`,
  );
  for (const one of [...moments].sort((a, b) => b.duration - a.duration).slice(0, SLOW_EVENTS)) {
    const parts = eventParts(one);
    say(
      `${seconds(one.startTime)} page ${pageOf(one.startTime)} ${one.name} on ${one.what}: ${ms(one.duration)} (waited ${ms(parts.waited)}, handlers ${ms(parts.handling)}, until drawn ${ms(parts.drawing)})`,
    );
  }

  say();
  say("--- long frames of the main thread ---");
  say(
    `${now.longFrames.length} long frames, ${ms(now.longFrames.reduce((sum, long) => sum + long.blockingDuration, 0))} blocking in all`,
  );
  for (const long of [...now.longFrames].sort((a, b) => b.duration - a.duration).slice(0, 15)) {
    const scripts = [...long.scripts]
      .sort((a, b) => b.duration - a.duration)
      .slice(0, 3)
      .map(
        (one) =>
          `${one.invoker || "?"} ${one.sourceFunctionName || ""}@${one.sourceURL ? shortName(one.sourceURL) : "?"}${one.sourceCharPosition >= 0 ? ` char ${one.sourceCharPosition}` : ""} ${ms(one.duration)}${one.forcedStyleAndLayoutDuration > 1 ? ` (forced layout ${ms(one.forcedStyleAndLayoutDuration)})` : ""}`,
      )
      .join("; ");
    const layout = long.startTime + long.duration - (long.styleAndLayoutStart || long.startTime + long.duration);
    say(
      `${seconds(long.startTime)} ${ms(long.duration)}, blocking ${ms(long.blockingDuration)}, style+layout ${ms(layout)}${scripts ? ` | ${scripts}` : ""}`,
    );
  }
  say("=== end ===");
  return lines.join("\n");
}

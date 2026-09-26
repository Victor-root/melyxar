/*
 * A recorder of the page's smoothness, asleep until somebody arms it.
 *
 * Typed into the browser's console on the machine where a page stutters,
 * which is the only place a stutter can be seen: `melyxar.measure()` arms it
 * for the next load of the page, `melyxar.report()` writes down what it saw
 * and puts it away again. Armed, it notes every frame drawn, the long frames
 * of the main thread with what kept them busy, every picture and question
 * the page sent, where the page stood as it was scrolled, and what the
 * machine is. Nothing leaves the browser.
 *
 * Armed through the storage of the tab rather than the page, so that it is
 * still armed after the reload that makes a load cold.
 */

import { putOnTheClipboard } from "./clipboard";
import type { Pass, Scrolled } from "./measure-report";
import { frameStats, framesLost, passesOf, percentile } from "./measure-report";

const ARMED = "melyxar.measure";

/** A frame counts as drawn while scrolling when the page moved this recently. */
const WHILE_SCROLLING_MS = 150;

/** How far around a stalled frame what happened is looked for. */
const AROUND_MS = 120;

/** How many of the worst frames are written out one by one. */
const WORST_FRAMES = 40;

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
  frames: { at: number; gap: number }[];
  scrolls: Scrolled[];
  sideways: number;
  loaded: Loaded[];
  longFrames: LongFrame[];
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
      "melyxar: measuring since this page loaded. Scroll, then run melyxar.report().",
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
    sideways: 0,
    loaded: [],
    longFrames: [],
    largestPaint: 0,
    shifts: 0,
  };
  recording = now;
  performance.setResourceTimingBufferSize(5000);

  let last = performance.now();
  const tick = (at: number) => {
    if (recording !== now) {
      return;
    }
    now.frames.push({ at, gap: at - last });
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
        now.sideways += 1;
      }
    },
    { capture: true, passive: true },
  );
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

  const watch = (type: string, heard: (entries: PerformanceEntryList) => void) => {
    if (PerformanceObserver.supportedEntryTypes.includes(type)) {
      new PerformanceObserver((list) => heard(list.getEntries())).observe({
        type,
        buffered: true,
      });
    }
  };
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
  const passes = passesOf(now.scrolls);
  const lastScrollBefore = (at: number) => {
    let found: Scrolled | null = null;
    for (const scrolled of now.scrolls) {
      if (scrolled.at > at) {
        break;
      }
      found = scrolled;
    }
    return found;
  };
  const whileScrolling = now.frames.filter((drawn) => {
    const scrolled = lastScrollBefore(drawn.at);
    return scrolled !== null && drawn.at - scrolled.at <= WHILE_SCROLLING_MS;
  });
  const passOf = (at: number): string => {
    const index = passes.findIndex((pass) => at >= pass.from && at <= pass.to + WHILE_SCROLLING_MS);
    return index < 0 ? "-" : `${index + 1}.${passes[index].direction}`;
  };
  const box = document.querySelector(".shell-scroll");
  say(
    `page ${box ? box.scrollHeight - box.clientHeight : "?"} points of scrolling, sideways scrolls of rows ${now.sideways}`,
  );
  passes.forEach((pass: Pass, index) => {
    const inside = whileScrolling
      .filter((drawn) => drawn.at >= pass.from && drawn.at <= pass.to + WHILE_SCROLLING_MS)
      .map((drawn) => drawn.gap);
    const stats = frameStats(inside, frame);
    say(
      `pass ${index + 1} ${pass.direction} ${pass.startTop} -> ${pass.endTop} from ${seconds(pass.from)} for ${seconds(pass.to - pass.from)}: ${stats.frames} frames, ${stats.lost} lost, ${stats.stalls} stalls, p50 ${stats.p50.toFixed(1)}, p95 ${stats.p95.toFixed(1)}, worst ${ms(stats.worst)}`,
    );
  });

  say();
  say("--- worst frames while scrolling ---");
  const overlapping = (from: number, to: number) =>
    now.longFrames.filter((long) => long.startTime < to && long.startTime + long.duration > from);
  const worst = [...whileScrolling]
    .filter((drawn) => drawn.gap > frame * 1.5)
    .sort((a, b) => b.gap - a.gap)
    .slice(0, WORST_FRAMES)
    .sort((a, b) => a.at - b.at);
  for (const drawn of worst) {
    const from = drawn.at - drawn.gap;
    const busy = overlapping(from, drawn.at);
    const near = (at: number) => at >= from - AROUND_MS && at <= drawn.at;
    const arrived = now.loaded.filter((loaded) => near(loaded.at)).length;
    const ended = resources.filter((entry) => near(entry.responseEnd)).length;
    const main =
      busy.length === 0
        ? "main thread free (compositor, pictures or graphics card)"
        : busy
            .map((long) => {
              const script = long.scripts.reduce((sum, one) => sum + one.duration, 0);
              const layout = long.startTime + long.duration - (long.styleAndLayoutStart || long.startTime + long.duration);
              const render = long.startTime + long.duration - (long.renderStart || long.startTime + long.duration);
              return `main ${ms(long.duration)} (script ${ms(script)}, render ${ms(render)}, style+layout ${ms(layout)})`;
            })
            .join(" + ");
    say(
      `${seconds(drawn.at)} pass ${passOf(drawn.at)} top ${lastScrollBefore(drawn.at)?.top ?? "?"}: ${ms(drawn.gap)} (${framesLost(drawn.gap, frame)} lost) | ${main} | pictures arrived ${arrived}, answers ended ${ended}`,
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
          `${one.invoker || "?"} ${one.sourceFunctionName || ""}@${one.sourceURL ? shortName(one.sourceURL) : "?"} ${ms(one.duration)}${one.forcedStyleAndLayoutDuration > 1 ? ` (forced layout ${ms(one.forcedStyleAndLayoutDuration)})` : ""}`,
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

/*
 * Any colour, chosen from a square of shades, a strip of hues or a typed code.
 *
 * The machine's own picker is drawn by the browser, in its own greys and its
 * own corners, and is the one thing on the appearance page that belongs to
 * something else. This one is dressed as every list of the interface is, and
 * follows the accent it is choosing as it moves.
 *
 * It is a button that opens a panel drawn at the end of the page and placed
 * against the window by hand, as the bar's lists are.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { fromHsv, hsvOf, parseHex } from "../colour";
import { useSettings } from "../settings";

const PANEL_WIDTH = 248;
const PANEL_HEIGHT = 300;
const WINDOW_AIR = 8;
/** How far an arrow key moves the square, and a shifted one. */
const STEP = 0.02;
const BIG_STEP = 0.1;

export function ColourPicker({
  colour,
  onPick,
  className,
}: {
  colour: string;
  onPick: (colour: string) => void;
  /** What the button wears, which is what makes it one of the swatches. */
  className: string;
}) {
  const { t } = useSettings();
  const [open, setOpen] = useState(false);
  const [under, setUnder] = useState<React.CSSProperties>({});
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (!open) {
      return;
    }
    const place = () => {
      const it = button.current?.getBoundingClientRect();
      if (!it) {
        return;
      }
      const left = Math.max(WINDOW_AIR, Math.min(it.right - PANEL_WIDTH, window.innerWidth - PANEL_WIDTH - WINDOW_AIR));
      const fitsBelow = it.bottom + WINDOW_AIR + PANEL_HEIGHT <= window.innerHeight;
      setUnder(
        fitsBelow
          ? { left, top: it.bottom + WINDOW_AIR }
          : { left, bottom: window.innerHeight - it.top + WINDOW_AIR },
      );
    };
    const shut = () => setOpen(false);
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", shut, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", shut, true);
    };
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const elsewhere = (event: MouseEvent) => {
      const on = event.target as Node;
      if (!button.current?.contains(on) && !panel.current?.contains(on)) {
        setOpen(false);
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        setOpen(false);
        button.current?.focus();
      }
    };
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
    };
  }, [open]);

  return (
    <>
      <button
        ref={button}
        type="button"
        className={className}
        title={t("me.accent_own")}
        aria-label={t("me.accent_own")}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
        style={{ ["--swatch" as string]: colour }}
      />
      {open &&
        createPortal(
          <div
            ref={panel}
            className="colour-picker"
            role="dialog"
            aria-label={t("me.accent_own")}
            style={under}
          >
            <Shades colour={colour} onPick={onPick} />
          </div>,
          document.body,
        )}
    </>
  );
}

function Shades({ colour, onPick }: { colour: string; onPick: (colour: string) => void }) {
  const { t } = useSettings();
  const [shade, setShade] = useState(() => hsvOf(colour));
  const [typed, setTyped] = useState(colour);
  const [typing, setTyping] = useState(false);
  const area = useRef<HTMLDivElement>(null);
  const sent = useRef(colour);

  /* A colour that came from elsewhere, a reset or another screen, is taken as
     it is. What this panel sent itself is not: reading it back would lose the
     hue of a grey and move the strip under the hand. */
  useEffect(() => {
    if (colour !== sent.current) {
      sent.current = colour;
      setShade(hsvOf(colour));
    }
    if (!typing) {
      setTyped(colour);
    }
  }, [colour, typing]);

  const move = (next: { hue: number; saturation: number; value: number }) => {
    const chosen = fromHsv(next.hue, next.saturation, next.value);
    setShade(next);
    sent.current = chosen;
    onPick(chosen);
  };

  const pointAt = (event: React.PointerEvent) => {
    const it = area.current?.getBoundingClientRect();
    if (!it) {
      return;
    }
    move({
      hue: shade.hue,
      saturation: Math.min(1, Math.max(0, (event.clientX - it.left) / it.width)),
      value: Math.min(1, Math.max(0, 1 - (event.clientY - it.top) / it.height)),
    });
  };

  const walk = (event: React.KeyboardEvent) => {
    const step = event.shiftKey ? BIG_STEP : STEP;
    const change: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, step],
      ArrowDown: [0, -step],
    };
    const by = change[event.key];
    if (!by) {
      return;
    }
    event.preventDefault();
    move({
      hue: shade.hue,
      saturation: Math.min(1, Math.max(0, shade.saturation + by[0])),
      value: Math.min(1, Math.max(0, shade.value + by[1])),
    });
  };

  const type = (text: string) => {
    setTyped(text);
    const read = parseHex(text);
    if (read) {
      sent.current = read;
      setShade(hsvOf(read));
      onPick(read);
    }
  };

  return (
    <>
      <div
        ref={area}
        className="colour-area"
        style={{ ["--hue" as string]: shade.hue, ["--swatch" as string]: colour }}
        role="group"
        tabIndex={0}
        aria-label={t("me.accent_shade")}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          pointAt(event);
        }}
        onPointerMove={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            pointAt(event);
          }
        }}
        onKeyDown={walk}
      >
        <span
          className="colour-thumb"
          style={{ left: `${shade.saturation * 100}%`, top: `${(1 - shade.value) * 100}%` }}
        />
      </div>
      <input
        type="range"
        className="colour-hue"
        min={0}
        max={360}
        step={1}
        value={Math.round(shade.hue)}
        aria-label={t("me.accent_hue")}
        onChange={(event) => move({ ...shade, hue: Number(event.target.value) })}
      />
      <div className="colour-code">
        <span className="colour-preview" style={{ ["--swatch" as string]: colour }} aria-hidden="true" />
        <input
          type="text"
          value={typed}
          maxLength={7}
          spellCheck={false}
          autoComplete="off"
          aria-label={t("me.accent_hex")}
          onChange={(event) => type(event.target.value)}
          onFocus={() => setTyping(true)}
          onBlur={() => {
            setTyping(false);
            setTyped(colour);
          }}
        />
      </div>
    </>
  );
}

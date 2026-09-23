/*
 * The pieces every page of the administration and of somebody's own settings
 * is built from: the head of a page, the panels under it, a setting on its
 * line, and the three controls a setting is made with.
 *
 * Drawn here once so that two pages never answer the same gesture two ways.
 * The controls are this interface's own rather than the browser's: a system
 * list or tick box is drawn by the machine, in its colours and its corners,
 * and it is the one thing on the page that belongs to something else.
 */

import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { ComponentType, ReactNode } from "react";
import { createPortal } from "react-dom";
import { ChevronDownIcon, TickIcon } from "../icons";
import type { IconProps } from "../icons";
import { insideTheRange } from "../readable";
import { useSettings } from "../settings";
import { useSection } from "./sectioned";

/**
 * The head of a page: what it is, and what it is for.
 *
 * The icon and the name come from the section the page is drawn in, so the
 * side bar and the head of the page can never name a page two ways.
 */
export function PageHead({ lead, actions }: { lead: string; actions?: ReactNode }) {
  const { t } = useSettings();
  const section = useSection();
  const SectionIcon = section.icon;

  return (
    <header className="page-head">
      <div className="page-head-line">
        <span className="page-head-mark" aria-hidden="true">
          <SectionIcon size={24} />
        </span>
        <div className="page-head-words">
          <h1>{t(section.label)}</h1>
          <p>{lead}</p>
        </div>
        {actions && <div className="page-head-actions">{actions}</div>}
      </div>
    </header>
  );
}

/**
 * A block of a page, headed by what it is about.
 *
 * One that is not wired to anything yet is drawn all the same, dimmed and
 * saying so: seeing where a thing will be is how the maintainer decides what
 * it should be.
 */
export function Panel({
  icon: PanelIcon,
  title,
  lead,
  action,
  soon,
  className,
  children,
}: {
  icon: ComponentType<IconProps>;
  title: string;
  lead?: string;
  action?: ReactNode;
  soon?: boolean;
  className?: string;
  children?: ReactNode;
}) {
  return (
    <section className={`panel${soon ? " panel-soon" : ""}${className ? ` ${className}` : ""}`}>
      <header className="panel-head">
        <span className="panel-mark" aria-hidden="true">
          <PanelIcon size={20} />
        </span>
        <div className="panel-words">
          <h2>
            {title}
            {soon && <Soon />}
          </h2>
          {lead && <p>{lead}</p>}
        </div>
        {action && <div className="panel-action">{action}</div>}
      </header>
      {children && <div className="panel-body">{children}</div>}
    </section>
  );
}

/** The word saying that something is drawn and not yet wired. */
export function Soon() {
  const { t } = useSettings();
  return <span className="soon">{t("admin.soon")}</span>;
}

/**
 * One setting: what it is and what it does on the left, the control on the
 * right. Said in full every time, since a setting nobody understands is a
 * setting nobody dares touch.
 */
export function Setting({
  label,
  why,
  soon,
  children,
}: {
  label: string;
  why?: ReactNode;
  soon?: boolean;
  children: ReactNode;
}) {
  return (
    <div className={`setting${soon ? " setting-soon" : ""}`}>
      <div className="setting-words">
        <span className="setting-label">
          {label}
          {soon && <Soon />}
        </span>
        {why && <span className="setting-why">{why}</span>}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

/** A switch: on or off, and nothing in between. */
export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** What it switches, for whoever cannot see the line it sits on. */
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      className="toggle"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-knob" aria-hidden="true" />
    </button>
  );
}

/**
 * One choice out of a short list, opened under the field.
 *
 * The list is drawn at the end of the page and placed against the window, for
 * the reason the menus of the bar at the top are: it is frosted glass, and
 * glass inside a panel frosts nothing but the panel.
 */
export function Picker<T extends string>({
  value,
  options,
  onPick,
  label,
  disabled,
}: {
  value: T;
  /** Each option as it is kept, and as it is read. */
  options: readonly (readonly [T, string])[];
  onPick: (value: T) => void;
  label: string;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const field = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [under, setUnder] = useState({ top: 0, left: 0, width: 0 });
  const listId = useId();

  useLayoutEffect(() => {
    if (!open) {
      return;
    }
    const place = () => {
      const it = field.current?.getBoundingClientRect();
      if (it) {
        setUnder({ top: it.bottom + 6, left: it.left, width: it.width });
      }
    };
    place();
    window.addEventListener("resize", place);
    // The page scrolls under a list that is fixed to the window, so it is
    // shut rather than left hanging where the field used to be. The list
    // scrolling through its own lines is not the page moving.
    const shut = (event: Event) => {
      if (!list.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
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
      if (!field.current?.contains(on) && !list.current?.contains(on)) {
        setOpen(false);
      }
    };
    const away = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        field.current?.focus();
      }
    };
    document.addEventListener("mousedown", elsewhere);
    document.addEventListener("keydown", away);
    // The chosen line takes the focus, so the arrow and tab keys start from
    // where the answer already is.
    list.current?.querySelector<HTMLButtonElement>("[aria-selected=true]")?.focus();
    return () => {
      document.removeEventListener("mousedown", elsewhere);
      document.removeEventListener("keydown", away);
    };
  }, [open]);

  const shown = options.find(([option]) => option === value)?.[1] ?? "";

  /* Up and down walk the lines, the way every list anybody has used does. */
  const walk = (event: React.KeyboardEvent) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }
    event.preventDefault();
    const lines = [...(list.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const at = lines.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "ArrowDown" ? at + 1 : at - 1;
    lines[(next + lines.length) % lines.length]?.focus();
  };

  return (
    <>
      <button
        ref={field}
        type="button"
        className={`picker${open ? " picker-open" : ""}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={`${label}: ${shown}`}
        disabled={disabled}
        onClick={() => setOpen((was) => !was)}
      >
        <span className="picker-value">{shown}</span>
        <ChevronDownIcon size={16} />
      </button>
      {open &&
        createPortal(
          <div
            ref={list}
            id={listId}
            role="listbox"
            aria-label={label}
            className="header-menu-list picker-list"
            style={{ top: under.top, left: under.left, minWidth: under.width }}
            onKeyDown={walk}
          >
            {options.map(([option, wording]) => {
              const chosen = option === value;
              return (
                <button
                  key={option}
                  type="button"
                  role="option"
                  aria-selected={chosen}
                  className={`header-menu-line${chosen ? " header-menu-line-on" : ""}`}
                  onClick={() => {
                    setOpen(false);
                    field.current?.focus();
                    if (!chosen) {
                      onPick(option);
                    }
                  }}
                >
                  <span className="picker-line-words">{wording}</span>
                  {chosen && <TickIcon size={16} />}
                </button>
              );
            })}
          </div>,
          document.body,
        )}
    </>
  );
}

/** A number chosen along a line, with what it comes to said beside it. */
export function Slider({
  value,
  min,
  max,
  step,
  onChange,
  label,
  shown,
  disabled,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
  label: string;
  /** The value as a person reads it. */
  shown: string;
  disabled?: boolean;
}) {
  // How far along the line is filled, for the part drawn in the accent.
  const filled = max > min ? ((value - min) / (max - min)) * 100 : 0;
  return (
    <span className="slider">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        aria-label={label}
        style={{ ["--filled" as string]: `${filled}%` }}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <span className="slider-value">{shown}</span>
    </span>
  );
}

/**
 * A count, with the range the server will keep it inside.
 *
 * The bounds are on the field as well as on the server, so somebody dragging
 * the arrows is stopped where the server would have stopped them rather than
 * being silently corrected afterwards.
 */
export function NumberField({
  value,
  min,
  max,
  onPick,
  label,
  disabled,
}: {
  value: number;
  min: number;
  max: number;
  onPick: (value: number) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <input
      type="number"
      className="field-line field-number"
      value={value}
      min={min}
      max={max}
      disabled={disabled}
      aria-label={label}
      onChange={(event) => {
        const asked = insideTheRange(Number(event.target.value), min, max);
        if (asked !== null) {
          onPick(asked);
        }
      }}
    />
  );
}

/**
 * One figure in a panel: what it is, how much, and a word under it.
 *
 * A figure not measured yet says so with a dash rather than a zero, which
 * would be a figure.
 */
export function Stat({
  icon: StatIcon,
  label,
  value,
  note,
  state,
}: {
  icon: ComponentType<IconProps>;
  label: string;
  value: ReactNode;
  note?: ReactNode;
  state?: State;
}) {
  return (
    <div className="stat">
      <span className="stat-mark" aria-hidden="true">
        <StatIcon size={20} />
      </span>
      <span className="stat-words">
        <span className="stat-label">{label}</span>
        <span className="stat-value">
          {value}
          {state && <span className={`state-dot state-${state}`} aria-hidden="true" />}
        </span>
        {note && <span className="stat-note">{note}</span>}
      </span>
    </div>
  );
}

/** What a state is, by name. The colours are the theme's. */
export type State = "ok" | "attention" | "trouble";

/** A state said in a word, in its own colour. */
export function StatePill({ state, children }: { state: State; children: ReactNode }) {
  return (
    <span className={`state-pill state-${state}`}>
      <span className="state-dot" aria-hidden="true" />
      {children}
    </span>
  );
}

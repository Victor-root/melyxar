/*
 * A word in the corner of the screen: the one place every message of the
 * interface is drawn, whether it confirms an action a moment ago or carries
 * a notification the server kept.
 *
 * Each word leaves on its own after a while, a failure later than a success
 * since it has more to be read, unless it was asked to stay; any of them can
 * be sent away sooner. Three are shown at most, the others waiting their turn
 * with a word saying how many there are.
 */

import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { Link } from "react-router-dom";
import type { Picture } from "../api";
import { useShownPicture } from "../components/picture";
import { CloseIcon, InfoIcon, TickIcon, WarningIcon } from "../icons";
import { useSettings } from "../settings";
import type { Level } from "./api";

/** How long a word stays, by what it says. */
const STAYS_FOR_MS: Record<Level, number> = {
  ok: 6_000,
  attention: 9_000,
  trouble: 10_000,
  news: 8_000,
};

/** How many words are shown at once. */
const SHOWN_AT_MOST = 3;

export interface Toast {
  state: Level;
  title: string;
  detail?: string | null;
  /** Every size of a poster to wear, largest first. */
  poster?: Picture[];
  /** Where pressing it leads. */
  to?: string | null;
  /** Stays until it is closed. */
  sticky?: boolean;
  /** How long it stays; absent, its level decides. */
  shownForMs?: number | null;
  /** Told when it is pressed, before it leads anywhere. */
  onOpen?: () => void;
}

interface Shown extends Toast {
  id: number;
}

const ToastContext = createContext<(toast: Toast) => void>(() => {});

/** Says a word in the corner of the screen. */
export function useToast(): (toast: Toast) => void {
  return useContext(ToastContext);
}

/** What is shown full screen, if anything: the browser draws nothing else,
 *  so the words are put inside it. */
function useFullscreenHolder(): Element | null {
  const [holder, setHolder] = useState<Element | null>(() => document.fullscreenElement);
  useEffect(() => {
    const follow = () => setHolder(document.fullscreenElement);
    document.addEventListener("fullscreenchange", follow);
    return () => document.removeEventListener("fullscreenchange", follow);
  }, []);
  return holder;
}

export function Toasts({ children }: { children: ReactNode }) {
  const { t } = useSettings();
  const [shown, setShown] = useState<Shown[]>([]);
  const next = useRef(0);
  const fullscreen = useFullscreenHolder();

  const dismiss = useCallback((id: number) => {
    setShown((all) => all.filter((toast) => toast.id !== id));
  }, []);

  const say = useCallback((toast: Toast) => {
    next.current += 1;
    const id = next.current;
    setShown((all) => [...all, { ...toast, id }]);
  }, []);

  const waiting = shown.length - SHOWN_AT_MOST;
  return (
    <ToastContext.Provider value={say}>
      {children}
      {createPortal(
        <div className="toasts" role="status" aria-live="polite">
          {shown.slice(0, SHOWN_AT_MOST).map((toast) => (
            <ToastCard key={toast.id} toast={toast} onGone={dismiss} />
          ))}
          {waiting > 0 && <span className="toasts-more">{t("toast.more", { count: waiting })}</span>}
        </div>,
        fullscreen ?? document.body,
      )}
    </ToastContext.Provider>
  );
}

const MARKS = { ok: TickIcon, attention: WarningIcon, trouble: WarningIcon, news: InfoIcon };

function ToastCard({ toast, onGone }: { toast: Shown; onGone: (id: number) => void }) {
  const { t } = useSettings();
  const { id, state, sticky, shownForMs } = toast;
  const poster = useShownPicture(toast.poster ?? []);
  const staysFor = shownForMs ?? STAYS_FOR_MS[state];

  /* Held while the pointer or the keyboard is on it: the time it has left
     stops, to be read, and carries on from where it stood. */
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const held = hovered || focused;
  const timeLeft = useRef(staysFor);

  useEffect(() => {
    if (sticky || held) {
      return;
    }
    const startedAt = Date.now();
    const timer = window.setTimeout(() => onGone(id), timeLeft.current);
    return () => {
      window.clearTimeout(timer);
      timeLeft.current -= Date.now() - startedAt;
    };
  }, [id, sticky, held, onGone]);

  const Mark = MARKS[state];
  const words = (
    <>
      <strong>{toast.title}</strong>
      {toast.detail && <span className="toast-detail">{toast.detail}</span>}
    </>
  );
  const open = () => {
    toast.onOpen?.();
    onGone(id);
  };
  return (
    <div
      className={`toast toast-${state}${held ? " toast-held" : ""}`}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
    >
      {poster.picture ? (
        <img
          className="toast-poster"
          src={poster.picture.src}
          srcSet={poster.picture.srcSet}
          sizes="44px"
          alt=""
          onError={poster.itDidNotLoad}
        />
      ) : (
        <span className="toast-mark" aria-hidden="true">
          <Mark size={18} />
        </span>
      )}
      {toast.to ? (
        <Link className="toast-words toast-link" to={toast.to} onClick={open}>
          {words}
        </Link>
      ) : (
        <span className="toast-words">{words}</span>
      )}
      <button
        type="button"
        className="toast-close"
        aria-label={t("toast.close")}
        onClick={() => onGone(id)}
      >
        <CloseIcon size={16} />
      </button>
      {!sticky && (
        <span className="toast-time" aria-hidden="true">
          <span className="toast-time-left" style={{ animationDuration: `${staysFor}ms` }} />
        </span>
      )}
    </div>
  );
}

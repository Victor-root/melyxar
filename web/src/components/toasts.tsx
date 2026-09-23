/*
 * A word in the corner of the screen about what an action did.
 *
 * For what happens after somebody said yes and the panel they said it in has
 * closed: that it went through, checked, or that it did not and why. Each
 * word leaves on its own after a while, a failure later than a success since
 * it has more to be read, and any of them can be sent away sooner.
 */

import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { CloseIcon, TickIcon, WarningIcon } from "../icons";
import { useSettings } from "../settings";
import type { State } from "./panel";

/** How long a word stays, by what it says. */
const STAYS_FOR_MS: Record<State, number> = {
  ok: 6_000,
  attention: 9_000,
  trouble: 10_000,
};

export interface Toast {
  state: State;
  title: string;
  detail?: string;
}

interface Shown extends Toast {
  id: number;
}

const ToastContext = createContext<(toast: Toast) => void>(() => {});

/** Says a word in the corner of the screen. */
export function useToast(): (toast: Toast) => void {
  return useContext(ToastContext);
}

export function Toasts({ children }: { children: ReactNode }) {
  const [shown, setShown] = useState<Shown[]>([]);
  const next = useRef(0);

  const dismiss = useCallback((id: number) => {
    setShown((all) => all.filter((toast) => toast.id !== id));
  }, []);

  const say = useCallback((toast: Toast) => {
    next.current += 1;
    const id = next.current;
    setShown((all) => [...all, { ...toast, id }]);
  }, []);

  return (
    <ToastContext.Provider value={say}>
      {children}
      {createPortal(
        <div className="toasts" role="status" aria-live="polite">
          {shown.map((toast) => (
            <ToastCard key={toast.id} toast={toast} onGone={dismiss} />
          ))}
        </div>,
        document.body,
      )}
    </ToastContext.Provider>
  );
}

function ToastCard({ toast, onGone }: { toast: Shown; onGone: (id: number) => void }) {
  const { t } = useSettings();
  const { id, state } = toast;

  useEffect(() => {
    const timer = window.setTimeout(() => onGone(id), STAYS_FOR_MS[state]);
    return () => window.clearTimeout(timer);
  }, [id, state, onGone]);

  const Mark = state === "ok" ? TickIcon : WarningIcon;
  return (
    <div className={`toast toast-${state}`}>
      <span className="toast-mark" aria-hidden="true">
        <Mark size={18} />
      </span>
      <span className="toast-words">
        <strong>{toast.title}</strong>
        {toast.detail && <span>{toast.detail}</span>}
      </span>
      <button
        type="button"
        className="toast-close"
        aria-label={t("toast.close")}
        onClick={() => onGone(id)}
      >
        <CloseIcon size={16} />
      </button>
    </div>
  );
}

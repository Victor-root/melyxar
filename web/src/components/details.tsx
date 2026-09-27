/*
 * Writing a work's details by hand: its title, what is said about it, its
 * year and ratings, its genres and studios.
 *
 * Each field carries a lock. A field somebody changes is locked as they type,
 * because what they wrote is what they want kept, and a locked field is left
 * alone by every later look up. Pressing the lock opens it again, and the
 * next look up gives the field back to the provider.
 */

import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import type { DetailField, WrittenDetails } from "../api";
import { refusalOf, useAsked } from "../asking";
import { refusalKey } from "../i18n";
import { CloseIcon, LockIcon } from "../icons";
import { useSettings } from "../settings";
import { Modal } from "./modal";

export function DetailsDialog({
  workId,
  onClose,
  /** Said once the details are kept, so the screens showing the work read it
      again. */
  onChanged,
}: {
  workId: string;
  onClose: () => void;
  onChanged: () => void;
}) {
  const { t } = useSettings();
  const held = useAsked((signal) => api.details(workId, signal), [workId]);
  const [written, setWritten] = useState<WrittenDetails | null>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  useEffect(() => {
    if (held.answer && written === null) {
      setWritten(held.answer);
    }
  }, [held.answer, written]);

  /** Changes one field and locks it, since what was typed is what is wanted. */
  const change = <K extends DetailField>(field: K, value: WrittenDetails[K]) =>
    setWritten((before) =>
      before && {
        ...before,
        [field]: value,
        locked: before.locked.includes(field) ? before.locked : [...before.locked, field],
      },
    );

  const keep = async () => {
    if (!written) {
      return;
    }
    setBusy(true);
    setRefused(null);
    try {
      await api.writeDetails(workId, written);
      onChanged();
      onClose();
    } catch (error) {
      setRefused(refusalOf(error));
      setBusy(false);
    }
  };

  const text = (field: "title" | "tagline" | "age_rating") => ({
    value: written?.[field] ?? "",
    onChange: (event: React.ChangeEvent<HTMLInputElement>) =>
      change(field, field === "title" ? event.target.value : event.target.value || null),
  });

  const number = (field: "release_year" | "community_rating") => ({
    value: written?.[field] ?? "",
    onChange: (event: React.ChangeEvent<HTMLInputElement>) =>
      change(field, event.target.value === "" ? null : Number(event.target.value)),
  });

  const lock = (field: DetailField) =>
    written && (
      <Lock
        locked={written.locked.includes(field)}
        onToggle={() =>
          setWritten({
            ...written,
            locked: written.locked.includes(field)
              ? written.locked.filter((one) => one !== field)
              : [...written.locked, field],
          })
        }
      />
    );

  return (
    <Modal
      title={t("details.title")}
      onClose={onClose}
      footer={
        <button
          className="button button-accent button-large"
          onClick={keep}
          disabled={busy || !written || written.title.trim() === ""}
        >
          {t("details.keep")}
        </button>
      }
    >
      {held.failure && <p className="notice">{t(refusalKey(refusalOf(held.failure)))}</p>}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}

      {written && (
        <form
          className="details-form"
          onSubmit={(event) => {
            event.preventDefault();
            keep();
          }}
        >
          <p className="identify-said">{t("details.how")}</p>

          <Field label={t("details.name")} lock={lock("title")}>
            <input type="text" autoFocus {...text("title")} />
          </Field>
          <Field label={t("details.tagline")} lock={lock("tagline")}>
            <input type="text" {...text("tagline")} />
          </Field>
          <Field label={t("details.overview")} lock={lock("overview")}>
            <textarea
              rows={6}
              value={written.overview ?? ""}
              onChange={(event) => change("overview", event.target.value || null)}
            />
          </Field>

          <div className="details-row">
            <Field label={t("details.year")} lock={lock("release_year")}>
              <input type="number" min={1850} max={2200} step={1} {...number("release_year")} />
            </Field>
            <Field label={t("details.rating")} lock={lock("community_rating")}>
              <input type="number" min={0} max={10} step={0.1} {...number("community_rating")} />
            </Field>
            <Field label={t("details.age_rating")} lock={lock("age_rating")}>
              <input type="text" maxLength={12} {...text("age_rating")} />
            </Field>
          </div>

          <Field label={t("details.genres")} lock={lock("genres")}>
            <Names
              names={written.genres}
              label={t("details.genres")}
              onChange={(genres) => change("genres", genres)}
            />
          </Field>
          <Field label={t("details.studios")} lock={lock("studios")}>
            <Names
              names={written.studios}
              label={t("details.studios")}
              onChange={(studios) => change("studios", studios)}
            />
          </Field>

          <button type="submit" className="visually-hidden" tabIndex={-1}>
            {t("details.keep")}
          </button>
        </form>
      )}
    </Modal>
  );
}

/** One field, its name, and its lock beside the name. */
function Field({ label, lock, children }: { label: string; lock: ReactNode; children: ReactNode }) {
  return (
    <div className="identify-field details-field">
      <span className="details-head">
        <span className="identify-label">{label}</span>
        {lock}
      </span>
      {children}
    </div>
  );
}

/** Whether a look up may still change the field, pressed to say the other. */
function Lock({ locked, onToggle }: { locked: boolean; onToggle: () => void }) {
  const { t } = useSettings();
  const said = t(locked ? "details.locked" : "details.unlocked");
  return (
    <button
      type="button"
      className={`details-lock${locked ? " details-lock-on" : ""}`}
      aria-pressed={locked}
      aria-label={said}
      title={said}
      onClick={onToggle}
    >
      <LockIcon size={14} />
      <span>{t(locked ? "details.lock_on" : "details.lock_off")}</span>
    </button>
  );
}

/** A list of names, each taken off by its cross, and one more typed in. */
function Names({
  names,
  label,
  onChange,
}: {
  names: string[];
  label: string;
  onChange: (names: string[]) => void;
}) {
  const { t } = useSettings();
  const [typed, setTyped] = useState("");

  const add = () => {
    const name = typed.trim();
    if (name !== "" && !names.some((held) => held.toLowerCase() === name.toLowerCase())) {
      onChange([...names, name]);
    }
    setTyped("");
  };

  return (
    <div className="details-names">
      {names.map((name) => (
        <span key={name} className="details-name">
          {name}
          <button
            type="button"
            aria-label={t("details.remove", { name })}
            onClick={() => onChange(names.filter((held) => held !== name))}
          >
            <CloseIcon size={12} />
          </button>
        </span>
      ))}
      <input
        type="text"
        aria-label={label}
        placeholder={t("details.add")}
        value={typed}
        onChange={(event) => setTyped(event.target.value)}
        onBlur={add}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === ",") {
            event.preventDefault();
            add();
          }
        }}
      />
    </div>
  );
}

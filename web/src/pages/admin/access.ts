/*
 * What the page of security works out about the way in: where the browser
 * goes once it changes, and whether a certificate is close to its end.
 */

import type { AccessMode, AccessStatus } from "../../api";

/** How many days before its end a certificate is said to be running out. */
const RUNNING_OUT_DAYS = 30;

/**
 * Where this page goes once the way in has changed, or nothing when it can
 * stay: an encrypted server sends a page in the clear to https, and behind a
 * proxy the server stops answering https on its own port.
 */
export function addressAfter(
  mode: AccessMode,
  encrypted: boolean,
  location: Location | URL,
): string | null {
  const wanted = mode === "proxy" ? "http:" : "https:";
  if (location.protocol === wanted || (mode === "proxy" && !encrypted)) {
    return null;
  }
  return `${wanted}//${location.host}${location.pathname}${location.search}`;
}

/** Whether a certificate ends within a month, or has already. */
export function runningOut(notAfter: string, now: Date): boolean {
  return (
    new Date(notAfter).getTime() - now.getTime() <
    RUNNING_OUT_DAYS * 24 * 3600 * 1000
  );
}

/** What the administrator has chosen on the page, not yet sent. */
export interface Draft {
  mode: AccessMode;
  redirect: boolean;
  names: string;
  certificatePath: string;
  keyPath: string;
}

/** The names in a list separated by commas or spaces. */
export function namesOf(text: string): string[] {
  return text.split(/[\s,]+/).filter(Boolean);
}

/** What the page shows for a status the server gave. */
export function draftOf(status: AccessStatus): Draft {
  return {
    mode: status.mode,
    redirect: status.redirect_to_https,
    names: status.public_names.join(", "),
    certificatePath: status.certificate_path ?? "",
    keyPath: status.private_key_path ?? "",
  };
}

/** Whether the choices differ from what the server holds, so there is
 *  something to apply. Choices that mean nothing for the mode chosen, such as
 *  the paths of a certificate nobody asked for, do not count. */
export function changed(status: AccessStatus, draft: Draft): boolean {
  const kept = draftOf(status);
  const same = (a: string, b: string) => a.trim() === b.trim();
  if (draft.mode !== kept.mode) {
    return true;
  }
  if (
    draft.mode === "provided" &&
    !(
      same(draft.certificatePath, kept.certificatePath) &&
      same(draft.keyPath, kept.keyPath)
    )
  ) {
    return true;
  }
  if (draft.mode === "proxy") {
    return false;
  }
  return (
    draft.redirect !== kept.redirect ||
    (draft.mode === "self_signed" &&
      namesOf(draft.names).join(",") !== namesOf(kept.names).join(","))
  );
}

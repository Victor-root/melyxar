/*
 * Passkeys, between the server and the device: what the server hands over is
 * turned into what the browser asks the device with, and what the device
 * answers is turned back into what the server reads.
 *
 * The server writes every binary piece as base64 for the web, without
 * padding; the browser takes and gives bytes. Nothing else changes on the
 * way, and nothing here decides anything: the checking is the server's.
 *
 * In the Android application, the WebView is not asked: the application asks
 * Android itself, for the server it shows and no other, through a channel it
 * opens to that server alone. It takes and gives the same base64 the server
 * writes.
 */

import { api } from "./api";
import type { Account, Passkey, PasskeyChallenge } from "./api";
import { appWindow } from "./app-window";
import { deviceIdentity } from "./deviceIdentity";

/** The application's channel for passkeys, on an Android that has them. */
interface AppPasskeys {
  postMessage(message: string): void;
  addEventListener(kind: "message", listener: (event: MessageEvent<string>) => void): void;
  removeEventListener(kind: "message", listener: (event: MessageEvent<string>) => void): void;
}

declare global {
  interface Window {
    MelyxarPasskeys?: AppPasskeys;
  }
}

/**
 * Whether passkeys can be made and used from this page: a browser that knows
 * them, on a page in https under a domain name. An address, even in https,
 * is no site a browser makes a key for, and the server refuses it too.
 */
export function passkeysHere(): boolean {
  if (appWindow) {
    return window.MelyxarPasskeys !== undefined;
  }
  return (
    typeof window.PublicKeyCredential === "function" &&
    window.isSecureContext &&
    window.location.protocol === "https:" &&
    isADomainName(window.location.hostname)
  );
}

/** Whether a host is a name rather than an address. */
export function isADomainName(host: string): boolean {
  return host !== "" && !host.startsWith("[") && !/^[\d.]+$/.test(host);
}

/** Bytes, as the server writes them: base64 for the web, unpadded. */
export function toBase64Url(bytes: ArrayBuffer | Uint8Array): string {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let text = "";
  for (const byte of view) {
    text += String.fromCharCode(byte);
  }
  return btoa(text).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** The bytes the server wrote, padded or not. */
export function fromBase64Url(text: string): Uint8Array<ArrayBuffer> {
  const plain = text.replace(/-/g, "+").replace(/_/g, "/");
  const padded = plain + "=".repeat((4 - (plain.length % 4)) % 4);
  const decoded = atob(padded);
  const bytes = new Uint8Array(decoded.length);
  for (let at = 0; at < decoded.length; at += 1) {
    bytes[at] = decoded.charCodeAt(at);
  }
  return bytes;
}

interface WrittenCredential {
  id: string;
  [other: string]: unknown;
}

function withBytes<T extends WrittenCredential>(credentials: T[] | undefined) {
  return credentials?.map((credential) => ({ ...credential, id: fromBase64Url(credential.id) }));
}

/** The options to make a key with, from what the server wrote. */
export function creationOptions(written: PasskeyChallenge["options"]): PublicKeyCredentialCreationOptions {
  const key = written.publicKey as unknown as PublicKeyCredentialCreationOptions & {
    challenge: string;
    user: { id: string; name: string; displayName: string };
    excludeCredentials?: WrittenCredential[];
  };
  return {
    ...key,
    challenge: fromBase64Url(key.challenge),
    user: { ...key.user, id: fromBase64Url(key.user.id) },
    excludeCredentials: withBytes(key.excludeCredentials) as PublicKeyCredentialDescriptor[] | undefined,
  };
}

/** The options to ask for a key with, from what the server wrote. */
export function requestOptions(written: PasskeyChallenge["options"]): PublicKeyCredentialRequestOptions {
  const key = written.publicKey as unknown as PublicKeyCredentialRequestOptions & {
    challenge: string;
    allowCredentials?: WrittenCredential[];
  };
  return {
    ...key,
    challenge: fromBase64Url(key.challenge),
    allowCredentials: withBytes(key.allowCredentials) as PublicKeyCredentialDescriptor[] | undefined,
  };
}

type Written = string;

/** A key just made, as the server reads it, from the browser's bytes or the
 *  application's base64. */
function madeAsWritten(credential: {
  id: string;
  rawId: Written;
  type: string;
  response: { attestationObject: Written; clientDataJSON: Written };
}) {
  return {
    id: credential.id,
    rawId: credential.rawId,
    type: credential.type,
    response: {
      attestationObject: credential.response.attestationObject,
      clientDataJSON: credential.response.clientDataJSON,
    },
  };
}

/** A key's answer to a challenge, as the server reads it. */
function answerAsWritten(credential: {
  id: string;
  rawId: Written;
  type: string;
  response: { authenticatorData: Written; clientDataJSON: Written; signature: Written; userHandle?: Written | null };
}) {
  return {
    id: credential.id,
    rawId: credential.rawId,
    type: credential.type,
    response: {
      authenticatorData: credential.response.authenticatorData,
      clientDataJSON: credential.response.clientDataJSON,
      signature: credential.response.signature,
      userHandle: credential.response.userHandle ?? null,
    },
  };
}

/** What the browser made, in base64. */
function madeByTheBrowser(credential: PublicKeyCredential) {
  const response = credential.response as AuthenticatorAttestationResponse;
  return madeAsWritten({
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      attestationObject: toBase64Url(response.attestationObject),
      clientDataJSON: toBase64Url(response.clientDataJSON),
    },
  });
}

/** What the browser answered, in base64. */
function answeredByTheBrowser(credential: PublicKeyCredential) {
  const response = credential.response as AuthenticatorAssertionResponse;
  return answerAsWritten({
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      authenticatorData: toBase64Url(response.authenticatorData),
      clientDataJSON: toBase64Url(response.clientDataJSON),
      signature: toBase64Url(response.signature),
      userHandle: response.userHandle ? toBase64Url(response.userHandle) : null,
    },
  });
}

/**
 * What the device said when it did not answer, worded on this side.
 *
 * Put away, or no key chosen, is somebody changing their mind: nothing to
 * say. A key already on this device for the account is worth saying.
 */
export type DeviceRefusal = "cancelled" | "already_here" | "failed";

export class DeviceSaidNo extends Error {
  constructor(readonly why: DeviceRefusal) {
    super(why);
  }
}

function whyTheDevice(error: unknown): DeviceSaidNo {
  if (error instanceof DOMException) {
    if (error.name === "NotAllowedError" || error.name === "AbortError") {
      return new DeviceSaidNo("cancelled");
    }
    if (error.name === "InvalidStateError") {
      return new DeviceSaidNo("already_here");
    }
  }
  return new DeviceSaidNo("failed");
}

/** Asks the application, which answers once under the word it was asked
 *  with. */
function askTheApp(app: AppPasskeys, kind: "make" | "use", options: PasskeyChallenge["options"]) {
  const id = `${Date.now()}-${Math.random()}`;
  return new Promise<unknown>((done, fail) => {
    const heard = (event: MessageEvent<string>) => {
      let said: { id?: string; credential?: unknown; refused?: DeviceRefusal };
      try {
        said = JSON.parse(event.data);
      } catch {
        return;
      }
      if (said.id !== id) {
        return;
      }
      app.removeEventListener("message", heard);
      if (said.credential) {
        done(said.credential);
      } else {
        fail(new DeviceSaidNo(said.refused ?? "failed"));
      }
    };
    app.addEventListener("message", heard);
    app.postMessage(JSON.stringify({ id, kind, options: options.publicKey }));
  });
}

/** Asks the browser, and words what it said when it did not answer. */
async function askTheBrowser(asking: () => Promise<Credential | null>): Promise<PublicKeyCredential> {
  let answered: Credential | null;
  try {
    answered = await asking();
  } catch (error) {
    throw whyTheDevice(error);
  }
  if (!(answered instanceof PublicKeyCredential)) {
    throw new DeviceSaidNo("failed");
  }
  return answered;
}

/** Makes a passkey for the account signed in, after its password. */
export async function makeAPasskey(password: string, name: string): Promise<Passkey> {
  const started = await api.startPasskey(password);
  const app = window.MelyxarPasskeys;
  const made = app
    ? madeAsWritten((await askTheApp(app, "make", started.options)) as Parameters<typeof madeAsWritten>[0])
    : madeByTheBrowser(
        await askTheBrowser(() => navigator.credentials.create({ publicKey: creationOptions(started.options) })),
      );
  return api.finishPasskey(started.ceremony, name, made);
}

/** Signs in with whichever passkey the device offers for this site. */
export async function signInWithAPasskey(remember: boolean): Promise<Account> {
  const started = await api.passkeyChallenge();
  const app = window.MelyxarPasskeys;
  const answered = app
    ? answerAsWritten((await askTheApp(app, "use", started.options)) as Parameters<typeof answerAsWritten>[0])
    : answeredByTheBrowser(
        await askTheBrowser(() => navigator.credentials.get({ publicKey: requestOptions(started.options) })),
      );
  return api.signInWithPasskey(started.ceremony, answered, remember, deviceIdentity());
}

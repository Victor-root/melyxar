/*
 * Sending one file into a library, with how far it has got.
 *
 * Its own file and not the exchange of the rest of the interface: a file is
 * sent as it is, for as long as it takes, and the one thing anybody sending
 * a film wants is to see it move, which a fetch cannot tell.
 */

import { ApiError } from "./api";

/** Where a file goes: a folder under a root of the library, or the folder of
 *  an album. */
export type Destination = { root: string; folder: string } | { album: string };

/** What the server says of a file it took. */
export interface Sent {
  /** Where it is, under its root. */
  path: string;
  bytes: number;
}

/** The address a file is sent to. */
export function uploadAddress(library: string, destination: Destination, name: string): string {
  const query = new URLSearchParams({ name });
  if ("album" in destination) {
    query.set("album", destination.album);
  } else {
    query.set("root", destination.root);
    if (destination.folder.trim() !== "") {
      query.set("folder", destination.folder.trim());
    }
  }
  return `/api/v1/libraries/${library}/upload?${query.toString()}`;
}

/** Sends a file, telling how much of it is over, from nought to one. */
export function sendFile(
  library: string,
  destination: Destination,
  file: File,
  onProgress: (share: number) => void,
  signal?: AbortSignal,
): Promise<Sent> {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest();
    request.open("POST", uploadAddress(library, destination, file.name));
    request.setRequestHeader("accept", "application/json");
    request.setRequestHeader("content-type", "application/octet-stream");
    request.upload.onprogress = (event) => {
      if (event.lengthComputable && event.total > 0) {
        onProgress(event.loaded / event.total);
      }
    };
    request.onload = () => {
      if (request.status >= 200 && request.status < 300) {
        resolve(JSON.parse(request.responseText) as Sent);
        return;
      }
      let said: { code?: string; details?: { reason?: string } } | null = null;
      try {
        said = JSON.parse(request.responseText);
      } catch {
        said = null;
      }
      reject(new ApiError(said?.code ?? "generic", request.status, said?.details?.reason));
    };
    request.onerror = () => reject(new ApiError("unreachable", 0));
    request.onabort = () => reject(new DOMException("aborted", "AbortError"));
    signal?.addEventListener("abort", () => request.abort(), { once: true });
    request.send(file);
  });
}

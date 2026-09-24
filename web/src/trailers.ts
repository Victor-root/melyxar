/*
 * Where a trailer hosted elsewhere is played from, inside this interface.
 *
 * The sites a provider links trailers to offer a player of their own for
 * other pages to hold. It is played whole, with everything it draws: the
 * server fetches nothing, and nothing of it is hidden or driven from here.
 */

/** The address of the player a site offers other pages for this link, or
 *  nothing for a link to a site that offers none this interface knows. */
export function embedOf(link: string): string | null {
  let address: URL;
  try {
    address = new URL(link);
  } catch {
    return null;
  }
  const host = address.hostname.replace(/^(www|m)\./, "");

  const youTube =
    host === "youtube.com" && address.pathname === "/watch"
      ? address.searchParams.get("v")
      : host === "youtu.be"
        ? address.pathname.slice(1)
        : null;
  if (youTube) {
    // The address YouTube offers for embedding that sets no cookie on the
    // viewer until they play, which is the same player.
    return `https://www.youtube-nocookie.com/embed/${encodeURIComponent(youTube)}?autoplay=1&rel=0`;
  }

  const vimeo = host === "vimeo.com" ? address.pathname.slice(1) : null;
  if (vimeo && /^\d+$/.test(vimeo)) {
    return `https://player.vimeo.com/video/${vimeo}?autoplay=1`;
  }
  return null;
}

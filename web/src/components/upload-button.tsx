/*
 * The button that opens the window for putting files into a library, drawn
 * only for an account that may.
 */

import { useState } from "react";
import { useAccount } from "../account";
import type { Library } from "../api";
import { UploadIcon } from "../icons";
import { useSettings } from "../settings";
import { UploadDialog } from "./upload";

export function UploadButton({
  library,
  album,
  className,
  bare = false,
}: {
  library: Library;
  album?: { id: string; title: string };
  className?: string;
  /** Only the icon, its words said to whoever cannot see it. */
  bare?: boolean;
}) {
  const { t } = useSettings();
  const { account } = useAccount();
  const [open, setOpen] = useState(false);
  if (!account?.may_upload) {
    return null;
  }
  const said = t(album ? "upload.button_album" : "upload.button");
  return (
    <>
      <button
        type="button"
        className={className}
        onClick={() => setOpen(true)}
        aria-label={bare ? said : undefined}
        title={said}
      >
        <UploadIcon size={16} />
        {bare ? null : said}
      </button>
      {open && <UploadDialog library={library} album={album} onClose={() => setOpen(false)} />}
    </>
  );
}

/*
 * The picture of another account, set or taken away by an administrator.
 *
 * Chosen from the files of the device in hand and framed by hand, as one's
 * own is from the profile; the server then cuts and keeps it the same way.
 */

import { useRef, useState } from "react";
import { api } from "../../api";
import type { ApiError, ManagedAccount } from "../../api";
import { refusalAbout } from "../../asking";
import type { Told } from "../../asking";
import { Cropper } from "../../components/cropper";
import { ImageIcon } from "../../icons";
import { useSettings } from "../../settings";

/** What the server answers a request carrying more than it takes. */
const TOO_LARGE = 413;

/** The words of a refusal of a picture, for the key the interface says. */
export function pictureRefusalKey(failure: ApiError | null): string | null {
  if (!failure) {
    return null;
  }
  return failure.status === TOO_LARGE
    ? "refused.avatar.too_large"
    : refusalAbout(failure, "avatar");
}

/** The buttons that choose and remove the picture of an account, and the
 *  frame it is cropped in. What they do goes through `told`. */
export function PictureButtons({
  account,
  told,
}: {
  account: ManagedAccount;
  told: Told<[() => Promise<unknown>]>;
}) {
  const { t } = useSettings();
  const chooser = useRef<HTMLInputElement>(null);
  const [framing, setFraming] = useState<File | null>(null);

  return (
    <>
      <input
        ref={chooser}
        type="file"
        accept="image/jpeg,image/png,image/webp,image/gif"
        hidden
        onChange={(event) => {
          const image = event.target.files?.[0];
          // Emptied, so choosing the same file again is still a choice.
          event.target.value = "";
          if (image) {
            setFraming(image);
          }
        }}
      />
      <button
        className="button button-small"
        disabled={told.busy}
        onClick={() => chooser.current?.click()}
      >
        <ImageIcon size={15} />
        {t("users.photo")}
      </button>
      {account.avatar && (
        <button
          className="button button-small"
          disabled={told.busy}
          onClick={() =>
            void told.tell(() => api.removeAccountAvatar(account.id))
          }
        >
          {t("users.photo_remove")}
        </button>
      )}
      {framing && (
        <Cropper
          image={framing}
          onClose={() => setFraming(null)}
          onFramed={(square) => {
            setFraming(null);
            void told.tell(() => api.setAccountAvatar(account.id, square));
          }}
        />
      )}
    </>
  );
}

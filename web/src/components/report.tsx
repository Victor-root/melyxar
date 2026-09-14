/*
 * One button that puts everything worth asking about this installation on the
 * clipboard.
 *
 * The point is that nobody should have to know which question to ask. The
 * server renders one block holding the tools, the folders, what the library
 * holds and what the last pieces of work did, failures and their reasons
 * included, and this hands it over ready to paste.
 *
 * A browser only grants the clipboard on a secure page, and a server reached
 * at an address on the local network is not one. So there are two ways down
 * from the modern one, and the last of them always works: the text appears,
 * already selected, to be copied by hand.
 */

import { useRef, useState } from "react";
import { api } from "../api";
import { putOnTheClipboard } from "../clipboard";
import { useSettings } from "../settings";

type State = "idle" | "working" | "copied" | "shown" | "failed";

export function CopyReport() {
  const { t } = useSettings();
  const [state, setState] = useState<State>("idle");
  const [text, setText] = useState("");
  const shown = useRef<HTMLTextAreaElement>(null);

  const copy = async () => {
    setState("working");
    let report: string;
    try {
      report = await api.report();
    } catch {
      setState("failed");
      return;
    }
    setText(report);

    if (await putOnTheClipboard(report)) {
      setState("copied");
      return;
    }
    // Nothing could take it, so it is shown instead and selected, which leaves
    // one key press to do rather than a terminal to open.
    setState("shown");
    window.setTimeout(() => shown.current?.select(), 0);
  };

  return (
    // Once the report is on the page it needs the whole width to be readable,
    // which the row it sits in does not give it.
    <div className={state === "shown" ? "report report-open" : "report"}>
      <button className="button" onClick={copy} disabled={state === "working"}>
        {t(state === "copied" ? "report.copied" : "report.copy")}
      </button>
      {state === "failed" && <span className="notice-faint">{t("error.unreachable")}</span>}
      {state === "shown" && (
        <>
          <p className="notice-faint">{t("report.select")}</p>
          <textarea className="report-text" ref={shown} readOnly value={text} rows={16} />
        </>
      )}
    </div>
  );
}

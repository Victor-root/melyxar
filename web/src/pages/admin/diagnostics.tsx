/*
 * Everything the server can say about itself, in the words it would paste
 * into a message asking for help.
 *
 * Read on the page rather than only copied: the report is where a missing
 * tool, a folder nobody can write to or a film nobody could name is found,
 * and finding it should not take a clipboard and a text editor.
 */

import { useState } from "react";
import { api } from "../../api";
import { useAsked } from "../../asking";
import { handOver } from "../../copying";
import { PageHead, Panel } from "../../components/panel";
import { DiagnosticsIcon, RefreshIcon } from "../../icons";
import { useSettings } from "../../settings";

export function AdminDiagnostics() {
  const { t } = useSettings();
  const report = useAsked((signal) => api.report(signal));
  const [copied, setCopied] = useState<"no" | "yes" | "failed">("no");

  const copy = async () => {
    const handed = await handOver(api.report);
    // The report is already on the page: when no clipboard takes it, it is
    // there to be selected, and saying so is all that is left to do.
    setCopied(handed.how === "clipboard" ? "yes" : "failed");
  };

  return (
    <>
      <PageHead
        lead={t("admin.diagnostics_lead")}
        actions={
          <>
            <button className="button button-accent" onClick={copy} disabled={!report.answer}>
              {t(copied === "yes" ? "report.copied" : "report.copy")}
            </button>
            <button className="button" onClick={report.again} disabled={report.waiting}>
              <RefreshIcon size={16} />
              {t("admin.read_again")}
            </button>
          </>
        }
      />

      {copied === "failed" && <p className="panel-notice">{t("report.select")}</p>}

      <Panel icon={DiagnosticsIcon} title={t("admin.report")} lead={t("admin.report_lead")}>
        {report.failure && (
          <p className="panel-notice panel-notice-trouble">{t("error.unreachable")}</p>
        )}
        {report.answer && <pre className="log report-view">{report.answer}</pre>}
      </Panel>
    </>
  );
}

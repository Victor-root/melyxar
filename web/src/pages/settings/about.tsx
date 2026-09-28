/*
 * What Melyxar is, which one this server runs, under which licence, and where
 * the pictures and words about films come from, which the provider asks to be
 * said.
 */

import { useEffect, useState } from "react";
import { api } from "../../api";
import type { SystemInfo } from "../../api";
import { PageHead, Panel, Setting } from "../../components/panel";
import { InfoIcon, MelyxarMark, ServerIcon, ShieldIcon, TagIcon } from "../../icons";
import { useSettings } from "../../settings";

const REPOSITORY = "https://github.com/Victor-root/melyxar";
const ISSUES = `${REPOSITORY}/issues`;
const AUTHOR = "https://github.com/Victor-root";
const LICENCE = "https://www.gnu.org/licenses/agpl-3.0.html";
const PROVIDER = "https://www.themoviedb.org";
const ICONS = "https://tabler.io/icons";

export function MyAbout() {
  const { t } = useSettings();
  const [system, setSystem] = useState<SystemInfo | null>(null);

  useEffect(() => {
    const leaving = new AbortController();
    api
      .system(leaving.signal)
      .then(setSystem)
      .catch(() => {});
    return () => leaving.abort();
  }, []);

  return (
    <>
      <PageHead lead={t("about.lead")} />

      <Panel icon={InfoIcon} title="Melyxar" lead={t("about.melyxar_why")} wide>
        <div className="about-melyxar">
          <MelyxarMark size={56} />
          <p>
            {t("about.made_by_before")}
            <a href={AUTHOR} target="_blank" rel="noopener noreferrer">
              Victor-root
            </a>
            {t("about.made_by_after")}
          </p>
          <a className="button button-small" href={REPOSITORY} target="_blank" rel="noopener noreferrer">
            {t("about.source")}
          </a>
        </div>
        <Setting label={t("about.report")} why={t("about.report_why")}>
          <a className="button button-small" href={ISSUES} target="_blank" rel="noopener noreferrer">
            {t("about.report_open")}
          </a>
        </Setting>
      </Panel>

      <Panel icon={ServerIcon} title={t("about.server")}>
        <Setting label={t("about.server_name")}>
          <span className="about-value">{system?.server_name ?? "…"}</span>
        </Setting>
        <Setting label={t("about.version")}>
          <span className="about-value">{system?.version ?? "…"}</span>
        </Setting>
        <Setting label={t("about.api_version")}>
          <span className="about-value">{system?.api_version ?? "…"}</span>
        </Setting>
      </Panel>

      <Panel icon={ShieldIcon} title={t("about.licence")} lead={t("about.licence_why")}>
        <Setting label="GNU AGPL 3.0">
          <a className="button button-small" href={LICENCE} target="_blank" rel="noopener noreferrer">
            {t("about.read_licence")}
          </a>
        </Setting>
        <Setting label={t("about.icons")} why={t("about.icons_why")}>
          <a className="button button-small" href={ICONS} target="_blank" rel="noopener noreferrer">
            tabler.io
          </a>
        </Setting>
      </Panel>

      <Panel icon={TagIcon} title={t("about.metadata")} lead={t("attribution.tmdb")}>
        <Setting label="TMDB" why={t("about.metadata_why")}>
          <a className="button button-small" href={PROVIDER} target="_blank" rel="noopener noreferrer">
            themoviedb.org
          </a>
        </Setting>
      </Panel>
    </>
  );
}

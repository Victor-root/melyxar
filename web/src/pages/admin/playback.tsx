/*
 * Who is watching what, and how it reaches them: drawn now, wired with the
 * lot that follows what is being played.
 */

import { PageHead, Panel } from "../../components/panel";
import { HistoryIcon, PlaybackIcon } from "../../icons";
import { useSettings } from "../../settings";
import { Ghosts } from "./ghosts";

export function AdminPlayback() {
  const { t } = useSettings();
  return (
    <>
      <PageHead lead={t("admin.playback_lead")} />
      <Panel icon={PlaybackIcon} title={t("admin.playing")} lead={t("admin.playing_lead")} soon>
        <Ghosts
          heads={["admin.col.who", "admin.col.work", "admin.col.device", "admin.col.way", "admin.col.where"]}
        />
      </Panel>
      <Panel icon={HistoryIcon} title={t("admin.history")} lead={t("admin.history_lead")} soon>
        <Ghosts heads={["admin.col.when", "admin.col.who", "admin.col.work", "admin.col.way"]} />
      </Panel>
    </>
  );
}

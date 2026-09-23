/*
 * The administration: every tool that changes the server for everybody, in
 * one place with its own list down the side.
 *
 * Only for an administrator. Somebody else who types the address is told there
 * is nothing here, which is what the server tells them too.
 *
 * The state of the server is read here, once for the whole place, and looked
 * at again on a slow beat: the foot of the list says whether the server is up,
 * and the summary draws from the same answer rather than asking twice.
 */

import { createContext, useContext, useEffect } from "react";
import type { Asked } from "../../asking";
import { useAsked } from "../../asking";
import { api } from "../../api";
import type { Overview } from "../../api";
import { useAccount } from "../../account";
import { Sectioned } from "../../components/sectioned";
import type { SectionGroup } from "../../components/sectioned";
import {
  DeviceIcon,
  DiagnosticsIcon,
  FolderIcon,
  GraphicsCardIcon,
  JournalIcon,
  PeopleIcon,
  PlaybackIcon,
  ShieldIcon,
  SlidersIcon,
  SummaryIcon,
  TagIcon,
  TasksIcon,
} from "../../icons";
import { releaseOf } from "../../readable";
import { useSettings } from "../../settings";

/** How often the state of the server is looked at again while this is open. */
const LOOKED_AT_EVERY_MS = 15_000;

const SECTIONS: SectionGroup[] = [
  { sections: [{ path: "", icon: SummaryIcon, label: "admin.overview" }] },
  {
    label: "admin.group.content",
    sections: [
      { path: "libraries", icon: FolderIcon, label: "admin.libraries" },
      { path: "metadata", icon: TagIcon, label: "admin.metadata" },
    ],
  },
  {
    label: "admin.group.playback",
    sections: [
      { path: "playback", icon: PlaybackIcon, label: "admin.playback" },
      { path: "transcoding", icon: GraphicsCardIcon, label: "admin.transcoding" },
    ],
  },
  {
    label: "admin.group.access",
    sections: [
      { path: "users", icon: PeopleIcon, label: "admin.users", soon: true },
      { path: "devices", icon: DeviceIcon, label: "admin.devices", soon: true },
      { path: "security", icon: ShieldIcon, label: "admin.security", soon: true },
    ],
  },
  {
    label: "admin.group.system",
    sections: [
      { path: "tasks", icon: TasksIcon, label: "admin.tasks" },
      { path: "journal", icon: JournalIcon, label: "admin.journal" },
      { path: "diagnostics", icon: DiagnosticsIcon, label: "admin.diagnostics" },
      { path: "settings", icon: SlidersIcon, label: "admin.settings" },
    ],
  },
];

const OverviewContext = createContext<Asked<Overview> | null>(null);

/** The state of the server, as the administration last read it. */
export function useOverview(): Asked<Overview> {
  const overview = useContext(OverviewContext);
  if (!overview) {
    throw new Error("the state of the server is read inside the administration");
  }
  return overview;
}

export function AdminLayout() {
  const { t } = useSettings();
  const { account } = useAccount();

  if (!account?.is_administrator) {
    return (
      <main className="page">
        <p className="notice">{t("error.not_found")}</p>
      </main>
    );
  }
  return <TheAdministration />;
}

function TheAdministration() {
  const { t } = useSettings();
  const overview = useAsked((signal) => api.overview(signal));
  const { look } = overview;

  useEffect(() => {
    const timer = window.setInterval(look, LOOKED_AT_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [look]);

  const cut = overview.failure !== null;

  return (
    <OverviewContext.Provider value={overview}>
      <Sectioned
        base="/admin"
        place="admin.title"
        groups={SECTIONS}
        foot={
          <span className="side-state">
            <span className={`state-dot state-${cut ? "trouble" : "ok"}`} aria-hidden="true" />
            <span>
              {cut || !overview.answer
                ? t(cut ? "admin.unreachable" : "admin.title")
                : t("admin.foot", { version: releaseOf(overview.answer.version) })}
            </span>
          </span>
        }
      />
    </OverviewContext.Provider>
  );
}

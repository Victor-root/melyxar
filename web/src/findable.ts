/*
 * The settings and the administration, as the search field finds them.
 *
 * Each section with the words its cards and settings are named by, so a
 * setting is found by its name in the language in force, and the search leads
 * to its page and to the setting itself. Kept in step with the pages by a test
 * that reads them: a setting added without its line here fails it.
 */

/** Where settings are searched: somebody's own, or the server's. */
export type Area = "settings" | "admin";

/** One section of an area, and what it holds. */
export interface FindableSection {
  area: Area;
  /** Under the area's address, empty for the first. */
  path: string;
  /** The key of the section's name. */
  name: string;
  /** The pages it is drawn by, for the test that keeps this in step. */
  files: string[];
  /** The keys of the names of its cards and settings. */
  keys: string[];
}

export const FINDABLE: FindableSection[] = [
  {
    area: "settings",
    path: "",
    name: "me.profile",
    files: ["pages/settings/profile.tsx"],
    keys: ["settings.door", "settings.door_hide_me", "settings.avatar", "me.name", "me.password", "me.devices"],
  },
  {
    area: "settings",
    path: "appearance",
    name: "me.appearance",
    files: ["pages/settings/appearance.tsx"],
    keys: ["settings.appearance", "nav.theme", "nav.language", "me.accent", "settings.header", "settings.header_hides"],
  },
  {
    area: "settings",
    path: "home",
    name: "me.home",
    files: ["pages/settings/home.tsx"],
    keys: [
      "settings.banner",
      "settings.banner_shown",
      "settings.banner_height",
      "settings.banner_cut",
      "settings.banner_whole",
      "settings.banner_at_random",
      "settings.home_sections",
      "settings.library_order",
    ],
  },
  {
    area: "settings",
    path: "playback",
    name: "me.playback",
    files: ["pages/settings/playback.tsx"],
    keys: [
      "settings.sound",
      "settings.downmix",
      "settings.downmix_gain",
      "settings.languages",
      "work.audio",
      "settings.picture",
      "settings.wide_gamut",
      "settings.steps",
      "settings.step_on",
      "settings.step_back",
      "settings.resuming",
      "settings.resume_rewind",
      "settings.resume_per_kind",
    ],
  },
  {
    area: "settings",
    path: "subtitles",
    name: "me.subtitles",
    files: ["pages/settings/subtitles.tsx"],
    keys: [
      "settings.subtitle_start",
      "settings.subtitle_mode",
      "settings.subtitle_language",
      "settings.subtitles",
      "player.subtitle_size",
      "player.subtitle_colour",
      "player.subtitle_edge",
      "player.subtitle_background",
      "player.subtitle_height",
    ],
  },
  {
    area: "admin",
    path: "",
    name: "admin.overview",
    files: ["pages/admin/overview.tsx", "pages/admin/machine.tsx"],
    keys: ["admin.libraries", "admin.tasks", "admin.watch", "admin.people", "admin.system"],
  },
  {
    area: "admin",
    path: "libraries",
    name: "admin.libraries",
    files: ["pages/admin/libraries.tsx"],
    keys: [
      "settings.library_name",
      "settings.extract_subtitles",
      "settings.make_thumbnails",
      "settings.detect_openings",
      "settings.process_on_arrival",
      "settings.watch_in_real_time",
      "settings.keeps_resume_points",
      "settings.keeps_watched_marks",
      "settings.add_library",
      "settings.library_kind",
      "settings.metadata_language",
    ],
  },
  {
    area: "admin",
    path: "metadata",
    name: "admin.metadata",
    files: ["pages/admin/metadata.tsx"],
    keys: [
      "settings.metadata_language",
      "settings.companion_files",
      "settings.read_companion_files",
      "admin.write_companion_files",
      "admin.provider",
      "admin.provider_state",
      "admin.unnamed",
    ],
  },
  {
    area: "admin",
    path: "playback",
    name: "admin.playback",
    files: ["pages/admin/playback.tsx"],
    keys: ["admin.history", "admin.playing"],
  },
  {
    area: "admin",
    path: "transcoding",
    name: "admin.transcoding",
    files: ["pages/admin/transcoding.tsx"],
    keys: [
      "admin.card",
      "admin.limits",
      "admin.limit_sessions",
      "admin.limit_room",
      "admin.limit_kept_behind",
      "admin.codecs",
      "settings.picture",
      "settings.tone_mapping_disabled",
      "settings.thumbnails",
      "settings.thumbnails_every",
      "settings.thumbnails_height",
      "admin.thumbnails_grid",
    ],
  },
  {
    area: "admin",
    path: "users",
    name: "admin.users",
    files: ["pages/admin/users.tsx"],
    keys: [
      "users.name",
      "users.administrator",
      "users.every_library",
      "admin.right.may_delete",
      "admin.right.may_delete_from_disk",
      "admin.limit_streams",
      "admin.right.may_download",
      "admin.limit_age",
      "users.new",
      "users.password",
      "users.password_again",
    ],
  },
  {
    area: "admin",
    path: "devices",
    name: "admin.devices",
    files: ["pages/admin/devices.tsx"],
    keys: ["admin.devices_signed_in", "admin.tv_code"],
  },
  {
    area: "admin",
    path: "security",
    name: "admin.security",
    files: ["pages/admin/security.tsx"],
    keys: ["admin.refused_sign_ins", "admin.brake", "admin.brake_after", "admin.access"],
  },
  {
    area: "admin",
    path: "tasks",
    name: "admin.tasks",
    files: ["pages/admin/tasks.tsx"],
    keys: ["jobs.running", "admin.start_work", "refresh.mode", "tasks.title", "jobs.recent"],
  },
  {
    area: "admin",
    path: "journal",
    name: "admin.journal",
    files: ["pages/admin/journal.tsx"],
    keys: ["admin.journal_lines", "activity.title"],
  },
  {
    area: "admin",
    path: "diagnostics",
    name: "admin.diagnostics",
    files: ["pages/admin/diagnostics.tsx"],
    keys: ["admin.report"],
  },
  {
    area: "admin",
    path: "settings",
    name: "admin.settings",
    files: ["pages/admin/settings.tsx"],
    keys: [
      "admin.maintenance",
      "admin.maintenance_on",
      "admin.maintenance_message",
      "admin.updates",
      "admin.updates_check",
      "admin.backups",
      "admin.backups_daily",
      "admin.server",
      "admin.server_name",
      "admin.logo",
      "admin.door",
      "admin.door_background",
      "admin.door_picture",
      "admin.door_slogan",
      "activity.title",
      "activity.kept",
    ],
  },
];

/** A setting or a section found, and where it leads. */
export interface Found {
  area: Area;
  path: string;
  /** The words found, in the language in force. */
  said: string;
  /** The name of the section it is in. */
  section: string;
  /** The key of the setting, or nothing for a section found by its own name. */
  key: string | null;
}

/** Words as they are compared: no case, no accents, no marks between them. */
export function folded(words: string): string {
  return words
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, " ")
    .trim();
}

/**
 * The settings and sections of one area whose name holds every word asked,
 * those beginning with the first of them first, then in the order the pages
 * show them. A section is found by its own name as well, before what it holds.
 */
export function find(
  words: string,
  area: Area,
  t: (key: string) => string,
  most: number,
): Found[] {
  const asked = folded(words).split(" ").filter(Boolean);
  if (asked.length === 0) {
    return [];
  }
  const matches = (said: string) => {
    const seen = folded(said);
    return asked.every((word) => seen.includes(word));
  };
  const found: Found[] = [];
  for (const section of FINDABLE.filter((one) => one.area === area)) {
    const name = t(section.name);
    if (matches(name)) {
      found.push({ area, path: section.path, said: name, section: name, key: null });
    }
    for (const key of new Set(section.keys)) {
      const said = t(key);
      if (matches(said)) {
        found.push({ area, path: section.path, said, section: name, key });
      }
    }
  }
  const first = asked[0];
  const leads = (one: Found) => folded(one.said).split(" ").some((word) => word.startsWith(first));
  return [...found.filter(leads), ...found.filter((one) => !leads(one))].slice(0, most);
}

/** Where a setting found is: its section's page, told which setting to
 *  bring into view. */
export function addressOf(found: Found): string {
  const base = found.area === "admin" ? "/admin" : "/settings";
  const page = found.path ? `${base}/${found.path}` : base;
  return found.key ? `${page}?find=${encodeURIComponent(found.key)}` : page;
}

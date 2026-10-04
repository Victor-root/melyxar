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

/** A card or a setting, as the keys of its name and of its explanation. */
export type Named = [name: string, why?: string];

/** One section of an area, and what it holds. */
export interface FindableSection {
  area: Area;
  /** Under the area's address, empty for the first. */
  path: string;
  /** The key of the section's name. */
  name: string;
  /** The pages it is drawn by, for the test that keeps this in step. */
  files: string[];
  /** Its cards and settings: the key of each one's name, and of what it says
   *  of itself under it when it says anything. */
  named: Named[];
  /** Words drawn from a list rather than written on the page, which lead to
   *  the card or setting they are chosen in: the choices of a list, the
   *  lines of a card. Keyed by that card or setting's name. */
  choices?: Record<string, string[]>;
}

export const FINDABLE: FindableSection[] = [
  {
    area: "settings",
    path: "",
    name: "me.profile",
    files: ["pages/settings/profile.tsx"],
    named: [
      ["settings.door", "settings.door_why"],
      ["settings.door_hide_me", "settings.door_hide_me_why"],
      ["settings.avatar", "settings.avatar_why"],
      ["me.name", "me.name_lead"],
      ["me.password", "me.password_lead"],
      ["me.devices", "me.devices_lead"],
    ],
  },
  {
    area: "settings",
    path: "appearance",
    name: "me.appearance",
    files: ["pages/settings/appearance.tsx"],
    named: [
      ["settings.appearance"],
      ["nav.theme", "me.theme_why"],
      ["nav.language"],
      ["me.accent", "me.accent_why"],
      ["settings.backdrop", "settings.backdrop_why"],
      ["settings.header", "settings.header_buttons_why"],
      ["settings.header_hides", "settings.header_hides_why"],
    ],
    choices: {
      "nav.theme": ["theme.system", "theme.dark", "theme.light"],
      "settings.backdrop": [
        "backdrop.light",
        "backdrop.library",
        "backdrop.none",
        "backdrop.light.1",
        "backdrop.light.2",
        "backdrop.light.3",
        "backdrop.light.4",
        "backdrop.light.5",
        "backdrop.light.6",
        "backdrop.light.7",
        "backdrop.light.8",
        "backdrop.light.9",
        "backdrop.light.10",
      ],
      "settings.header": [
        "nav.search",
        "nav.favourites",
        "nav.watch_later",
        "requests.title",
        "nav.notifications",
        "home.scan",
        "nav.administration",
        "nav.cast",
        "nav.settings",
      ],
    },
  },
  {
    area: "settings",
    path: "home",
    name: "me.home",
    files: ["pages/settings/home.tsx"],
    named: [
      ["settings.banner", "settings.banner_why"],
      ["settings.banner_shown"],
      ["settings.banner_height"],
      ["settings.banner_cut", "settings.banner_cut_why"],
      ["settings.banner_whole", "settings.banner_whole_why"],
      ["settings.banner_at_random", "settings.banner_at_random_why"],
      ["settings.home_sections", "settings.home_sections_why"],
      ["settings.library_order", "settings.library_order_why"],
    ],
    choices: {
      "settings.home_sections": [
        "home_section.band",
        "home_section.carry_on",
        "home_section.up_next",
        "home_section.recently_added",
      ],
    },
  },
  {
    area: "settings",
    path: "playback",
    name: "me.playback",
    files: ["pages/settings/playback.tsx"],
    named: [
      ["settings.sound", "settings.sound_why"],
      ["settings.downmix"],
      ["settings.downmix_gain"],
      ["settings.languages", "settings.languages_why"],
      ["work.audio"],
      ["settings.picture", "settings.picture_why"],
      ["settings.wide_gamut"],
      ["settings.steps", "settings.steps_why"],
      ["settings.step_on"],
      ["settings.step_back"],
      ["settings.resuming", "settings.resuming_why"],
      ["settings.resume_rewind", "settings.resume_rewind_why"],
      ["settings.resume_per_kind", "settings.resume_per_kind_why"],
    ],
    choices: {
      "settings.downmix": [
        "downmix.none",
        "downmix.centre_and_bass_split",
        "downmix.night_dialogue",
        "downmix.intensity_preserving",
        "downmix.broadcast_standard",
      ],
      "settings.wide_gamut": [
        "wide_gamut.automatic",
        "wide_gamut.always_convert",
        "wide_gamut.never_convert",
      ],
      "settings.resuming": [
        "settings.resume_min_percent",
        "settings.resume_max_percent",
        "settings.resume_min_seconds",
      ],
    },
  },
  {
    area: "settings",
    path: "music",
    name: "me.music",
    files: ["music/settings.tsx"],
    named: [
      ["settings.music_listening", "settings.music_listening_why"],
      ["settings.music_film", "settings.music_film_why"],
      ["settings.music_volume_mode", "settings.music_volume_mode_why"],
      ["settings.music_crossfade", "settings.music_crossfade_why"],
      ["settings.music_resume_queue", "settings.music_resume_queue_why"],
      ["settings.music_spectrum", "settings.music_spectrum_why"],
      ["settings.music_spectrum_amplitude"],
      ["settings.music_network", "settings.music_network_why"],
      ["settings.music_max_bitrate", "settings.music_max_bitrate_why"],
      ["settings.music_tags", "settings.music_tags_why"],
      ["settings.music_skips", "settings.music_skips_why"],
      ["settings.music_skip_on"],
      ["settings.music_skip_back"],
      ["settings.music_tabs", "settings.music_tabs_why"],
      ["settings.music_tag_preview", "settings.music_tag_preview_why"],
    ],
    choices: {
      "settings.music_film": [
        "settings.music_film.stop",
        "settings.music_film.pause",
      ],
      "settings.music_volume_mode": [
        "settings.music_volume_mode.track",
        "settings.music_volume_mode.album",
        "settings.music_volume_mode.off",
      ],
    },
  },
  {
    area: "settings",
    path: "subtitles",
    name: "me.subtitles",
    files: ["pages/settings/subtitles.tsx"],
    named: [
      ["settings.subtitle_start", "settings.subtitle_start_why"],
      ["settings.subtitle_mode"],
      ["settings.subtitle_language", "settings.subtitle_language_why"],
      ["settings.subtitles", "settings.subtitles_why"],
      ["player.subtitle_size"],
      ["player.subtitle_colour"],
      ["player.subtitle_edge"],
      ["player.subtitle_background"],
      ["player.subtitle_height"],
    ],
    choices: {
      "settings.subtitle_mode": [
        "subtitle_mode.smart",
        "subtitle_mode.from_the_file",
        "subtitle_mode.only_forced",
        "subtitle_mode.always",
        "subtitle_mode.never",
      ],
    },
  },
  {
    area: "settings",
    path: "about",
    name: "me.about",
    files: ["pages/settings/about.tsx"],
    named: [
      ["about.report", "about.report_why"],
      ["about.server"],
      ["about.server_name"],
      ["about.version"],
      ["about.api_version"],
      ["about.licence", "about.licence_why"],
      ["about.icons", "about.icons_why"],
      ["about.metadata", "attribution.tmdb"],
    ],
  },
  {
    area: "admin",
    path: "",
    name: "admin.overview",
    files: ["pages/admin/overview.tsx", "pages/admin/machine.tsx"],
    named: [
      ["admin.libraries", "admin.libraries_lead"],
      ["admin.tasks", "admin.tasks_lead"],
      ["admin.watch", "admin.watch_lead"],
      ["admin.people", "admin.people_lead"],
      ["admin.system", "admin.system_lead"],
    ],
  },
  {
    area: "admin",
    path: "libraries",
    name: "admin.libraries",
    files: ["pages/admin/libraries.tsx", "music/library-options.tsx"],
    named: [
      ["settings.library_name"],
      ["settings.extract_subtitles", "admin.extract_subtitles_why"],
      ["settings.make_thumbnails", "admin.make_thumbnails_why"],
      ["settings.detect_openings", "admin.detect_openings_why"],
      ["settings.generate_subtitles", "admin.generate_subtitles_why"],
      ["settings.process_on_arrival", "admin.process_on_arrival_why"],
      ["settings.watch_in_real_time"],
      ["settings.keeps_resume_points", "admin.resume_points_why"],
      ["settings.keeps_watched_marks", "admin.watched_marks_why"],
      ["music.lyrics_online", "music.lyrics_online_why"],
      ["music.covers_online", "music.covers_online_why"],
      ["music.artist_photos_online", "music.artist_photos_online_why"],
      ["music.tag_writing", "music.tag_writing_why"],
      ["settings.add_library", "settings.new_library_why"],
      ["settings.library_kind"],
      ["settings.metadata_language"],
    ],
  },
  {
    area: "admin",
    path: "metadata",
    name: "admin.metadata",
    files: ["pages/admin/metadata.tsx"],
    named: [
      ["settings.metadata_language", "settings.metadata_language_why"],
      ["settings.companion_files", "settings.companion_files_why"],
      ["settings.read_companion_files"],
      ["admin.write_companion_files", "admin.write_companion_files_why"],
      ["admin.provider", "admin.provider_lead"],
      ["admin.provider_state"],
      ["admin.unnamed", "admin.unnamed_why"],
    ],
  },
  {
    area: "admin",
    path: "playback",
    name: "admin.playback",
    files: ["pages/admin/playback.tsx"],
    named: [
      ["admin.history", "admin.history_lead"],
      ["admin.playing", "admin.playing_lead"],
    ],
  },
  {
    area: "admin",
    path: "transcoding",
    name: "admin.transcoding",
    files: ["pages/admin/transcoding.tsx"],
    named: [
      ["admin.card", "admin.card_lead"],
      ["admin.card_choice", "admin.card_choice_why"],
      ["admin.card_fallback", "admin.card_fallback_why"],
      ["admin.card_processor_fallback", "admin.card_processor_fallback_why"],
      ["admin.limits", "admin.limits_lead"],
      ["admin.limit_sessions", "admin.limit_sessions_why"],
      ["admin.limit_room", "admin.limit_room_why"],
      ["admin.limit_kept_behind", "admin.limit_kept_behind_why"],
      ["admin.codecs", "admin.codecs_why"],
      ["settings.picture"],
      ["settings.tone_mapping_disabled", "settings.tone_mapping_disabled_why"],
      ["settings.thumbnails", "settings.thumbnails_why"],
      ["settings.thumbnails_every"],
      ["settings.thumbnails_height"],
      ["admin.thumbnails_grid", "settings.thumbnails_shape_why"],
    ],
    choices: {
      "admin.codecs": [
        "admin.codec.av1",
        "admin.codec.hevc",
        "admin.codec.h264",
      ],
    },
  },
  {
    area: "admin",
    path: "users",
    name: "admin.users",
    files: ["pages/admin/users.tsx"],
    named: [
      ["users.name"],
      ["users.administrator", "users.administrator_why"],
      ["users.every_library", "users.every_library_why"],
      ["admin.right.may_delete", "users.may_delete_why"],
      ["admin.right.may_delete_from_disk", "users.may_delete_from_disk_why"],
      ["admin.limit_streams", "users.streams_why"],
      ["admin.right.may_download", "users.may_download_why"],
      [
        "admin.right.may_manage_collections",
        "users.may_manage_collections_why",
      ],
      ["admin.right.may_edit_tags", "users.may_edit_tags_why"],
      ["admin.right.may_upload", "users.may_upload_why"],
      ["admin.limit_age"],
      ["users.new", "users.new_why"],
      ["users.password"],
      ["users.password_again"],
    ],
  },
  {
    area: "admin",
    path: "devices",
    name: "admin.devices",
    files: ["pages/admin/devices.tsx"],
    named: [
      ["admin.devices_signed_in", "admin.devices_signed_in_lead"],
      ["admin.tv_code", "admin.tv_code_lead"],
    ],
  },
  {
    area: "admin",
    path: "security",
    name: "admin.security",
    files: ["pages/admin/security.tsx", "pages/admin/access-panel.tsx"],
    named: [
      ["admin.refused_sign_ins", "admin.refused_sign_ins_lead"],
      ["admin.brake_after", "admin.brake_after_why"],
      ["admin.access", "admin.access_lead"],
      ["admin.access_redirect"],
    ],
    choices: {
      "admin.access_redirect": [
        "admin.access_redirect.on",
        "admin.access_redirect.off",
      ],
    },
  },
  {
    area: "admin",
    path: "tasks",
    name: "admin.tasks",
    files: ["pages/admin/tasks.tsx"],
    named: [
      ["jobs.running", "admin.running_lead"],
      ["admin.start_work", "admin.start_work_lead"],
      ["refresh.mode"],
      ["tasks.title", "tasks.why"],
      ["jobs.recent", "admin.recent_jobs_lead"],
    ],
  },
  {
    area: "admin",
    path: "journal",
    name: "admin.journal",
    files: ["pages/admin/journal.tsx"],
    named: [["admin.journal_lines"], ["activity.title", "activity.lead"]],
  },
  {
    area: "admin",
    path: "diagnostics",
    name: "admin.diagnostics",
    files: ["pages/admin/diagnostics.tsx"],
    named: [["admin.report", "admin.report_lead"]],
  },
  {
    area: "admin",
    path: "settings",
    name: "admin.settings",
    files: ["pages/admin/settings.tsx"],
    named: [
      ["admin.maintenance", "admin.maintenance_lead"],
      ["admin.maintenance_on"],
      ["admin.maintenance_message"],
      ["admin.updates", "admin.updates_lead"],
      ["admin.updates_check"],
      ["admin.backups", "admin.backups_lead"],
      ["admin.backups_daily"],
      ["admin.server", "admin.server_lead"],
      ["admin.server_name"],
      ["admin.theme", "admin.theme_why"],
      ["admin.logo", "admin.logo_why"],
      ["admin.door", "admin.door_lead"],
      ["admin.door_background", "admin.door_background_why"],
      ["admin.door_picture", "admin.door_picture_why"],
      ["admin.door_slogan", "admin.door_slogan_why"],
      ["activity.title", "activity.kept_lead"],
      ["activity.kept", "activity.kept_why"],
    ],
    choices: {
      "admin.door_background": [
        "admin.door_background.abstract",
        "admin.door_background.library",
      ],
    },
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
 * The settings and sections of one area whose name, one of their choices, or
 * name and explanation together, hold every word asked: those named by the words first, the ones
 * beginning with the first word leading, then those only their explanation
 * speaks of, each in the order the pages show them. A section is found by its
 * own name as well, before what it holds.
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
  const byName: Found[] = [];
  const byWhy: Found[] = [];
  const seen = new Set<string>();
  for (const section of FINDABLE.filter((one) => one.area === area)) {
    const name = t(section.name);
    if (matches(name)) {
      byName.push({
        area,
        path: section.path,
        said: name,
        section: name,
        key: null,
      });
    }
    for (const [key, why] of section.named) {
      const said = t(key);
      const place = `${section.path}:${key}`;
      if (seen.has(place)) {
        continue;
      }
      const found = { area, path: section.path, said, section: name, key };
      if (matches(said)) {
        byName.push(found);
        seen.add(place);
        continue;
      }
      // Found by one of its choices, it is said by that choice.
      const choice = (section.choices?.[key] ?? []).map(t).find(matches);
      if (choice) {
        byName.push({ ...found, said: choice });
        seen.add(place);
      } else if (why && matches(`${said} ${t(why)}`)) {
        byWhy.push(found);
        seen.add(place);
      }
    }
  }
  const first = asked[0];
  const leads = (one: Found) =>
    folded(one.said)
      .split(" ")
      .some((word) => word.startsWith(first));
  return [
    ...byName.filter(leads),
    ...byName.filter((one) => !leads(one)),
    ...byWhy,
  ].slice(0, most);
}

/** Where a setting found is: its section's page, told which setting to
 *  bring into view. */
export function addressOf(found: Found): string {
  const base = found.area === "admin" ? "/admin" : "/settings";
  const page = found.path ? `${base}/${found.path}` : base;
  return found.key ? `${page}?find=${encodeURIComponent(found.key)}` : page;
}

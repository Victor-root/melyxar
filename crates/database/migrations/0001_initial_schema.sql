-- Initial schema.
--
-- The whole model agreed before any code was written lives here, including
-- tables that stay empty until a later milestone. Creating them now is cheap;
-- adding them later would mean a migration on a live library, and several of
-- them slip into navigation queries that would be painful to revisit.
--
-- Conventions, applied everywhere:
--   * identifiers are UUID version 7 stored as text;
--   * instants are ISO 8601 in UTC, stored as text, which sorts correctly;
--   * durations and positions are whole milliseconds stored as integers;
--   * booleans are 0 or 1;
--   * every foreign key cascades on delete unless losing the row would lose
--     history that must survive, in which case it is set to null.

-- ---------------------------------------------------------------------------
-- Libraries and roots
--
-- Declared first because accounts reference them when their access is limited
-- to a subset.
-- ---------------------------------------------------------------------------

CREATE TABLE libraries (
    id                  TEXT PRIMARY KEY NOT NULL,
    name                TEXT NOT NULL,
    -- movies, series, anime, shows, music. Explicit so that no query has to
    -- assume a film, which is what lets music arrive as an addition.
    kind                TEXT NOT NULL,
    metadata_language   TEXT NOT NULL DEFAULT 'fr',
    -- Bumped on every write touching this library. Backs entity tags, client
    -- cache invalidation and the change events.
    version             INTEGER NOT NULL DEFAULT 1,
    -- Counts kept up to date on write, never counted at read time.
    work_count          INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
) STRICT;

CREATE TABLE library_roots (
    id              TEXT PRIMARY KEY NOT NULL,
    library_id      TEXT NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Short label shown in logs and in the interface instead of the path.
    label           TEXT NOT NULL,
    path            TEXT NOT NULL,
    -- missing, unreadable, read_only, read_write. Established by a real access
    -- test, never by reading the permission bits, which lie as soon as groups,
    -- network mounts or an unprivileged container are involved.
    access_state    TEXT NOT NULL DEFAULT 'missing',
    access_checked_at TEXT
) STRICT;

CREATE INDEX library_roots_by_library ON library_roots (library_id);

-- ---------------------------------------------------------------------------
-- Accounts, rights, preferences, devices
-- ---------------------------------------------------------------------------

CREATE TABLE users (
    id                  TEXT PRIMARY KEY NOT NULL,
    name                TEXT NOT NULL,
    -- Hash of the password. Empty until a password is set, which is the state
    -- of the default account created at first start.
    password_hash       TEXT,
    avatar_path         TEXT,
    is_administrator    INTEGER NOT NULL DEFAULT 0,
    max_age_rating      INTEGER,
    may_download        INTEGER NOT NULL DEFAULT 0,
    may_delete          INTEGER NOT NULL DEFAULT 0,
    may_delete_from_disk INTEGER NOT NULL DEFAULT 0,
    max_sessions        INTEGER,
    created_at          TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX users_name_unique ON users (name COLLATE NOCASE);

-- Libraries a person may see. No row for a person means every library, which
-- is the ordinary case and avoids having to grant each new library.
CREATE TABLE user_library_access (
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    library_id  TEXT NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, library_id)
) STRICT;

CREATE TABLE user_preferences (
    user_id                     TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    interface_language          TEXT NOT NULL DEFAULT 'en',
    preferred_audio_language    TEXT,
    preferred_subtitle_language TEXT,
    theme_mode                  TEXT NOT NULL DEFAULT 'system',
    accent_color                TEXT NOT NULL DEFAULT '#c81e1e',
    custom_css                  TEXT,
    volume                      REAL NOT NULL DEFAULT 1.0,
    downmix_method              TEXT NOT NULL DEFAULT 'broadcast_standard',
    downmix_gain                REAL NOT NULL DEFAULT 2.0,
    subtitle_appearance         TEXT
) STRICT;

-- One long lived token per device, individually revocable, so a television
-- never has to sign in again and a lost device can be cut off alone.
CREATE TABLE devices (
    id              TEXT PRIMARY KEY NOT NULL,
    user_id         TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    token_hash      TEXT NOT NULL,
    -- Optional short code that unlocks an already authorised device, never a
    -- way to create access on its own.
    unlock_code_hash TEXT,
    created_at      TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX devices_token_hash_unique ON devices (token_hash);
CREATE INDEX devices_by_user ON devices (user_id, last_seen_at DESC);

-- ---------------------------------------------------------------------------
-- Server settings and branding
-- ---------------------------------------------------------------------------

-- A single row, guarded by a fixed primary key. Typed columns rather than a
-- key and value pair, so that a typo is a compile error rather than a silent
-- default at runtime.
CREATE TABLE server_settings (
    id                      INTEGER PRIMARY KEY CHECK (id = 1),
    server_name             TEXT NOT NULL DEFAULT 'Melyxar',
    logo_path               TEXT,
    splash_path             TEXT,
    login_background_path   TEXT,
    global_custom_css       TEXT,
    -- Shows the account list before the password is typed. A deliberate
    -- disclosure, hence the switch.
    show_user_picker        INTEGER NOT NULL DEFAULT 1,
    maintenance_enabled     INTEGER NOT NULL DEFAULT 0,
    maintenance_message     TEXT,
    maintenance_until       TEXT,
    -- Read companion metadata files when they sit next to a media file.
    read_companion_files    INTEGER NOT NULL DEFAULT 0,
    -- Write them. Needs a writable root, checked before anything is attempted.
    write_companion_files   INTEGER NOT NULL DEFAULT 0,
    -- Share of a work that counts as watched.
    watched_threshold       REAL NOT NULL DEFAULT 0.9,
    -- Days of activity kept before automatic purge.
    activity_retention_days INTEGER NOT NULL DEFAULT 180,
    -- Ask the code hosting service whether a newer version exists.
    check_for_updates       INTEGER NOT NULL DEFAULT 1,
    updated_at              TEXT NOT NULL
) STRICT;

INSERT INTO server_settings (id, updated_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

-- ---------------------------------------------------------------------------
-- Works: the common trunk
-- ---------------------------------------------------------------------------

CREATE TABLE works (
    id              TEXT PRIMARY KEY NOT NULL,
    library_id      TEXT NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    -- Parent work: a season points at its series, an episode at its season.
    parent_id       TEXT REFERENCES works (id) ON DELETE CASCADE,
    -- movie, series, season, episode, artist, album, song.
    kind            TEXT NOT NULL,
    title           TEXT NOT NULL,
    -- Leading article removed and accents folded, computed on write so that
    -- ordering a hundred thousand rows costs an index walk.
    sort_title      TEXT NOT NULL,
    release_year    INTEGER,
    -- Whole minutes are not enough: runtime is shown to the second.
    runtime_ms      INTEGER,
    age_rating      INTEGER,
    age_rating_label TEXT,
    community_rating REAL,
    critic_rating   REAL,
    -- Ordering inside a parent: season number, episode number, track number.
    ordinal         INTEGER,
    -- pending, identified, unidentified, manual.
    identification  TEXT NOT NULL DEFAULT 'pending',
    -- Sent with every card so a grid shows colour before an image arrives.
    dominant_color  TEXT,
    -- Kept up to date on write for episodic works.
    child_count     INTEGER NOT NULL DEFAULT 0,
    added_at        TEXT NOT NULL,
    updated_at      TEXT NOT NULL
) STRICT;

-- One index per offered sort, each scoped to the library, so that ordering a
-- large collection stays an index walk rather than a sort.
CREATE INDEX works_by_sort_title ON works (library_id, sort_title);
CREATE INDEX works_by_added_at ON works (library_id, added_at DESC);
CREATE INDEX works_by_year ON works (library_id, release_year DESC);
CREATE INDEX works_by_rating ON works (library_id, community_rating DESC);
CREATE INDEX works_by_runtime ON works (library_id, runtime_ms);
CREATE INDEX works_by_parent ON works (parent_id, ordinal);
CREATE INDEX works_by_identification ON works (library_id, identification);

-- Text stored per language, so the preferred language can fall back to
-- English without a second request to the provider, and so a detail page can
-- switch language without one either.
CREATE TABLE work_translations (
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    language    TEXT NOT NULL,
    title       TEXT,
    tagline     TEXT,
    overview    TEXT,
    PRIMARY KEY (work_id, language)
) STRICT;

CREATE TABLE work_external_ids (
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    -- tmdb, imdb, tvdb, musicbrainz.
    provider    TEXT NOT NULL,
    external_id TEXT NOT NULL,
    PRIMARY KEY (work_id, provider)
) STRICT;

CREATE INDEX work_external_ids_lookup ON work_external_ids (provider, external_id);

-- Fields edited by hand, which a metadata refresh must never overwrite.
CREATE TABLE work_locked_fields (
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    field       TEXT NOT NULL,
    locked_at   TEXT NOT NULL,
    PRIMARY KEY (work_id, field)
) STRICT;

-- Which provider supplied which field and when, so a refresh can tell what it
-- owns and what it must leave alone.
CREATE TABLE work_field_provenance (
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    field       TEXT NOT NULL,
    provider    TEXT NOT NULL,
    fetched_at  TEXT NOT NULL,
    PRIMARY KEY (work_id, field)
) STRICT;

-- ---------------------------------------------------------------------------
-- Media sources and tracks
-- ---------------------------------------------------------------------------

CREATE TABLE media_sources (
    id                  TEXT PRIMARY KEY NOT NULL,
    work_id             TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    root_id             TEXT NOT NULL REFERENCES library_roots (id) ON DELETE CASCADE,
    -- Relative to the root, so moving a library to another disk is a change of
    -- root rather than a full reindex.
    relative_path       TEXT NOT NULL,
    container           TEXT,
    duration_ms         INTEGER,
    overall_bitrate     INTEGER,
    size_bytes          INTEGER NOT NULL,
    modified_at         TEXT NOT NULL,
    -- Fingerprint of the first megabytes, computed only when size and date are
    -- ambiguous, because reading from a slow share is expensive.
    content_fingerprint TEXT,
    -- Set when a scan no longer finds the file. Never deleted outright, so
    -- that a disconnected mount does not destroy history.
    missing_since       TEXT,
    added_at            TEXT NOT NULL,
    analysed_at         TEXT
) STRICT;

CREATE UNIQUE INDEX media_sources_by_path ON media_sources (root_id, relative_path);
CREATE INDEX media_sources_by_work ON media_sources (work_id);
CREATE INDEX media_sources_by_identity ON media_sources (size_bytes, modified_at);

CREATE TABLE tracks (
    id              TEXT PRIMARY KEY NOT NULL,
    source_id       TEXT NOT NULL REFERENCES media_sources (id) ON DELETE CASCADE,
    -- Index of the stream inside the container, as the analyser reports it.
    stream_index    INTEGER NOT NULL,
    -- video, audio, subtitle.
    kind            TEXT NOT NULL,
    language        TEXT,
    title           TEXT,
    is_default      INTEGER NOT NULL DEFAULT 0,
    is_forced       INTEGER NOT NULL DEFAULT 0,
    codec           TEXT NOT NULL,
    profile         TEXT,
    level           INTEGER,
    bitrate         INTEGER,

    -- Video only.
    width           INTEGER,
    height          INTEGER,
    aspect_ratio    TEXT,
    is_interlaced   INTEGER,
    frame_rate      REAL,
    pixel_format    TEXT,
    reference_frames INTEGER,
    -- These four decide whether a stream, and any image pulled out of it, has
    -- to be converted to standard dynamic range. Functional, not decorative.
    color_primaries TEXT,
    color_space     TEXT,
    color_transfer  TEXT,
    bit_depth       INTEGER,
    -- hdr10, hlg, dolby_vision.
    hdr_format      TEXT,
    dolby_vision_profile INTEGER,

    -- Audio only.
    channels        INTEGER,
    channel_layout  TEXT,
    sample_rate     INTEGER,
    -- Loudness measured during a background pass, applied later as a plain
    -- gain. The columns exist from the first migration so that adding the
    -- measurement later does not mean reanalysing the whole library.
    loudness_integrated_lufs REAL,
    loudness_true_peak_dbfs  REAL,
    loudness_range_lu        REAL,

    -- Subtitle only.
    -- text or bitmap. Picture subtitles can only be burnt in, which forces a
    -- full transcode, so the distinction belongs in the model.
    subtitle_layout TEXT,
    is_hearing_impaired INTEGER,
    is_external     INTEGER NOT NULL DEFAULT 0,
    external_relative_path TEXT
) STRICT;

CREATE INDEX tracks_by_source ON tracks (source_id, kind, stream_index);

-- Trailers and other secondary videos, local or remote.
CREATE TABLE extra_videos (
    id              TEXT PRIMARY KEY NOT NULL,
    work_id         TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    -- trailer, behind_the_scenes, deleted_scene, featurette.
    kind            TEXT NOT NULL,
    name            TEXT,
    -- Set for a file sitting next to the media.
    root_id         TEXT REFERENCES library_roots (id) ON DELETE CASCADE,
    relative_path   TEXT,
    -- Set for a link supplied by a provider. Playing it leaves the local
    -- server, hence a setting to allow it or not.
    remote_url      TEXT,
    created_at      TEXT NOT NULL
) STRICT;

CREATE INDEX extra_videos_by_work ON extra_videos (work_id, kind);

-- Chapters, either read from the file or generated at a fixed interval.
CREATE TABLE chapters (
    id              TEXT PRIMARY KEY NOT NULL,
    source_id       TEXT NOT NULL REFERENCES media_sources (id) ON DELETE CASCADE,
    ordinal         INTEGER NOT NULL,
    start_ms        INTEGER NOT NULL,
    title           TEXT,
    -- Path of the generated thumbnail, relative to the image cache. Always
    -- converted to standard range when the source is wide gamut, otherwise it
    -- comes out washed out and grey.
    thumbnail_path  TEXT
) STRICT;

CREATE INDEX chapters_by_source ON chapters (source_id, ordinal);

-- Marked stretches inside a source: a recap, an opening, closing credits.
-- Backs the skip button and the next episode prompt. Filled once series
-- arrive, but the table exists now because the analysis writes into it.
CREATE TABLE media_segments (
    id          TEXT PRIMARY KEY NOT NULL,
    source_id   TEXT NOT NULL REFERENCES media_sources (id) ON DELETE CASCADE,
    -- recap, intro, outro, advertisement.
    kind        TEXT NOT NULL,
    start_ms    INTEGER NOT NULL,
    end_ms      INTEGER NOT NULL,
    -- chapter, detected, manual. A manual correction wins over a detection.
    origin      TEXT NOT NULL,
    created_at  TEXT NOT NULL
) STRICT;

CREATE INDEX media_segments_by_source ON media_segments (source_id, start_ms);

-- ---------------------------------------------------------------------------
-- People, collections, tags, playlists
-- ---------------------------------------------------------------------------

-- A person exists once, whatever the number of works they appear in. Without
-- that single identity, a filmography is impossible to build.
CREATE TABLE people (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    sort_name       TEXT NOT NULL,
    image_path      TEXT,
    created_at      TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX people_by_sort_name ON people (sort_name, name);

CREATE TABLE person_external_ids (
    person_id   TEXT NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    provider    TEXT NOT NULL,
    external_id TEXT NOT NULL,
    PRIMARY KEY (person_id, provider)
) STRICT;

CREATE INDEX person_external_ids_lookup ON person_external_ids (provider, external_id);

CREATE TABLE credits (
    id          TEXT PRIMARY KEY NOT NULL,
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    person_id   TEXT NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    -- actor, director, writer, producer, composer.
    role        TEXT NOT NULL,
    -- Name of the character played, for an actor.
    character_name TEXT,
    -- Billing order, so the cast row shows the leads first.
    ordinal     INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX credits_by_work ON credits (work_id, role, ordinal);
CREATE INDEX credits_by_person ON credits (person_id, role);

-- Automatic and hand made collections share one table with their origin
-- recorded, otherwise a metadata refresh would wipe the hand made ones.
CREATE TABLE collections (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    sort_name   TEXT NOT NULL,
    overview    TEXT,
    image_path  TEXT,
    -- provider or manual.
    origin      TEXT NOT NULL DEFAULT 'manual',
    created_at  TEXT NOT NULL
) STRICT;

CREATE TABLE collection_external_ids (
    collection_id TEXT NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
    provider      TEXT NOT NULL,
    external_id   TEXT NOT NULL,
    PRIMARY KEY (collection_id, provider)
) STRICT;

CREATE TABLE collection_items (
    collection_id TEXT NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
    work_id       TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    ordinal       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (collection_id, work_id)
) STRICT;

CREATE INDEX collection_items_by_work ON collection_items (work_id);

CREATE TABLE genres (
    id      TEXT PRIMARY KEY NOT NULL,
    name    TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX genres_by_name ON genres (name COLLATE NOCASE);

CREATE TABLE work_genres (
    work_id  TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    genre_id TEXT NOT NULL REFERENCES genres (id) ON DELETE CASCADE,
    PRIMARY KEY (work_id, genre_id)
) STRICT;

CREATE INDEX work_genres_by_genre ON work_genres (genre_id);

CREATE TABLE studios (
    id      TEXT PRIMARY KEY NOT NULL,
    name    TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX studios_by_name ON studios (name COLLATE NOCASE);

CREATE TABLE work_studios (
    work_id   TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    studio_id TEXT NOT NULL REFERENCES studios (id) ON DELETE CASCADE,
    PRIMARY KEY (work_id, studio_id)
) STRICT;

-- Free labels, shared across accounts.
CREATE TABLE tags (
    id      TEXT PRIMARY KEY NOT NULL,
    name    TEXT NOT NULL
) STRICT;

CREATE UNIQUE INDEX tags_by_name ON tags (name COLLATE NOCASE);

CREATE TABLE work_tags (
    work_id TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    PRIMARY KEY (work_id, tag_id)
) STRICT;

CREATE INDEX work_tags_by_tag ON work_tags (tag_id);

-- Ordered, and owned by one account.
CREATE TABLE playlists (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
) STRICT;

CREATE INDEX playlists_by_user ON playlists (user_id, name);

CREATE TABLE playlist_items (
    playlist_id TEXT NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    ordinal     INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, work_id)
) STRICT;

CREATE INDEX playlist_items_ordered ON playlist_items (playlist_id, ordinal);

-- ---------------------------------------------------------------------------
-- Images
-- ---------------------------------------------------------------------------

-- Generated on write in a few fixed sizes. The content fingerprint travels in
-- the served address, which is what lets a browser keep an image for a year
-- and skip the request entirely on a second visit.
CREATE TABLE images (
    id              TEXT PRIMARY KEY NOT NULL,
    -- work, person, collection, library, server.
    owner_kind      TEXT NOT NULL,
    owner_id        TEXT NOT NULL,
    -- poster, backdrop, logo, thumbnail, banner. The title image shown on the
    -- detail page is a kind of its own, distinct from poster and backdrop.
    image_kind      TEXT NOT NULL,
    -- Relative to the image cache directory.
    relative_path   TEXT NOT NULL,
    width           INTEGER,
    height          INTEGER,
    fingerprint     TEXT NOT NULL,
    dominant_color  TEXT,
    created_at      TEXT NOT NULL
) STRICT;

CREATE INDEX images_by_owner ON images (owner_kind, owner_id, image_kind);
CREATE UNIQUE INDEX images_by_path ON images (relative_path);

-- ---------------------------------------------------------------------------
-- Watching: progress, favourites, watchlist, aggregates, history
-- ---------------------------------------------------------------------------

CREATE TABLE playback_progress (
    user_id         TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    work_id         TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    position_ms     INTEGER NOT NULL DEFAULT 0,
    -- not_started, in_progress, watched.
    state           TEXT NOT NULL DEFAULT 'not_started',
    -- A manual mark wins over the automatic threshold, for good.
    marked_manually INTEGER NOT NULL DEFAULT 0,
    play_count      INTEGER NOT NULL DEFAULT 0,
    -- Chosen tracks, remembered so the next session starts the same way.
    audio_track_id      TEXT,
    subtitle_track_id   TEXT,
    -- Instant the position was measured on the client. A late report arriving
    -- after a fresher one is refused, otherwise the resume point goes
    -- backwards when a client flushes a stale local copy.
    reported_at     TEXT,
    last_played_at  TEXT,
    PRIMARY KEY (user_id, work_id)
) STRICT;

CREATE INDEX playback_progress_resume ON playback_progress (user_id, state, last_played_at DESC);

CREATE TABLE favorites (
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL,
    PRIMARY KEY (user_id, work_id)
) STRICT;

-- Distinct from favourites: what one plans to watch, not what one likes.
CREATE TABLE watchlist (
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL,
    PRIMARY KEY (user_id, work_id)
) STRICT;

-- Unwatched counts per person and per parent work, updated on every progress
-- change. Counting them at read time would walk every episode of every season
-- on each display, which is exactly what ruins responsiveness.
CREATE TABLE progress_counters (
    user_id         TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    work_id         TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    child_count     INTEGER NOT NULL DEFAULT 0,
    unwatched_count INTEGER NOT NULL DEFAULT 0,
    latest_child_added_at TEXT,
    PRIMARY KEY (user_id, work_id)
) STRICT;

-- Distinct from progress: progress says where one is, history says what
-- happened. Indexed by date and purged automatically, and never read on the
-- hot path.
CREATE TABLE activity_log (
    id          TEXT PRIMARY KEY NOT NULL,
    occurred_at TEXT NOT NULL,
    user_id     TEXT REFERENCES users (id) ON DELETE SET NULL,
    -- playback_started, playback_finished, signed_in, work_deleted, scan_run.
    kind        TEXT NOT NULL,
    work_id     TEXT REFERENCES works (id) ON DELETE SET NULL,
    device_name TEXT,
    -- Free form details as a JSON object, for what does not deserve a column.
    details     TEXT
) STRICT;

CREATE INDEX activity_log_by_date ON activity_log (occurred_at DESC);
CREATE INDEX activity_log_by_user ON activity_log (user_id, occurred_at DESC);

-- Monthly roll-up kept beyond the purge, so personal statistics survive.
CREATE TABLE activity_monthly_summary (
    user_id         TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- First day of the month, in the same text form as every other instant.
    month           TEXT NOT NULL,
    works_watched   INTEGER NOT NULL DEFAULT 0,
    watch_time_ms   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, month)
) STRICT;

-- ---------------------------------------------------------------------------
-- Background work
-- ---------------------------------------------------------------------------

-- Queue held in memory and persisted here, so an interrupted scan resumes
-- instead of starting over.
CREATE TABLE jobs (
    id              TEXT PRIMARY KEY NOT NULL,
    -- scan_library, identify_work, fetch_images, analyse_loudness,
    -- generate_thumbnails, purge_activity, backup.
    kind            TEXT NOT NULL,
    -- Higher runs first: a request made by a person beats a background
    -- refresh.
    priority        INTEGER NOT NULL DEFAULT 0,
    -- queued, running, succeeded, failed, cancelled.
    state           TEXT NOT NULL DEFAULT 'queued',
    -- Subject of the job, for example a library or a work.
    target_id       TEXT,
    -- Arguments as a JSON object.
    payload         TEXT,
    progress_done   INTEGER NOT NULL DEFAULT 0,
    progress_total  INTEGER,
    -- Message shown to the administrator when the job failed.
    failure_reason  TEXT,
    attempts        INTEGER NOT NULL DEFAULT 0,
    -- Set when a provider was unreachable: a scan never stops for that, the
    -- identification is simply retried later with widening intervals.
    retry_after     TEXT,
    created_at      TEXT NOT NULL,
    started_at      TEXT,
    finished_at     TEXT
) STRICT;

CREATE INDEX jobs_queue ON jobs (state, priority DESC, created_at);
CREATE INDEX jobs_by_target ON jobs (target_id, kind);

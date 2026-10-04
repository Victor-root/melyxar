-- Whether accounts may ask for titles the server does not hold. One row,
-- off until the administrator switches it on.
CREATE TABLE request_settings (
    id      INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0
) STRICT;

INSERT INTO request_settings (id, enabled) VALUES (1, 0);

-- The accounts given the right to ask. An administrator always may.
CREATE TABLE request_rights (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE
) STRICT;

-- One account's request for one title, named by the provider's catalogue and
-- identifier, with what the provider said of it when it was asked for.
CREATE TABLE title_requests (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- films or series.
    catalogue   TEXT NOT NULL,
    tmdb_id     TEXT NOT NULL,
    title       TEXT NOT NULL,
    year        INTEGER,
    -- The poster's path at the provider.
    poster_path TEXT,
    overview    TEXT,
    -- The season numbers asked for, a JSON array: empty for a film or a
    -- whole series.
    seasons     TEXT NOT NULL DEFAULT '[]',
    -- What the account added to its request.
    note        TEXT NOT NULL DEFAULT '',
    -- pending, accepted, refused, added.
    state       TEXT NOT NULL DEFAULT 'pending',
    -- The administrator's word on a refusal.
    answer      TEXT NOT NULL DEFAULT '',
    -- The work it became, once added and found.
    work_id     TEXT REFERENCES works (id) ON DELETE SET NULL,
    created_at  TEXT NOT NULL,
    decided_at  TEXT
) STRICT;

-- An account's requests, newest first.
CREATE INDEX title_requests_by_user ON title_requests (user_id, id DESC);
-- Everybody's requests for one title.
CREATE INDEX title_requests_by_title ON title_requests (catalogue, tmdb_id);
CREATE INDEX title_requests_by_work ON title_requests (work_id) WHERE work_id IS NOT NULL;
-- A title is asked for once by an account while its request is open.
CREATE UNIQUE INDEX title_requests_open_once ON title_requests (user_id, catalogue, tmdb_id)
    WHERE state IN ('pending', 'accepted');

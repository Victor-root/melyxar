-- Notifications kept for every account, one row per account that receives
-- one: each has its own read mark, its own removal and its own history, and
-- an account created later has none of what came before it.
CREATE TABLE notifications (
    id           TEXT PRIMARY KEY NOT NULL,
    user_id      TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- message, maintenance, new_content, deletion.
    kind         TEXT NOT NULL,
    -- ok, attention, trouble, news: the colour it wears.
    level        TEXT NOT NULL,
    -- A JSON object, translated when shown. This crate does not look inside.
    data         TEXT NOT NULL,
    -- The work it shows, whose poster it wears and whose page it opens.
    work_id      TEXT REFERENCES works (id) ON DELETE SET NULL,
    -- Shown on the screen even during playback.
    priority     INTEGER NOT NULL DEFAULT 0,
    -- No account may switch it off or remove it.
    mandatory    INTEGER NOT NULL DEFAULT 0,
    -- Stays on the screen until it is closed.
    sticky       INTEGER NOT NULL DEFAULT 0,
    -- How long it stays on the screen; absent, its level decides.
    shown_for_ms INTEGER,
    -- When an announced maintenance happens. It is recalled before and
    -- removed after.
    due_at       TEXT,
    reminded     INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    read_at      TEXT
) STRICT;

-- An account's history, newest first; the identifier is ordered by time.
CREATE INDEX notifications_by_user ON notifications (user_id, id DESC);
CREATE INDEX notifications_by_due ON notifications (due_at) WHERE due_at IS NOT NULL;
CREATE INDEX notifications_by_creation ON notifications (created_at);
CREATE INDEX notifications_by_work ON notifications (work_id) WHERE work_id IS NOT NULL;

-- What an account gets of each kind it may choose about, when it has not
-- chosen: set by the administrator.
CREATE TABLE notification_defaults (
    kind   TEXT PRIMARY KEY NOT NULL,
    bell   INTEGER NOT NULL,
    screen INTEGER NOT NULL
) STRICT;

INSERT INTO notification_defaults (kind, bell, screen) VALUES
    ('message', 1, 1),
    ('new_content', 1, 1),
    ('deletion', 1, 1);

-- What one account chose for one kind.
CREATE TABLE notification_choices (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind    TEXT NOT NULL,
    bell    INTEGER NOT NULL,
    screen  INTEGER NOT NULL,
    PRIMARY KEY (user_id, kind)
) STRICT;

-- An account's quiet hours, in minutes after midnight on the clock of the
-- device that shows them, and whether what is urgent still shows during
-- playback and during those hours.
CREATE TABLE notification_settings (
    user_id                 TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    quiet_from              INTEGER,
    quiet_until             INTEGER,
    priority_while_playing  INTEGER NOT NULL DEFAULT 1,
    priority_while_quiet    INTEGER NOT NULL DEFAULT 1
) STRICT;

-- Whether a library announces what arrives in it, and up to when it has.
CREATE TABLE notification_libraries (
    library_id      TEXT PRIMARY KEY NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    announces       INTEGER NOT NULL DEFAULT 1,
    announced_until TEXT NOT NULL
) STRICT;

-- The collection already here is not news.
INSERT INTO notification_libraries (library_id, announces, announced_until)
SELECT id, 1, strftime('%Y-%m-%dT%H:%M:%f', 'now') || '000000Z' FROM libraries;

-- A library an account chose not to hear about. Absent, it hears.
CREATE TABLE notification_library_choices (
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    library_id TEXT NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    announced  INTEGER NOT NULL,
    PRIMARY KEY (user_id, library_id)
) STRICT;

CREATE INDEX notification_library_choices_by_library ON notification_library_choices (library_id);

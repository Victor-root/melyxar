-- The cameras of a library of cameras, declared one by one in the
-- administration. The password is kept sealed with the server's own key,
-- never as it was typed: the server has to hand it back to an application,
-- so it cannot be kept as a password of an account is.
CREATE TABLE cameras (
    id              TEXT PRIMARY KEY NOT NULL,
    library_id      TEXT NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    -- The RTSP address, without the account it is opened with.
    address         TEXT NOT NULL,
    username        TEXT NOT NULL DEFAULT '',
    sealed_password BLOB,
    created_at      TEXT NOT NULL
) STRICT;

CREATE INDEX cameras_of_a_library ON cameras (library_id, name);

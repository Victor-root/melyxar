-- Whether a library of music looks up online the covers its albums lack,
-- and which albums were already looked up, found or not, so each is asked
-- once.
ALTER TABLE music_library_options ADD COLUMN covers_online INTEGER NOT NULL DEFAULT 0;

CREATE TABLE music_cover_lookups (
    album_id      TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    looked_up_at  TEXT NOT NULL
) STRICT;

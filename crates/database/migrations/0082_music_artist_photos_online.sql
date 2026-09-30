-- Whether a library of music looks up online the photos its artists lack,
-- and which artists were already looked up, found or not, so each is asked
-- once.
ALTER TABLE music_library_options ADD COLUMN artist_photos_online INTEGER NOT NULL DEFAULT 0;

CREATE TABLE music_artist_photo_lookups (
    artist_id     TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    looked_up_at  TEXT NOT NULL
) STRICT;

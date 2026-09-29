-- What LRCLIB answered for a song, found or not, so it is asked once: the
-- words it gave, or nothing at all with the moment it was asked.
CREATE TABLE music_lyrics (
    song_id       TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    plain         TEXT,
    synced        TEXT,
    instrumental  INTEGER NOT NULL DEFAULT 0,
    looked_up_at  TEXT NOT NULL
) STRICT;

-- What a library of music does beyond what every library does. A library
-- with no row asks nothing of anybody online.
CREATE TABLE music_library_options (
    library_id     TEXT PRIMARY KEY NOT NULL REFERENCES libraries (id) ON DELETE CASCADE,
    lyrics_online  INTEGER NOT NULL DEFAULT 0
) STRICT;

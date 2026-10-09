-- How the stamps of the lyrics of a song were moved to fall where its voice
-- starts: one move in milliseconds for each line, in the order of the lines,
-- as the lines were when it was measured. Forgotten with the lyrics it was
-- measured on, since other lyrics have other lines.
CREATE TABLE music_lyrics_timing (
    song_id      TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    shift_ms     INTEGER NOT NULL,
    moves_ms     TEXT NOT NULL,
    lines_moved  INTEGER NOT NULL,
    confidence   REAL NOT NULL,
    measured_at  TEXT NOT NULL
) STRICT;

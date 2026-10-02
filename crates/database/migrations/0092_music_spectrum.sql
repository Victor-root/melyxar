-- How the sound of each song is spread, a few times a second, for the
-- wave behind the player of music. Read in the same pass of the file as its
-- loudness.
--
-- Kept with the file as it stood then, like the loudness: a file changed since
-- is read again, and one that could not be read is kept with no levels and not
-- read again until it changes. The levels are one byte each, `bands` of them
-- for every reading, `frames_a_second` readings a second.
CREATE TABLE music_spectrum (
    source_id         TEXT PRIMARY KEY NOT NULL REFERENCES media_sources (id) ON DELETE CASCADE,
    file_modified_at  TEXT NOT NULL,
    measured_at       TEXT NOT NULL,
    bands             INTEGER NOT NULL,
    frames_a_second   INTEGER NOT NULL,
    levels            BLOB NOT NULL
) STRICT;

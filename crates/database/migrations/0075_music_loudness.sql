-- The songs whose loudness was measured, or tried and could not be, with
-- the file as it stood then: a file changed since is measured again, and
-- one that could not be read is not read again until it changes.
CREATE TABLE music_loudness_measured (
    source_id         TEXT PRIMARY KEY NOT NULL REFERENCES media_sources (id) ON DELETE CASCADE,
    file_modified_at  TEXT NOT NULL,
    measured_at       TEXT NOT NULL
) STRICT;

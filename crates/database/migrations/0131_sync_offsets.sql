-- How far each account moved the sound and the subtitles of one file against
-- its picture, for a file whose own sound or subtitles run early or late.
-- Kept per account and per file: one person putting a file back in step
-- changes nothing for anyone else, and another copy of the same film is
-- another file. A row only stands while one of the two is not zero.

CREATE TABLE sync_offsets (
    user_id          TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    source_id        TEXT NOT NULL REFERENCES media_sources (id) ON DELETE CASCADE,
    sound_offset_ms  INTEGER NOT NULL DEFAULT 0,
    words_offset_ms  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, source_id)
) STRICT;

-- A file taken away takes its rows with it, found by this rather than by
-- reading the whole table.
CREATE INDEX sync_offsets_source ON sync_offsets (source_id);

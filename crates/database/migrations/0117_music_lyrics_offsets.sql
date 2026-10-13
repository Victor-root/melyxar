-- How much later, or sooner when negative, the lines of a song are shown than
-- they are stamped, set by hand on top of any lining up. Kept for the song,
-- like the lining up, and forgotten with it.
CREATE TABLE music_lyrics_offsets (
    song_id TEXT PRIMARY KEY REFERENCES works(id) ON DELETE CASCADE,
    offset_ms INTEGER NOT NULL,
    set_at TEXT NOT NULL
) STRICT;

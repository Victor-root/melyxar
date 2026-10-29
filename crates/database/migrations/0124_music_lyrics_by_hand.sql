-- The lyrics of a song as an administrator wrote them in the editor: line by
-- line with their moments in the LRC form, or plain words when no line has
-- one. Read before the song's file, the file beside it and LRCLIB.
CREATE TABLE music_lyrics_by_hand (
    song_id     TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    words       TEXT NOT NULL,
    written_at  TEXT NOT NULL
) STRICT;

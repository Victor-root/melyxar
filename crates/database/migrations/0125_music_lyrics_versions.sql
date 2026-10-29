-- Every version of the lyrics of a song an administrator wrote in the editor,
-- the one in service marked. Never tied to the song by the database: deleting
-- the song, or the folder it is in, leaves them here, with the folder and the
-- path of its file, so a file back at the same place takes them up again.
-- Only somebody deleting a version by hand takes it away.
CREATE TABLE music_lyrics_versions (
    id             INTEGER PRIMARY KEY,
    song_id        TEXT NOT NULL,
    root_path      TEXT,
    relative_path  TEXT,
    words          TEXT NOT NULL,
    written_at     TEXT NOT NULL,
    in_use         INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX music_lyrics_versions_by_song ON music_lyrics_versions (song_id);

INSERT INTO music_lyrics_versions (song_id, root_path, relative_path, words, written_at, in_use)
SELECT h.song_id,
       (SELECT r.path FROM media_sources s JOIN library_roots r ON r.id = s.root_id
         WHERE s.work_id = h.song_id ORDER BY s.relative_path LIMIT 1),
       (SELECT s.relative_path FROM media_sources s
         WHERE s.work_id = h.song_id ORDER BY s.relative_path LIMIT 1),
       h.words, h.written_at, 1
  FROM music_lyrics_by_hand h;

DROP TABLE music_lyrics_by_hand;

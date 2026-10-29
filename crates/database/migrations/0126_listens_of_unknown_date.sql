-- A listen taken up from another server may come without the moment it was
-- made: Emby counts how often a song was played without always keeping when.
-- The moment becomes optional rather than invented, and a song listened to
-- at a moment nobody knows comes after every dated one among the songs
-- listened to lately.
CREATE TABLE music_listens_kept (
    user_id           TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    song_id           TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    listens           INTEGER NOT NULL,
    last_listened_at  TEXT,
    PRIMARY KEY (user_id, song_id)
) STRICT;

INSERT INTO music_listens_kept (user_id, song_id, listens, last_listened_at)
SELECT user_id, song_id, listens, last_listened_at FROM music_listens;

DROP TABLE music_listens;
ALTER TABLE music_listens_kept RENAME TO music_listens;

CREATE INDEX music_listens_lately ON music_listens (user_id, last_listened_at);
CREATE INDEX music_listens_most ON music_listens (user_id, listens);
CREATE INDEX music_listens_song ON music_listens (song_id);

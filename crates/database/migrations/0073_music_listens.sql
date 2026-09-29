-- How often each account listened to each song, and when last: what
-- "listened to lately" and "listened to most" are read from.
CREATE TABLE music_listens (
    user_id           TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    song_id           TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    listens           INTEGER NOT NULL,
    last_listened_at  TEXT NOT NULL,
    PRIMARY KEY (user_id, song_id)
) STRICT;

CREATE INDEX music_listens_lately ON music_listens (user_id, last_listened_at);
CREATE INDEX music_listens_most ON music_listens (user_id, listens);

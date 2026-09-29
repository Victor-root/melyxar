-- Each account's playlists of songs, apart from the playlists of films: a
-- song may be in one twice, which a list of films never wants.
CREATE TABLE music_playlists (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
) STRICT;

CREATE INDEX music_playlists_by_user ON music_playlists (user_id, name);

CREATE TABLE music_playlist_songs (
    playlist_id  TEXT NOT NULL REFERENCES music_playlists (id) ON DELETE CASCADE,
    position     INTEGER NOT NULL,
    song_id      TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id, position)
) STRICT;

CREATE INDEX music_playlist_songs_song ON music_playlist_songs (song_id);

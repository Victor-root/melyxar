-- Music: what a song, an album and an artist are beyond the common trunk.
--
-- The three are works like any other, so everything hung on a work hangs on
-- them too: favourites, playlists, pictures, how often somebody played one.
-- What belongs to music alone lives in the tables below, as the model agreed
-- from the start (see docs/architecture/04-fonctionnalites.md, section 5).
--
-- A song hangs under its album, ranked by its track number, which is how every
-- work is ranked under its parent. An album and an artist hang under nothing:
-- an album can be several artists', and an artist is found once in a library
-- whatever number of albums and songs name them.

-- Which disc of its album a song is on. Its track is the work's own rank.
CREATE TABLE music_songs (
    work_id     TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    disc_number INTEGER
) STRICT;

-- Whose album an album is, written as the sort names of its artists joined.
-- With the album's own sort title, that is what tells two albums of the same
-- name apart, and what keeps one album spread over two folders one album.
CREATE TABLE music_albums (
    work_id        TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    artists_key    TEXT NOT NULL,
    is_compilation INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX music_albums_by_artists ON music_albums (artists_key);

-- Who plays a song, and whose album an album is, each an artist work, in the
-- order the files name them.
CREATE TABLE music_credits (
    work_id    TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    artist_id  TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    -- artist on a song, album_artist on an album.
    role       TEXT NOT NULL,
    ordinal    INTEGER NOT NULL,
    PRIMARY KEY (work_id, role, artist_id)
) STRICT;

CREATE INDEX music_credits_by_artist ON music_credits (artist_id, role);

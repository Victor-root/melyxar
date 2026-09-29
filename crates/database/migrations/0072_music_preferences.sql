-- What each account chose for its music, apart from the preferences of
-- films. An account with no row has chosen nothing and gets the defaults.
CREATE TABLE music_preferences (
    user_id          TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    film_on_screen   TEXT NOT NULL DEFAULT 'stop',
    resume_queue     INTEGER NOT NULL DEFAULT 1,
    max_bitrate_kbps INTEGER
);

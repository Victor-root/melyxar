-- How far the buttons that skip back and on move within a song, and which
-- tabs of a library of music an account hides (a bit for each tab). Apart
-- from the steps of the player of films, which a song has no use for.
ALTER TABLE music_preferences ADD COLUMN skip_back_seconds INTEGER NOT NULL DEFAULT 10;
ALTER TABLE music_preferences ADD COLUMN skip_on_seconds INTEGER NOT NULL DEFAULT 10;
ALTER TABLE music_preferences ADD COLUMN hidden_tabs INTEGER NOT NULL DEFAULT 0;

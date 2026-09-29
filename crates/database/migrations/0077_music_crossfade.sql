-- How many seconds one song fades into the next, nought for none: the songs
-- then follow one another without a gap.
ALTER TABLE music_preferences ADD COLUMN crossfade_seconds INTEGER NOT NULL DEFAULT 0;

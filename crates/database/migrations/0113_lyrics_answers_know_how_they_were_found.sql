-- How a song was looked up when its answer was kept, a number that rises each
-- time the finding of songs gets better, so that what was "unknown" to an
-- older way of looking is asked again instead of being believed for good.
ALTER TABLE music_lyrics ADD COLUMN method INTEGER NOT NULL DEFAULT 0;

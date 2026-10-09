-- What came of the last attempt to line the lines of a song up with its sound,
-- kept even when nothing was moved, so that the window can say why. The moves
-- are empty unless the lines were lined up.
ALTER TABLE music_lyrics_timing ADD COLUMN conclusion TEXT NOT NULL DEFAULT 'aligned';

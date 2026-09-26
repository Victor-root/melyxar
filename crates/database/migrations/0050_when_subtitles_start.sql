-- When a film starts with subtitles nobody picked for it: whole ones in the
-- language read, only when the film is heard in another, only the forced
-- ones, the ones the file marks, or none. The first is what every account
-- already had.
ALTER TABLE user_preferences ADD COLUMN subtitle_mode TEXT NOT NULL DEFAULT 'always';

-- Whether this viewer turned the subtitles of this work off. Not the same as
-- never having chosen any: the account's mode picks subtitles for a work
-- nobody said anything about, and none for one somebody turned them off in.
ALTER TABLE playback_progress ADD COLUMN subtitles_off INTEGER NOT NULL DEFAULT 0;

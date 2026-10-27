-- When somebody last chose the tracks of a work by hand. An episode nobody
-- chose anything for starts with the soundtrack last chosen in its series,
-- and only a moment tells which choice was the last.
ALTER TABLE playback_progress ADD COLUMN tracks_chosen_at TEXT;

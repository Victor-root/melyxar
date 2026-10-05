-- Whether the bar of the player closes when the last song of the queue ends.
ALTER TABLE music_preferences ADD COLUMN close_when_done INTEGER NOT NULL DEFAULT 1;

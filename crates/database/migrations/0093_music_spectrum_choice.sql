-- Whether the wave of the sound plays behind the bar of the player, for this
-- account. On, until it is turned off.
ALTER TABLE music_preferences ADD COLUMN spectrum INTEGER NOT NULL DEFAULT 1;

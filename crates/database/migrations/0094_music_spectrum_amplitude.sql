-- How high the wave of the sound rises behind the bar of the player, from
-- nought to a hundred, for this account. As high as it goes, until it is
-- turned down.
ALTER TABLE music_preferences ADD COLUMN spectrum_amplitude INTEGER NOT NULL DEFAULT 100;

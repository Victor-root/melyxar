-- How each account wants songs levelled: not at all, every song to the
-- same level, or every album to the same level with its songs kept apart.
ALTER TABLE music_preferences ADD COLUMN volume_mode TEXT NOT NULL DEFAULT 'track';

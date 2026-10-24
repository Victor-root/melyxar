-- What each person holds the picture of a film to, the player's name for it:
-- the film as it is until somebody chooses.
ALTER TABLE user_preferences ADD COLUMN playback_quality TEXT NOT NULL DEFAULT 'auto';

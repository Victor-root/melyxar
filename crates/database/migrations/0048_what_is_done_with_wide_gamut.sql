-- What this account wants done with a film of wide gamut colour: kept where
-- the screen shows it and converted elsewhere, always converted, or never
-- converted for its colour alone. The first is what every account starts on.
ALTER TABLE user_preferences ADD COLUMN wide_gamut TEXT NOT NULL DEFAULT 'automatic';

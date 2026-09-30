-- The theme a visitor sees until they choose one, set for the whole server.
-- An account that never chose follows it: what was stored as the automatic
-- theme before this existed could not tell "chose automatic" from "chose
-- nothing", and the second is far the commoner, so those accounts follow the
-- server too. A server left at its own default still shows them the same as
-- before.
ALTER TABLE server_settings ADD COLUMN default_theme TEXT NOT NULL DEFAULT 'system';
UPDATE user_preferences SET theme_mode = 'server' WHERE theme_mode = 'system';

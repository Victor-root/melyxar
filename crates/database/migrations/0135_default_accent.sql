-- The accent whoever has not chosen one wears, as the administrator sets it:
-- Melyxar's red until then.
ALTER TABLE server_settings ADD COLUMN default_accent TEXT NOT NULL DEFAULT '#c81e1e';

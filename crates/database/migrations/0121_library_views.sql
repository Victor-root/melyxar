-- How each person lays out the libraries they changed: `library:view` pairs
-- separated by commas, nothing for a library left with its posters.
ALTER TABLE user_preferences ADD COLUMN library_views TEXT NOT NULL DEFAULT '';

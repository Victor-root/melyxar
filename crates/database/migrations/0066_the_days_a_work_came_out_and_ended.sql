-- The day a work came out, and for a series the day its last episode did,
-- each written year, month, day. The year stays where it was, since every
-- grid sorts and filters on it; a day, when there is one, agrees with it.
ALTER TABLE works ADD COLUMN release_date TEXT;
ALTER TABLE works ADD COLUMN end_date TEXT;

-- The kinds of library in the order one person's home page lays them out.
--
-- Written as the stored names of the kinds, joined by commas. A kind missing
-- from it goes after the others when it is read, so a kind the server learns
-- later is never missing from anybody's home page.
ALTER TABLE user_preferences ADD COLUMN home_order TEXT NOT NULL
    DEFAULT 'movies,series,anime,shows,music';

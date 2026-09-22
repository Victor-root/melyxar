-- Anime episodes numbered from one end of their series to the other.
--
-- An anime release says `Series - 29` and never which season that is, while
-- the provider describes the series season by season. The file's own number
-- is kept on the episode it became, so the episode can be put back in the
-- right season whenever the series is described again, however the provider
-- cuts it up that time. Empty for every episode whose name or folder said
-- the season: that one is where it was written to be and is never moved.
ALTER TABLE works ADD COLUMN absolute_number INTEGER;

-- How many episodes each season of a series holds, as its provider counts
-- them.
--
-- Written when the series is described, and read when a file numbered across
-- the series arrives afterwards: it is placed straight into its season rather
-- than asking the provider to describe the whole series again.
CREATE TABLE season_lengths (
    series_id TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    season    INTEGER NOT NULL,
    episodes  INTEGER NOT NULL,
    PRIMARY KEY (series_id, season)
) STRICT;

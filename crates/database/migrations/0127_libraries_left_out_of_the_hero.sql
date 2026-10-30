-- Whether a library's works may stand in the banner at the top of the home
-- page. On for every library, which is what they all did until now: a library
-- of trials or of odds and ends is turned off by hand.

ALTER TABLE libraries ADD COLUMN in_the_hero INTEGER NOT NULL DEFAULT 1;

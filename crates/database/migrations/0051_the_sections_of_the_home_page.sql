-- The sections of the home page below its banner, in the order this account
-- lays them out, and those it leaves off. Kept apart, so a section shown
-- again comes back where it was. The order is the one every account had.
ALTER TABLE user_preferences ADD COLUMN home_sections TEXT NOT NULL
    DEFAULT 'band,carry_on,up_next,recently_added,libraries';
ALTER TABLE user_preferences ADD COLUMN hidden_home_sections TEXT NOT NULL DEFAULT '';

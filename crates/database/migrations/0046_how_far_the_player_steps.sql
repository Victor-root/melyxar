-- How far the player's two step buttons jump, in seconds, for this account.
--
-- One for stepping back and one for stepping on, because they do not serve
-- the same thing: a line heard again, a title sequence passed over. Ten
-- each, the length the buttons were written with.
ALTER TABLE user_preferences ADD COLUMN step_back_seconds INTEGER NOT NULL DEFAULT 10;
ALTER TABLE user_preferences ADD COLUMN step_on_seconds INTEGER NOT NULL DEFAULT 10;

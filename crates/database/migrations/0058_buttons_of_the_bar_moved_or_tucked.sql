-- The order of the buttons at the right end of the bar at the top, and those
-- moved into the account's menu next to them.
--
-- Empty until somebody chooses: an empty order reads as the one everybody
-- starts with, and nothing is in the menu.
ALTER TABLE user_preferences ADD COLUMN header_buttons TEXT NOT NULL DEFAULT '';
ALTER TABLE user_preferences ADD COLUMN buttons_in_the_menu TEXT NOT NULL DEFAULT '';
